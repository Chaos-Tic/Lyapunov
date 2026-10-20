//! Resuming a card: what will happen, then doing it and reporting exactly what
//! happened.
//!
//! The plan lists one step per application and per folder or document. The
//! user can leave any step out. Applications already open are reused, never
//! started a second time; an application is started only when none of its
//! windows is open. Windows are then moved to their saved places, on a
//! screen that exists. The report distinguishes a window that was found, an
//! application that was started, and an item that is unavailable. A failure
//! never stops the other steps.

use crate::apps::{self, AppIndex};
use crate::backend::{Backend, Capabilities, Placement, Snapshot, Window};
use crate::card::{AppEntry, Card, PlaceKind, Rect, WindowSlot, WindowState};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StepKind {
    /// The application is open: its windows are arranged.
    Running,
    /// The application is not open: it is started, then arranged.
    Launch,
    /// A folder or document is opened with its default application.
    Open,
    /// Not installed, not found, or cannot be started.
    Unavailable,
}

#[derive(Serialize, Clone, Debug)]
pub struct Step {
    pub key: String,
    pub kind: StepKind,
    pub name: String,
    /// Windows saved in the card for this application.
    pub windows: usize,
    /// Windows of this application open now.
    pub open: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Plan {
    pub card: String,
    pub name: String,
    pub backend: String,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
    pub arrange_open_windows: bool,
    pub steps: Vec<Step>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Open windows were found and placed.
    WindowFound,
    /// The application was started.
    AppLaunched,
    /// The application was already open and left as it was.
    LeftOpen,
    /// The folder or document was opened.
    Opened,
    /// Not installed or not found.
    Unavailable,
    /// Starting or opening failed.
    Failed,
    /// Left out by the user.
    Skipped,
}

#[derive(Serialize, Clone, Debug)]
pub struct Outcome {
    pub key: String,
    pub name: String,
    pub status: Status,
    /// Windows moved to their saved place, out of `windows` saved.
    pub placed: usize,
    pub windows: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct Report {
    pub card: String,
    pub name: String,
    pub backend: String,
    pub outcomes: Vec<Outcome>,
    pub summary: Summary,
}

#[derive(Serialize, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub windows_found: usize,
    pub apps_launched: usize,
    pub opened: usize,
    pub windows_placed: usize,
    pub windows_expected: usize,
    pub unavailable: usize,
    pub failed: usize,
    pub skipped: usize,
}

pub struct Options {
    /// Step keys to run; `None` runs every available step.
    pub only: Option<Vec<String>>,
    pub skip: Vec<String>,
    pub arrange_open_windows: bool,
    /// How long to wait for started applications to show a window.
    pub timeout: Duration,
}

impl Options {
    pub fn for_card(card: &Card) -> Options {
        Options {
            only: None,
            skip: Vec::new(),
            arrange_open_windows: card.resume.arrange_open_windows,
            timeout: Duration::from_secs(20),
        }
    }
    fn selected(&self, key: &str) -> bool {
        !self.skip.iter().any(|s| s == key) && self.only.as_ref().is_none_or(|o| o.iter().any(|s| s == key))
    }
}

/// Whether an open window belongs to a saved application.
pub fn belongs(app: &AppEntry, w: &Window, index: &AppIndex) -> bool {
    let id = index.identify(w);
    let probe = AppEntry {
        name: id.name,
        desktop: id.desktop,
        exe: id.exe,
        class: id.class,
        windows: Vec::new(),
    };
    if probe.key() == app.key() {
        return true;
    }
    // An application recognised by its class only at capture (no menu entry
    // then) still matches its windows by that class.
    !app.class.is_empty()
        && app.desktop.is_none()
        && app.exe.is_none()
        && (w.class.eq_ignore_ascii_case(&app.class) || w.instance.eq_ignore_ascii_case(&app.class))
}

fn installed(app: &AppEntry, index: &AppIndex) -> Result<(), String> {
    if let Some(d) = &app.desktop {
        return match index.get(d) {
            Some(e) if e.terminal => Err("runs in a terminal".into()),
            Some(_) => Ok(()),
            None => Err("not installed".into()),
        };
    }
    if let Some(e) = &app.exe {
        return if cfg!(windows) && Path::new(e).is_file() {
            Ok(())
        } else {
            Err("not installed".into())
        };
    }
    Err("Lyapunov cannot start this application (no menu entry)".into())
}

pub fn plan(card: &Card, snap: &Snapshot, index: &AppIndex, backend: &dyn Backend, limitation: Option<String>) -> Plan {
    let mut steps = Vec::new();
    for (i, app) in card.apps.iter().enumerate() {
        let open = snap.windows.iter().filter(|w| belongs(app, w, index)).count();
        let (kind, reason) = if open > 0 {
            (StepKind::Running, None)
        } else {
            match installed(app, index) {
                Ok(()) => (StepKind::Launch, None),
                Err(r) => (StepKind::Unavailable, Some(r)),
            }
        };
        steps.push(Step {
            key: format!("app:{i}"),
            kind,
            name: app.name.clone(),
            windows: app.windows.len(),
            open,
            path: None,
            folder: None,
            reason,
        });
    }
    for (i, place) in card.places.iter().enumerate() {
        let exists = Path::new(&place.path).exists();
        let name = Path::new(&place.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| place.path.clone());
        steps.push(Step {
            key: format!("place:{i}"),
            kind: if exists { StepKind::Open } else { StepKind::Unavailable },
            name,
            windows: 0,
            open: 0,
            path: Some(place.path.clone()),
            folder: Some(place.kind == PlaceKind::Folder),
            reason: (!exists).then(|| "not found".into()),
        });
    }
    Plan {
        card: card.id.clone(),
        name: card.name.clone(),
        backend: backend.name().into(),
        capabilities: backend.capabilities(),
        limitation,
        arrange_open_windows: card.resume.arrange_open_windows,
        steps,
    }
}

/// Maps a saved window onto the screens present now. A window whose screen
/// is gone or changed size keeps its relative position and size on the
/// primary (or same-named) screen; every window ends fully on a screen.
pub fn fit(slot: &WindowSlot, snap: &Snapshot) -> (Rect, Option<String>) {
    let r = slot.rect;
    if snap.outputs.is_empty() {
        return (r, slot.output.clone());
    }
    let same = slot.output.as_deref().and_then(|n| snap.output(n));
    let target = same
        .or_else(|| {
            slot.output_rect
                .and_then(|old| snap.outputs.iter().find(|o| o.rect == old))
        })
        .or_else(|| {
            if slot.output.is_none() {
                snap.output_at(&r)
            } else {
                None
            }
        })
        .or_else(|| snap.primary_output())
        .expect("outputs is not empty");
    let mut out = r;
    if let Some(old) = slot.output_rect.filter(|o| !o.is_empty()) {
        if old != target.rect {
            let sx = f64::from(target.rect.width) / f64::from(old.width);
            let sy = f64::from(target.rect.height) / f64::from(old.height);
            out = Rect::new(
                target.rect.x + (f64::from(r.x - old.x) * sx).round() as i32,
                target.rect.y + (f64::from(r.y - old.y) * sy).round() as i32,
                (f64::from(r.width) * sx).round() as i32,
                (f64::from(r.height) * sy).round() as i32,
            );
        }
    }
    let t = target.rect;
    out.width = out.width.clamp(1, t.width.max(1));
    out.height = out.height.clamp(1, t.height.max(1));
    out.x = out.x.clamp(t.x, t.right() - out.width);
    out.y = out.y.clamp(t.y, t.bottom() - out.height);
    (out, Some(target.name.clone()))
}

/// The current desktop that best matches a saved one: same identifier, then
/// same name, then same position.
fn find_desktop(slot: &WindowSlot, snap: &Snapshot) -> Option<String> {
    let d = slot.desktop.as_ref()?;
    snap.desktops
        .iter()
        .find(|x| d.id.as_deref() == Some(x.id.as_str()))
        .or_else(|| {
            snap.desktops
                .iter()
                .find(|x| d.name.as_deref() == Some(x.name.as_str()))
        })
        .or_else(|| snap.desktops.iter().find(|x| d.index == Some(x.index)))
        .map(|x| x.id.clone())
}

pub fn placement(slot: &WindowSlot, w: &Window, snap: &Snapshot, caps: Capabilities) -> (Placement, Vec<String>) {
    let mut notes = Vec::new();
    let (rect, output) = fit(slot, snap);
    if slot.output.is_some() && output != slot.output {
        notes.push(format!(
            "screen {} is not connected; moved to {}",
            slot.output.as_deref().unwrap_or("?"),
            output.as_deref().unwrap_or("another screen")
        ));
    }
    let desktop = if caps.desktops { find_desktop(slot, snap) } else { None };
    if caps.desktops && slot.desktop.is_some() && desktop.is_none() {
        let wanted = slot
            .desktop
            .as_ref()
            .and_then(|d| d.name.clone().or(d.id.clone()))
            .unwrap_or_default();
        if caps.tiling {
            notes.push(format!("workspace {wanted} will be created"));
        } else {
            notes.push(format!(
                "desktop {wanted} no longer exists; left on its current desktop"
            ));
        }
    }
    // Tiling window managers create a workspace when a window is sent to it.
    let desktop = desktop.or_else(|| {
        if caps.tiling {
            slot.desktop.as_ref().and_then(|d| d.name.clone().or(d.id.clone()))
        } else {
            None
        }
    });
    let state = if caps.states { slot.state } else { WindowState::Normal };
    (
        Placement {
            handle: w.handle.clone(),
            rect: if caps.geometry { Some(rect) } else { None },
            state,
            desktop,
            output,
            floating: slot.floating,
        },
        notes,
    )
}

/// Progress for the desktop's status line, one event at a time.
#[derive(Serialize, Debug)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Progress<'a> {
    Starting { key: &'a str, name: &'a str },
    Opening { key: &'a str, name: &'a str },
    Waiting { pending: usize },
    Arranging { windows: usize },
}

pub fn execute(
    card: &Card,
    opts: &Options,
    backend: &mut dyn Backend,
    index: &AppIndex,
    mut progress: impl FnMut(Progress),
) -> Result<Report> {
    let caps = backend.capabilities();
    let before = if caps.list {
        backend.snapshot()?
    } else {
        Snapshot::default()
    };
    let pl = plan(card, &before, index, backend, None);
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut launched: Vec<usize> = Vec::new();

    for (i, app) in card.apps.iter().enumerate() {
        let step = &pl.steps[i];
        let mut o = Outcome {
            key: step.key.clone(),
            name: app.name.clone(),
            status: Status::Skipped,
            placed: 0,
            windows: app.windows.len(),
            notes: Vec::new(),
        };
        if !opts.selected(&step.key) {
            outcomes.push(o);
            continue;
        }
        match step.kind {
            StepKind::Unavailable => {
                o.status = Status::Unavailable;
                o.notes.extend(step.reason.clone());
            }
            StepKind::Running => {
                o.status = if opts.arrange_open_windows && caps.geometry && !app.windows.is_empty() {
                    Status::WindowFound
                } else {
                    Status::LeftOpen
                };
            }
            StepKind::Launch => {
                progress(Progress::Starting {
                    key: &step.key,
                    name: &app.name,
                });
                let started = match (&app.desktop, &app.exe) {
                    (Some(d), _) => index.launch(d).map(|_| ()),
                    (None, Some(e)) => apps::spawn_detached(std::process::Command::new(e)).map(|_| ()),
                    _ => Err(anyhow::anyhow!("cannot be started")),
                };
                match started {
                    Ok(()) => {
                        o.status = Status::AppLaunched;
                        launched.push(i);
                    }
                    Err(e) => {
                        o.status = Status::Failed;
                        o.notes.push(format!("{e:#}"));
                    }
                }
            }
            StepKind::Open => unreachable!("applications are never Open steps"),
        }
        outcomes.push(o);
    }

    for (j, place) in card.places.iter().enumerate() {
        let step = &pl.steps[card.apps.len() + j];
        let mut o = Outcome {
            key: step.key.clone(),
            name: step.name.clone(),
            status: Status::Skipped,
            placed: 0,
            windows: 0,
            notes: Vec::new(),
        };
        if opts.selected(&step.key) {
            if step.kind == StepKind::Unavailable {
                o.status = Status::Unavailable;
                o.notes.push(format!("{} was not found", place.path));
            } else {
                progress(Progress::Opening {
                    key: &step.key,
                    name: &step.name,
                });
                match apps::open_place(Path::new(&place.path)) {
                    Ok(()) => o.status = Status::Opened,
                    Err(e) => {
                        o.status = Status::Failed;
                        o.notes.push(format!("{e:#}"));
                    }
                }
            }
        }
        outcomes.push(o);
    }

    let wants_windows = |i: usize| !card.apps[i].windows.is_empty();
    if caps.list && caps.geometry {
        // Wait until each started application shows a window, then a moment
        // more: many applications resize themselves right after appearing.
        let mut snap = before.clone();
        let waiting: Vec<usize> = launched.iter().copied().filter(|i| wants_windows(*i)).collect();
        if !waiting.is_empty() {
            let deadline = Instant::now() + opts.timeout;
            loop {
                let pending = waiting
                    .iter()
                    .filter(|i| !snap.windows.iter().any(|w| belongs(&card.apps[**i], w, index)))
                    .count();
                if pending == 0 || Instant::now() >= deadline {
                    break;
                }
                progress(Progress::Waiting { pending });
                std::thread::sleep(Duration::from_millis(400));
                snap = backend.snapshot()?;
            }
            std::thread::sleep(Duration::from_millis(900));
            snap = backend.snapshot()?;
        }

        let known: HashSet<&str> = before.windows.iter().map(|w| w.handle.as_str()).collect();
        let mut claimed: HashSet<String> = HashSet::new();
        let mut placements: Vec<(usize, Placement)> = Vec::new();
        for (i, app) in card.apps.iter().enumerate() {
            let o = &outcomes[i];
            let arrange = match o.status {
                Status::AppLaunched => true,
                Status::WindowFound => opts.arrange_open_windows,
                _ => false,
            };
            if !arrange || app.windows.is_empty() {
                continue;
            }
            // New windows first, then those that were already open, each in
            // stacking order, matched to the saved windows in order.
            let mut mine: Vec<&Window> = snap
                .windows
                .iter()
                .filter(|w| !claimed.contains(&w.handle) && belongs(app, w, index))
                .collect();
            mine.sort_by_key(|w| known.contains(w.handle.as_str()));
            for (slot, w) in app.windows.iter().zip(mine) {
                claimed.insert(w.handle.clone());
                let (p, notes) = placement(slot, w, &snap, caps);
                outcomes[i].notes.extend(notes);
                placements.push((i, p));
            }
        }
        if !placements.is_empty() {
            progress(Progress::Arranging {
                windows: placements.len(),
            });
            let ops: Vec<Placement> = placements.iter().map(|(_, p)| p.clone()).collect();
            let mut results = backend.place(&ops)?;
            debug("placed", &(&ops, &results));
            // A second pass for windows an application moved again after the
            // first one (session restore, remembered geometry).
            if !caps.tiling {
                // Wait until every placed window reports a real size: right
                // after a resize, a compositor may report an empty frame while
                // the application commits the new one. Those are never moved
                // again; a second request would race the first.
                let mut now = Snapshot::default();
                for _ in 0..6 {
                    std::thread::sleep(Duration::from_millis(500));
                    now = backend.snapshot()?;
                    let settling = ops
                        .iter()
                        .any(|p| now.windows.iter().any(|w| w.handle == p.handle && w.rect.is_empty()));
                    if !settling {
                        break;
                    }
                }
                let again: Vec<Placement> = ops
                    .iter()
                    .filter(|p| p.state == WindowState::Normal)
                    .filter(|p| {
                        let ok = results.iter().any(|r| r.handle == p.handle && r.ok);
                        ok && now.windows.iter().any(|w| {
                            w.handle == p.handle && !w.rect.is_empty() && p.rect.is_some_and(|r| !near(&w.rect, &r))
                        })
                    })
                    .cloned()
                    .collect();
                if !again.is_empty() {
                    debug("moved again by the application", &now.windows);
                    for r2 in backend.place(&again)? {
                        if let Some(r) = results.iter_mut().find(|r| r.handle == r2.handle) {
                            *r = r2;
                        }
                    }
                }
            }
            for (i, p) in &placements {
                match results.iter().find(|r| r.handle == p.handle) {
                    Some(r) if r.ok => {
                        outcomes[*i].placed += 1;
                        outcomes[*i].notes.extend(r.note.clone());
                    }
                    Some(r) => outcomes[*i]
                        .notes
                        .push(r.note.clone().unwrap_or_else(|| "a window could not be moved".into())),
                    None => outcomes[*i].notes.push("a window could not be moved".into()),
                }
            }
        }
        for (i, app) in card.apps.iter().enumerate() {
            let o = &mut outcomes[i];
            let arranged = matches!(o.status, Status::AppLaunched | Status::WindowFound);
            if arranged && o.placed < app.windows.len() {
                let missing = app.windows.len() - o.placed;
                if o.status == Status::AppLaunched && o.placed == 0 && !app.windows.is_empty() {
                    o.notes
                        .push("started, but no window appeared in time to be arranged".into());
                } else if missing > 0 {
                    o.notes.push(format!("{missing} saved window(s) not reopened"));
                }
            }
            if o.status == Status::WindowFound && o.placed == 0 {
                o.status = Status::LeftOpen;
            }
        }
    } else if !caps.geometry {
        for (i, app) in card.apps.iter().enumerate() {
            if !app.windows.is_empty() && matches!(outcomes[i].status, Status::AppLaunched | Status::LeftOpen) {
                outcomes[i]
                    .notes
                    .push("windows are not arranged on this desktop".into());
            }
        }
    }

    let mut summary = Summary::default();
    for o in &outcomes {
        match o.status {
            Status::WindowFound => summary.windows_found += 1,
            Status::AppLaunched => summary.apps_launched += 1,
            Status::Opened => summary.opened += 1,
            Status::Unavailable => summary.unavailable += 1,
            Status::Failed => summary.failed += 1,
            Status::Skipped => summary.skipped += 1,
            Status::LeftOpen => {}
        }
        if o.status != Status::Skipped {
            summary.windows_placed += o.placed;
            summary.windows_expected += o.windows;
        }
    }
    Ok(Report {
        card: card.id.clone(),
        name: card.name.clone(),
        backend: backend.name().into(),
        outcomes,
        summary,
    })
}

/// With `LYAPUNOV_DEBUG=1`, what was asked of the backend and what it answered
/// goes to standard error, for troubleshooting.
fn debug<T: Serialize>(what: &str, value: &T) {
    if std::env::var_os("LYAPUNOV_DEBUG").is_some_and(|v| v != "0") {
        eprintln!("lyapunov: {what}: {}", serde_json::to_string(value).unwrap_or_default());
    }
}

fn near(a: &Rect, b: &Rect) -> bool {
    (a.x - b.x).abs() <= 12
        && (a.y - b.y).abs() <= 12
        && (a.width - b.width).abs() <= 12
        && (a.height - b.height).abs() <= 12
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Desktop, Output, PlaceResult};
    use crate::card::DesktopRef;

    fn screens(outputs: &[(&str, Rect)]) -> Snapshot {
        Snapshot {
            outputs: outputs
                .iter()
                .enumerate()
                .map(|(i, (n, r))| Output {
                    name: (*n).into(),
                    rect: *r,
                    primary: i == 0,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_window_on_a_present_screen_keeps_its_place() {
        let snap = screens(&[("DP-1", Rect::new(0, 0, 1920, 1080))]);
        let slot = WindowSlot {
            rect: Rect::new(100, 50, 800, 600),
            output: Some("DP-1".into()),
            output_rect: Some(Rect::new(0, 0, 1920, 1080)),
            ..Default::default()
        };
        assert_eq!(fit(&slot, &snap), (Rect::new(100, 50, 800, 600), Some("DP-1".into())));
    }

    #[test]
    fn a_window_from_a_missing_screen_lands_on_the_primary_one() {
        let snap = screens(&[("eDP-1", Rect::new(0, 0, 1280, 800))]);
        let slot = WindowSlot {
            rect: Rect::new(2880, 540, 960, 540),
            output: Some("HDMI-1".into()),
            output_rect: Some(Rect::new(1920, 0, 1920, 1080)),
            ..Default::default()
        };
        let (r, out) = fit(&slot, &snap);
        assert_eq!(out.as_deref(), Some("eDP-1"));
        // Right half, lower half of the old screen -> same on the new one.
        assert_eq!(r, Rect::new(640, 400, 640, 400));
    }

    #[test]
    fn windows_never_end_off_screen() {
        let snap = screens(&[("DP-1", Rect::new(0, 0, 1920, 1080))]);
        for rect in [
            Rect::new(-5000, -5000, 300, 200),
            Rect::new(1900, 1070, 4000, 3000),
            Rect::new(0, 0, 0, 0),
        ] {
            let slot = WindowSlot {
                rect,
                output: Some("DP-1".into()),
                ..Default::default()
            };
            let (r, _) = fit(&slot, &snap);
            assert!(
                r.x >= 0 && r.y >= 0 && r.right() <= 1920 && r.bottom() <= 1080 && !r.is_empty(),
                "{r:?}"
            );
        }
    }

    #[test]
    fn desktops_are_found_by_id_then_name_then_position() {
        let mut snap = screens(&[("DP-1", Rect::new(0, 0, 1920, 1080))]);
        snap.desktops = vec![
            Desktop {
                id: "a".into(),
                name: "Mail".into(),
                index: 0,
            },
            Desktop {
                id: "b".into(),
                name: "Code".into(),
                index: 1,
            },
        ];
        let slot = |id: &str, name: &str, index: u32| WindowSlot {
            desktop: Some(DesktopRef {
                id: Some(id.into()),
                name: Some(name.into()),
                index: Some(index),
            }),
            ..Default::default()
        };
        assert_eq!(find_desktop(&slot("b", "x", 9), &snap).as_deref(), Some("b"));
        assert_eq!(find_desktop(&slot("z", "Code", 9), &snap).as_deref(), Some("b"));
        assert_eq!(find_desktop(&slot("z", "x", 0), &snap).as_deref(), Some("a"));
        assert_eq!(find_desktop(&slot("z", "x", 7), &snap), None);
    }

    /// A backend that records placements and makes started apps appear.
    struct Fake {
        snap: Snapshot,
        placed: Vec<Placement>,
    }

    impl Backend for Fake {
        fn name(&self) -> &'static str {
            "fake"
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
            Ok(self.snap.clone())
        }
        fn place(&mut self, p: &[Placement]) -> Result<Vec<PlaceResult>> {
            self.placed.extend_from_slice(p);
            for op in p {
                if let Some(w) = self.snap.windows.iter_mut().find(|w| w.handle == op.handle) {
                    if let Some(r) = op.rect {
                        w.rect = r;
                    }
                }
            }
            Ok(p.iter()
                .map(|op| PlaceResult {
                    handle: op.handle.clone(),
                    ok: true,
                    note: None,
                })
                .collect())
        }
    }

    fn card() -> Card {
        let mut c = Card::new("Site");
        let slot = |x| WindowSlot {
            rect: Rect::new(x, 0, 500, 400),
            output: Some("DP-1".into()),
            ..Default::default()
        };
        c.apps.push(AppEntry {
            name: "Kate".into(),
            class: "kate".into(),
            windows: vec![slot(0), slot(600)],
            ..Default::default()
        });
        c.apps.push(AppEntry {
            name: "Gone".into(),
            desktop: Some("gone.desktop".into()),
            windows: vec![slot(0)],
            ..Default::default()
        });
        c.places.push(crate::card::Place {
            path: "/nonexistent/lyapunov-test".into(),
            kind: PlaceKind::Folder,
        });
        c
    }

    fn fake() -> Fake {
        let mut snap = screens(&[("DP-1", Rect::new(0, 0, 1920, 1080))]);
        snap.windows = vec![
            Window {
                handle: "k1".into(),
                class: "kate".into(),
                rect: Rect::new(50, 50, 10, 10),
                ..Default::default()
            },
            Window {
                handle: "k2".into(),
                class: "kate".into(),
                rect: Rect::new(70, 70, 10, 10),
                ..Default::default()
            },
            Window {
                handle: "x".into(),
                class: "other".into(),
                ..Default::default()
            },
        ];
        Fake {
            snap,
            placed: Vec::new(),
        }
    }

    #[test]
    fn plan_reuses_open_apps_and_names_missing_ones() {
        let f = fake();
        let p = plan(&card(), &f.snap, &AppIndex::default(), &f, None);
        let kinds: Vec<_> = p.steps.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [StepKind::Running, StepKind::Unavailable, StepKind::Unavailable]);
        assert_eq!(p.steps[0].open, 2);
        assert_eq!(p.steps[1].reason.as_deref(), Some("not installed"));
        assert_eq!(p.steps[2].reason.as_deref(), Some("not found"));
    }

    #[test]
    fn resume_arranges_found_windows_and_reports_the_rest() {
        let mut f = fake();
        let c = card();
        let r = execute(&c, &Options::for_card(&c), &mut f, &AppIndex::default(), |_| {}).unwrap();
        assert_eq!(f.placed.len(), 2);
        assert_eq!(f.placed[0].rect, Some(Rect::new(0, 0, 500, 400)));
        assert_eq!(f.placed[1].rect, Some(Rect::new(600, 0, 500, 400)));
        assert_eq!(r.outcomes[0].status, Status::WindowFound);
        assert_eq!(r.outcomes[0].placed, 2);
        assert_eq!(r.outcomes[1].status, Status::Unavailable);
        assert_eq!(r.outcomes[2].status, Status::Unavailable);
        assert_eq!(r.summary.windows_placed, 2);
        assert_eq!(r.summary.windows_expected, 3);
        assert_eq!(r.summary.unavailable, 2);
    }

    #[test]
    fn left_out_steps_and_open_windows_are_respected() {
        let mut f = fake();
        let c = card();
        let mut o = Options::for_card(&c);
        o.arrange_open_windows = false;
        o.skip = vec!["place:0".into()];
        let r = execute(&c, &o, &mut f, &AppIndex::default(), |_| {}).unwrap();
        assert!(f.placed.is_empty(), "open windows stay where they are");
        assert_eq!(r.outcomes[0].status, Status::LeftOpen);
        assert_eq!(r.outcomes[2].status, Status::Skipped);
        let mut f = fake();
        let mut o = Options::for_card(&c);
        o.only = Some(vec!["app:1".into()]);
        let r = execute(&c, &o, &mut f, &AppIndex::default(), |_| {}).unwrap();
        assert!(f.placed.is_empty());
        assert_eq!(r.outcomes[0].status, Status::Skipped);
    }
}
