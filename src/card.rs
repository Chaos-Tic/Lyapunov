//! A card: one activity, the applications and windows that belong to it, and
//! the folders and documents the user chose to keep with it.
//!
//! Cards are plain JSON files. The format is versioned; a card written by a
//! newer Lyapunov is reported, never rewritten. Window titles and URLs are not
//! stored: an application's identity and the user's explicit choices suffice.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "lyapunov-card";
pub const VERSION: u32 = 2;

pub const MAX_NAME: usize = 80;
pub const MAX_NOTE: usize = 280;
pub const MAX_APPS: usize = 64;
pub const MAX_WINDOWS: usize = 32;
pub const MAX_PLACES: usize = 128;
const MAX_TEXT: usize = 512;
const MAX_COORD: i32 = 1_000_000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tint {
    #[default]
    Peach,
    Sky,
    Lilac,
    Mint,
    Amber,
    Rose,
    Slate,
}

impl Tint {
    pub const ALL: [Tint; 7] = [
        Tint::Peach,
        Tint::Sky,
        Tint::Lilac,
        Tint::Mint,
        Tint::Amber,
        Tint::Rose,
        Tint::Slate,
    ];
    /// A stable default for a new card, spread across the palette.
    pub fn for_index(i: usize) -> Tint {
        Tint::ALL[i % Tint::ALL.len()]
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowState {
    #[default]
    Normal,
    Maximized,
    Minimized,
    Fullscreen,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect { x, y, width, height }
    }
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width)
    }
    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height)
    }
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }
}

/// A virtual desktop or workspace, recorded by every name the platform gives it,
/// so it can be found again even after its identifier changed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct DesktopRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

/// Where one window of an application was, and in which state.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct WindowSlot {
    pub rect: Rect,
    #[serde(default)]
    pub state: WindowState,
    /// The screen's connector name (`DP-1`, `\\.\DISPLAY2`) and its geometry at
    /// capture, so the window can be moved proportionally if it is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_rect: Option<Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop: Option<DesktopRef>,
    /// Tiling window managers: the window floated rather than tiled.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub floating: bool,
}

/// How an application is found and started again.
///
/// - `desktop`: a desktop entry ID (`org.kde.dolphin.desktop`), started through
///   its `Exec` line, without a shell.
/// - `exe`: an executable path (Windows), started with no arguments.
/// - `class` alone: recognised only to arrange its windows, never started.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct AppEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub class: String,
    /// Explicit local folder to open in VS Code. Never inferred from a title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default)]
    pub windows: Vec<WindowSlot>,
}

impl AppEntry {
    /// A stable key that identifies the application across captures.
    pub fn key(&self) -> String {
        if let Some(d) = &self.desktop {
            return format!("desktop:{d}");
        }
        if let Some(e) = &self.exe {
            return format!("exe:{}", e.to_lowercase());
        }
        format!("class:{}", self.class.to_lowercase())
    }
    pub fn launchable(&self) -> bool {
        self.desktop.is_some() || self.exe.is_some()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlaceKind {
    Folder,
    File,
}

/// A folder or document the user explicitly kept with the card.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub path: String,
    pub kind: PlaceKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumePrefs {
    /// Show the planned actions before resuming.
    #[serde(default = "yes")]
    pub ask: bool,
    /// Opt in to moving windows that are already open. App identity alone
    /// cannot establish which project or document an existing window holds.
    #[serde(default)]
    pub arrange_open_windows: bool,
}

fn yes() -> bool {
    true
}

impl Default for ResumePrefs {
    fn default() -> Self {
        ResumePrefs {
            ask: true,
            arrange_open_windows: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub created: String,
    pub modified: String,
    #[serde(default)]
    pub tint: Tint,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub places: Vec<Place>,
    #[serde(default)]
    pub resume: ResumePrefs,
}

impl Card {
    pub fn new(name: &str) -> Card {
        let now = crate::time::now_rfc3339();
        Card {
            format: FORMAT.into(),
            version: VERSION,
            id: new_id(),
            name: name.trim().into(),
            note: String::new(),
            created: now.clone(),
            modified: now,
            tint: Tint::default(),
            apps: Vec::new(),
            places: Vec::new(),
            resume: ResumePrefs::default(),
        }
    }

    pub fn window_count(&self) -> usize {
        self.apps.iter().map(|a| a.windows.len()).sum()
    }

    /// Checks every field a card may contain. Imported or hand-edited cards go
    /// through the same rules as captured ones.
    pub fn validate(&self) -> Result<()> {
        if self.format != FORMAT {
            bail!("not a Lyapunov card (format {:?})", self.format);
        }
        if self.version > VERSION {
            bail!(
                "this card was made by a newer Lyapunov (format version {}); update Lyapunov to open it",
                self.version
            );
        }
        if self.version == 0 {
            bail!("invalid format version 0");
        }
        if !valid_id(&self.id) {
            bail!("invalid card identifier");
        }
        let name = self.name.trim();
        if name.is_empty() {
            bail!("the card needs a name");
        }
        if name.chars().count() > MAX_NAME {
            bail!("the name is longer than {MAX_NAME} characters");
        }
        check_text("name", &self.name)?;
        if self.note.chars().count() > MAX_NOTE {
            bail!("the note is longer than {MAX_NOTE} characters");
        }
        check_text("note", &self.note)?;
        if !crate::time::valid_rfc3339(&self.created) || !crate::time::valid_rfc3339(&self.modified) {
            bail!("invalid date");
        }
        if self.apps.len() > MAX_APPS {
            bail!("more than {MAX_APPS} applications");
        }
        for app in &self.apps {
            check_text("application name", &app.name)?;
            if app.name.trim().is_empty() {
                bail!("an application has no name");
            }
            if let Some(d) = &app.desktop {
                if !valid_desktop_id(d) {
                    bail!("invalid desktop entry {d:?}");
                }
            }
            if let Some(e) = &app.exe {
                check_text("executable", e)?;
                if !valid_exe(e) {
                    bail!("invalid executable path {e:?}");
                }
            }
            check_text("window class", &app.class)?;
            if let Some(folder) = &app.workspace {
                check_text("workspace", folder)?;
                if !is_absolute_anywhere(folder) || is_network_path(folder) {
                    bail!("the VS Code workspace must be an absolute local folder");
                }
                if app.desktop.as_deref() != Some("code.desktop") {
                    bail!("workspace folders are currently supported only for VS Code");
                }
            }
            if app.desktop.is_none() && app.exe.is_none() && app.class.trim().is_empty() {
                bail!("application {:?} cannot be recognised", app.name);
            }
            if app.windows.len() > MAX_WINDOWS {
                bail!("more than {MAX_WINDOWS} windows for {:?}", app.name);
            }
            for w in &app.windows {
                check_rect(&w.rect)?;
                if let Some(r) = &w.output_rect {
                    check_rect(r)?;
                }
                if let Some(o) = &w.output {
                    check_text("screen", o)?;
                }
                if let Some(d) = &w.desktop {
                    if let Some(id) = &d.id {
                        check_text("desktop", id)?;
                    }
                    if let Some(n) = &d.name {
                        check_text("desktop", n)?;
                    }
                }
            }
        }
        if self.places.len() > MAX_PLACES {
            bail!("more than {MAX_PLACES} folders and documents");
        }
        for p in &self.places {
            check_text("path", &p.path)?;
            if !is_absolute_anywhere(&p.path) {
                bail!("{:?} is not an absolute path", p.path);
            }
        }
        Ok(())
    }

    /// Removes duplicate applications and places, keeping the first of each.
    pub fn dedupe(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.apps.retain(|a| seen.insert((a.key(), a.workspace.clone())));
        let mut paths = std::collections::HashSet::new();
        self.places.retain(|p| paths.insert(p.path.clone()));
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok() && id.len() == 36
}

/// Desktop entry IDs per the XDG specification: `name.desktop`, with letters,
/// digits, `-`, `_` and `.` only. No separators, so it never escapes a folder.
pub fn valid_desktop_id(id: &str) -> bool {
    id.len() <= 255
        && id.ends_with(".desktop")
        && id.len() > ".desktop".len()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
}

/// Windows executables: an absolute path to an `.exe`, nothing else.
pub fn valid_exe(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".exe") && is_absolute_anywhere(path) && !path.contains('"')
}

/// Absolute on Linux (`/home/…`) or on Windows (`C:\…`), so a card stays valid
/// when inspected on another platform. Network paths (`\\server\share`,
/// `//server`) are refused: opening one would contact that server, and on
/// Windows send it the user's credentials.
pub fn is_absolute_anywhere(path: &str) -> bool {
    if is_network_path(path) {
        return false;
    }
    if path.starts_with('/') {
        return true;
    }
    let b = path.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

pub fn is_network_path(path: &str) -> bool {
    let p = path.trim_start();
    p.starts_with("\\\\") || p.starts_with("//") || p.starts_with("\\/") || p.starts_with("/\\")
}

fn check_text(what: &str, value: &str) -> Result<()> {
    if value.len() > MAX_TEXT {
        bail!("the {what} is too long");
    }
    if value.chars().any(|c| c == '\0' || (c.is_control() && c != '\t')) {
        bail!("the {what} contains control characters");
    }
    Ok(())
}

fn check_rect(r: &Rect) -> Result<()> {
    let ok = |v: i32| (-MAX_COORD..=MAX_COORD).contains(&v);
    if !(ok(r.x) && ok(r.y) && (0..=MAX_COORD).contains(&r.width) && (0..=MAX_COORD).contains(&r.height)) {
        bail!("window geometry out of range");
    }
    Ok(())
}

/// What an update from the current state would change.
#[derive(Serialize, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub added_apps: Vec<String>,
    pub removed_apps: Vec<String>,
    pub moved_apps: Vec<String>,
    pub added_places: Vec<String>,
    pub removed_places: Vec<String>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        *self == Diff::default()
    }
}

pub fn diff(old: &Card, new: &Card) -> Diff {
    let mut d = Diff::default();
    for a in &new.apps {
        match old.apps.iter().find(|o| o.key() == a.key()) {
            None => d.added_apps.push(a.name.clone()),
            Some(o) if o.windows != a.windows || o.workspace != a.workspace => d.moved_apps.push(a.name.clone()),
            Some(_) => {}
        }
    }
    for o in &old.apps {
        if !new.apps.iter().any(|a| a.key() == o.key()) {
            d.removed_apps.push(o.name.clone());
        }
    }
    for p in &new.places {
        if !old.places.iter().any(|o| o.path == p.path) {
            d.added_places.push(p.path.clone());
        }
    }
    for o in &old.places {
        if !new.places.iter().any(|p| p.path == o.path) {
            d.removed_places.push(o.path.clone());
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Card {
        let mut c = Card::new("Site personnel");
        c.apps.push(AppEntry {
            name: "Dolphin".into(),
            desktop: Some("org.kde.dolphin.desktop".into()),
            class: "dolphin".into(),
            windows: vec![WindowSlot {
                rect: Rect::new(10, 20, 800, 600),
                ..Default::default()
            }],
            ..Default::default()
        });
        c.places.push(Place {
            path: "/home/user/site".into(),
            kind: PlaceKind::Folder,
        });
        c
    }

    #[test]
    fn a_captured_card_is_valid() {
        sample().validate().unwrap();
    }

    #[test]
    fn workspace_requires_a_local_absolute_folder_and_vscode() {
        let mut c = sample();
        c.apps[0].workspace = Some("/home/user/site".into());
        assert!(c.validate().is_err());
        c.apps[0].desktop = Some("code.desktop".into());
        assert!(c.validate().is_ok());
        c.apps[0].workspace = Some("relative/site".into());
        assert!(c.validate().is_err());
        c.apps[0].workspace = Some("//server/site".into());
        assert!(c.validate().is_err());
    }

    #[test]
    fn rejects_hostile_or_broken_fields() {
        let mut c = sample();
        c.apps[0].desktop = Some("../../bin/sh.desktop".into());
        assert!(c.validate().is_err());
        let mut c = sample();
        c.name = "   ".into();
        assert!(c.validate().is_err());
        let mut c = sample();
        c.name = "a\u{1b}[2Jb".into();
        assert!(c.validate().is_err());
        let mut c = sample();
        c.places[0].path = "relative/path".into();
        assert!(c.validate().is_err());
        let mut c = sample();
        c.apps[0].exe = Some("cmd.exe /c del".into());
        assert!(c.validate().is_err());
        let mut c = sample();
        c.version = VERSION + 1;
        assert!(c.validate().unwrap_err().to_string().contains("newer"));
        let mut c = sample();
        c.apps[0].windows[0].rect.width = -5;
        assert!(c.validate().is_err());
    }

    #[test]
    fn windows_paths_are_absolute_too() {
        assert!(is_absolute_anywhere("C:\\Users\\a\\Documents"));
        assert!(
            !is_absolute_anywhere("\\\\attacker\\share\\doc.pdf"),
            "network paths are refused"
        );
        assert!(!is_absolute_anywhere("//attacker/share"));
        assert!(!valid_exe("\\\\attacker\\share\\app.exe"));
        assert!(valid_exe("C:\\Program Files\\App\\app.exe"));
        assert!(!valid_exe("app.exe"));
    }

    #[test]
    fn diff_names_what_changes() {
        let old = sample();
        let mut new = old.clone();
        new.apps[0].windows[0].rect.x = 400;
        new.apps.push(AppEntry {
            name: "Kate".into(),
            desktop: Some("org.kde.kate.desktop".into()),
            ..Default::default()
        });
        new.places.clear();
        let d = diff(&old, &new);
        assert_eq!(d.added_apps, ["Kate"]);
        assert_eq!(d.moved_apps, ["Dolphin"]);
        assert_eq!(d.removed_places, ["/home/user/site"]);
        assert!(diff(&old, &old).is_empty());
    }

    #[test]
    fn dedupe_keeps_the_first_of_each() {
        let mut c = sample();
        c.apps.push(c.apps[0].clone());
        c.places.push(c.places[0].clone());
        c.dedupe();
        assert_eq!(c.apps.len(), 1);
        assert_eq!(c.places.len(), 1);
    }
}
