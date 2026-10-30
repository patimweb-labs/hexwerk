//! Faugus/UMU, Heroic, Lutris, Bottles, PortProton - je ein Pfadsatz.
//!
//! Gemeinsam: ein Praefix-Ordner enthaelt `drive_c` **oder** `pfx/drive_c`
//! (gemessen bei Faugus 2026-10-30: beides nebeneinander).

use std::path::{Path, PathBuf};

use super::super::{describe, Client, Origin};

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.push(p)
            }
        }
    }
    out
}

fn from_prefix(origin: Origin, prefix: &Path) -> Option<Client> {
    let drive = if prefix.join("drive_c").is_dir() {
        prefix.join("drive_c")
    } else {
        prefix.join("pfx").join("drive_c")
    };
    if !drive.is_dir() {
        return None;
    }
    for rel in [
        "Program Files (x86)/World of Warcraft",
        "Program Files/World of Warcraft",
    ] {
        let game_dir = drive.join(rel);
        if !game_dir.is_dir() {
            continue;
        }
        let flavors = describe(&game_dir);
        if flavors.is_empty() {
            continue;
        }
        return Some(Client {
            id: format!("{}:{}", origin.label().to_lowercase(), prefix.display()),
            origin,
            label: prefix
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Praefix")
                .to_string(),
            prefix: prefix.to_path_buf(),
            game_dir,
            flavors,
        });
    }
    None
}

fn gather(origin: Origin, roots: Vec<PathBuf>) -> Vec<Client> {
    candidates(&roots)
        .iter()
        .filter_map(|p| from_prefix(origin, p))
        .collect()
}

pub(crate) fn find_faugus() -> Vec<Client> {
    let h = home();
    gather(
        Origin::Faugus,
        vec![
            h.join("Faugus"),
            h.join(".config/faugus-launcher/prefixes"),
            h.join(".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher"),
        ],
    )
}

pub(crate) fn find_heroic() -> Vec<Client> {
    let h = home();
    gather(Origin::Heroic, vec![h.join("Games/Heroic/Prefixes")])
}

pub(crate) fn find_lutris() -> Vec<Client> {
    let h = home();
    gather(
        Origin::Lutris,
        vec![
            h.join("Games"),
            h.join(".var/app/net.lutris.Lutris/data/lutris/games"),
        ],
    )
}

pub(crate) fn find_bottles() -> Vec<Client> {
    let h = home();
    gather(
        Origin::Bottles,
        vec![
            h.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
            h.join(".local/share/bottles/bottles"),
        ],
    )
}

pub(crate) fn find_portproton() -> Vec<Client> {
    let h = home();
    gather(Origin::PortProton, vec![h.join(".local/share/PortWine")])
}
