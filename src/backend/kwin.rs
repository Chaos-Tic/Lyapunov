//! KDE Plasma (KWin 6, Wayland and X11).
//!
//! KWin has no D-Bus call that lists or moves windows, but it runs scripts
//! loaded over D-Bus. Each request writes a short script to a private file,
//! loads it, runs it once and unloads it. The script answers by calling a
//! D-Bus method on this process's unique bus name; the data it embeds is JSON
//! produced here, so nothing from a card is ever evaluated as code.

use super::{Backend, Capabilities, Desktop, Output, PlaceResult, Placement, Snapshot, Window};
use crate::card::{Rect, WindowState};
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::sync::mpsc;
use std::time::Duration;
use zbus::blocking::Connection;

const KWIN: &str = "org.kde.KWin";
const PATH: &str = "/Lyapunov";
const IFACE: &str = "io.github.ChaosTic.Lyapunov.Reply";
const TIMEOUT: Duration = Duration::from_secs(6);

pub struct KWin {
    conn: Connection,
    replies: mpsc::Receiver<String>,
    counter: u32,
}

struct Reply {
    tx: mpsc::Sender<String>,
}

#[zbus::interface(name = "io.github.ChaosTic.Lyapunov.Reply")]
impl Reply {
    #[zbus(name = "Report")]
    fn report(&self, json: String) {
        let _ = self.tx.send(json);
    }
}

#[derive(Deserialize)]
struct RawSnapshot {
    outputs: Vec<RawOutput>,
    desktops: Vec<RawDesktop>,
    current: Option<String>,
    windows: Vec<RawWindow>,
}

#[derive(Deserialize)]
struct RawOutput {
    name: String,
    rect: Rect,
    index: u32,
}

#[derive(Deserialize)]
struct RawDesktop {
    id: String,
    name: String,
    index: u32,
}

#[derive(Deserialize)]
struct RawWindow {
    handle: String,
    class: String,
    instance: String,
    desktop_file: String,
    pid: i64,
    title: String,
    rect: Rect,
    state: String,
    output: Option<String>,
    desktop: Option<String>,
}

#[derive(Deserialize)]
struct RawResult {
    handle: String,
    ok: bool,
    note: Option<String>,
}

impl KWin {
    /// KWin owns its name on the session bus.
    pub fn available() -> bool {
        let Ok(conn) = Connection::session() else { return false };
        name_has_owner(&conn, KWIN)
    }

    pub fn connect() -> Result<KWin> {
        let conn = Connection::session().context("connecting to the session bus")?;
        if !name_has_owner(&conn, KWIN) {
            bail!("KWin is not running in this session");
        }
        let (tx, replies) = mpsc::channel();
        conn.object_server()
            .at(PATH, Reply { tx })
            .context("registering the reply object")?;
        Ok(KWin {
            conn,
            replies,
            counter: 0,
        })
    }

    /// Loads `body` as a KWin script with `REPLY(json)` available, runs it,
    /// unloads it, and returns what it reported.
    fn run(&mut self, body: &str) -> Result<String> {
        let unique = self
            .conn
            .unique_name()
            .ok_or_else(|| anyhow!("no unique bus name"))?
            .to_string();
        let target = serde_json::json!({ "service": unique, "path": PATH, "iface": IFACE });
        let script = format!(
            "(function () {{\nconst T = {};\nfunction REPLY(v) {{ callDBus(T.service, T.path, T.iface, \"Report\", JSON.stringify(v)); }}\ntry {{\n{body}\n}} catch (e) {{ REPLY({{ error: String(e) }}); }}\n}})();\n",
            js_literal(&target.to_string())
        );
        let dir = tempfile::Builder::new().prefix("lyapunov-kwin-").tempdir()?;
        let file = dir.path().join("script.js");
        std::fs::write(&file, script)?;
        self.counter += 1;
        let plugin = format!("lyapunov-{}-{}", std::process::id(), self.counter);
        while self.replies.try_recv().is_ok() {}
        let id: i32 = self
            .conn
            .call_method(
                Some(KWIN),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "loadScript",
                &(file.to_string_lossy().as_ref(), plugin.as_str()),
            )
            .context("loading the KWin script")?
            .body()
            .deserialize()?;
        if id < 0 {
            bail!("KWin refused the script");
        }
        let ran = self.conn.call_method(
            Some(KWIN),
            format!("/Scripting/Script{id}").as_str(),
            Some("org.kde.kwin.Script"),
            "run",
            &(),
        );
        let reply = ran
            .context("running the KWin script")
            .and_then(|_| self.replies.recv_timeout(TIMEOUT).context("KWin did not answer"));
        let _ = self.conn.call_method(
            Some(KWIN),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "unloadScript",
            &(plugin.as_str(),),
        );
        let reply = reply?;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&reply) {
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                bail!("KWin script error: {e}");
            }
        }
        Ok(reply)
    }
}

fn name_has_owner(conn: &Connection, name: &str) -> bool {
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &(name,),
    )
    .ok()
    .and_then(|m| m.body().deserialize::<bool>().ok())
    .unwrap_or(false)
}

/// JSON is a JavaScript expression, except for two line separators that old
/// engines reject inside strings.
fn js_literal(json: &str) -> String {
    json.replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

const SNAPSHOT: &str = r#"
function rect(g) { return { x: Math.round(g.x), y: Math.round(g.y), width: Math.round(g.width), height: Math.round(g.height) }; }
const out = { outputs: [], desktops: [], current: workspace.currentDesktop ? workspace.currentDesktop.id : null, windows: [] };
workspace.screens.forEach(function (s, i) { out.outputs.push({ name: s.name, rect: rect(s.geometry), index: i }); });
workspace.desktops.forEach(function (d, i) { out.desktops.push({ id: d.id, name: d.name, index: i }); });
for (const w of workspace.stackingOrder) {
  if (!w.normalWindow || w.specialWindow || w.skipTaskbar || w.deleted || !w.managed || w.transient) continue;
  let state = "normal";
  if (w.fullScreen) state = "fullscreen";
  else if (w.minimized) state = "minimized";
  else if (w.maximizeMode === 3) state = "maximized";
  const ds = w.onAllDesktops || !w.desktops || w.desktops.length === 0 ? null : w.desktops[0].id;
  out.windows.push({ handle: String(w.internalId), class: String(w.resourceClass || ""), instance: String(w.resourceName || ""),
    desktop_file: String(w.desktopFileName || ""), pid: w.pid || 0, title: String(w.caption || ""), rect: rect(w.frameGeometry),
    state: state, output: w.output ? w.output.name : null, desktop: ds });
}
REPLY(out);
"#;

const PLACE: &str = r#"
const byId = {};
for (const w of workspace.windowList()) byId[String(w.internalId)] = w;
const results = [];
for (const op of OPS) {
  const w = byId[op.handle];
  if (!w || w.deleted) { results.push({ handle: op.handle, ok: false, note: "window closed" }); continue; }
  try {
    if (op.desktop) {
      const d = workspace.desktops.find(function (x) { return x.id === op.desktop; });
      if (d) w.desktops = [d];
    }
    if (op.output) {
      const o = workspace.screens.find(function (s) { return s.name === op.output; });
      if (o && (!w.output || w.output.name !== o.name)) workspace.sendClientToScreen(w, o);
    }
    if (w.fullScreen && op.state !== "fullscreen") w.fullScreen = false;
    if (w.minimized && op.state !== "minimized") w.minimized = false;
    if (w.maximizeMode !== 0 && op.state !== "maximized") w.setMaximize(false, false);
    let moved = true;
    if (op.rect && op.state !== "maximized" && op.state !== "fullscreen") {
      if (w.moveable && w.resizeable) w.frameGeometry = op.rect;
      else if (w.moveable) w.frameGeometry = { x: op.rect.x, y: op.rect.y, width: w.frameGeometry.width, height: w.frameGeometry.height };
      else moved = false;
    }
    if (op.state === "maximized") { if (op.rect && w.moveable) w.frameGeometry = op.rect; w.setMaximize(true, true); }
    if (op.state === "fullscreen") w.fullScreen = true;
    if (op.state === "minimized") w.minimized = true;
    results.push({ handle: op.handle, ok: moved, note: moved ? null : "this window cannot be moved" });
  } catch (e) {
    results.push({ handle: op.handle, ok: false, note: String(e) });
  }
}
REPLY(results);
"#;

impl Backend for KWin {
    fn name(&self) -> &'static str {
        "kwin"
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
        snapshot_from_json(&self.run(SNAPSHOT)?)
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        if placements.is_empty() {
            return Ok(Vec::new());
        }
        let ops = serde_json::to_string(placements)?;
        let body = format!("const OPS = {};\n{PLACE}", js_literal(&ops));
        results_from_json(&self.run(&body)?)
    }
}

/// The snapshot shape shared by the KWin script and the GNOME Shell extension.
pub(super) fn snapshot_from_json(json: &str) -> Result<Snapshot> {
    let raw: RawSnapshot = serde_json::from_str(json).context("reading the compositor's answer")?;
    let own = std::process::id();
    Ok(Snapshot {
        outputs: raw
            .outputs
            .into_iter()
            .map(|o| Output {
                primary: o.index == 0,
                name: o.name,
                rect: o.rect,
            })
            .collect(),
        desktops: raw
            .desktops
            .into_iter()
            .map(|d| Desktop {
                id: d.id,
                name: d.name,
                index: d.index,
            })
            .collect(),
        current_desktop: raw.current,
        windows: raw
            .windows
            .into_iter()
            .filter(|w| w.pid != i64::from(own))
            .map(|w| {
                let pid = u32::try_from(w.pid).ok().filter(|p| *p > 0);
                Window {
                    handle: w.handle,
                    class: w.class,
                    instance: w.instance,
                    desktop_file: Some(w.desktop_file).filter(|d| !d.is_empty()),
                    exe: pid.and_then(crate::apps::process_exe),
                    pid,
                    title: w.title,
                    rect: w.rect,
                    state: parse_state(&w.state),
                    output: w.output,
                    desktop: w.desktop,
                    floating: false,
                }
            })
            .collect(),
    })
}

pub(super) fn results_from_json(json: &str) -> Result<Vec<PlaceResult>> {
    let raw: Vec<RawResult> = serde_json::from_str(json).context("reading the compositor's answer")?;
    Ok(raw
        .into_iter()
        .map(|r| PlaceResult {
            handle: r.handle,
            ok: r.ok,
            note: r.note,
        })
        .collect())
}

pub fn parse_state(s: &str) -> WindowState {
    match s {
        "maximized" => WindowState::Maximized,
        "minimized" => WindowState::Minimized,
        "fullscreen" => WindowState::Fullscreen,
        _ => WindowState::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_data_cannot_break_out_of_the_script() {
        let p = Placement {
            handle: "\"); evil(); (\"\u{2028}".into(),
            rect: None,
            state: WindowState::Normal,
            desktop: None,
            output: None,
            floating: false,
        };
        let lit = js_literal(&serde_json::to_string(&[p]).unwrap());
        assert!(lit.contains(r#"\"); evil(); (\"\u2028"#));
        assert!(!lit.contains('\u{2028}'));
    }
}
