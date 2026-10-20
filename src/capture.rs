//! Capturing a space: the open windows, grouped by application.
//!
//! Capture only reads. It never moves, closes or focuses a window. Titles are
//! returned for the preview so the user recognises each window, and are not
//! part of the card.

use crate::apps::AppIndex;
use crate::backend::{Capabilities, Snapshot, Window};
use crate::card::{AppEntry, DesktopRef, WindowSlot};
use serde::Serialize;

/// One application found on screen, as it would be saved, plus what the
/// preview needs to show it.
#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub key: String,
    pub app: AppEntry,
    /// Window titles, in the same order as `app.windows`; never saved.
    pub titles: Vec<String>,
    /// Lyapunov can start it again (it has a menu entry or an executable).
    pub launchable: bool,
}

#[derive(Serialize, Debug)]
pub struct Scan {
    pub backend: String,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
    pub apps: Vec<Candidate>,
}

/// Applications Lyapunov never captures: itself and the desktop shell.
const IGNORED: &[&str] = &[
    "lyapunov",
    "lyapunov-desktop",
    "lyapunov.desktop",
    "plasmashell",
    "org.kde.plasmashell",
    "org.kde.plasmashell.desktop",
    "gnome-shell",
    "org.gnome.shell.extensions",
    "xfdesktop",
    "xfce4-panel",
    "nemo-desktop",
    "desktop_window",
    "conky",
];

fn ignored(w: &Window, extra_pids: &[u32]) -> bool {
    if w.pid.is_some_and(|p| extra_pids.contains(&p)) {
        return true;
    }
    [&w.class, &w.instance]
        .into_iter()
        .chain(w.desktop_file.as_ref())
        .any(|v| IGNORED.contains(&v.to_lowercase().as_str()))
}

pub fn slot_for(w: &Window, snap: &Snapshot) -> WindowSlot {
    let output = w
        .output
        .clone()
        .or_else(|| snap.output_at(&w.rect).map(|o| o.name.clone()));
    let output_rect = output.as_deref().and_then(|n| snap.output(n)).map(|o| o.rect);
    let desktop = w.desktop.as_ref().map(|id| {
        let d = snap.desktops.iter().find(|d| &d.id == id);
        DesktopRef {
            id: Some(id.clone()),
            name: d.map(|d| d.name.clone()),
            index: d.map(|d| d.index),
        }
    });
    WindowSlot {
        rect: w.rect,
        state: w.state,
        output,
        output_rect,
        desktop,
        floating: w.floating,
    }
}

/// Groups windows by application, in stacking order, skipping Lyapunov
/// itself, the desktop shell and processes in `exclude_pids`.
pub fn candidates(snap: &Snapshot, index: &AppIndex, exclude_pids: &[u32]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for w in &snap.windows {
        if ignored(w, exclude_pids) {
            continue;
        }
        let id = index.identify(w);
        if id.desktop.is_none() && id.exe.is_none() && id.class.trim().is_empty() {
            continue;
        }
        if id
            .desktop
            .as_deref()
            .is_some_and(|d| IGNORED.contains(&d.to_lowercase().as_str()))
        {
            continue;
        }
        let app = AppEntry {
            name: id.name,
            desktop: id.desktop,
            exe: id.exe,
            class: id.class,
            workspace: None,
            windows: Vec::new(),
        };
        let key = app.key();
        let pos = match out.iter().position(|c| c.key == key) {
            Some(p) => p,
            None => {
                out.push(Candidate {
                    key,
                    launchable: app.launchable(),
                    app,
                    titles: Vec::new(),
                });
                out.len() - 1
            }
        };
        let c = &mut out[pos];
        if c.app.windows.len() < crate::card::MAX_WINDOWS {
            c.app.windows.push(slot_for(w, snap));
            c.titles.push(w.title.clone());
        }
    }
    out.truncate(crate::card::MAX_APPS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Desktop, Output};
    use crate::card::Rect;

    fn snap() -> Snapshot {
        let win = |h: &str, class: &str, x: i32| Window {
            handle: h.into(),
            class: class.into(),
            title: format!("{class} {h}"),
            rect: Rect::new(x, 10, 400, 300),
            desktop: Some("d1".into()),
            ..Default::default()
        };
        Snapshot {
            outputs: vec![
                Output {
                    name: "DP-1".into(),
                    rect: Rect::new(0, 0, 1920, 1080),
                    primary: true,
                },
                Output {
                    name: "HDMI-1".into(),
                    rect: Rect::new(1920, 0, 1280, 1024),
                    primary: false,
                },
            ],
            desktops: vec![Desktop {
                id: "d1".into(),
                name: "Travail".into(),
                index: 0,
            }],
            current_desktop: Some("d1".into()),
            windows: vec![
                win("1", "kate", 10),
                win("2", "plasmashell", 0),
                win("3", "kate", 2000),
                win("4", "", 0),
                Window {
                    pid: Some(4242),
                    ..win("5", "lyapunov-desktop", 0)
                },
            ],
        }
    }

    #[test]
    fn groups_windows_and_skips_the_shell() {
        let c = candidates(&snap(), &AppIndex::default(), &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].app.class, "kate");
        assert_eq!(c[0].app.windows.len(), 2);
        assert_eq!(c[0].titles, ["kate 1", "kate 3"]);
        let second = &c[0].app.windows[1];
        assert_eq!(second.output.as_deref(), Some("HDMI-1"));
        assert_eq!(second.output_rect, Some(Rect::new(1920, 0, 1280, 1024)));
        assert_eq!(second.desktop.as_ref().unwrap().name.as_deref(), Some("Travail"));
        assert!(!c[0].launchable, "no desktop entry in an empty index");
    }

    #[test]
    fn titles_never_reach_the_card() {
        let c = candidates(&snap(), &AppIndex::default(), &[]);
        let json = serde_json::to_string(&c[0].app).unwrap();
        assert!(!json.contains("kate 1"));
    }
}
