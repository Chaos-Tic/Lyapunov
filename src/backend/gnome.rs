//! GNOME Shell, through the Lyapunov Shell extension.
//!
//! GNOME does not let applications list or move other windows. The extension
//! in `integrations/gnome-shell` runs inside the Shell and offers two D-Bus
//! methods on the session bus, answering with the same JSON shapes as the
//! KWin script. Without it, GNOME sessions fall back to launching only.

use super::kwin::{results_from_json, snapshot_from_json};
use super::{Backend, Capabilities, PlaceResult, Placement, Snapshot};
use anyhow::{Context, Result, bail};
use zbus::blocking::Connection;

pub const SERVICE: &str = "io.github.ChaosTic.Lyapunov.Shell";
const PATH: &str = "/io/github/ChaosTic/Lyapunov/Shell";

pub struct Gnome {
    conn: Connection,
}

impl Gnome {
    pub fn available() -> bool {
        let Ok(conn) = Connection::session() else { return false };
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(SERVICE,),
        )
        .ok()
        .and_then(|m| m.body().deserialize::<bool>().ok())
        .unwrap_or(false)
    }

    pub fn connect() -> Result<Gnome> {
        if !Gnome::available() {
            bail!("the Lyapunov GNOME Shell extension is not enabled");
        }
        Ok(Gnome {
            conn: Connection::session().context("connecting to the session bus")?,
        })
    }

    fn call(&self, method: &str, arg: Option<&str>) -> Result<String> {
        let reply = match arg {
            Some(a) => self.conn.call_method(Some(SERVICE), PATH, Some(SERVICE), method, &(a,)),
            None => self.conn.call_method(Some(SERVICE), PATH, Some(SERVICE), method, &()),
        }
        .with_context(|| format!("calling the GNOME Shell extension ({method})"))?;
        Ok(reply.body().deserialize::<String>()?)
    }
}

impl Backend for Gnome {
    fn name(&self) -> &'static str {
        "gnome"
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
        snapshot_from_json(&self.call("Snapshot", None)?)
    }

    fn place(&mut self, placements: &[Placement]) -> Result<Vec<PlaceResult>> {
        if placements.is_empty() {
            return Ok(Vec::new());
        }
        results_from_json(&self.call("Place", Some(&serde_json::to_string(placements)?))?)
    }
}
