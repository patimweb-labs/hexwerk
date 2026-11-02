//! Wine-Ordnerlogik, die alle Herkuenfte teilen.
//!
//! Zwei Eigenheiten bestimmen diesen Code, beide gemessen (README, 2026-10-30):
//!
//! 1. Ein UMU-Praefix legt `drive_c` **und** `pfx/drive_c` auf dieselbe Ebene
//!    (so macht es Faugus). Wer nur `drive_c` prueft, landet auf einem leeren
//!    Platzhalter, waehrend das echte Spiel eine Ebene daneben liegt.
//! 2. Der Installationsort innerhalb von `drive_c` streut (`Program Files
//!    (x86)`, `Program Files`, `Games`, direkt unter `drive_c`). Eine feste
//!    Pfadliste ist deshalb unvollstaendig, ein unbegrenzter Lauf durch das
//!    Praefix zu teuer - also eine Suche mit Tiefenbegrenzung.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::super::describe;
use crate::flavor::FlavorDir;

/// Wie viele Ebenen unterhalb von `drive_c` gesucht wird. Drei deckt
/// `Program Files (x86)/World of Warcraft` und `Games/<Launcher>/World of
/// Warcraft` ab und haelt den Lauf im Millisekundenbereich.
const MAX_DEPTH: usize = 3;

/// Systemordner, in denen WoW nie liegt, die aber viele Eintraege haben.
const SKIP: &[&str] = &["windows"];

/// Die `drive_c`-Wurzeln eines Praefixes, die es wirklich gibt. Reihenfolge
/// ist fest, damit bei Gleichstand die flache Form gewinnt.
pub(crate) fn drive_c_roots(prefix: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for rel in [Path::new("drive_c"), Path::new("pfx/drive_c")] {
        let p = prefix.join(rel);
        if p.is_dir() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Sucht das Spielverzeichnis in einem Praefix und waehlt bei mehreren
/// Fundstellen die ergiebigere: mehr Client-Varianten, dann mehr
/// Interface-Nummern. Das entscheidet den Faugus-Fall, in dem beide
/// `drive_c`-Formen existieren.
pub(crate) fn resolve_game(prefix: &Path) -> Option<(PathBuf, Vec<FlavorDir>)> {
    let mut best: Option<(PathBuf, Vec<FlavorDir>)> = None;
    for drive in drive_c_roots(prefix) {
        let Some(game_dir) = find_wow(&drive) else {
            continue;
        };
        let flavors = describe(&game_dir);
        if flavors.is_empty() {
            continue;
        }
        let better = match &best {
            None => true,
            Some((_, current)) => score(&flavors) > score(current),
        };
        if better {
            best = Some((game_dir, flavors));
        }
    }
    best
}

fn score(flavors: &[FlavorDir]) -> (usize, usize) {
    let interfaces: usize = flavors.iter().map(|f| f.interface_numbers.len()).sum();
    (flavors.len(), interfaces)
}

/// Sucht unterhalb einer `drive_c`-Wurzel nach `World of Warcraft`.
///
/// Breitensuche nach sortierten Kindern: das Ergebnis haengt damit nicht an der
/// Reihenfolge des Dateisystems. Ein Ordner mit passendem Namen zaehlt nur, wenn
/// er auch Client-Varianten enthaelt - ein leeres `World of Warcraft` neben dem
/// echten (etwa ein Rest nach einem fehlgeschlagenen Update) darf den Lauf nicht
/// vorzeitig beenden.
/// Alle Fundstellen unterhalb einer Wurzel, in sortierter Reihenfolge.
///
/// Gebraucht bei Wurzeln, unter denen mehrere Spiele liegen können (Windows:
/// zwei Laufwerke, macOS: `/Applications` neben einer zweiten Installation).
pub(crate) fn find_all_wow(drive_c: &Path) -> Vec<PathBuf> {
    let mut treffer = Vec::new();
    sammle_wow(drive_c, &mut treffer, None);
    treffer
}

pub(crate) fn find_wow(drive_c: &Path) -> Option<PathBuf> {
    let mut treffer = Vec::new();
    sammle_wow(drive_c, &mut treffer, Some(1));
    treffer.into_iter().next()
}

/// gemeinsame Suche; `grenze` begrenzt die Zahl der Treffer (None = alle).
fn sammle_wow(drive_c: &Path, treffer: &mut Vec<PathBuf>, grenze: Option<usize>) {
    if !drive_c.is_dir() {
        return;
    }
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((drive_c.to_path_buf(), 0));
    seen.insert(canonical(drive_c));

    while let Some((dir, level)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            // Symlinks nicht verfolgen: Praefixe verlinken `drive_c` gern nach
            // aussen, ein Zyklus wuerde den Scan endlos laufen lassen.
            match entry.file_type() {
                Ok(t) if t.is_symlink() => continue,
                Ok(_) => {}
                Err(_) => continue,
            }
            let path = entry.path();
            if path.is_dir() {
                children.push(path);
            }
        }
        children.sort();

        for child in children {
            let name = child
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name == "world of warcraft" {
                if !describe(&child).is_empty() {
                    treffer.push(child);
                    if grenze.is_some_and(|g| treffer.len() >= g) {
                        return;
                    }
                }
                // Namensgleicher, aber leerer Ordner: weiter suchen.
                continue;
            }
            if level + 1 < MAX_DEPTH
                && !SKIP.contains(&name.as_str())
                && seen.insert(canonical(&child))
            {
                queue.push_back((child, level + 1));
            }
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Ein Ordner zaehlt als Praefix, wenn er eine `drive_c`-Wurzel hat.
pub(crate) fn looks_like_prefix(prefix: &Path) -> bool {
    !drive_c_roots(prefix).is_empty()
}
