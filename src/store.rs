//! The card collection on disk.
//!
//! One JSON file per card in `cards/`, plus `order.json` for the manual order.
//! Every write goes to a temporary file in the same folder, is flushed, then
//! renamed over the old one: an interrupted save leaves the previous card
//! intact. A card that cannot be read is reported and left untouched; it never
//! prevents the others from loading. The folder and files are private to the
//! user (0700 / 0600 on Unix).

use crate::card::{self, Card};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Store {
    root: PathBuf,
}

/// A file in `cards/` that could not be loaded, and why.
#[derive(Serialize, Debug, Clone)]
pub struct Problem {
    pub file: String,
    pub error: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Order {
    order: Vec<String>,
}

/// The data folder: `LYAPUNOV_DATA_DIR`, else `$XDG_DATA_HOME/lyapunov`
/// (`~/.local/share/lyapunov`), or `%LOCALAPPDATA%\Lyapunov` on Windows.
/// Window positions belong to one computer, so nothing roams.
pub fn default_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("LYAPUNOV_DATA_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        Ok(PathBuf::from(base).join("Lyapunov"))
    }
    #[cfg(not(windows))]
    {
        if let Some(dir) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            let dir = PathBuf::from(dir);
            if dir.is_absolute() {
                return Ok(dir.join("lyapunov"));
            }
        }
        let home = std::env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home).join(".local/share/lyapunov"))
    }
}

impl Store {
    pub fn open_default() -> Result<Store> {
        Store::open(default_root()?)
    }

    pub fn open(root: PathBuf) -> Result<Store> {
        let store = Store { root };
        private_dir(&store.root)?;
        private_dir(&store.cards_dir())?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn cards_dir(&self) -> PathBuf {
        self.root.join("cards")
    }

    fn card_path(&self, id: &str) -> Result<PathBuf> {
        if !card::valid_id(id) {
            bail!("invalid card identifier {id:?}");
        }
        Ok(self.cards_dir().join(format!("{id}.json")))
    }

    /// Every readable card in the user's order, then the problems found.
    pub fn list(&self) -> Result<(Vec<Card>, Vec<Problem>)> {
        let mut cards = Vec::new();
        let mut problems = Vec::new();
        let entries = fs::read_dir(self.cards_dir()).context("reading the cards folder")?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file = entry.file_name().to_string_lossy().into_owned();
            if path.extension().and_then(|e| e.to_str()) != Some("json") || file.starts_with('.') {
                continue;
            }
            match read_card(&path) {
                Ok(card) if path.file_stem().and_then(|s| s.to_str()) == Some(card.id.as_str()) => cards.push(card),
                Ok(_) => problems.push(Problem {
                    file,
                    error: "the file name does not match the card identifier".into(),
                }),
                Err(e) => problems.push(Problem {
                    file,
                    error: format!("{e:#}"),
                }),
            }
        }
        let order = self.read_order();
        let rank = |c: &Card| order.iter().position(|id| *id == c.id).unwrap_or(usize::MAX);
        cards.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.created.cmp(&b.created)));
        problems.sort_by(|a, b| a.file.cmp(&b.file));
        Ok((cards, problems))
    }

    pub fn get(&self, id: &str) -> Result<Card> {
        let path = self.card_path(id)?;
        if !path.exists() {
            bail!("no card with identifier {id}");
        }
        read_card(&path)
    }

    /// Finds a card by identifier, or by its exact name when unambiguous.
    pub fn find(&self, key: &str) -> Result<Card> {
        if card::valid_id(key) {
            return self.get(key);
        }
        let (cards, _) = self.list()?;
        let matches: Vec<_> = cards
            .into_iter()
            .filter(|c| c.name.trim().eq_ignore_ascii_case(key.trim()))
            .collect();
        match matches.len() {
            0 => bail!("no card named {key:?}"),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => bail!("several cards are named {key:?}; use the identifier"),
        }
    }

    /// Validates and writes a card atomically. A new card goes to the end of
    /// the order.
    pub fn save(&self, card: &Card) -> Result<()> {
        card.validate()?;
        let path = self.card_path(&card.id)?;
        let mut json = serde_json::to_string_pretty(card)?;
        json.push('\n');
        atomic_write(&path, json.as_bytes())?;
        let mut order = self.read_order();
        if !order.contains(&card.id) {
            order.push(card.id.clone());
            self.write_order(&order)?;
        }
        Ok(())
    }

    /// Deletes the card and its metadata only; the files it references stay.
    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.card_path(id)?;
        fs::remove_file(&path).with_context(|| format!("no card with identifier {id}"))?;
        let mut order = self.read_order();
        order.retain(|o| o != id);
        self.write_order(&order)
    }

    /// Stores a new manual order. Identifiers that are not cards are dropped;
    /// cards missing from the list keep their relative order after it.
    pub fn reorder(&self, ids: &[String]) -> Result<()> {
        let (cards, _) = self.list()?;
        let mut order: Vec<String> = Vec::new();
        for id in ids {
            if cards.iter().any(|c| &c.id == id) && !order.contains(id) {
                order.push(id.clone());
            }
        }
        for c in &cards {
            if !order.contains(&c.id) {
                order.push(c.id.clone());
            }
        }
        self.write_order(&order)
    }

    fn read_order(&self) -> Vec<String> {
        fs::read(self.root.join("order.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Order>(&b).ok())
            .map(|o| o.order)
            .unwrap_or_default()
    }

    fn write_order(&self, order: &[String]) -> Result<()> {
        let json = serde_json::to_string_pretty(&Order { order: order.to_vec() })?;
        atomic_write(&self.root.join("order.json"), json.as_bytes())
    }
}

pub fn read_card(path: &Path) -> Result<Card> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_card(&bytes)
}

/// Parses and validates card JSON from any source (file, import, standard input).
pub fn parse_card(bytes: &[u8]) -> Result<Card> {
    if bytes.len() > 4 * 1024 * 1024 {
        bail!("the card is larger than 4 MiB");
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).context("not valid JSON")?;
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version > u64::from(card::VERSION) {
        bail!("this card was made by a newer Lyapunov (format version {version}); update Lyapunov to open it");
    }
    // Migrations from older format versions go here, one step at a time.
    let card: Card = serde_json::from_value(value).context("not a Lyapunov card")?;
    card.validate()?;
    Ok(card)
}

fn private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::symlink_metadata(dir)?;
        if meta.file_type().is_symlink() {
            bail!("{} is a symbolic link; refusing to use it", dir.display());
        }
        if meta.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Writes through a private temporary file in the destination folder, flushes
/// it, renames it over the destination, then flushes the folder.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("no parent folder")?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".tmp-")
        .suffix(".part")
        .tempfile_in(dir)
        .with_context(|| format!("writing in {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file().set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("saving {}", path.display()))?;
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Card;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("data")).unwrap();
        (dir, store)
    }

    #[test]
    fn save_list_reorder_delete() {
        let (_d, s) = store();
        let a = Card::new("Administratif");
        let b = Card::new("À lire");
        s.save(&a).unwrap();
        s.save(&b).unwrap();
        let names = |s: &Store| s.list().unwrap().0.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names(&s), ["Administratif", "À lire"]);
        s.reorder(std::slice::from_ref(&b.id)).unwrap();
        assert_eq!(names(&s), ["À lire", "Administratif"]);
        assert_eq!(s.find("à lire").unwrap_err().to_string(), "no card named \"à lire\"");
        assert_eq!(s.find("À lire").unwrap().id, b.id);
        s.delete(&a.id).unwrap();
        assert_eq!(names(&s), ["À lire"]);
    }

    #[test]
    fn a_corrupt_card_does_not_hide_the_others() {
        let (_d, s) = store();
        let a = Card::new("Photos du voyage");
        s.save(&a).unwrap();
        let broken = s.cards_dir().join(format!("{}.json", card::new_id()));
        fs::write(&broken, b"{\"format\": \"lyapunov-card\", \"vers").unwrap();
        let (cards, problems) = s.list().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(problems.len(), 1);
        assert!(broken.exists(), "a problem file is never deleted");
    }

    #[test]
    fn newer_formats_are_reported_not_rewritten() {
        let (_d, s) = store();
        let mut a = Card::new("Futur");
        a.version = card::VERSION + 1;
        let path = s.cards_dir().join(format!("{}.json", a.id));
        fs::write(&path, serde_json::to_vec(&a).unwrap()).unwrap();
        let (cards, problems) = s.list().unwrap();
        assert!(cards.is_empty());
        assert!(problems[0].error.contains("newer Lyapunov"));
    }

    #[test]
    fn interrupted_writes_leave_no_partial_card() {
        let (_d, s) = store();
        let a = Card::new("Stable");
        s.save(&a).unwrap();
        // A crash between creating the temporary file and renaming it leaves a
        // hidden .part file that listing ignores.
        fs::write(s.cards_dir().join(".tmp-x.part"), b"{").unwrap();
        let (cards, problems) = s.list().unwrap();
        assert_eq!(cards.len(), 1);
        assert!(problems.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, s) = store();
        let a = Card::new("Privé");
        s.save(&a).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&s.card_path(&a.id).unwrap()), 0o600);
        assert_eq!(mode(&s.cards_dir()), 0o700);
        assert_eq!(mode(s.root()), 0o700);
    }

    #[test]
    fn identifiers_cannot_escape_the_folder() {
        let (_d, s) = store();
        assert!(s.get("../../etc/passwd").is_err());
        assert!(s.delete("../order").is_err());
    }
}
