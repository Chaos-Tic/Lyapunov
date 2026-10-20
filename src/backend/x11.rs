//! X11 window managers that follow the Extended Window Manager Hints (EWMH):
//! Xfce, Cinnamon, MATE, Openbox, Fluxbox, LXDE, Budgie on X11, KDE and GNOME
//! on X11 when their own backend is not used.
//!
//! Windows are read from `_NET_CLIENT_LIST`; moves go through the window
//! manager with `_NET_MOVERESIZE_WINDOW` and `_NET_WM_STATE` requests, as a
//! pager would. Each move is checked afterwards: a window manager that ignores
//! a request is reported, not assumed to have obeyed.

use super::{Backend, Capabilities, Desktop, Output, PlaceResult, Placement, Snapshot, Window};
use crate::card::{Rect, WindowState};
use anyhow::{Context, Result, bail};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask, Window as XWindow,
};
use x11rb::rust_connection::RustConnection;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        UTF8_STRING,
        _NET_CLIENT_LIST,
        _NET_CLIENT_LIST_STACKING,
        _NET_WM_NAME,
        _NET_WM_PID,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_STATE,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_DESKTOP,
        _NET_NUMBER_OF_DESKTOPS,
        _NET_DESKTOP_NAMES,
        _NET_CURRENT_DESKTOP,
        _NET_FRAME_EXTENTS,
        _NET_MOVERESIZE_WINDOW,
        _NET_ACTIVE_WINDOW,
        _NET_SUPPORTING_WM_CHECK,
        WM_CHANGE_STATE,
        _GTK_APPLICATION_ID,
        _KDE_NET_WM_DESKTOP_FILE,
    }
}

pub struct X11 {
    conn: RustConnection,
    root: XWindow,
    atoms: Atoms,
}

const ALL_DESKTOPS: u32 = 0xFFFF_FFFF;
const TOLERANCE: i32 = 12;

impl X11 {
    pub fn connect() -> Result<X11> {
        let (conn, screen) = x11rb::connect(None).context("connecting to the X server")?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)?.reply()?;
        let x = X11 { conn, root, atoms };
        // A window manager that supports EWMH announces itself on the root window.
        if x.u32s(root, x.atoms._NET_SUPPORTING_WM_CHECK, AtomEnum::WINDOW.into())?
            .is_empty()
        {
            bail!("no EWMH window manager is running");
        }
        Ok(x)
    }

    fn prop(&self, win: XWindow, prop: Atom, ty: Atom) -> Result<Vec<u8>> {
        let r = self.conn.get_property(false, win, prop, ty, 0, 1 << 16)?.reply()?;
        Ok(r.value)
    }

    fn u32s(&self, win: XWindow, prop: Atom, ty: Atom) -> Result<Vec<u32>> {
        let r = self.conn.get_property(false, win, prop, ty, 0, 1 << 16)?.reply()?;
        Ok(r.value32().map(|v| v.collect()).unwrap_or_default())
    }

    fn text(&self, win: XWindow, prop: Atom) -> String {
        self.prop(win, prop, self.atoms.UTF8_STRING)
            .ok()
            .filter(|v| !v.is_empty())
            .map(|v| String::from_utf8_lossy(&v).trim_end_matches('\0').to_string())
            .unwrap_or_default()
    }

    fn clients(&self) -> Result<Vec<XWindow>> {
        let stacking = self.u32s(self.root, self.atoms._NET_CLIENT_LIST_STACKING, AtomEnum::WINDOW.into())?;
        if !stacking.is_empty() {
            return Ok(stacking);
        }
        self.u32s(self.root, self.atoms._NET_CLIENT_LIST, AtomEnum::WINDOW.into())
    }

    fn frame(&self, win: XWindow) -> Result<Rect> {
        let geo = self.conn.get_geometry(win)?.reply()?;
        let pos = self.conn.translate_coordinates(win, self.root, 0, 0)?.reply()?;
        let ext = self.u32s(win, self.atoms._NET_FRAME_EXTENTS, AtomEnum::CARDINAL.into())?;
        let [l, r, t, b] = match ext.as_slice() {
            [l, r, t, b] => [*l as i32, *r as i32, *t as i32, *b as i32],
            _ => [0; 4],
        };
        Ok(Rect::new(
            i32::from(pos.dst_x) - l,
            i32::from(pos.dst_y) - t,
            i32::from(geo.width) + l + r,
            i32::from(geo.height) + t + b,
        ))
    }

    fn extents(&self, win: XWindow) -> [i32; 4] {
        match self
            .u32s(win, self.atoms._NET_FRAME_EXTENTS, AtomEnum::CARDINAL.into())
            .unwrap_or_default()
            .as_slice()
        {
            [l, r, t, b] => [*l as i32, *r as i32, *t as i32, *b as i32],
            _ => [0; 4],
        }
    }

    fn window(&self, win: XWindow) -> Result<Option<Window>> {
        let types = self.u32s(win, self.atoms._NET_WM_WINDOW_TYPE, AtomEnum::ATOM.into())?;
        if !types.is_empty() && !types.contains(&self.atoms._NET_WM_WINDOW_TYPE_NORMAL) {
            return Ok(None);
        }
        let transient = self.u32s(win, AtomEnum::WM_TRANSIENT_FOR.into(), AtomEnum::WINDOW.into())?;
        if types.is_empty() && transient.iter().any(|w| *w != 0) {
            return Ok(None);
        }
        let states = self.u32s(win, self.atoms._NET_WM_STATE, AtomEnum::ATOM.into())?;
        let has = |a: Atom| states.contains(&a);
        if has(self.atoms._NET_WM_STATE_SKIP_TASKBAR) {
            return Ok(None);
        }
        let state = if has(self.atoms._NET_WM_STATE_FULLSCREEN) {
            WindowState::Fullscreen
        } else if has(self.atoms._NET_WM_STATE_HIDDEN) {
            WindowState::Minimized
        } else if has(self.atoms._NET_WM_STATE_MAXIMIZED_VERT) && has(self.atoms._NET_WM_STATE_MAXIMIZED_HORZ) {
            WindowState::Maximized
        } else {
            WindowState::Normal
        };
        let class = self.prop(win, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
        let mut parts = class
            .split(|b| *b == 0)
            .map(|p| String::from_utf8_lossy(p).into_owned());
        let instance = parts.next().unwrap_or_default();
        let class = parts.next().unwrap_or_default();
        let mut title = self.text(win, self.atoms._NET_WM_NAME);
        if title.is_empty() {
            title = self
                .prop(win, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())
                .map(|v| String::from_utf8_lossy(&v).into_owned())
                .unwrap_or_default();
        }
        let pid = self
            .u32s(win, self.atoms._NET_WM_PID, AtomEnum::CARDINAL.into())?
            .first()
            .copied()
            .filter(|p| *p > 0);
        if pid == Some(std::process::id()) {
            return Ok(None);
        }
        let desktop = self
            .u32s(win, self.atoms._NET_WM_DESKTOP, AtomEnum::CARDINAL.into())?
            .first()
            .copied()
            .filter(|d| *d != ALL_DESKTOPS)
            .map(|d| d.to_string());
        let mut desktop_file = self.text(win, self.atoms._KDE_NET_WM_DESKTOP_FILE);
        if desktop_file.is_empty() {
            desktop_file = self.text(win, self.atoms._GTK_APPLICATION_ID);
        }
        Ok(Some(Window {
            handle: format!("0x{win:x}"),
            class,
            instance,
            desktop_file: Some(desktop_file).filter(|d| !d.is_empty()),
            exe: pid.and_then(crate::apps::process_exe),
            pid,
            title,
            rect: self.frame(win)?,
            state,
            output: None,
            desktop,
            floating: false,
        }))
    }

    fn outputs(&self) -> Vec<Output> {
        let monitors = self
            .conn
            .randr_get_monitors(self.root, true)
            .ok()
            .and_then(|c| c.reply().ok());
        let mut out = Vec::new();
        if let Some(m) = monitors {
            for mon in m.monitors {
                let name = self
                    .conn
                    .get_atom_name(mon.name)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.name).into_owned())
                    .unwrap_or_else(|| format!("monitor-{}", out.len()));
                out.push(Output {
                    name,
                    rect: Rect::new(
                        i32::from(mon.x),
                        i32::from(mon.y),
                        i32::from(mon.width),
                        i32::from(mon.height),
                    ),
                    primary: mon.primary,
                });
            }
        }
        if out.is_empty() {
            let s = &self.conn.setup().roots[0];
            out.push(Output {
                name: "screen".into(),
                rect: Rect::new(0, 0, i32::from(s.width_in_pixels), i32::from(s.height_in_pixels)),
                primary: true,
            });
        }
        out
    }

    fn desktops(&self) -> Result<(Vec<Desktop>, Option<String>)> {
        let count = self
            .u32s(self.root, self.atoms._NET_NUMBER_OF_DESKTOPS, AtomEnum::CARDINAL.into())?
            .first()
            .copied()
            .unwrap_or(1)
            .min(64);
        let names = self.prop(self.root, self.atoms._NET_DESKTOP_NAMES, self.atoms.UTF8_STRING)?;
        let names: Vec<String> = names
            .split(|b| *b == 0)
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .collect();
        let current = self
            .u32s(self.root, self.atoms._NET_CURRENT_DESKTOP, AtomEnum::CARDINAL.into())?
            .first()
            .map(|c| c.to_string());
        let desktops = (0..count)
            .map(|i| Desktop {
                id: i.to_string(),
                name: names
                    .get(i as usize)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("{}", i + 1)),
                index: i,
            })
            .collect();
        Ok((desktops, current))
    }

    fn send(&self, win: XWindow, ty: Atom, data: [u32; 5]) -> Result<()> {
        let event = ClientMessageEvent::new(32, win, ty, data);
        self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;
        Ok(())
    }

    fn set_states(&self, win: XWindow, add: bool, a: Atom, b: Atom) -> Result<()> {
        // Source indication 2: a pager acting for the user.
        self.send(win, self.atoms._NET_WM_STATE, [u32::from(add), a, b, 2, 0])
    }

    fn apply(&self, p: &Placement, win: XWindow, current: &Window) -> Result<()> {
        let a = &self.atoms;
        if let Some(d) = p.desktop.as_deref().and_then(|d| d.parse::<u32>().ok()) {
            self.send(win, a._NET_WM_DESKTOP, [d, 2, 0, 0, 0])?;
        }
        if current.state == WindowState::Fullscreen && p.state != WindowState::Fullscreen {
            self.set_states(win, false, a._NET_WM_STATE_FULLSCREEN, 0)?;
        }
        if current.state == WindowState::Maximized && p.state != WindowState::Maximized {
            self.set_states(
                win,
                false,
                a._NET_WM_STATE_MAXIMIZED_VERT,
                a._NET_WM_STATE_MAXIMIZED_HORZ,
            )?;
        }
        if current.state == WindowState::Minimized && p.state != WindowState::Minimized {
            self.send(win, a._NET_ACTIVE_WINDOW, [2, 0, 0, 0, 0])?;
        }
        if let Some(r) = p.rect {
            let [l, rr, t, b] = self.extents(win);
            let w = (r.width - l - rr).max(1) as u32;
            let h = (r.height - t - b).max(1) as u32;
            // NorthWest gravity, x/y/width/height present, source: pager.
            let flags = 1 | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (2 << 12);
            self.send(win, a._NET_MOVERESIZE_WINDOW, [flags, r.x as u32, r.y as u32, w, h])?;
        }
        match p.state {
            WindowState::Maximized => self.set_states(
                win,
                true,
                a._NET_WM_STATE_MAXIMIZED_VERT,
                a._NET_WM_STATE_MAXIMIZED_HORZ,
            )?,
            WindowState::Fullscreen => self.set_states(win, true, a._NET_WM_STATE_FULLSCREEN, 0)?,
            // ICCCM IconicState.
            WindowState::Minimized => self.send(win, a.WM_CHANGE_STATE, [3, 0, 0, 0, 0])?,
            WindowState::Normal => {}
        }
        self.conn.flush()?;
        Ok(())
    }

    /// Window managers that ignore `_NET_MOVERESIZE_WINDOW` usually honour a
    /// plain configure request on the client window.
    fn configure(&self, win: XWindow, r: Rect) -> Result<()> {
        let [l, rr, t, b] = self.extents(win);
        let aux = ConfigureWindowAux::new()
            .x(r.x)
            .y(r.y)
            .width((r.width - l - rr).max(1) as u32)
            .height((r.height - t - b).max(1) as u32);
        self.conn.configure_window(win, &aux)?;
        self.conn.flush()?;
        Ok(())
    }
}

fn close(a: &Rect, b: &Rect) -> bool {
    (a.x - b.x).abs() <= TOLERANCE
        && (a.y - b.y).abs() <= TOLERANCE
        && (a.width - b.width).abs() <= TOLERANCE
        && (a.height - b.height).abs() <= TOLERANCE
}

fn parse_handle(h: &str) -> Option<XWindow> {
    u32::from_str_radix(h.trim_start_matches("0x"), 16).ok()
}

impl Backend for X11 {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            list: true,
            geometry: true,
            desktops: true,
            states: true,
            tiling: false,
        }
    }

    fn snapshot(&mut self) -> Result<Snapshot> {
        let mut windows = Vec::new();
        for win in self.clients()? {
            // A window can disappear between listing and reading it.
            if let Ok(Some(w)) = self.window(win) {
                windows.push(w);
            }
        }
        let (desktops, current_desktop) = self.desktops()?;
        let mut snap = Snapshot {
            outputs: self.outputs(),
            desktops,
            current_desktop,
            windows,
        };
        let outputs = snap.outputs.clone();
        for w in &mut snap.windows {
            let (x, y) = w.rect.center();
            w.output = outputs.iter().find(|o| o.rect.contains(x, y)).map(|o| o.name.clone());
        }
        Ok(snap)
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        let before = self.snapshot()?;
        let mut results = Vec::new();
        let mut pending = Vec::new();
        for p in placements {
            let current = before.windows.iter().find(|w| w.handle == p.handle);
            match (parse_handle(&p.handle), current) {
                (Some(win), Some(cur)) => match self.apply(p, win, cur) {
                    Ok(()) => pending.push(p),
                    Err(e) => results.push(PlaceResult {
                        handle: p.handle.clone(),
                        ok: false,
                        note: Some(format!("{e:#}")),
                    }),
                },
                _ => results.push(PlaceResult {
                    handle: p.handle.clone(),
                    ok: false,
                    note: Some("window closed".into()),
                }),
            }
        }
        // Give the window manager time to act, then check, retrying once with
        // a configure request where the geometry did not apply.
        let check = |x: &mut X11, pending: &[&Placement]| -> Result<Vec<String>> {
            std::thread::sleep(Duration::from_millis(250));
            let after = x.snapshot()?;
            Ok(pending
                .iter()
                .filter(|p| {
                    let Some(r) = p.rect else { return false };
                    p.state == WindowState::Normal
                        && after
                            .windows
                            .iter()
                            .find(|w| w.handle == p.handle)
                            .is_some_and(|w| !close(&w.rect, &r))
                })
                .map(|p| p.handle.clone())
                .collect())
        };
        let mut wrong = check(self, &pending)?;
        if !wrong.is_empty() {
            for p in pending.iter().filter(|p| wrong.contains(&p.handle)) {
                if let (Some(win), Some(r)) = (parse_handle(&p.handle), p.rect) {
                    let _ = self.configure(win, r);
                }
            }
            wrong = check(self, &pending)?;
        }
        for p in pending {
            let ok = !wrong.contains(&p.handle);
            results.push(PlaceResult {
                handle: p.handle.clone(),
                ok,
                note: (!ok).then(|| "the window manager did not apply the position".into()),
            });
        }
        Ok(results)
    }
}
