//! WoW-Installationen finden - Steam, Faugus/UMU, Heroic, Lutris, Bottles,
//! PortProton und vom Nutzer gewaehlte Ordner.
//!
//! Die Sucher bekommen ihre Wurzeln als Parameter ([`SearchRoots`]). Nur
//! [`discover`] liest die echten Wurzeln dieses Rechners; [`discover_at`] nimmt
//! sie von aussen. Der Grund ist Prufbarkeit: ein Test darf `$HOME` nicht
//! anfassen, sonst haengt sein Ergebnis davon ab, was auf dem Rechner des
//! Entwicklers installiert ist (hier: ein echtes Faugus-Praefix mit WoW).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::addon::scan_addons;
use crate::flavor::{Flavor, FlavorDir};
use crate::game_version;

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

    /// Stabiler, ASCII-Kennwert fuer `Client::id`. Bewusst getrennt von
    /// `label()`: das Label ist Anzeigetext und darf sich aendern, ohne dass
    /// gespeicherte Client-Kennungen im Index unauffindbar werden.
    pub fn slug(&self) -> &'static str {
        match self {
            Origin::Steam => "steam",
            Origin::Faugus => "faugus",
            Origin::Heroic => "heroic",
            Origin::Lutris => "lutris",
            Origin::Bottles => "bottles",
            Origin::PortProton => "portproton",
            Origin::Manual => "manual",
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

/// Die Wurzeln, in denen gesucht wird - je Herkunft getrennt, weil jede
/// Herkunft eine andere Ordnertiefe hat: Steam liefert eine Bibliothek, die
/// `steamapps/compatdata` enthaelt, Faugus einen Ordner, dessen Kinder
/// Praefixe sind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchRoots {
    pub steam: Vec<PathBuf>,
    pub faugus: Vec<PathBuf>,
    pub heroic: Vec<PathBuf>,
    pub lutris: Vec<PathBuf>,
    pub bottles: Vec<PathBuf>,
    pub portproton: Vec<PathBuf>,
}

impl SearchRoots {
    /// Die ueblichen Wurzeln unterhalb eines Home-Verzeichnisses (siehe
    /// PLAN.md §4). Als Parameter statt aus `$HOME` gelesen, damit Tests einen
    /// kuenstlichen Baum unter Schieben koennen.
    pub fn from_home(home: &Path) -> Self {
        // Die Pfadsaetze stehen bei den Suchern, die sie brauchen; hier werden
        // sie nur zusammengelegt. Sonst gaebe es zwei Listen, die auseinander-
        // laufen.
        Self {
            steam: finders::steam::roots(home),
            faugus: finders::generic::faugus_roots(home),
            heroic: finders::generic::heroic_roots(home),
            lutris: finders::generic::lutris_roots(home),
            bottles: finders::generic::bottles_roots(home),
            portproton: finders::generic::portproton_roots(home),
        }
    }

    /// Die echten Wurzeln dieses Rechners. Einzige Stelle im Kern, die `$HOME`
    /// liest - dadurch bleibt die Erkennung selbst seiteneffektfrei pruefbar.
    pub fn local() -> Self {
        match std::env::var_os("HOME") {
            Some(home) => Self::from_home(Path::new(&home)),
            None => Self::default(),
        }
    }

    /// Wurzeln einer Herkunft; `Manual` hat keine, seine Ordner kommen als
    /// `extra_paths` herein.
    pub fn for_origin(&self, origin: Origin) -> &[PathBuf] {
        match origin {
            Origin::Steam => &self.steam,
            Origin::Faugus => &self.faugus,
            Origin::Heroic => &self.heroic,
            Origin::Lutris => &self.lutris,
            Origin::Bottles => &self.bottles,
            Origin::PortProton => &self.portproton,
            Origin::Manual => &[],
        }
    }
}

pub mod finders;

/// Sucht in allen bekannten Herkuenften plus den uebergebenen Zusatzordnern.
pub fn discover(extra_paths: &[PathBuf]) -> Discovery {
    discover_at(&SearchRoots::local(), extra_paths)
}

/// Wie [`discover`], aber mit vorgegebenen Wurzeln. Das ist der Einstieg fuer
/// Tests und fuer Aufrufer, die ihre Wurzeln selbst kennen (z. B. eine
/// Flatpak-Sandbox mit eigenem Home).
pub fn discover_at(roots: &SearchRoots, extra_paths: &[PathBuf]) -> Discovery {
    let mut d = Discovery::default();
    for origin in Origin::all() {
        let (found, note) = finders::run(*origin, roots.for_origin(*origin));
        d.clients.extend(found);
        d.notes.push(note);
    }
    let (manual, note) = finders::manual(extra_paths);
    d.clients.extend(manual);
    d.notes.push(note);
    dedup(&mut d.clients);
    d
}

/// Ein Spielordner kann von mehreren Herkuenften erreichbar sein (Faugus-
/// Praefix mit `drive_c` und `pfx/drive_c`, Steam-Praefix unter `~/Games`).
/// Gezaehlt wird deshalb der echte Pfad, nicht der Weg dorthin.
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
            .then(a.game_dir.cmp(&b.game_dir))
    });
}

/// Prueft einen Ordner auf `_…_`-Verzeichnisse mit `Interface/AddOns`.
///
/// Ein `_retail_` gilt auch ohne AddOns als Client, wenn eine `Wow*.exe`
/// daneben liegt: nach einer frischen Installation ist `Interface/AddOns` noch
/// nicht angelegt, der Client aber vorhanden.
pub(crate) fn describe(game_dir: &Path) -> Vec<FlavorDir> {
    let Ok(entries) = std::fs::read_dir(game_dir) else {
        return Vec::new();
    };
    // `read_dir` liefert in Dateisystem-Reihenfolge; sortieren macht den Scan
    // bei gleichem Baum reproduzierbar.
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut out = Vec::new();
    for path in dirs {
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !(name.starts_with('_') && name.ends_with('_')) {
            continue;
        }
        let addons_dir = path.join("Interface").join("AddOns");
        let has_addons = addons_dir.is_dir();
        if !has_addons && !has_game_exe(&path) {
            continue;
        }
        // Version aus den Blizzard-Dateien: sie gilt auch dann, wenn noch kein
        // Addon im Ordner liegt (frischer Client) - genau der Fall, in dem die
        // `.toc`-Nummern unten leer bleiben.
        let game = game_version::read(game_dir, &path);
        let game_short = game.as_ref().map(|g| g.short_version().to_string());
        let mut interface_numbers: Vec<u32> = if has_addons {
            scan_addons(&addons_dir)
                .iter()
                .flat_map(|a| a.interface_numbers.iter().copied())
                .collect::<BTreeSet<u32>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        // Ohne Addons sagt die `.toc` nichts; dann ist die Spielversion die
        // einzige ehrliche Quelle fuer die Interface-Nummer.
        if interface_numbers.is_empty() {
            if let Some(number) = game.as_ref().and_then(|g| g.interface_number()) {
                interface_numbers.push(number);
            }
        }
        out.push(FlavorDir {
            flavor: Flavor::from_dir_name(name),
            dir: path,
            addons_dir,
            interface_numbers,
            game_version: game_short,
        });
    }
    out.sort_by_key(|a| a.flavor.dir_name());
    out
}

/// Liegt in einem Client-Ordner ein Spiel-Binary? `Wow.exe`, `WowClassic.exe`,
/// `WowClassicT.exe` … - gemeinsam ist das Praefix `Wow` und die Endung `.exe`.
fn has_game_exe(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        name.starts_with("wow") && name.ends_with(".exe")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_ist_stabil_und_ascii() {
        for origin in Origin::all() {
            assert_eq!(origin.slug(), origin.slug().to_ascii_lowercase());
            assert!(origin.slug().is_ascii());
        }
        assert_eq!(Origin::PortProton.slug(), "portproton");
        assert_eq!(Origin::Manual.slug(), "manual");
    }

    #[test]
    fn from_home_haengt_an_das_home() {
        let roots = SearchRoots::from_home(Path::new("/tmp/x"));
        assert_eq!(roots.steam[0], PathBuf::from("/tmp/x/.local/share/Steam"));
        assert_eq!(
            roots.faugus[0],
            PathBuf::from("/tmp/x/Faugus"),
            "Faugus legt Praefixe direkt unter ~/Faugus ab"
        );
        assert_eq!(
            roots.heroic,
            vec![PathBuf::from("/tmp/x/Games/Heroic/Prefixes")]
        );
        assert_eq!(roots.for_origin(Origin::Manual), &[] as &[PathBuf]);
    }
}
