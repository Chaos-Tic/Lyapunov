//! Window backends: one per desktop technology, chosen at run time.
//!
//! A backend lists the outputs (screens), virtual desktops and application
//! windows, and places windows. Every backend reports what it can really do;
//! the rest of Lyapunov never assumes more. Where no backend applies, cards
//! still reopen their applications, folders and documents.

use crate::card::{Rect, WindowState};
use anyhow::Result;
use serde::Serialize;

#[cfg(unix)]
pub mod gnome;
#[cfg(unix)]
pub mod hyprland;
#[cfg(unix)]
pub mod i3;
#[cfg(unix)]
pub mod kwin;
#[cfg(windows)]
pub mod win32;
#[cfg(unix)]
pub mod x11;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub name: String,
    pub rect: Rect,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub primary: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Desktop {
    /// The backend's own identifier (a UUID on KWin, an index on X11, a name on
    /// tiling window managers).
    pub id: String,
    pub name: String,
    pub index: u32,
}

/// A window as the backend sees it now. `title` is shown while capturing and
/// never written to a card.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// Opaque handle, valid for this session only.
    pub handle: String,
    /// Window class or Wayland app ID.
    pub class: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub instance: String,
    /// Desktop entry name reported by the compositor, when it knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Executable path (Windows), or `/proc/PID/exe` when readable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    pub title: String,
    pub rect: Rect,
    pub state: WindowState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// `None` when the window is on every desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub floating: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Snapshot {
    pub outputs: Vec<Output>,
    pub desktops: Vec<Desktop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_desktop: Option<String>,
    pub windows: Vec<Window>,
}

impl Snapshot {
    pub fn output(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.name == name)
    }
    pub fn primary_output(&self) -> Option<&Output> {
        self.outputs.iter().find(|o| o.primary).or_else(|| self.outputs.first())
    }
    /// The output that contains the centre of a rectangle.
    pub fn output_at(&self, r: &Rect) -> Option<&Output> {
        let (x, y) = r.center();
        self.outputs.iter().find(|o| o.rect.contains(x, y))
    }
}

/// One window to move. Absent fields are left as they are.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub handle: String,
    pub rect: Option<Rect>,
    pub state: WindowState,
    pub desktop: Option<String>,
    pub output: Option<String>,
    /// Tiling window managers: float the window so its geometry applies.
    pub floating: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PlaceResult {
    pub handle: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Can list application windows.
    pub list: bool,
    /// Can move and resize windows.
    pub geometry: bool,
    /// Can move windows between virtual desktops or workspaces.
    pub desktops: bool,
    /// Can maximise, minimise and make windows full screen.
    pub states: bool,
    /// Windows are tiled: only floating windows keep an exact geometry.
    pub tiling: bool,
}

pub trait Backend {
    /// Short identifier: `kwin`, `gnome`, `x11`, `sway`, `i3`, `hyprland`,
    /// `windows`, `launch-only`.
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn snapshot(&mut self) -> Result<Snapshot>;
    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>>;
}

/// Used where no backend applies: applications, folders and documents still
/// reopen, and no window is moved.
pub struct LaunchOnly {
    pub reason: String,
}

impl Backend for LaunchOnly {
    fn name(&self) -> &'static str {
        "launch-only"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        Ok(Snapshot::default())
    }
    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        Ok(placements
            .iter()
            .map(|p| PlaceResult {
                handle: p.handle.clone(),
                ok: false,
                note: Some(self.reason.clone()),
            })
            .collect())
    }
}

/// What the session looks like, from the environment only.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Session {
    pub os: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session_type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub desktop: String,
}

pub fn session() -> Session {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    let mut session_type = var("XDG_SESSION_TYPE");
    if session_type.is_empty() || session_type == "tty" {
        session_type = if !var("WAYLAND_DISPLAY").is_empty() {
            "wayland".into()
        } else if !var("DISPLAY").is_empty() {
            "x11".into()
        } else {
            session_type
        };
    }
    Session {
        os: std::env::consts::OS,
        session_type,
        desktop: var("XDG_CURRENT_DESKTOP"),
    }
}

/// Picks the backend for this session. `LYAPUNOV_BACKEND` forces one (tests,
/// troubleshooting); an unavailable forced backend is an error, not a fallback.
pub fn detect() -> Result<Box<dyn Backend>> {
    let forced = std::env::var("LYAPUNOV_BACKEND").unwrap_or_default();
    #[cfg(windows)]
    {
        if forced.is_empty() || forced == "windows" {
            return Ok(Box::new(win32::Win32::new()));
        }
        if forced == "launch-only" {
            return Ok(Box::new(LaunchOnly {
                reason: "window placement disabled (LYAPUNOV_BACKEND)".into(),
            }));
        }
        anyhow::bail!("backend {forced:?} is not available on Windows");
    }
    #[cfg(unix)]
    {
        let var = |k: &str| std::env::var(k).unwrap_or_default();
        match forced.as_str() {
            "" => {}
            "kwin" => return Ok(Box::new(kwin::KWin::connect()?)),
            "gnome" => return Ok(Box::new(gnome::Gnome::connect()?)),
            "x11" => return Ok(Box::new(x11::X11::connect()?)),
            "sway" | "i3" => return Ok(Box::new(i3::I3::connect()?)),
            "hyprland" => return Ok(Box::new(hyprland::Hyprland::connect()?)),
            "launch-only" => {
                return Ok(Box::new(LaunchOnly {
                    reason: "window placement disabled (LYAPUNOV_BACKEND)".into(),
                }));
            }
            other => anyhow::bail!("unknown backend {other:?}"),
        }
        if !var("HYPRLAND_INSTANCE_SIGNATURE").is_empty() {
            if let Ok(b) = hyprland::Hyprland::connect() {
                return Ok(Box::new(b));
            }
        }
        if !var("SWAYSOCK").is_empty() || !var("I3SOCK").is_empty() {
            if let Ok(b) = i3::I3::connect() {
                return Ok(Box::new(b));
            }
        }
        if kwin::KWin::available() {
            if let Ok(b) = kwin::KWin::connect() {
                return Ok(Box::new(b));
            }
        }
        if gnome::Gnome::available() {
            if let Ok(b) = gnome::Gnome::connect() {
                return Ok(Box::new(b));
            }
        }
        let s = session();
        // On Wayland, an X11 connection only sees XWayland windows: placing
        // them would be misleading, so Wayland sessions never use it.
        if s.session_type != "wayland" && !var("DISPLAY").is_empty() {
            if let Ok(b) = x11::X11::connect() {
                return Ok(Box::new(b));
            }
        }
        let reason = if s.session_type == "wayland" && s.desktop.to_lowercase().contains("gnome") {
            "GNOME on Wayland needs the Lyapunov Shell extension to arrange windows".to_string()
        } else if s.session_type == "wayland" {
            format!(
                "this Wayland desktop ({}) does not let applications arrange windows",
                if s.desktop.is_empty() { "unknown" } else { &s.desktop }
            )
        } else {
            "no supported window manager found".to_string()
        };
        Ok(Box::new(LaunchOnly { reason }))
    }
}
