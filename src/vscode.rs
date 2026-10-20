//! Opt-in VS Code integration. A tiny local extension reports the one folder
//! open in each VS Code window. No window title, network service or project
//! contents are read. The CLI opens a missing folder with --new-window.

use crate::apps;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

pub const EXTENSION_ID: &str = "chaos-tic.lyapunov-workspace";

#[derive(Deserialize)]
struct Marker {
    folder: String,
    pid: u32,
}

pub fn folder(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if crate::card::is_network_path(path) || !p.is_absolute() {
        bail!("workspace must be an absolute local folder");
    }
    let real = fs::canonicalize(p)?;
    if !real.is_dir() {
        bail!("workspace is not a folder");
    }
    Ok(real)
}

pub fn marker_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
        if !runtime.is_absolute() {
            return None;
        }
        Some(runtime.join("lyapunov-vscode"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn is_open_in(dir: &Path, target: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
            continue;
        }
        let Ok(age) = SystemTime::now().duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)) else {
            continue;
        };
        if age > Duration::from_secs(8) {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(marker) = serde_json::from_slice::<Marker>(&bytes) else {
            continue;
        };
        if marker.pid == 0 || !Path::new(&format!("/proc/{}", marker.pid)).exists() {
            continue;
        }
        if Path::new(&marker.folder) == target {
            return true;
        }
    }
    false
}

pub fn is_open(target: &Path) -> bool {
    let open = marker_dir().is_some_and(|dir| is_open_in(&dir, target));
    if open {
        clear_pending(target);
    }
    open
}

pub fn available() -> bool {
    let Some(code) = apps::find_program("code") else {
        return false;
    };
    if marker_dir().is_none() {
        return false;
    }
    let mut cmd = Command::new(code);
    apps::clean_environment(&mut cmd);
    cmd.arg("--list-extensions").output().ok().is_some_and(|out| {
        out.status.success()
            && String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|line| line.trim().eq_ignore_ascii_case(EXTENSION_ID))
    })
}

pub fn pending(target: &Path) -> bool {
    let Some(dir) = marker_dir() else {
        return false;
    };
    dir.join(format!(
        "pending-{}",
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, target.to_string_lossy().as_bytes())
    ))
    .exists()
}

fn mark_pending(target: &Path) -> Result<()> {
    let dir = marker_dir().ok_or_else(|| anyhow::anyhow!("local runtime directory unavailable"))?;
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            bail!("runtime directory is a symbolic link");
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    let path = dir.join(format!(
        "pending-{}",
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, target.to_string_lossy().as_bytes())
    ));
    fs::write(path, b"launch requested")?;
    Ok(())
}

fn clear_pending(target: &Path) {
    if let Some(dir) = marker_dir() {
        let path = dir.join(format!(
            "pending-{}",
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, target.to_string_lossy().as_bytes())
        ));
        let _ = fs::remove_file(path);
    }
}

pub fn launch(target: &Path, timeout: Duration) -> Result<bool> {
    if is_open(target) {
        clear_pending(target);
        return Ok(true);
    }
    if pending(target) {
        bail!("previous VS Code launch is unverified; refusing to open another window");
    }
    if !available() {
        bail!("install and enable the Lyapunov Workspace VS Code extension first");
    }
    mark_pending(target)?;
    let code = apps::find_program("code").ok_or_else(|| anyhow::anyhow!("VS Code is not installed"))?;
    let mut cmd = Command::new(code);
    cmd.arg("--new-window").arg(target);
    if let Err(e) = apps::spawn_detached(cmd) {
        clear_pending(target);
        return Err(e);
    }
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if is_open(target) {
            clear_pending(target);
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Ok(false)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn marker_requires_matching_folder_live_pid_and_fresh_file() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("A");
        let b = tmp.path().join("B");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        assert!(!is_open_in(tmp.path(), &a));
        fs::write(
            tmp.path().join("one.json"),
            serde_json::json!({"folder":a,"pid":std::process::id()}).to_string(),
        )
        .unwrap();
        assert!(is_open_in(tmp.path(), &a));
        assert!(!is_open_in(tmp.path(), &b));
        fs::write(
            tmp.path().join("one.json"),
            serde_json::json!({"folder":a,"pid":u32::MAX}).to_string(),
        )
        .unwrap();
        assert!(!is_open_in(tmp.path(), &a));
    }
}
