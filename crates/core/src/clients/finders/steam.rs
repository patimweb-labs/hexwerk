//! Steam: Bibliotheken aus `libraryfolders.vdf`, dann `compatdata/<appid>/pfx`.

use std::path::PathBuf;

use super::super::{describe, Client, Origin};

pub(crate) fn find() -> Vec<Client> {
    let mut out = Vec::new();
    for lib in libraries() {
        let compat = lib.join("steamapps").join("compatdata");
        let Ok(entries) = std::fs::read_dir(&compat) else {
            continue;
        };
        for e in entries.flatten() {
            let pfx = e.path().join("pfx");
            let game = find_game_dir(&pfx);
            let Some(game_dir) = game else { continue };
            let flavors = describe(&game_dir);
            if flavors.is_empty() {
                continue;
            }
            let appid = e.file_name().to_string_lossy().to_string();
            out.push(Client {
                id: format!("steam:{appid}"),
                origin: Origin::Steam,
                label: format!("Steam {appid}"),
                prefix: pfx,
                game_dir,
                flavors,
            });
        }
    }
    out
}

fn libraries() -> Vec<PathBuf> {
    let mut libs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    for root in [".local/share/Steam", ".steam/steam"] {
        let base = PathBuf::from(&home).join(root);
        if base.is_dir() && !libs.contains(&base) {
            libs.push(base.clone())
        }
        let vdf = base.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            for line in text.lines() {
                let t = line.trim();
                if !t.starts_with("\"path\"") {
                    continue;
                }
                if let Some(v) = t.split('"').nth(3) {
                    let p = PathBuf::from(v);
                    if !libs.contains(&p) {
                        libs.push(p)
                    }
                }
            }
        }
    }
    libs
}

fn find_game_dir(pfx: &std::path::Path) -> Option<PathBuf> {
    for rel in [
        "drive_c/Program Files (x86)/World of Warcraft",
        "drive_c/Program Files/World of Warcraft",
    ] {
        let p = pfx.join(rel);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}
