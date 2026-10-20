//! The launcher entry and icon, for one user (`~/.local/share`) or for a
//! package (`--data-dir $pkgdir/usr/share`).

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub const ICON: &str = include_str!("../assets/lyapunov.svg");
pub const SYMBOLIC_ICON: &str = include_str!("../assets/lyapunov-symbolic.svg");
pub const DESKTOP_ID: &str = "lyapunov.desktop";
pub const GNOME_UUID: &str = "lyapunov@chaos-tic.github.io";
const GNOME_METADATA: &str = include_str!("../integrations/gnome-shell/lyapunov@chaos-tic.github.io/metadata.json");
const GNOME_EXTENSION: &str = include_str!("../integrations/gnome-shell/lyapunov@chaos-tic.github.io/extension.js");

/// Quotes a program path for an `Exec` line: inside double quotes, `"`, `` ` ``,
/// `$` and `\` are escaped, and `%` is doubled.
pub fn exec_quote(path: &str) -> String {
    let mut out = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn desktop_entry(exe: &str) -> String {
    let exec = exec_quote(exe);
    format!(
        "[Desktop Entry]
Type=Application
Name=Lyapunov
GenericName=Workspace cards
GenericName[fr]=Cartes d'espaces de travail
Comment=Save an activity as a card and pick it up later
Comment[fr]=Enregistrer une activité sous forme de carte et la reprendre plus tard
Exec={exec}
Icon=lyapunov
Terminal=false
Categories=Utility;
Keywords=workspace;session;windows;restore;resume;cards;espace;fenêtres;reprendre;cartes;
StartupNotify=true
StartupWMClass=lyapunov-desktop
SingleMainWindow=true
X-KDE-Shortcuts=Meta+Alt+Space
Actions=capture;

[Desktop Action capture]
Name=Capture this space
Name[fr]=Capturer cet espace
Exec={exec} --capture
"
    )
}

/// Every file `install` writes, relative to the data folder.
pub fn files() -> [&'static str; 3] {
    [
        "applications/lyapunov.desktop",
        "icons/hicolor/scalable/apps/lyapunov.svg",
        "icons/hicolor/symbolic/apps/lyapunov-symbolic.svg",
    ]
}

pub fn default_data_dir() -> Result<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        let d = PathBuf::from(d);
        if d.is_absolute() {
            return Ok(d);
        }
    }
    Ok(PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".local/share"))
}

pub fn install(data: &Path, exe: &str) -> Result<Vec<PathBuf>> {
    let [entry, icon, symbolic] = files().map(|f| data.join(f));
    let mut written = Vec::new();
    for (path, body) in [
        (&entry, desktop_entry(exe)),
        (&icon, ICON.to_string()),
        (&symbolic, SYMBOLIC_ICON.to_string()),
    ] {
        fs::create_dir_all(path.parent().expect("has a parent"))?;
        fs::write(path, body).with_context(|| format!("writing {}", path.display()))?;
        written.push(path.clone());
    }
    Ok(written)
}

pub fn remove(data: &Path) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for f in files() {
        let p = data.join(f);
        if p.exists() {
            fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
            removed.push(p);
        }
    }
    Ok(removed)
}

/// Where the GNOME Shell extension is installed, if anywhere: the user's copy
/// first, then the system's (from a package).
pub fn gnome_extension_dirs(data: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![data.join("gnome-shell/extensions").join(GNOME_UUID)];
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in system.split(':').filter(|d| !d.is_empty()) {
        dirs.push(Path::new(d).join("gnome-shell/extensions").join(GNOME_UUID));
    }
    dirs
}

pub fn gnome_extension_installed(data: &Path) -> Option<PathBuf> {
    gnome_extension_dirs(data)
        .into_iter()
        .find(|d| d.join("metadata.json").is_file())
}

/// Writes the extension for this user, unless a package already provides it.
pub fn install_gnome_extension(data: &Path) -> Result<PathBuf> {
    match gnome_extension_installed(data) {
        Some(found) => Ok(found),
        None => stage_gnome_extension(data),
    }
}

/// Writes the extension into a package's data folder.
pub fn stage_gnome_extension(data: &Path) -> Result<PathBuf> {
    let dir = data.join("gnome-shell/extensions").join(GNOME_UUID);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("metadata.json"), GNOME_METADATA)?;
    fs::write(dir.join("extension.js"), GNOME_EXTENSION)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::{parse_desktop_entry, split_exec};

    #[test]
    fn the_entry_starts_the_given_executable() {
        let text = desktop_entry("/opt/My $App/lyapunov-desktop");
        let e = parse_desktop_entry(&text, DESKTOP_ID, None).unwrap();
        assert_eq!(split_exec(&e.exec, &e).unwrap(), ["/opt/My $App/lyapunov-desktop"]);
        assert_eq!(e.startup_wm_class.as_deref(), Some("lyapunov-desktop"));
    }

    #[test]
    fn the_gnome_extension_is_embedded_whole() {
        let d = tempfile::tempdir().unwrap();
        let dir = stage_gnome_extension(d.path()).unwrap();
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("metadata.json")).unwrap()).unwrap();
        assert_eq!(meta["uuid"], GNOME_UUID);
        assert!(
            std::fs::read_to_string(dir.join("extension.js"))
                .unwrap()
                .contains("io.github.ChaosTic.Lyapunov.Shell")
        );
    }

    #[test]
    fn install_and_remove() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(install(d.path(), "/usr/bin/lyapunov-desktop").unwrap().len(), 3);
        assert!(d.path().join("applications/lyapunov.desktop").exists());
        assert_eq!(remove(d.path()).unwrap().len(), 3);
        assert!(!d.path().join("applications/lyapunov.desktop").exists());
    }
}
