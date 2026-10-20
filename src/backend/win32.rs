//! Windows 10 and 11.
//!
//! Top-level application windows come from `EnumWindows`, filtered the way the
//! taskbar does. Positions use the window placement (the size a window returns
//! to when it is restored), converted to screen coordinates so cards share the
//! same meaning on every platform. Windows on other virtual desktops are
//! cloaked by the system and are not captured.

use super::{Backend, Capabilities, Output, PlaceResult, Placement, Snapshot, Window};
use crate::card::{Rect, WindowState};
use anyhow::{Result, bail};
use std::path::Path;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW,
    MonitorFromRect, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetWindow, GetWindowLongW, GetWindowPlacement,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, IsZoomed,
    SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SetWindowPlacement,
    WINDOWPLACEMENT, WPF_ASYNCWINDOWPLACEMENT, WS_EX_TOOLWINDOW,
};

const MONITORINFOF_PRIMARY: u32 = 1;

pub struct Win32;

impl Win32 {
    pub fn new() -> Win32 {
        Win32
    }
}

impl Default for Win32 {
    fn default() -> Self {
        Win32::new()
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn handle_of(hwnd: HWND) -> String {
    format!("{:x}", hwnd as usize)
}

fn hwnd_of(handle: &str) -> Option<HWND> {
    usize::from_str_radix(handle, 16).ok().map(|v| v as HWND)
}

fn rect_of(r: &RECT) -> Rect {
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: the buffer length is passed with the buffer.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    from_wide(&buf[..n.max(0) as usize])
}

fn title(hwnd: HWND) -> String {
    // SAFETY: the buffer is sized from the reported length plus the terminator.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        from_wide(&buf[..n.max(0) as usize])
    }
}

fn process_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed on every path; the buffer size is in/out.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        (ok != 0).then(|| from_wide(&buf[..size as usize]))
    }
}

fn cloaked(hwnd: HWND) -> bool {
    let mut value: u32 = 0;
    // SAFETY: DWMWA_CLOAKED writes one DWORD.
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut value as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    hr == 0 && value != 0
}

fn monitor_info(m: HMONITOR) -> Option<MONITORINFOEXW> {
    // SAFETY: cbSize announces the extended structure.
    unsafe {
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        (GetMonitorInfoW(m, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) != 0).then_some(info)
    }
}

/// The offset between workspace coordinates (used by window placements, which
/// exclude the taskbar) and screen coordinates, on a monitor.
fn work_offset(m: HMONITOR) -> (i32, i32) {
    monitor_info(m)
        .map(|i| {
            let mi = i.monitorInfo;
            (mi.rcWork.left - mi.rcMonitor.left, mi.rcWork.top - mi.rcMonitor.top)
        })
        .unwrap_or((0, 0))
}

unsafe extern "system" fn collect_windows(hwnd: HWND, data: LPARAM) -> windows_sys::core::BOOL {
    // SAFETY: data is the Vec passed by snapshot(), alive for the enumeration.
    let list = unsafe { &mut *(data as *mut Vec<HWND>) };
    list.push(hwnd);
    1
}

unsafe extern "system" fn collect_monitors(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> windows_sys::core::BOOL {
    // SAFETY: data is the Vec passed by outputs(), alive for the enumeration.
    let list = unsafe { &mut *(data as *mut Vec<HMONITOR>) };
    list.push(m);
    1
}

fn outputs() -> Vec<Output> {
    let mut monitors: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into the Vec.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect_monitors),
            &mut monitors as *mut _ as LPARAM,
        );
    }
    monitors
        .into_iter()
        .filter_map(monitor_info)
        .map(|i| Output {
            name: from_wide(&i.szDevice),
            rect: rect_of(&i.monitorInfo.rcMonitor),
            primary: i.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        })
        .collect()
}

/// Shell windows that are not applications.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
];

fn window(hwnd: HWND, own: u32) -> Option<Window> {
    // SAFETY: plain queries on a window handle from EnumWindows.
    unsafe {
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return None;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
            return None;
        }
    }
    let class = class_name(hwnd);
    if SHELL_CLASSES.contains(&class.as_str()) || cloaked(hwnd) {
        return None;
    }
    let title = title(hwnd);
    if title.is_empty() {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: pid is a valid out pointer.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == own {
        return None;
    }
    let mut exe = process_path(pid);
    // Store apps run inside ApplicationFrameHost, which cannot start them.
    if exe
        .as_deref()
        .is_some_and(|e| e.to_lowercase().ends_with("\\applicationframehost.exe"))
    {
        exe = None;
    }
    // SAFETY: length is set before the call, as the API requires.
    let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
    wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
    if unsafe { GetWindowPlacement(hwnd, &mut wp) } == 0 {
        return None;
    }
    // SAFETY: plain query on a valid handle.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let (dx, dy) = work_offset(monitor);
    let mut rect = rect_of(&wp.rcNormalPosition);
    rect.x += dx;
    rect.y += dy;
    let state = if wp.showCmd == SW_SHOWMINIMIZED as u32 {
        WindowState::Minimized
    } else if unsafe { IsZoomed(hwnd) } != 0 {
        WindowState::Maximized
    } else {
        WindowState::Normal
    };
    let output = monitor_info(monitor).map(|i| from_wide(&i.szDevice));
    Some(Window {
        handle: handle_of(hwnd),
        class: if exe.is_none() { title.clone() } else { class },
        instance: String::new(),
        desktop_file: None,
        pid: Some(pid),
        exe,
        title,
        rect,
        state,
        output,
        desktop: None,
        floating: false,
    })
}

impl Backend for Win32 {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            list: true,
            geometry: true,
            desktops: false,
            states: true,
            tiling: false,
        }
    }

    fn snapshot(&mut self) -> Result<Snapshot> {
        let mut handles: Vec<HWND> = Vec::new();
        // SAFETY: the callback only pushes into the Vec.
        unsafe { EnumWindows(Some(collect_windows), &mut handles as *mut _ as LPARAM) };
        let own = std::process::id();
        // EnumWindows lists from the top of the stack; cards store bottom to top.
        let windows = handles.into_iter().rev().filter_map(|h| window(h, own)).collect();
        Ok(Snapshot {
            outputs: outputs(),
            desktops: Vec::new(),
            current_desktop: None,
            windows,
        })
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        let mut results = Vec::new();
        for p in placements {
            let fail = |note: &str| PlaceResult {
                handle: p.handle.clone(),
                ok: false,
                note: Some(note.into()),
            };
            let Some(hwnd) = hwnd_of(&p.handle).filter(|h| unsafe { IsWindow(*h) } != 0) else {
                results.push(fail("window closed"));
                continue;
            };
            let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
            wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            if unsafe { GetWindowPlacement(hwnd, &mut wp) } == 0 {
                results.push(fail("the window cannot be read"));
                continue;
            }
            if let Some(r) = p.rect {
                let screen = RECT {
                    left: r.x,
                    top: r.y,
                    right: r.right(),
                    bottom: r.bottom(),
                };
                // SAFETY: plain query.
                let m = unsafe { MonitorFromRect(&screen, MONITOR_DEFAULTTONEAREST) };
                let (dx, dy) = work_offset(m);
                wp.rcNormalPosition = RECT {
                    left: r.x - dx,
                    top: r.y - dy,
                    right: r.right() - dx,
                    bottom: r.bottom() - dy,
                };
            }
            wp.flags = WPF_ASYNCWINDOWPLACEMENT;
            wp.showCmd = match p.state {
                WindowState::Maximized | WindowState::Fullscreen => SW_SHOWMAXIMIZED,
                WindowState::Minimized => SW_SHOWMINNOACTIVE,
                WindowState::Normal => SW_SHOWNOACTIVATE,
            } as u32;
            // SAFETY: wp is a fully initialised placement for this window.
            let ok = unsafe { SetWindowPlacement(hwnd, &wp) } != 0;
            results.push(if ok {
                PlaceResult {
                    handle: p.handle.clone(),
                    ok: true,
                    note: None,
                }
            } else {
                fail("Windows refused to move this window (it may run as administrator)")
            });
        }
        Ok(results)
    }
}

/// Opens a folder or document with its default application.
pub fn shell_open(path: &Path) -> Result<()> {
    let verb = wide("open");
    let file = wide(&path.to_string_lossy());
    // SAFETY: both strings are NUL-terminated and outlive the call.
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if r as usize <= 32 {
        bail!("Windows could not open it (error {})", r as usize);
    }
    Ok(())
}
