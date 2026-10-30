//! Ein Sucher je Herkunft. Jeder liest nur, schreibt nie und bekommt seine
//! Wurzeln von aussen - so ist keiner von ihnen an `$HOME` gebunden.

use std::path::{Path, PathBuf};

use super::{Client, Note, NoteStatus, Origin};

pub mod generic;
pub mod steam;

pub(crate) mod vdf;
pub(crate) mod wine;

/// Fuehrt den Sucher einer Herkunft aus und beschreibt das Ergebnis fuer die
/// Oberflaeche.
pub(crate) fn run(origin: Origin, roots: &[PathBuf]) -> (Vec<Client>, Note) {
    let found = match origin {
        Origin::Steam => steam::find_in(roots),
        Origin::Faugus => generic::find_faugus_in(roots),
        Origin::Heroic => generic::find_heroic_in(roots),
        Origin::Lutris => generic::find_lutris_in(roots),
        Origin::Bottles => generic::find_bottles_in(roots),
        Origin::PortProton => generic::find_portproton_in(roots),
        Origin::Manual => Vec::new(),
    };
    let note = note_for(origin, roots, &found);
    (found, note)
}

/// Notiz je Herkunft. Der Unterschied zwischen "kein Ordner vorhanden" und
/// "Ordner da, aber kein WoW drin" ist fuer den Nutzer wichtig: das eine heisst
/// "nicht installiert", das andere "Praefix da, Spiel fehlt".
fn note_for(origin: Origin, roots: &[PathBuf], found: &[Client]) -> Note {
    let (status, detail) = if !found.is_empty() {
        (
            NoteStatus::Ok,
            format!("{}: {} Installation(en)", origin.label(), found.len()),
        )
    } else if roots.is_empty() {
        (
            NoteStatus::Skip,
            format!("{}: keine Wurzel", origin.label()),
        )
    } else if !roots.iter().any(|r| r.is_dir()) {
        (
            NoteStatus::Skip,
            format!("{}: keine WoW-Installation gefunden", origin.label()),
        )
    } else {
        (
            NoteStatus::Skip,
            format!("{}: kein Praefix mit World of Warcraft", origin.label()),
        )
    };
    Note {
        step: origin.label().to_string(),
        status,
        detail,
    }
}

/// Vom Nutzer gewaehlte Ordner. Sie liegen nicht unter einer Wurzel, sondern
/// werden direkt geprueft - der Ordner selbst enthaelt die `_retail_`-artigen
/// Verzeichnisse.
pub(crate) fn manual(paths: &[PathBuf]) -> (Vec<Client>, Note) {
    let mut out: Vec<Client> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in paths {
        // Derselbe Ordner zweimal uebergeben ist ein Tippfehler, kein zweiter
        // Client.
        let key = canonical(p);
        if !seen.insert(key) {
            continue;
        }
        let flavors = super::describe(p);
        if flavors.is_empty() {
            continue;
        }
        out.push(Client {
            id: format!("manual:{}", p.display()),
            origin: Origin::Manual,
            label: label_for(p),
            prefix: p.clone(),
            game_dir: p.clone(),
            flavors,
        });
    }
    let (status, detail) = if out.is_empty() {
        (NoteStatus::Skip, "Manuell: nichts".to_string())
    } else {
        (NoteStatus::Ok, format!("Manuell: {}", out.len()))
    };
    (
        out,
        Note {
            step: "Manuell".into(),
            status,
            detail,
        },
    )
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn label_for(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Ordner")
        .to_string()
}
