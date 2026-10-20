use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use lyapunov::apps::AppIndex;
use lyapunov::backend::{self, Backend};
use lyapunov::card::{self, AppEntry, Card, Place, PlaceKind, ResumePrefs, Tint};
use lyapunov::store::{self, Store};
use lyapunov::{capture, integrate, resume};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "lyapunov",
    version,
    about = "Save an activity as a card and resume it later",
    long_about = "Save an activity as a card and resume it later.\n\nA card names an activity and keeps the applications, windows, folders and documents that \
belong to it. Resuming a card reopens what is missing and puts windows back where they were, as far as this desktop allows.\n\n\
Every command accepts --json for machine-readable output."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the cards, in order
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show one card (by identifier or exact name)
    Show {
        card: String,
        #[arg(long)]
        json: bool,
    },
    /// List the applications and windows open now, as a card would keep them
    Scan {
        /// Ignore windows of this process (the desktop interface itself)
        #[arg(long = "exclude-pid")]
        exclude_pids: Vec<u32>,
        /// Also show what would change in this card
        #[arg(long)]
        compare: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Capture every open application into a new card
    Capture {
        name: String,
        #[arg(long)]
        tint: Option<String>,
        #[arg(long)]
        note: Option<String>,
        /// Keep a folder or document with the card (repeatable)
        #[arg(long = "place")]
        places: Vec<PathBuf>,
        #[arg(long = "exclude-pid")]
        exclude_pids: Vec<u32>,
        #[arg(long)]
        json: bool,
    },
    /// Save a card read as JSON on standard input (new, or replacing one with the same id)
    Save {
        #[arg(long)]
        json: bool,
    },
    /// Change a card's name, note, colour, places or resume preferences
    Edit {
        card: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        tint: Option<String>,
        #[arg(long = "add-place")]
        add_places: Vec<PathBuf>,
        #[arg(long = "remove-place")]
        remove_places: Vec<String>,
        /// Show the plan before resuming (yes or no)
        #[arg(long)]
        ask: Option<Toggle>,
        /// Move windows that are already open (yes or no)
        #[arg(long)]
        arrange: Option<Toggle>,
        #[arg(long)]
        json: bool,
    },
    /// Copy a card under a new name
    Duplicate {
        card: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Delete a card (its applications, folders and documents are not touched)
    Delete {
        card: String,
        #[arg(long)]
        json: bool,
    },
    /// Put cards in this order (identifiers); unlisted cards follow
    Reorder {
        ids: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show what resuming a card would do
    Plan {
        card: String,
        #[arg(long)]
        json: bool,
    },
    /// Resume a card: reopen what is missing and arrange windows
    Resume {
        card: String,
        /// Run only these steps (app:N, place:N; repeatable)
        #[arg(long)]
        only: Vec<String>,
        /// Leave these steps out (repeatable)
        #[arg(long)]
        skip: Vec<String>,
        /// Do not move windows that are already open
        #[arg(long)]
        no_arrange: bool,
        /// Move windows that are already open, whatever the card says
        #[arg(long, conflicts_with = "no_arrange")]
        arrange: bool,
        /// Seconds to wait for started applications to show a window
        #[arg(long, default_value_t = 20)]
        timeout: u64,
        #[arg(long)]
        json: bool,
    },
    /// Write cards to a JSON file you can keep or move to another computer
    Export {
        cards: Vec<String>,
        #[arg(long, short)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Add the cards of an export file, after checking every field
    Import {
        file: PathBuf,
        /// Show what would be added without saving
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Describe this session and what Lyapunov can do in it
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Install and enable the GNOME Shell extension that lets Lyapunov arrange windows on GNOME
    GnomeExtension {
        /// Only report whether it is installed, enabled and running
        #[arg(long)]
        status: bool,
        #[arg(long)]
        json: bool,
    },
    /// Add Lyapunov to the applications menu (or remove it)
    Integrate {
        /// Data folder (default: ~/.local/share)
        #[arg(long)]
        data_dir: Option<PathBuf>,
        /// The desktop executable the menu entry starts
        #[arg(long)]
        exe: Option<String>,
        /// Also install the GNOME Shell extension in the data folder (packages)
        #[arg(long)]
        gnome_extension: bool,
        #[arg(long)]
        remove: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Toggle {
    Yes,
    No,
}

fn main() {
    let cli = Cli::parse();
    let json = match &cli.command {
        Command::List { json }
        | Command::Show { json, .. }
        | Command::Scan { json, .. }
        | Command::Capture { json, .. }
        | Command::Save { json }
        | Command::Edit { json, .. }
        | Command::Duplicate { json, .. }
        | Command::Delete { json, .. }
        | Command::Reorder { json, .. }
        | Command::Plan { json, .. }
        | Command::Resume { json, .. }
        | Command::Export { json, .. }
        | Command::Import { json, .. }
        | Command::Doctor { json }
        | Command::GnomeExtension { json, .. } => *json,
        Command::Integrate { .. } => false,
    };
    if let Err(e) = run(cli.command) {
        if json {
            println!("{}", serde_json::json!({ "error": format!("{e:#}") }));
        } else {
            eprintln!("lyapunov: {e:#}");
        }
        std::process::exit(1);
    }
}

fn print_json<T: Serialize>(v: &T) -> Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, v)?;
    writeln!(out)?;
    Ok(())
}

fn parse_tint(s: &str) -> Result<Tint> {
    serde_json::from_value(serde_json::Value::String(s.to_lowercase()))
        .map_err(|_| anyhow::anyhow!("unknown colour {s:?}; choose peach, sky, lilac, mint, amber, rose or slate"))
}

fn place_for(path: &Path) -> Result<Place> {
    let abs = std::fs::canonicalize(path).with_context(|| format!("{} was not found", path.display()))?;
    let kind = if abs.is_dir() {
        PlaceKind::Folder
    } else {
        PlaceKind::File
    };
    let path = abs.to_string_lossy().into_owned();
    // canonicalize returns \\?\C:\... on Windows; cards keep the usual form.
    let path = path.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(path);
    Ok(Place { path, kind })
}

/// Opens the backend, falling back to launching only, with the reason.
fn open_backend() -> (Box<dyn Backend>, Option<String>) {
    match backend::detect() {
        Ok(b) => {
            let reason = (b.name() == "launch-only").then(limitation_text);
            (b, reason)
        }
        Err(e) => (
            Box::new(backend::LaunchOnly {
                reason: format!("{e:#}"),
            }),
            Some(format!("{e:#}")),
        ),
    }
}

fn limitation_text() -> String {
    let s = backend::session();
    if s.session_type == "wayland" && s.desktop.to_lowercase().contains("gnome") {
        "GNOME on Wayland needs the Lyapunov Shell extension to arrange windows; applications, folders and documents still reopen".into()
    } else if s.session_type == "wayland" {
        "this Wayland desktop does not let applications arrange windows; applications, folders and documents still reopen".into()
    } else {
        "no supported window manager found; applications, folders and documents still reopen".into()
    }
}

#[derive(Deserialize)]
struct Draft {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    tint: Option<Tint>,
    #[serde(default)]
    apps: Vec<AppEntry>,
    #[serde(default)]
    places: Vec<Place>,
    #[serde(default)]
    resume: Option<ResumePrefs>,
}

#[derive(Serialize)]
struct ListOut {
    cards: Vec<Card>,
    problems: Vec<store::Problem>,
}

#[derive(Serialize)]
struct ScanOut {
    #[serde(flatten)]
    scan: capture::Scan,
    #[serde(skip_serializing_if = "Option::is_none")]
    compare: Option<card::Diff>,
}

#[derive(Serialize, Deserialize)]
struct Export {
    format: String,
    version: u32,
    exported: String,
    cards: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct ImportItem {
    name: String,
    apps: Vec<String>,
    /// Programs a resume would start: shown so an imported card never starts
    /// something unexpected.
    executables: Vec<String>,
    places: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::List { json } => {
            let store = Store::open_default()?;
            let (cards, problems) = store.list()?;
            if json {
                return print_json(&ListOut { cards, problems });
            }
            if cards.is_empty() {
                println!("No cards yet. Capture one with: lyapunov capture \"Name\"");
            }
            for c in &cards {
                println!(
                    "{}  {}  ({} apps, {} windows, {} places)",
                    c.id,
                    c.name,
                    c.apps.len(),
                    c.window_count(),
                    c.places.len()
                );
            }
            for p in &problems {
                eprintln!("unreadable: {} ({})", p.file, p.error);
            }
            Ok(())
        }
        Command::Show { card, json } => {
            let c = Store::open_default()?.find(&card)?;
            if json {
                return print_json(&c);
            }
            println!("{}\n  id: {}\n  colour: {:?}", c.name, c.id, c.tint);
            if !c.note.is_empty() {
                println!("  note: {}", c.note);
            }
            for a in &c.apps {
                println!("  app: {} ({} windows)", a.name, a.windows.len());
            }
            for p in &c.places {
                println!(
                    "  {}: {}",
                    if p.kind == PlaceKind::Folder { "folder" } else { "file" },
                    p.path
                );
            }
            Ok(())
        }
        Command::Scan {
            exclude_pids,
            compare,
            json,
        } => {
            let (mut b, limitation) = open_backend();
            let snap = b.snapshot()?;
            let index = AppIndex::load();
            let apps = capture::candidates(&snap, &index, &exclude_pids);
            let compare = match compare {
                Some(key) => {
                    let old = Store::open_default()?.find(&key)?;
                    let mut new = old.clone();
                    new.apps = apps.iter().map(|c| c.app.clone()).collect();
                    Some(card::diff(&old, &new))
                }
                None => None,
            };
            let out = ScanOut {
                scan: capture::Scan {
                    backend: b.name().into(),
                    capabilities: b.capabilities(),
                    limitation,
                    apps,
                },
                compare,
            };
            if json {
                return print_json(&out);
            }
            println!("Backend: {}", out.scan.backend);
            if let Some(l) = &out.scan.limitation {
                println!("Note: {l}");
            }
            for c in &out.scan.apps {
                println!(
                    "{}  {} window(s){}",
                    c.app.name,
                    c.app.windows.len(),
                    if c.launchable { "" } else { "  (cannot be restarted)" }
                );
            }
            Ok(())
        }
        Command::Capture {
            name,
            tint,
            note,
            places,
            exclude_pids,
            json,
        } => {
            let store = Store::open_default()?;
            let (mut b, _) = open_backend();
            let snap = b.snapshot()?;
            let index = AppIndex::load();
            let mut c = Card::new(&name);
            c.tint = match tint {
                Some(t) => parse_tint(&t)?,
                None => Tint::for_index(store.list()?.0.len()),
            };
            c.note = note.unwrap_or_default();
            c.apps = capture::candidates(&snap, &index, &exclude_pids)
                .into_iter()
                .map(|c| c.app)
                .collect();
            for p in places {
                c.places.push(place_for(&p)?);
            }
            c.dedupe();
            store.save(&c)?;
            if json {
                return print_json(&c);
            }
            println!(
                "Saved {:?}: {} apps, {} windows, {} places",
                c.name,
                c.apps.len(),
                c.window_count(),
                c.places.len()
            );
            Ok(())
        }
        Command::Save { json } => {
            let mut input = Vec::new();
            std::io::stdin().take(4 * 1024 * 1024 + 1).read_to_end(&mut input)?;
            let draft: Draft = serde_json::from_slice(&input).context("reading the card from standard input")?;
            let store = Store::open_default()?;
            let existing = match &draft.id {
                Some(id) => Some(store.get(id)?),
                None => None,
            };
            let mut c = match &existing {
                Some(old) => {
                    let mut c = old.clone();
                    c.name = draft.name.trim().to_string();
                    c.modified = lyapunov::time::now_rfc3339();
                    c
                }
                None => {
                    let mut c = Card::new(&draft.name);
                    c.tint = Tint::for_index(store.list()?.0.len());
                    c
                }
            };
            if let Some(n) = draft.note {
                c.note = n;
            }
            if let Some(t) = draft.tint {
                c.tint = t;
            }
            if let Some(r) = draft.resume {
                c.resume = r;
            }
            c.apps = draft.apps;
            c.places = draft.places;
            c.dedupe();
            store.save(&c)?;
            if json {
                return print_json(&c);
            }
            println!("Saved {:?} ({})", c.name, c.id);
            Ok(())
        }
        Command::Edit {
            card,
            name,
            note,
            tint,
            add_places,
            remove_places,
            ask,
            arrange,
            json,
        } => {
            let store = Store::open_default()?;
            let mut c = store.find(&card)?;
            if let Some(n) = name {
                c.name = n.trim().to_string();
            }
            if let Some(n) = note {
                c.note = n;
            }
            if let Some(t) = tint {
                c.tint = parse_tint(&t)?;
            }
            for p in add_places {
                c.places.push(place_for(&p)?);
            }
            c.places.retain(|p| !remove_places.contains(&p.path));
            if let Some(a) = ask {
                c.resume.ask = matches!(a, Toggle::Yes);
            }
            if let Some(a) = arrange {
                c.resume.arrange_open_windows = matches!(a, Toggle::Yes);
            }
            c.dedupe();
            c.modified = lyapunov::time::now_rfc3339();
            store.save(&c)?;
            if json {
                return print_json(&c);
            }
            println!("Saved {:?}", c.name);
            Ok(())
        }
        Command::Duplicate { card, name, json } => {
            let store = Store::open_default()?;
            let old = store.find(&card)?;
            let mut c = old.clone();
            let fresh = Card::new(&old.name);
            c.id = fresh.id;
            c.created = fresh.created;
            c.modified = fresh.modified;
            c.name = match name {
                Some(n) => n.trim().to_string(),
                None => {
                    let base: String = old.name.chars().take(card::MAX_NAME - 7).collect();
                    format!("{base} (copy)")
                }
            };
            store.save(&c)?;
            if json {
                return print_json(&c);
            }
            println!("Saved {:?} ({})", c.name, c.id);
            Ok(())
        }
        Command::Delete { card, json } => {
            let store = Store::open_default()?;
            let c = store.find(&card)?;
            store.delete(&c.id)?;
            if json {
                return print_json(&serde_json::json!({ "deleted": c.id }));
            }
            println!(
                "Deleted {:?}. Its applications, folders and documents were not touched.",
                c.name
            );
            Ok(())
        }
        Command::Reorder { ids, json } => {
            let store = Store::open_default()?;
            store.reorder(&ids)?;
            let (cards, _) = store.list()?;
            let order: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
            if json {
                return print_json(&serde_json::json!({ "order": order }));
            }
            for c in cards {
                println!("{}  {}", c.id, c.name);
            }
            Ok(())
        }
        Command::Plan { card, json } => {
            let c = Store::open_default()?.find(&card)?;
            let (mut b, limitation) = open_backend();
            let snap = b.snapshot()?;
            let index = AppIndex::load();
            let p = resume::plan(&c, &snap, &index, b.as_ref(), limitation);
            if json {
                return print_json(&p);
            }
            println!("{} — backend: {}", p.name, p.backend);
            if let Some(l) = &p.limitation {
                println!("Note: {l}");
            }
            for s in &p.steps {
                let what = match s.kind {
                    resume::StepKind::Running => {
                        format!("open; {} of {} windows to arrange", s.open.min(s.windows), s.windows)
                    }
                    resume::StepKind::Launch => format!("start; {} windows to arrange", s.windows),
                    resume::StepKind::Open => "open".into(),
                    resume::StepKind::Unavailable => format!("unavailable: {}", s.reason.as_deref().unwrap_or("")),
                };
                println!("  [{}] {}: {}", s.key, s.name, what);
            }
            Ok(())
        }
        Command::Resume {
            card,
            only,
            skip,
            no_arrange,
            arrange,
            timeout,
            json,
        } => {
            let c = Store::open_default()?.find(&card)?;
            let (mut b, _) = open_backend();
            let index = AppIndex::load();
            let mut opts = resume::Options::for_card(&c);
            opts.only = (!only.is_empty()).then_some(only);
            opts.skip = skip;
            if no_arrange {
                opts.arrange_open_windows = false;
            }
            if arrange {
                opts.arrange_open_windows = true;
            }
            opts.timeout = std::time::Duration::from_secs(timeout.clamp(1, 300));
            let report = resume::execute(&c, &opts, b.as_mut(), &index, |p| {
                if json {
                    let _ = print_json(&p);
                }
            })?;
            if json {
                let mut v = serde_json::to_value(&report)?;
                v["event"] = "report".into();
                return print_json(&v);
            }
            for o in &report.outcomes {
                let status = match o.status {
                    resume::Status::WindowFound => "window found",
                    resume::Status::AppLaunched => "application started",
                    resume::Status::LeftOpen => "already open",
                    resume::Status::Opened => "opened",
                    resume::Status::Unavailable => "unavailable",
                    resume::Status::Failed => "failed",
                    resume::Status::Skipped => "left out",
                };
                let windows = if o.windows > 0 {
                    format!(" — {}/{} windows placed", o.placed, o.windows)
                } else {
                    String::new()
                };
                println!("{}: {status}{windows}", o.name);
                for n in &o.notes {
                    println!("    {n}");
                }
            }
            Ok(())
        }
        Command::Export { cards, output, json } => {
            let store = Store::open_default()?;
            let list: Vec<Card> = if cards.is_empty() {
                store.list()?.0
            } else {
                cards.iter().map(|k| store.find(k)).collect::<Result<_>>()?
            };
            let export = Export {
                format: "lyapunov-export".into(),
                version: 1,
                exported: lyapunov::time::now_rfc3339(),
                cards: list.iter().map(serde_json::to_value).collect::<Result<_, _>>()?,
            };
            let text = serde_json::to_string_pretty(&export)? + "\n";
            match output {
                Some(path) => {
                    store::atomic_write(&path, text.as_bytes())?;
                    if json {
                        return print_json(&serde_json::json!({ "exported": list.len(), "file": path }));
                    }
                    println!("Exported {} card(s) to {}", list.len(), path.display());
                }
                None => print!("{text}"),
            }
            Ok(())
        }
        Command::Import { file, dry_run, json } => {
            let meta = std::fs::metadata(&file).with_context(|| format!("{} was not found", file.display()))?;
            if meta.len() > 16 * 1024 * 1024 {
                bail!("the file is larger than 16 MiB");
            }
            let bytes = std::fs::read(&file)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes).context("not valid JSON")?;
            let raw: Vec<serde_json::Value> = if value.get("format").and_then(|f| f.as_str()) == Some("lyapunov-export")
            {
                serde_json::from_value::<Export>(value)?.cards
            } else {
                vec![value]
            };
            let store = Store::open_default()?;
            let mut items = Vec::new();
            let mut added = 0;
            for v in raw {
                let parsed = store::parse_card(&serde_json::to_vec(&v)?);
                let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string();
                match parsed {
                    Ok(mut c) => {
                        items.push(ImportItem {
                            name: c.name.clone(),
                            apps: c.apps.iter().map(|a| a.name.clone()).collect(),
                            executables: c
                                .apps
                                .iter()
                                .filter_map(|a| a.exe.clone().or_else(|| a.desktop.clone()))
                                .collect(),
                            places: c.places.iter().map(|p| p.path.clone()).collect(),
                            error: None,
                        });
                        if !dry_run {
                            // An import never replaces a card: it gets a new identity,
                            // and its first resume always shows the plan first.
                            let fresh = Card::new(&c.name);
                            c.id = fresh.id;
                            c.modified = fresh.modified;
                            c.resume.ask = true;
                            store.save(&c)?;
                            added += 1;
                        }
                    }
                    Err(e) => items.push(ImportItem {
                        name,
                        apps: Vec::new(),
                        executables: Vec::new(),
                        places: Vec::new(),
                        error: Some(format!("{e:#}")),
                    }),
                }
            }
            if json {
                return print_json(&serde_json::json!({ "dry_run": dry_run, "added": added, "cards": items }));
            }
            for i in &items {
                match &i.error {
                    Some(e) => println!("rejected {:?}: {e}", i.name),
                    None => {
                        println!("{} {:?}", if dry_run { "would add" } else { "added" }, i.name);
                        for e in &i.executables {
                            println!("    starts: {e}");
                        }
                        for p in &i.places {
                            println!("    opens: {p}");
                        }
                    }
                }
            }
            Ok(())
        }
        Command::Doctor { json } => {
            let session = backend::session();
            let (mut b, limitation) = open_backend();
            let snap = b.snapshot();
            let index = AppIndex::load();
            let store = Store::open_default();
            let (cards, problems) = match &store {
                Ok(s) => s.list().map(|(c, p)| (c.len(), p.len())).unwrap_or((0, 0)),
                Err(_) => (0, 0),
            };
            let out = serde_json::json!({
                "version": lyapunov::VERSION,
                "session": session,
                "backend": b.name(),
                "capabilities": b.capabilities(),
                "limitation": limitation,
                "snapshot_error": snap.as_ref().err().map(|e| format!("{e:#}")),
                "outputs": snap.as_ref().map(|s| s.outputs.len()).unwrap_or(0),
                "windows": snap.as_ref().map(|s| s.windows.len()).unwrap_or(0),
                "desktops": snap.as_ref().map(|s| s.desktops.len()).unwrap_or(0),
                "applications_known": index.len(),
                "data_dir": store.as_ref().map(|s| s.root().display().to_string()).unwrap_or_default(),
                "cards": cards,
                "unreadable_cards": problems,
            });
            if json {
                return print_json(&out);
            }
            println!("Lyapunov {}", lyapunov::VERSION);
            println!(
                "Session: {} {} {}",
                session.os,
                if session.session_type.is_empty() {
                    "-"
                } else {
                    &session.session_type
                },
                if session.desktop.is_empty() {
                    "-"
                } else {
                    &session.desktop
                }
            );
            let caps = b.capabilities();
            println!(
                "Backend: {} (list {}, move {}, desktops {}, states {}{})",
                b.name(),
                yes(caps.list),
                yes(caps.geometry),
                yes(caps.desktops),
                yes(caps.states),
                if caps.tiling { ", tiling" } else { "" }
            );
            if let Some(l) = limitation {
                println!("Note: {l}");
            }
            match &snap {
                Ok(s) => println!(
                    "Now: {} screens, {} desktops, {} windows",
                    s.outputs.len(),
                    s.desktops.len(),
                    s.windows.len()
                ),
                Err(e) => println!("Reading windows failed: {e:#}"),
            }
            println!("Applications known: {}", index.len());
            println!("Cards: {cards} in {}", out["data_dir"].as_str().unwrap_or("?"));
            if problems > 0 {
                println!("Unreadable cards: {problems} (see lyapunov list)");
            }
            Ok(())
        }
        Command::GnomeExtension { status, json } => {
            let data = integrate::default_data_dir()?;
            let mut installed = integrate::gnome_extension_installed(&data);
            let mut enabled = false;
            let mut note = None;
            if !status {
                installed = Some(integrate::install_gnome_extension(&data)?);
                match lyapunov::apps::find_program("gnome-extensions") {
                    Some(tool) => {
                        let out = std::process::Command::new(tool)
                            .args(["enable", integrate::GNOME_UUID])
                            .output()
                            .context("running gnome-extensions")?;
                        enabled = out.status.success();
                        if !enabled {
                            // GNOME on Wayland only discovers a new extension at login.
                            note = Some("log out and back in, then run this again".to_string());
                        }
                    }
                    None => {
                        note = Some("gnome-extensions is not installed: enable Lyapunov in the Extensions app".into())
                    }
                }
            }
            let running = backend::gnome::Gnome::available();
            let out = serde_json::json!({
                "uuid": integrate::GNOME_UUID,
                "installed": installed,
                "enabled": enabled || running,
                "running": running,
                "note": note,
            });
            if json {
                return print_json(&out);
            }
            match &installed {
                Some(p) => println!("Installed: {}", p.display()),
                None => println!("Not installed"),
            }
            println!("Running: {}", yes(running));
            if let Some(n) = note {
                println!("Note: {n}");
            }
            Ok(())
        }
        Command::Integrate {
            data_dir,
            exe,
            gnome_extension,
            remove,
        } => {
            let data = match data_dir {
                Some(d) => d,
                None => integrate::default_data_dir()?,
            };
            if remove {
                for p in integrate::remove(&data)? {
                    println!("removed {}", p.display());
                }
                return Ok(());
            }
            let exe = match exe {
                Some(e) => e,
                None => {
                    let me = std::env::current_exe()?;
                    me.with_file_name("lyapunov-desktop").to_string_lossy().into_owned()
                }
            };
            for p in integrate::install(&data, &exe)? {
                println!("wrote {}", p.display());
            }
            if gnome_extension {
                println!("wrote {}", integrate::stage_gnome_extension(&data)?.display());
            }
            Ok(())
        }
    }
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}
