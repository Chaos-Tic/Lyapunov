//! Sway and i3, through their shared IPC protocol.
//!
//! Tiling window managers decide where tiled windows go: Lyapunov restores
//! their workspace, and the exact geometry of floating windows. The socket is
//! the one the window manager advertises (`SWAYSOCK`, `I3SOCK`).

use super::{Backend, Capabilities, Desktop, Output, PlaceResult, Placement, Snapshot, Window};
use crate::card::{Rect, WindowState};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const RUN_COMMAND: u32 = 0;
const GET_WORKSPACES: u32 = 1;
const GET_OUTPUTS: u32 = 3;
const GET_TREE: u32 = 4;
const GET_VERSION: u32 = 7;

pub struct I3 {
    sock: UnixStream,
    sway: bool,
}

impl I3 {
    pub fn connect() -> Result<I3> {
        let path = std::env::var("SWAYSOCK")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("I3SOCK").ok().filter(|s| !s.is_empty()))
            .or_else(|| {
                let out = std::process::Command::new("i3").arg("--get-socketpath").output().ok()?;
                let p = String::from_utf8(out.stdout).ok()?.trim().to_string();
                (!p.is_empty()).then_some(p)
            })
            .context("no sway or i3 socket in this session")?;
        let sock = UnixStream::connect(&path).with_context(|| format!("connecting to {path}"))?;
        sock.set_read_timeout(Some(Duration::from_secs(5)))?;
        sock.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut me = I3 { sock, sway: false };
        let version = me.msg(GET_VERSION, "")?;
        me.sway = version["human_readable"].as_str().unwrap_or("").contains("sway")
            || version.get("variant").is_some()
            || std::env::var("SWAYSOCK").is_ok_and(|s| s == path);
        Ok(me)
    }

    fn msg(&mut self, ty: u32, payload: &str) -> Result<Value> {
        let mut buf = Vec::with_capacity(14 + payload.len());
        buf.extend_from_slice(b"i3-ipc");
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(payload.as_bytes());
        self.sock.write_all(&buf)?;
        let mut head = [0u8; 14];
        self.sock.read_exact(&mut head)?;
        if &head[..6] != b"i3-ipc" {
            bail!("unexpected reply from the window manager");
        }
        let len = u32::from_le_bytes(head[6..10].try_into()?) as usize;
        if len > 64 * 1024 * 1024 {
            bail!("reply too large");
        }
        let mut body = vec![0u8; len];
        self.sock.read_exact(&mut body)?;
        Ok(serde_json::from_slice(&body)?)
    }

    fn command(&mut self, cmd: &str) -> Result<Vec<Option<String>>> {
        let reply = self.msg(RUN_COMMAND, cmd)?;
        Ok(reply
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| {
                        if r["success"].as_bool() == Some(true) {
                            None
                        } else {
                            Some(r["error"].as_str().unwrap_or("command failed").to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn rect(v: &Value) -> Rect {
    let n = |k: &str| v[k].as_i64().unwrap_or(0) as i32;
    Rect::new(n("x"), n("y"), n("width"), n("height"))
}

/// Collects leaf windows with their output and workspace. Sway reports the
/// title bar apart (`deco_rect`); it is added back so the rectangle is the
/// whole frame, as on other desktops.
pub fn windows_from_tree(tree: &Value, sway: bool) -> Vec<Window> {
    fn walk(node: &Value, sway: bool, output: &str, workspace: &str, floating: bool, out: &mut Vec<Window>) {
        let ty = node["type"].as_str().unwrap_or("");
        let name = node["name"].as_str().unwrap_or("");
        let output = if ty == "output" { name } else { output };
        let workspace = if ty == "workspace" { name } else { workspace };
        if output == "__i3" || workspace == "__i3_scratch" {
            return;
        }
        let floating = floating || ty == "floating_con";
        let has_children = ["nodes", "floating_nodes"]
            .iter()
            .any(|k| node[*k].as_array().is_some_and(|a| !a.is_empty()));
        let is_window = node["window"].is_u64() || node["app_id"].is_string();
        if is_window && !has_children {
            let props = &node["window_properties"];
            let class = node["app_id"]
                .as_str()
                .or_else(|| props["class"].as_str())
                .unwrap_or("")
                .to_string();
            let state = if node["fullscreen_mode"].as_u64().unwrap_or(0) > 0 {
                WindowState::Fullscreen
            } else {
                WindowState::Normal
            };
            let pid = node["pid"]
                .as_u64()
                .and_then(|p| u32::try_from(p).ok())
                .filter(|p| *p > 0);
            let mut frame = rect(&node["rect"]);
            if sway {
                let title = node["deco_rect"]["height"].as_i64().unwrap_or(0) as i32;
                frame.y -= title;
                frame.height += title;
            }
            out.push(Window {
                handle: node["id"].as_u64().unwrap_or(0).to_string(),
                class,
                instance: props["instance"].as_str().unwrap_or("").to_string(),
                desktop_file: None,
                exe: pid.and_then(crate::apps::process_exe),
                pid,
                title: node["name"].as_str().unwrap_or("").to_string(),
                rect: frame,
                state,
                output: Some(output.to_string()).filter(|o| !o.is_empty()),
                desktop: Some(workspace.to_string()).filter(|w| !w.is_empty()),
                floating: floating || node["type"].as_str() == Some("floating_con"),
            });
            return;
        }
        for n in node["nodes"].as_array().into_iter().flatten() {
            walk(n, sway, output, workspace, floating, out);
        }
        for n in node["floating_nodes"].as_array().into_iter().flatten() {
            walk(n, sway, output, workspace, true, out);
        }
    }
    let mut out = Vec::new();
    walk(tree, sway, "", "", false, &mut out);
    out
}

/// A command argument in double quotes, as i3 and sway parse them.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn commands_for(p: &Placement, current: &Window) -> Vec<String> {
    let Ok(id) = p.handle.parse::<u64>() else {
        return Vec::new();
    };
    let target = format!("[con_id={id}]");
    let mut cmds = Vec::new();
    if let Some(ws) = &p.desktop {
        if current.desktop.as_deref() != Some(ws.as_str()) {
            cmds.push(format!("{target} move container to workspace {}", quote(ws)));
        }
    }
    if current.state == WindowState::Fullscreen && p.state != WindowState::Fullscreen {
        cmds.push(format!("{target} fullscreen disable"));
    }
    if p.floating {
        if !current.floating {
            cmds.push(format!("{target} floating enable"));
        }
        if let Some(r) = p.rect {
            cmds.push(format!(
                "{target} resize set width {} px height {} px",
                r.width, r.height
            ));
            cmds.push(format!("{target} move absolute position {} px {} px", r.x, r.y));
        }
    } else if current.floating {
        cmds.push(format!("{target} floating disable"));
    }
    if p.state == WindowState::Fullscreen {
        cmds.push(format!("{target} fullscreen enable"));
    }
    cmds
}

impl Backend for I3 {
    fn name(&self) -> &'static str {
        if self.sway { "sway" } else { "i3" }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            list: true,
            geometry: true,
            desktops: true,
            states: true,
            tiling: true,
        }
    }

    fn snapshot(&mut self) -> Result<Snapshot> {
        let tree = self.msg(GET_TREE, "")?;
        let own = std::process::id();
        let windows = windows_from_tree(&tree, self.sway)
            .into_iter()
            .filter(|w| w.pid != Some(own))
            .collect();
        let outputs = self.msg(GET_OUTPUTS, "")?;
        let outputs: Vec<Output> = outputs
            .as_array()
            .into_iter()
            .flatten()
            .filter(|o| o["active"].as_bool().unwrap_or(true) && o["rect"].is_object())
            .map(|o| Output {
                name: o["name"].as_str().unwrap_or("").to_string(),
                rect: rect(&o["rect"]),
                primary: o["primary"].as_bool().unwrap_or(false),
            })
            .collect();
        let ws = self.msg(GET_WORKSPACES, "")?;
        let mut current = None;
        let desktops = ws
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, w)| {
                let name = w["name"].as_str().unwrap_or("").to_string();
                if w["focused"].as_bool() == Some(true) {
                    current = Some(name.clone());
                }
                Desktop {
                    id: name.clone(),
                    name,
                    index: w["num"]
                        .as_i64()
                        .filter(|n| *n >= 0)
                        .map(|n| n as u32)
                        .unwrap_or(i as u32),
                }
            })
            .collect();
        Ok(Snapshot {
            outputs,
            desktops,
            current_desktop: current,
            windows,
        })
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        let snap = self.snapshot()?;
        let mut results = Vec::new();
        let mut retry: Vec<&Placement> = Vec::new();
        for p in placements {
            let Some(cur) = snap.windows.iter().find(|w| w.handle == p.handle) else {
                results.push(PlaceResult {
                    handle: p.handle.clone(),
                    ok: false,
                    note: Some("window closed".into()),
                });
                continue;
            };
            let cmds = commands_for(p, cur);
            let mut errors = Vec::new();
            for c in &cmds {
                errors.extend(self.command(c)?.into_iter().flatten());
            }
            if errors.is_empty() && p.floating && p.rect.is_some() {
                retry.push(p);
            }
            let ok = errors.is_empty();
            let note = if !ok {
                Some(errors.join("; "))
            } else if !p.floating {
                Some("tiled: the window manager chooses its place on the workspace".into())
            } else {
                None
            };
            results.push(PlaceResult {
                handle: p.handle.clone(),
                ok,
                note,
            });
        }
        // A window that just turned floating may answer with its own size
        // after the resize: check, and send the geometry once more.
        if !retry.is_empty() {
            std::thread::sleep(Duration::from_millis(300));
            let now = self.snapshot()?;
            for p in retry {
                let (Some(r), Some(w)) = (p.rect, now.windows.iter().find(|w| w.handle == p.handle)) else {
                    continue;
                };
                let off = |a: i32, b: i32| (a - b).abs() > 12;
                if off(w.rect.x, r.x)
                    || off(w.rect.y, r.y)
                    || off(w.rect.width, r.width)
                    || off(w.rect.height, r.height)
                {
                    let target = format!("[con_id={}]", p.handle);
                    self.command(&format!(
                        "{target} resize set width {} px height {} px",
                        r.width, r.height
                    ))?;
                    self.command(&format!("{target} move absolute position {} px {} px", r.x, r.y))?;
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tiled_and_floating_windows() {
        let tree: Value = serde_json::from_str(
            r#"{"type":"root","name":"root","nodes":[
                {"type":"output","name":"__i3","nodes":[{"type":"workspace","name":"__i3_scratch","nodes":[{"type":"con","id":9,"window":5,"nodes":[]}]}]},
                {"type":"output","name":"HDMI-A-1","nodes":[{"type":"workspace","name":"2: web","nodes":[
                    {"type":"con","id":11,"app_id":"firefox","pid":0,"name":"Page","rect":{"x":0,"y":0,"width":960,"height":1080},"nodes":[],"floating_nodes":[]}
                ],"floating_nodes":[
                    {"type":"floating_con","id":12,"window":4194307,"window_properties":{"class":"Gimp","instance":"gimp"},"name":"GIMP","rect":{"x":100,"y":120,"width":800,"height":600},"nodes":[],"floating_nodes":[]}
                ]}]}
            ]}"#,
        )
        .unwrap();
        let w = windows_from_tree(&tree, false);
        assert_eq!(w.len(), 2, "the scratchpad is skipped");
        assert_eq!(w[0].class, "firefox");
        assert_eq!(w[0].desktop.as_deref(), Some("2: web"));
        assert!(!w[0].floating);
        assert_eq!(w[1].class, "Gimp");
        assert!(w[1].floating);
        assert_eq!(w[1].rect, Rect::new(100, 120, 800, 600));
    }

    #[test]
    fn commands_quote_workspace_names() {
        let cur = Window {
            handle: "12".into(),
            desktop: Some("1".into()),
            ..Default::default()
        };
        let p = Placement {
            handle: "12".into(),
            rect: Some(Rect::new(10, 20, 300, 200)),
            state: WindowState::Normal,
            desktop: Some("2: \"web\"; exec rm".into()),
            output: None,
            floating: true,
        };
        let cmds = commands_for(&p, &cur);
        assert_eq!(
            cmds[0],
            r#"[con_id=12] move container to workspace "2: \"web\"; exec rm""#
        );
        assert!(cmds.contains(&"[con_id=12] floating enable".to_string()));
        assert!(cmds.contains(&"[con_id=12] move absolute position 10 px 20 px".to_string()));
    }
}
