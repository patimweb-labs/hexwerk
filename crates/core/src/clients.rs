//! WoW-Installationen finden - Steam, Faugus/UMU, Heroic, Lutris, Bottles,
//! PortProton und vom Nutzer gewaehlte Ordner.

use std::path::{Path, PathBuf};

use crate::addon::scan_addons;
use crate::flavor::{Flavor, FlavorDir};

/// Herkunft eines Praefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    Steam,
    Faugus,
    Heroic,
    Lutris,
    Bottles,
    PortProton,
    Manual,
}

impl Origin {
    pub fn label(&self) -> &'static str {
        match self {
            Origin::Steam => "Steam",
            Origin::Faugus => "Faugus",
            Origin::Heroic => "Heroic",
            Origin::Lutris => "Lutris",
            Origin::Bottles => "Bottles",
            Origin::PortProton => "PortProton",
            Origin::Manual => "Manuell",
        }
    }
    pub fn all() -> &'static [Origin] {
        &[
            Origin::Steam,
            Origin::Faugus,
            Origin::Heroic,
            Origin::Lutris,
            Origin::Bottles,
            Origin::PortProton,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Stabiler Schluessel, z. B. `faugus:/home/u/Faugus/battlenet`.
    pub id: String,
    pub origin: Origin,
    pub label: String,
    /// Wurzel des Wine-Praefixes.
    pub prefix: PathBuf,
    /// Ordner, in dem die `_retail_`-artigen Verzeichnisse liegen.
    pub game_dir: PathBuf,
    pub flavors: Vec<FlavorDir>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStatus {
    Ok,
    Skip,
    Error,
    Running,
}

/// Ein Schritt des Scans, damit die Oberflaeche zeigen kann, was passiert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub step: String,
    pub status: NoteStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    pub clients: Vec<Client>,
    pub notes: Vec<Note>,
}

mod finders;

/// Sucht in allen bekannten Herkuenften plus den uebergebenen Zusatzordnern.
pub fn discover(extra_paths: &[PathBuf]) -> Discovery {
    let mut d = Discovery::default();
    for origin in Origin::all() {
        let (found, note) = finders::run(*origin);
        d.clients.extend(found);
        d.notes.push(note);
    }
    let (manual, note) = finders::manual(extra_paths);
    d.clients.extend(manual);
    d.notes.push(note);
    dedup(&mut d.clients);
    d
}

fn dedup(clients: &mut Vec<Client>) {
    let mut seen = std::collections::HashSet::new();
    clients.retain(|c| {
        let key = std::fs::canonicalize(&c.game_dir).unwrap_or_else(|_| c.game_dir.clone());
        seen.insert(key)
    });
    clients.sort_by(|a, b| {
        a.origin
            .label()
            .cmp(b.origin.label())
            .then(a.label.cmp(&b.label))
    });
}

/// Prueft einen Ordner auf `_…_`-Verzeichnisse mit `Interface/AddOns`.
pub(crate) fn describe(game_dir: &Path) -> Vec<FlavorDir> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(game_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !(name.starts_with('_') && name.ends_with('_')) {
            continue;
        }
        let addons_dir = path.join("Interface").join("AddOns");
        if !addons_dir.is_dir() {
            continue;
        }
        let interface_numbers = scan_addons(&addons_dir)
            .iter()
            .flat_map(|a| a.interface_numbers.clone())
            .collect::<std::collections::BTreeSet<u32>>()
            .into_iter()
            .collect();
        out.push(FlavorDir {
            flavor: Flavor::from_dir_name(name),
            dir: path,
            addons_dir,
            interface_numbers,
        });
    }
    out.sort_by(|a, b| a.flavor.dir_name().cmp(&b.flavor.dir_name()));
    out
}
