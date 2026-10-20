//! Recognising an application from its window, and starting it again.
//!
//! On Linux, applications are known by their desktop entry (the `.desktop`
//! file the menu uses), found in the XDG data folders, Flatpak and Snap
//! exports. A window is matched to its entry by the compositor's desktop file
//! name, the Flatpak or Snap sandbox, the window class, or the executable.
//! An entry is started from its `Exec` line, split as the specification
//! says, without a shell. On Windows, applications are executables.

use crate::backend::Window;
#[cfg(unix)]
use anyhow::Context;
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What a window was recognised as.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub desktop: Option<String>,
    pub exe: Option<String>,
    pub class: String,
}

#[derive(Clone, Debug, Default)]
pub struct DesktopEntry {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub localized_name: Option<String>,
    pub exec: String,
    pub try_exec: Option<String>,
    pub icon: Option<String>,
    pub working_dir: Option<String>,
    pub startup_wm_class: Option<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub hidden: bool,
}

impl DesktopEntry {
    pub fn display_name(&self) -> &str {
        self.localized_name.as_deref().unwrap_or(&self.name)
    }
    /// The program the `Exec` line starts, without its folder.
    pub fn program(&self) -> Option<String> {
        let args = split_exec(&self.exec, self).ok()?;
        let first = args.first()?;
        Some(Path::new(first).file_name()?.to_string_lossy().into_owned())
    }
}

#[derive(Default)]
pub struct AppIndex {
    entries: HashMap<String, DesktopEntry>,
}

/// Programs that run many unrelated applications: never used to match one.
const GENERIC_PROGRAMS: &[&str] = &[
    "sh",
    "bash",
    "dash",
    "zsh",
    "fish",
    "env",
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
    "java",
    "electron",
    "flatpak",
    "snap",
    "bwrap",
    "wine",
    "wine64",
    "mono",
    "dotnet",
    "gjs",
    "qml",
    "qmlscene",
    "kioclient",
    "kioclient5",
    "kde-open",
    "xdg-open",
    "gio",
    "gtk-launch",
    "steam",
    "appimagelauncher",
];

impl AppIndex {
    /// Reads every desktop entry the menus would show, plus hidden ones that
    /// still name an application window.
    pub fn load() -> AppIndex {
        let mut index = AppIndex::default();
        for dir in application_dirs() {
            index.scan(&dir);
        }
        index
    }

    pub fn from_entries(entries: Vec<DesktopEntry>) -> AppIndex {
        AppIndex {
            entries: entries.into_iter().map(|e| (e.id.clone(), e)).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn scan(&mut self, apps: &Path) {
        let mut stack = vec![apps.to_path_buf()];
        let mut seen = 0usize;
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else { continue };
            for entry in read.flatten() {
                seen += 1;
                if seen > 20_000 {
                    return;
                }
                let path = entry.path();
                let Ok(kind) = entry.file_type() else { continue };
                if kind.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                let Ok(rel) = path.strip_prefix(apps) else { continue };
                let id = rel.to_string_lossy().replace(['/', '\\'], "-");
                // The first folder in XDG order wins, including a Hidden entry
                // that masks a system one.
                if self.entries.contains_key(&id) {
                    continue;
                }
                if let Some(e) = parse_desktop_file(&path, &id) {
                    self.entries.insert(id, e);
                }
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&DesktopEntry> {
        self.entries.get(id).filter(|e| !e.hidden)
    }

    fn get_ci(&self, id: &str) -> Option<&DesktopEntry> {
        self.get(id).or_else(|| {
            let lower = id.to_lowercase();
            self.entries
                .values()
                .find(|e| !e.hidden && e.id.to_lowercase() == lower)
        })
    }

    /// The only entry that satisfies a test, or none when it is ambiguous.
    fn unique(&self, test: impl Fn(&DesktopEntry) -> bool) -> Option<&DesktopEntry> {
        let mut found = self.entries.values().filter(|e| !e.hidden && test(e));
        let first = found.next()?;
        let mut rest: Vec<_> = found.collect();
        if rest.is_empty() {
            return Some(first);
        }
        // Prefer the entry the menu shows when duplicates differ only by that.
        rest.push(first);
        let shown: Vec<_> = rest.into_iter().filter(|e| !e.no_display).collect();
        if shown.len() == 1 { Some(shown[0]) } else { None }
    }

    pub fn identify(&self, w: &Window) -> Identity {
        let class = if w.class.is_empty() {
            w.instance.clone()
        } else {
            w.class.clone()
        };
        if let Some(exe) = &w.exe {
            if is_windows_exe(exe) {
                return Identity {
                    name: windows_app_name(exe),
                    desktop: None,
                    exe: Some(exe.clone()),
                    class,
                };
            }
        }
        if let Some(e) = self.entry_for(w) {
            return Identity {
                name: e.display_name().to_string(),
                desktop: Some(e.id.clone()),
                exe: None,
                class,
            };
        }
        Identity {
            name: pretty_class(&class),
            desktop: None,
            exe: None,
            class,
        }
    }

    fn entry_for(&self, w: &Window) -> Option<&DesktopEntry> {
        let with_suffix = |s: &str| {
            if s.ends_with(".desktop") {
                s.to_string()
            } else {
                format!("{s}.desktop")
            }
        };
        if let Some(df) = w.desktop_file.as_deref().filter(|s| !s.is_empty()) {
            let df = Path::new(df)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(e) = self.get_ci(&with_suffix(&df)) {
                return Some(e);
            }
        }
        if let Some(pid) = w.pid {
            if let Some(app) = sandbox_app(pid) {
                if let Some(e) = self.get_ci(&with_suffix(&app)) {
                    return Some(e);
                }
            }
        }
        for key in [&w.class, &w.instance] {
            if key.is_empty() {
                continue;
            }
            let lower = key.to_lowercase();
            if let Some(e) = self.get_ci(&with_suffix(key)) {
                return Some(e);
            }
            if let Some(e) = self.unique(|e| {
                e.startup_wm_class
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(key))
            }) {
                return Some(e);
            }
            // `dolphin` for org.kde.dolphin.desktop.
            if let Some(e) = self.unique(|e| {
                let stem = e.id.trim_end_matches(".desktop").to_lowercase();
                stem.rsplit('.').next() == Some(lower.as_str())
            }) {
                return Some(e);
            }
        }
        let program = w
            .exe
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .map(|f| f.to_string_lossy().into_owned())?;
        if GENERIC_PROGRAMS.contains(&program.as_str()) {
            return None;
        }
        self.unique(|e| e.program().as_deref() == Some(program.as_str()))
    }

    /// Starts a desktop entry with no file arguments. Returns the process ID.
    pub fn launch(&self, id: &str) -> Result<u32> {
        let Some(e) = self.get(id) else {
            bail!("not installed");
        };
        if e.terminal {
            bail!("runs in a terminal; open it from your terminal");
        }
        if let Some(t) = &e.try_exec {
            if find_program(t).is_none() {
                bail!("not installed ({t} is missing)");
            }
        }
        let args = split_exec(&e.exec, e)?;
        let Some((program, rest)) = args.split_first() else {
            bail!("the desktop entry has no command");
        };
        let resolved = find_program(program).ok_or_else(|| anyhow::anyhow!("{program} is not installed"))?;
        let mut cmd = Command::new(resolved);
        cmd.args(rest);
        if let Some(dir) = e.working_dir.as_deref().filter(|d| Path::new(d).is_dir()) {
            cmd.current_dir(dir);
        }
        spawn_detached(cmd)
    }
}

/// The application folders in XDG order, then Flatpak and Snap exports.
pub fn application_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let home = var("HOME").map(PathBuf::from);
    match var("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()) {
        Some(d) => dirs.push(d),
        None => {
            if let Some(h) = &home {
                dirs.push(h.join(".local/share"));
            }
        }
    }
    if let Some(h) = &home {
        dirs.push(h.join(".local/share/flatpak/exports/share"));
    }
    let system = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        system
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute()),
    );
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    let mut out: Vec<PathBuf> = Vec::new();
    for d in dirs {
        let apps = d.join("applications");
        if !out.contains(&apps) {
            out.push(apps);
        }
    }
    let snap = PathBuf::from("/var/lib/snapd/desktop/applications");
    if !out.contains(&snap) {
        out.push(snap);
    }
    out
}

fn language() -> Option<String> {
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() && v != "C" && v != "POSIX" {
                return Some(v);
            }
        }
    }
    None
}

pub fn parse_desktop_file(path: &Path, id: &str) -> Option<DesktopEntry> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 512 * 1024 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let mut e = parse_desktop_entry(&text, id, language().as_deref())?;
    e.path = path.to_path_buf();
    Some(e)
}

/// Parses the `[Desktop Entry]` group. Returns `None` for anything that is not
/// an application with a command, except Hidden entries, which mask others.
pub fn parse_desktop_entry(text: &str, id: &str, lang: Option<&str>) -> Option<DesktopEntry> {
    let mut in_group = false;
    let mut kv: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            kv.entry(k.trim().to_string()).or_insert_with(|| unescape(v.trim()));
        }
    }
    let flag = |k: &str| kv.get(k).is_some_and(|v| v == "true");
    let hidden = flag("Hidden");
    if !hidden && kv.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }
    let exec = kv.get("Exec").cloned().unwrap_or_default();
    if !hidden && exec.trim().is_empty() {
        return None;
    }
    // Name[fr_BE], then Name[fr], per the specification's locale matching.
    let localized_name = lang.and_then(|l| {
        let l = l.split(['.', '@']).next().unwrap_or(l);
        let short = l.split('_').next().unwrap_or(l);
        kv.get(&format!("Name[{l}]"))
            .or_else(|| kv.get(&format!("Name[{short}]")))
            .cloned()
    });
    Some(DesktopEntry {
        id: id.to_string(),
        path: PathBuf::new(),
        name: kv
            .get("Name")
            .cloned()
            .unwrap_or_else(|| id.trim_end_matches(".desktop").into()),
        localized_name,
        exec,
        try_exec: kv.get("TryExec").cloned().filter(|s| !s.is_empty()),
        icon: kv.get("Icon").cloned().filter(|s| !s.is_empty()),
        working_dir: kv.get("Path").cloned().filter(|s| !s.is_empty()),
        startup_wm_class: kv.get("StartupWMClass").cloned().filter(|s| !s.is_empty()),
        terminal: flag("Terminal"),
        no_display: flag("NoDisplay"),
        hidden,
    })
}

/// String values: `\s`, `\n`, `\t`, `\r` and `\\`.
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Splits an `Exec` value into arguments: double quotes group, a backslash
/// escapes `"`, `` ` ``, `$` and `\` inside them. File field codes are dropped
/// (no file is passed); `%i`, `%c` and `%k` expand as the specification says.
pub fn split_exec(exec: &str, e: &DesktopEntry) -> Result<Vec<String>> {
    // Each argument with a flag telling whether it was exactly one field code.
    let mut raw: Vec<(String, Option<char>)> = Vec::new();
    let mut cur = String::new();
    let mut has_token = false;
    let mut only_code: Option<char> = None;
    let mut in_quotes = false;
    let mut chars = exec.chars().peekable();
    let push = |raw: &mut Vec<(String, Option<char>)>, cur: &mut String, code: Option<char>| {
        raw.push((std::mem::take(cur), code));
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
                only_code = None;
            }
            '\\' if in_quotes => match chars.next() {
                Some(n) => cur.push(n),
                None => bail!("unfinished escape in Exec"),
            },
            ' ' | '\t' if !in_quotes => {
                if has_token {
                    push(&mut raw, &mut cur, only_code);
                    has_token = false;
                    only_code = None;
                }
            }
            '%' => {
                let Some(code) = chars.next() else {
                    bail!("unfinished field code in Exec")
                };
                let starts_token = !has_token;
                has_token = true;
                match code {
                    '%' => {
                        cur.push('%');
                        only_code = None;
                    }
                    'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm' | 'i' | 'c' | 'k' => {
                        let ends_token = matches!(chars.peek(), None | Some(' ') | Some('\t'));
                        only_code = if starts_token && ends_token && !in_quotes {
                            Some(code)
                        } else {
                            None
                        };
                        match code {
                            'c' => cur.push_str(e.display_name()),
                            'k' => cur.push_str(&e.path.to_string_lossy()),
                            _ => {}
                        }
                    }
                    other => bail!("unknown field code %{other} in Exec"),
                }
            }
            _ => {
                cur.push(c);
                has_token = true;
                only_code = None;
            }
        }
    }
    if in_quotes {
        bail!("unbalanced quotes in Exec");
    }
    if has_token {
        push(&mut raw, &mut cur, only_code);
    }
    let mut args = Vec::new();
    for (arg, code) in raw {
        match code {
            Some('i') => {
                if let Some(icon) = &e.icon {
                    args.push("--icon".to_string());
                    args.push(icon.clone());
                }
            }
            Some('c') | Some('k') | None => args.push(arg),
            // A standalone file or deprecated code expands to nothing.
            Some(_) => {}
        }
    }
    Ok(args)
}

/// Resolves a program through `PATH` unless it is already a path.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.components().count() > 1 || p.is_absolute() {
        return is_executable(p).then(|| p.to_path_buf());
    }
    let path = clean_path_var("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|c| is_executable(c))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Inside an AppImage, library and plugin paths point into the image. Started
/// applications must not inherit them.
const APPIMAGE_PATH_VARS: &[&str] = &[
    "LD_LIBRARY_PATH",
    "QT_PLUGIN_PATH",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
    "QML2_IMPORT_PATH",
    "XDG_DATA_DIRS",
    "PATH",
    "PYTHONPATH",
    "PERLLIB",
    "GSETTINGS_SCHEMA_DIR",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "GTK_PATH",
    "GDK_PIXBUF_MODULE_FILE",
];

fn appdir() -> Option<String> {
    std::env::var("APPDIR").ok().filter(|d| d.len() > 1)
}

fn clean_path_var(name: &str) -> Option<std::ffi::OsString> {
    let value = std::env::var_os(name)?;
    let Some(dir) = appdir() else { return Some(value) };
    let kept: Vec<PathBuf> = std::env::split_paths(&value).filter(|p| !p.starts_with(&dir)).collect();
    std::env::join_paths(kept).ok()
}

fn clean_environment(cmd: &mut Command) {
    for k in ["LYAPUNOV_BACKEND", "LYAPUNOV_DATA_DIR"] {
        cmd.env_remove(k);
    }
    if appdir().is_none() {
        return;
    }
    for k in ["APPDIR", "APPIMAGE", "ARGV0", "OWD"] {
        cmd.env_remove(k);
    }
    for k in APPIMAGE_PATH_VARS {
        match clean_path_var(k) {
            Some(v) if !v.is_empty() => cmd.env(k, v),
            _ => cmd.env_remove(k),
        };
    }
}

/// Starts a process that outlives Lyapunov, with no terminal attached.
pub fn spawn_detached(mut cmd: Command) -> Result<u32> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    clean_environment(&mut cmd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and touches no Rust state.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let child = cmd.spawn()?;
    Ok(child.id())
}

/// File types that run code when "opened": the default opener would execute
/// them. A card only ever opens folders and documents.
const RUNNABLE: &[&str] = &[
    "exe",
    "com",
    "bat",
    "cmd",
    "ps1",
    "psm1",
    "psd1",
    "ps1xml",
    "ps2",
    "ps2xml",
    "psc1",
    "psc2",
    "msh",
    "msh1",
    "msh2",
    "mshxml",
    "msh1xml",
    "msh2xml",
    "vb",
    "vbs",
    "vbe",
    "vbscript",
    "js",
    "jse",
    "ws",
    "wsc",
    "wsf",
    "wsh",
    "sct",
    "shb",
    "shs",
    "msi",
    "msp",
    "mst",
    "scr",
    "lnk",
    "url",
    "website",
    "hta",
    "chm",
    "cpl",
    "pif",
    "reg",
    "inf",
    "ins",
    "isp",
    "jar",
    "jnlp",
    "appref-ms",
    "application",
    "xbap",
    "gadget",
    "msc",
    "mcf",
    "appx",
    "appxbundle",
    "msix",
    "msixbundle",
    "appinstaller",
    "settingcontent-ms",
    "library-ms",
    "searchconnector-ms",
    "appcontent-ms",
    "scf",
    "diagcab",
    "iqy",
    "slk",
    "xll",
    "ade",
    "adp",
    "mda",
    "mde",
    "fxp",
    "prg",
    "dll",
    "sys",
    "ocx",
    "drv",
    "elf",
    "so",
    "desktop",
    "sh",
    "bash",
    "zsh",
    "fish",
    "csh",
    "ksh",
    "run",
    "bin",
    "out",
    "appimage",
    "flatpakref",
    "flatpakrepo",
    "snap",
    "deb",
    "rpm",
    "pkg",
    "dmg",
    "command",
    "tool",
    "workflow",
    "py",
    "pyw",
    "pyc",
    "pyo",
    "pyz",
    "pyzw",
    "pl",
    "rb",
    "php",
    "lua",
    "tcl",
];

/// The real path of a folder or document a card may open, or why it must not
/// be opened. Links are resolved first, so the checks apply to what would
/// really be opened, and only that resolved path is ever passed to the opener.
pub fn checked_place(path: &Path, folder: bool) -> std::result::Result<std::path::PathBuf, &'static str> {
    let raw = path.to_string_lossy();
    if crate::card::is_network_path(&raw) {
        return Err("network locations are not opened");
    }
    let real = std::fs::canonicalize(path).map_err(|_| "not found")?;
    let real = strip_verbatim(real)?;
    let meta = std::fs::metadata(&real).map_err(|_| "not found")?;
    if folder != meta.is_dir() {
        return Err(if folder {
            "no longer a folder"
        } else {
            "no longer a document"
        });
    }
    if meta.is_dir() {
        return Ok(real);
    }
    for candidate in [path, real.as_path()] {
        let name = candidate
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Windows ignores trailing dots and spaces ("run.exe." runs run.exe).
        if name.ends_with('.') || name.ends_with(' ') {
            return Err("programs and scripts are not opened from a card");
        }
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
        if RUNNABLE.contains(&ext.as_str()) {
            return Err("programs and scripts are not opened from a card");
        }
    }
    #[cfg(windows)]
    {
        // An alternate data stream (doc.txt:payload.exe) names something else.
        let s = real.to_string_lossy();
        if s.get(2..).is_some_and(|rest| rest.contains(':')) {
            return Err("programs and scripts are not opened from a card");
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 != 0 {
            return Err("programs and scripts are not opened from a card");
        }
    }
    Ok(real)
}

/// `canonicalize` returns `\\?\C:\…` on Windows; openers expect `C:\…`. A
/// verbatim network path (`\\?\UNC\…`) is refused.
fn strip_verbatim(p: std::path::PathBuf) -> std::result::Result<std::path::PathBuf, &'static str> {
    let s = p.to_string_lossy();
    if s.starts_with(r"\\?\UNC\") || s.starts_with(r"\\.\") {
        return Err("network locations are not opened");
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) => Ok(std::path::PathBuf::from(rest)),
        None => Ok(p),
    }
}

/// Why a place must not be opened, if it must not.
pub fn refuse_place(path: &Path, folder: bool) -> Option<&'static str> {
    checked_place(path, folder).err()
}

/// Opens a folder or document with the user's default application. Programs,
/// scripts, launchers and network locations are refused (`checked_place`).
pub fn open_place(path: &Path, folder: bool) -> Result<()> {
    let real = checked_place(path, folder).map_err(|e| anyhow::anyhow!("{e}"))?;
    #[cfg(windows)]
    {
        crate::backend::win32::shell_open(&real)
    }
    #[cfg(not(windows))]
    {
        // Paths are absolute, so the opener never reads one as an option.
        if !real.is_absolute() {
            bail!("not an absolute path");
        }
        let openers: [(&str, &[&str]); 3] = [("xdg-open", &[]), ("gio", &["open"]), ("kde-open", &[])];
        for (program, pre) in openers {
            if let Some(p) = find_program(program) {
                let mut cmd = Command::new(p);
                cmd.args(pre).arg(&real);
                spawn_detached(cmd).context("starting the default application")?;
                return Ok(());
            }
        }
        bail!("no opener found (install xdg-utils)")
    }
}

/// The Flatpak or Snap application a process belongs to.
#[cfg(unix)]
fn sandbox_app(pid: u32) -> Option<String> {
    if let Ok(info) = std::fs::read_to_string(format!("/proc/{pid}/root/.flatpak-info")) {
        let mut in_app = false;
        for line in info.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_app = line == "[Application]";
            } else if in_app {
                if let Some(name) = line.strip_prefix("name=") {
                    return Some(name.to_string());
                }
            }
        }
    }
    // snap.firefox.firefox-1234.scope -> firefox_firefox
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    cgroup.lines().find_map(|line| {
        let unit = line.rsplit('/').next()?;
        let rest = unit.strip_prefix("snap.")?;
        let mut parts = rest.split('.');
        let snap = parts.next()?;
        let app = parts.next()?;
        let app = app.split('-').next().unwrap_or(app);
        Some(format!("{snap}_{app}"))
    })
}

#[cfg(not(unix))]
fn sandbox_app(_pid: u32) -> Option<String> {
    None
}

/// The executable of a process, when the system lets us read it.
pub fn process_exe(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn is_windows_exe(path: &str) -> bool {
    crate::card::valid_exe(path) && !path.starts_with('/')
}

fn windows_app_name(exe: &str) -> String {
    let stem = exe
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(exe)
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE");
    pretty_class(stem)
}

/// `org.gnome.TextEditor` -> `TextEditor`, `firefox` -> `Firefox`.
pub fn pretty_class(class: &str) -> String {
    let last = class.rsplit('.').next().unwrap_or(class);
    let mut c = last.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Unknown application".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, text: &str) -> DesktopEntry {
        parse_desktop_entry(text, id, Some("fr_BE.UTF-8")).unwrap()
    }

    #[test]
    fn parses_the_desktop_entry_group_only() {
        let e = entry(
            "org.kde.dolphin.desktop",
            "[Desktop Entry]\nType=Application\nName=Dolphin\nName[fr]=Dolphin FR\nExec=dolphin %u\nIcon=system-file-manager\n\n[Desktop Action new]\nExec=dolphin --new-window\nName=Other\n",
        );
        assert_eq!(e.display_name(), "Dolphin FR");
        assert_eq!(e.exec, "dolphin %u");
        assert_eq!(e.program().as_deref(), Some("dolphin"));
        assert!(parse_desktop_entry("[Desktop Entry]\nType=Link\nURL=x\n", "a.desktop", None).is_none());
    }

    #[test]
    fn splits_exec_lines_like_the_specification() {
        let e = entry(
            "x.desktop",
            "[Desktop Entry]\nType=Application\nName=X\nIcon=x-icon\nExec=x\n",
        );
        let split = |s: &str| split_exec(s, &e).unwrap();
        assert_eq!(split("firefox %u"), ["firefox"]);
        assert_eq!(split("app --file=%f -x"), ["app", "--file=", "-x"]);
        assert_eq!(split("\"/opt/My App/run\" --flag %F"), ["/opt/My App/run", "--flag"]);
        assert_eq!(split(r#"sh -c "echo \"\$HOME\"""#), ["sh", "-c", "echo \"$HOME\""]);
        assert_eq!(split("app %i %c 100%%"), ["app", "--icon", "x-icon", "X", "100%"]);
        assert_eq!(
            split("/usr/bin/flatpak run --branch=stable --command=foo org.foo.Foo @@u %U @@"),
            [
                "/usr/bin/flatpak",
                "run",
                "--branch=stable",
                "--command=foo",
                "org.foo.Foo",
                "@@u",
                "@@"
            ]
        );
        assert!(split_exec("app \"unclosed", &e).is_err());
        assert!(split_exec("app %z", &e).is_err());
    }

    #[test]
    fn hidden_entries_mask_and_are_not_launchable() {
        let idx = AppIndex::from_entries(vec![
            parse_desktop_entry("[Desktop Entry]\nHidden=true\n", "gone.desktop", None).unwrap(),
        ]);
        assert!(idx.get("gone.desktop").is_none());
        assert!(idx.launch("gone.desktop").is_err());
    }

    fn index() -> AppIndex {
        AppIndex::from_entries(vec![
            entry(
                "org.kde.dolphin.desktop",
                "[Desktop Entry]\nType=Application\nName=Dolphin\nExec=dolphin %u\n",
            ),
            entry(
                "firefox.desktop",
                "[Desktop Entry]\nType=Application\nName=Firefox\nExec=/usr/lib/firefox/firefox %u\nStartupWMClass=firefox\n",
            ),
            entry(
                "code.desktop",
                "[Desktop Entry]\nType=Application\nName=Visual Studio Code\nExec=/usr/bin/code %F\nStartupWMClass=Code\n",
            ),
            entry(
                "org.gnome.TextEditor.desktop",
                "[Desktop Entry]\nType=Application\nName=Text Editor\nExec=gnome-text-editor %U\n",
            ),
        ])
    }

    #[test]
    fn identifies_windows_by_every_clue() {
        let idx = index();
        let by = |w: Window| idx.identify(&w).desktop;
        assert_eq!(
            by(Window {
                desktop_file: Some("org.kde.dolphin".into()),
                class: "dolphin".into(),
                ..Default::default()
            })
            .as_deref(),
            Some("org.kde.dolphin.desktop")
        );
        assert_eq!(
            by(Window {
                class: "dolphin".into(),
                ..Default::default()
            })
            .as_deref(),
            Some("org.kde.dolphin.desktop")
        );
        assert_eq!(
            by(Window {
                class: "Code".into(),
                ..Default::default()
            })
            .as_deref(),
            Some("code.desktop")
        );
        assert_eq!(
            by(Window {
                class: "org.gnome.TextEditor".into(),
                ..Default::default()
            })
            .as_deref(),
            Some("org.gnome.TextEditor.desktop")
        );
        assert_eq!(
            by(Window {
                class: "Navigator".into(),
                exe: Some("/usr/lib/firefox/firefox".into()),
                ..Default::default()
            })
            .as_deref(),
            Some("firefox.desktop")
        );
        let unknown = idx.identify(&Window {
            class: "my-tool".into(),
            exe: Some("/usr/bin/python3".into()),
            ..Default::default()
        });
        assert_eq!(unknown.desktop, None);
        assert_eq!(unknown.name, "My-tool");
    }

    #[test]
    fn programs_and_scripts_are_never_opened() {
        let d = tempfile::tempdir().unwrap();
        let write = |name: &str| {
            let p = d.path().join(name);
            std::fs::write(&p, b"x").unwrap();
            p
        };
        assert_eq!(refuse_place(&write("report.pdf"), false), None);
        assert!(refuse_place(&write("setup.EXE"), false).is_some());
        assert!(refuse_place(&write("evil.desktop"), false).is_some());
        assert!(refuse_place(&write("run.sh"), false).is_some());
        assert!(
            refuse_place(&write("trick.exe."), false).is_some(),
            "Windows drops trailing dots"
        );
        assert!(refuse_place(&write("help.CHM"), false).is_some());
        #[cfg(unix)]
        {
            let link = d.path().join("innocent.pdf");
            std::os::unix::fs::symlink(write("payload.desktop"), &link).unwrap();
            assert!(refuse_place(&link, false).is_some(), "links are judged by their target");
        }
        assert!(refuse_place(&write("notes.txt"), true).is_some(), "the kind must match");
        assert_eq!(refuse_place(d.path(), true), None);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let bin = write("tool");
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(refuse_place(&bin, false).is_some());
        }
    }

    #[test]
    fn windows_executables_are_their_own_identity() {
        let idx = AppIndex::default();
        let id = idx.identify(&Window {
            class: "Notepad".into(),
            exe: Some("C:\\Windows\\System32\\notepad.exe".into()),
            ..Default::default()
        });
        assert_eq!(id.exe.as_deref(), Some("C:\\Windows\\System32\\notepad.exe"));
        assert_eq!(id.name, "Notepad");
    }
}
