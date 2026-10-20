//! Hyprland, through its request socket.
//!
//! Like other tiling compositors, Hyprland places tiled windows itself:
//! Lyapunov restores their workspace and the geometry of floating windows.

use super::{Backend, Capabilities, Desktop, Output, PlaceResult, Placement, Snapshot, Window};
use crate::card::{Rect, WindowState};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

pub struct Hyprland {
    socket: PathBuf,
}

impl Hyprland {
    pub fn connect() -> Result<Hyprland> {
        let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .ok()
            .filter(|s| !s.is_empty() && !s.contains('/'))
            .context("not a Hyprland session")?;
        let mut candidates = Vec::new();
        if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
            candidates.push(PathBuf::from(rt).join("hypr").join(&sig).join(".socket.sock"));
        }
        candidates.push(PathBuf::from("/tmp/hypr").join(&sig).join(".socket.sock"));
        let socket = candidates
            .into_iter()
            .find(|p| p.exists())
            .context("the Hyprland socket was not found")?;
        let h = Hyprland { socket };
        h.request("j/version")?;
        Ok(h)
    }

    fn request(&self, req: &str) -> Result<String> {
        let mut s = UnixStream::connect(&self.socket)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        s.write_all(req.as_bytes())?;
        let mut out = String::new();
        s.take(64 * 1024 * 1024).read_to_string(&mut out)?;
        Ok(out)
    }

    fn json(&self, req: &str) -> Result<Value> {
        let text = self.request(req)?;
        serde_json::from_str(&text).with_context(|| format!("unexpected answer to {req}"))
    }
}

pub fn outputs_from(monitors: &Value) -> Vec<Output> {
    monitors
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| !m["disabled"].as_bool().unwrap_or(false))
        .map(|m| {
            let scale = m["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0);
            let (mut w, mut h) = (m["width"].as_f64().unwrap_or(0.0), m["height"].as_f64().unwrap_or(0.0));
            if m["transform"].as_i64().unwrap_or(0) % 2 == 1 {
                std::mem::swap(&mut w, &mut h);
            }
            Output {
                name: m["name"].as_str().unwrap_or("").to_string(),
                rect: Rect::new(
                    m["x"].as_i64().unwrap_or(0) as i32,
                    m["y"].as_i64().unwrap_or(0) as i32,
                    (w / scale).round() as i32,
                    (h / scale).round() as i32,
                ),
                primary: m["id"].as_i64() == Some(0),
            }
        })
        .collect()
}

pub fn windows_from(clients: &Value, monitors: &[(i64, String)]) -> Vec<Window> {
    clients
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["mapped"].as_bool().unwrap_or(true))
        .filter(|c| c["workspace"]["id"].as_i64().unwrap_or(0) > -99)
        .map(|c| {
            let pair = |k: &str| {
                let a = c[k].as_array();
                let at = |i: usize| a.and_then(|a| a.get(i)).and_then(Value::as_i64).unwrap_or(0) as i32;
                (at(0), at(1))
            };
            let (x, y) = pair("at");
            let (w, h) = pair("size");
            let fullscreen = c["fullscreen"].as_bool().unwrap_or(false) || c["fullscreen"].as_i64().unwrap_or(0) > 0;
            let pid = c["pid"].as_i64().and_then(|p| u32::try_from(p).ok()).filter(|p| *p > 0);
            let monitor = c["monitor"].as_i64();
            Window {
                handle: c["address"].as_str().unwrap_or("").to_string(),
                class: c["class"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| c["initialClass"].as_str())
                    .unwrap_or("")
                    .to_string(),
                instance: String::new(),
                desktop_file: None,
                exe: pid.and_then(crate::apps::process_exe),
                pid,
                title: c["title"].as_str().unwrap_or("").to_string(),
                rect: Rect::new(x, y, w, h),
                state: if fullscreen {
                    WindowState::Fullscreen
                } else {
                    WindowState::Normal
                },
                output: monitors
                    .iter()
                    .find(|(id, _)| Some(*id) == monitor)
                    .map(|(_, n)| n.clone()),
                desktop: c["workspace"]["name"].as_str().map(str::to_string),
                floating: c["floating"].as_bool().unwrap_or(false),
            }
        })
        .collect()
}

/// A workspace by number when it has one, otherwise by name.
fn workspace_arg(name: &str) -> String {
    match name.parse::<i64>() {
        Ok(n) if n > 0 => n.to_string(),
        _ => format!("name:{name}"),
    }
}

pub fn batch_for(p: &Placement, current: &Window) -> Result<String> {
    let addr = &p.handle;
    if !addr.starts_with("0x") || !addr[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("invalid window address");
    }
    let mut cmds = Vec::new();
    if let Some(ws) = &p.desktop {
        if ws.contains([';', ',', '\n']) {
            bail!("unsupported workspace name");
        }
        if current.desktop.as_deref() != Some(ws.as_str()) {
            cmds.push(format!(
                "dispatch movetoworkspacesilent {},address:{addr}",
                workspace_arg(ws)
            ));
        }
    }
    if p.floating && !current.floating {
        cmds.push(format!("dispatch setfloating address:{addr}"));
    } else if !p.floating && current.floating {
        cmds.push(format!("dispatch settiled address:{addr}"));
    }
    if p.floating {
        if let Some(r) = p.rect {
            cmds.push(format!(
                "dispatch resizewindowpixel exact {} {},address:{addr}",
                r.width, r.height
            ));
            cmds.push(format!("dispatch movewindowpixel exact {} {},address:{addr}", r.x, r.y));
        }
    }
    Ok(cmds.join(";"))
}

impl Backend for Hyprland {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            list: true,
            geometry: true,
            desktops: true,
            states: false,
            tiling: true,
        }
    }

    fn snapshot(&mut self) -> Result<Snapshot> {
        let monitors = self.json("j/monitors")?;
        let ids: Vec<(i64, String)> = monitors
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| {
                (
                    m["id"].as_i64().unwrap_or(-1),
                    m["name"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let own = std::process::id();
        let windows = windows_from(&self.json("j/clients")?, &ids)
            .into_iter()
            .filter(|w| w.pid != Some(own))
            .collect();
        let ws = self.json("j/workspaces")?;
        let mut desktops: Vec<Desktop> = ws
            .as_array()
            .into_iter()
            .flatten()
            .filter(|w| w["id"].as_i64().unwrap_or(0) > 0)
            .map(|w| {
                let name = w["name"].as_str().unwrap_or("").to_string();
                Desktop {
                    id: name.clone(),
                    name,
                    index: w["id"].as_i64().unwrap_or(0) as u32,
                }
            })
            .collect();
        desktops.sort_by_key(|d| d.index);
        let current = self
            .json("j/activeworkspace")
            .ok()
            .and_then(|w| w["name"].as_str().map(str::to_string));
        Ok(Snapshot {
            outputs: outputs_from(&monitors),
            desktops,
            current_desktop: current,
            windows,
        })
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        let snap = self.snapshot()?;
        let mut results = Vec::new();
        for p in placements {
            let Some(cur) = snap.windows.iter().find(|w| w.handle == p.handle) else {
                results.push(PlaceResult {
                    handle: p.handle.clone(),
                    ok: false,
                    note: Some("window closed".into()),
                });
                continue;
            };
            let batch = match batch_for(p, cur) {
                Ok(b) => b,
                Err(e) => {
                    results.push(PlaceResult {
                        handle: p.handle.clone(),
                        ok: false,
                        note: Some(e.to_string()),
                    });
                    continue;
                }
            };
            let answer = if batch.is_empty() {
                String::new()
            } else {
                self.request(&format!("[[BATCH]]{batch}"))?
            };
            let failed: Vec<&str> = answer
                .split("\n\n")
                .map(str::trim)
                .filter(|a| !a.is_empty() && *a != "ok")
                .collect();
            let ok = failed.is_empty();
            results.push(PlaceResult {
                handle: p.handle.clone(),
                ok,
                note: if !ok {
                    Some(failed.join("; "))
                } else if !p.floating {
                    Some("tiled: Hyprland chooses its place on the workspace".into())
                } else {
                    None
                },
            });
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_clients_and_scaled_monitors() {
        let monitors: Value = serde_json::from_str(
            r#"[{"id":0,"name":"eDP-1","x":0,"y":0,"width":2880,"height":1800,"scale":2.0,"transform":0},
                {"id":1,"name":"DP-2","x":1440,"y":0,"width":1080,"height":1920,"scale":1.0,"transform":1}]"#,
        )
        .unwrap();
        let outs = outputs_from(&monitors);
        assert_eq!(outs[0].rect, Rect::new(0, 0, 1440, 900));
        assert_eq!(outs[1].rect, Rect::new(1440, 0, 1920, 1080));
        let clients: Value = serde_json::from_str(
            r#"[{"address":"0x5a1","mapped":true,"at":[10,20],"size":[800,600],"workspace":{"id":3,"name":"3"},
                 "floating":true,"monitor":1,"class":"kitty","title":"zsh","pid":0,"fullscreen":0},
                {"address":"0x5a2","mapped":true,"at":[0,0],"size":[10,10],"workspace":{"id":-98,"name":"special:magic"},
                 "floating":false,"monitor":0,"class":"x","title":"","pid":0,"fullscreen":false}]"#,
        )
        .unwrap();
        let ids = vec![(0, "eDP-1".to_string()), (1, "DP-2".to_string())];
        let w = windows_from(&clients, &ids);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].output.as_deref(), Some("DP-2"));
        assert!(w[0].floating);
    }

    #[test]
    fn batches_refuse_injection() {
        let cur = Window {
            handle: "0x5a1".into(),
            desktop: Some("1".into()),
            ..Default::default()
        };
        let mut p = Placement {
            handle: "0x5a1".into(),
            rect: Some(Rect::new(1, 2, 3, 4)),
            state: WindowState::Normal,
            desktop: Some("web".into()),
            output: None,
            floating: true,
        };
        assert_eq!(
            batch_for(&p, &cur).unwrap(),
            "dispatch movetoworkspacesilent name:web,address:0x5a1;dispatch setfloating address:0x5a1;\
dispatch resizewindowpixel exact 3 4,address:0x5a1;dispatch movewindowpixel exact 1 2,address:0x5a1"
        );
        p.desktop = Some("x;dispatch exec rm".into());
        assert!(batch_for(&p, &cur).is_err());
        p.handle = "0x5a1;dispatch exec".into();
        assert!(batch_for(&p, &cur).is_err());
    }
}
