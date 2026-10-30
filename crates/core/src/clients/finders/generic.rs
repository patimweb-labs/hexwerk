//! Faugus/UMU, Heroic, Lutris, Bottles, PortProton - je ein eigener Pfadsatz,
//! aber dieselbe Regel: die Kinder der Wurzel sind Praefixe.
//!
//! Die Wurzeln sind Parameter, nicht `$HOME`. Nur so laesst sich die Erkennung
//! gegen kuenstliche Baeume pruefen, ohne dass das echte Home des Entwicklers
//! das Ergebnis verfaelscht.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::super::{Client, Origin};
use super::wine;

/// Faugus legt den Wine-Praefix unter `~/<name>` ab; die beiden anderen Orte
/// sind die Flatpak- und die Alt-Ablage.
pub fn faugus_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Faugus"),
        home.join(".config/faugus-launcher/prefixes"),
        home.join(".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher"),
    ]
}

pub fn heroic_roots(home: &Path) -> Vec<PathBuf> {
    vec![home.join("Games/Heroic/Prefixes")]
}

pub fn lutris_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Games"),
        home.join(".var/app/net.lutris.Lutris/data/lutris/games"),
    ]
}

pub fn bottles_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
        home.join(".local/share/bottles/bottles"),
    ]
}

pub fn portproton_roots(home: &Path) -> Vec<PathBuf> {
    vec![home.join(".local/share/PortWine")]
}

pub fn find_faugus_in(roots: &[PathBuf]) -> Vec<Client> {
    gather(Origin::Faugus, roots)
}

pub fn find_heroic_in(roots: &[PathBuf]) -> Vec<Client> {
    gather(Origin::Heroic, roots)
}

pub fn find_lutris_in(roots: &[PathBuf]) -> Vec<Client> {
    gather(Origin::Lutris, roots)
}

pub fn find_bottles_in(roots: &[PathBuf]) -> Vec<Client> {
    gather(Origin::Bottles, roots)
}

pub fn find_portproton_in(roots: &[PathBuf]) -> Vec<Client> {
    gather(Origin::PortProton, roots)
}

/// Sammelt Praefix-Kandidaten aus mehreren Wurzeln und prueft jeden einmal.
///
/// Zwei Dinge werden bewusst mitgenommen, weil sie in der Praxis vorkommen:
/// eine Wurzel darf selbst ein Praefix sein (Nutzer zeigen auf einen konkreten
/// Ordner), und Wurzeln duerfen sich ueberlappen (`~/Games` enthaelt
/// `~/Games/Heroic/Prefixes`). Ohne Deduplizierung ueber den echten Pfad
/// erscheint derselbe Client zweimal mit unterschiedlichen IDs.
fn gather(origin: Origin, roots: &[PathBuf]) -> Vec<Client> {
    let mut out: Vec<Client> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for prefix in candidates(roots) {
        let key = std::fs::canonicalize(&prefix).unwrap_or_else(|_| prefix.clone());
        if !seen.insert(key) {
            continue;
        }
        let Some((game_dir, flavors)) = wine::resolve_game(&prefix) else {
            continue;
        };
        out.push(Client {
            id: format!("{}:{}", origin.slug(), prefix.display()),
            origin,
            label: label_for(&prefix),
            prefix,
            game_dir,
            flavors,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Kandidaten: die Wurzeln selbst (falls sie wie ein Praefix aussehen) und
/// deren Kinder. Eine Ebene reicht - alle Herkuenfte legen ihre Praefixe
/// unmittelbar unter die Wurzel.
fn candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for root in roots {
        if wine::looks_like_prefix(root) {
            out.push(root.clone());
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        children.sort();
        out.extend(children);
    }
    out
}

/// Anzeigename: der Ordnername des Praefixes ist das, was der Nutzer im
/// Launcher sieht (`battlenet`, `DivinityOS2`).
fn label_for(prefix: &Path) -> String {
    prefix
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Praefix")
        .to_string()
}
