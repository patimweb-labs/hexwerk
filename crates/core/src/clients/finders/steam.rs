//! Steam: Bibliotheken aus `libraryfolders.vdf`, dann `compatdata/<appid>/pfx`.
//!
//! Eine Steam-Bibliothek kann auf einem anderen Datentraeger liegen; die
//! Wurzelliste ist deshalb nur der Einstieg, die tatsaechlichen Bibliotheken
//! stehen in der VDF-Datei. Beide Formate davon sind im Umlauf (siehe
//! [`vdf`]).

use std::path::{Path, PathBuf};

use super::super::{Client, Origin};
use super::vdf;
use super::wine;

/// Die ueblichen Steam-Wurzeln unterhalb eines Home-Verzeichnisses. `~/.steam/steam`
/// ist bei vielen Distributionen ein Symlink auf `.local/share/Steam`; doppelte
/// Treffer faengt die Deduplizierung ueber den echten Pfad ab.
pub fn roots(home: &Path) -> Vec<PathBuf> {
    vec![home.join(".local/share/Steam"), home.join(".steam/steam")]
}

/// Sucht WoW-Praefixe in den uebergebenen Bibliothekswurzeln. Zusaetzlich
/// gelesene Bibliotheken aus `libraryfolders.vdf` werden mit durchsucht.
pub fn find_in(roots: &[PathBuf]) -> Vec<Client> {
    let mut out: Vec<Client> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for library in expand_libraries(roots) {
        let steamapps = library.join("steamapps");
        let compat = steamapps.join("compatdata");
        let Ok(entries) = std::fs::read_dir(&compat) else {
            continue;
        };
        // Sortiert, damit zwei Laeufe ueber denselben Baum dieselbe Reihenfolge
        // liefern (der Index speichert Client-IDs).
        let mut app_dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        app_dirs.sort();

        for app_dir in app_dirs {
            let Some(appid) = app_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let pfx = app_dir.join("pfx");
            let Some((game_dir, flavors)) = wine::resolve_game(&pfx) else {
                continue;
            };
            // Nur einmal je AppID: zwei Bibliotheken koennen denselben
            // compatdata-Ordner ueber verschiedene Pfade zeigen. Erst nach
            // einem Treffer zaehlen, damit eine leere Kopie in einer zweiten
            // Bibliothek den Fund nicht blockiert.
            if seen.iter().any(|id| id == &appid) {
                continue;
            }
            seen.push(appid.clone());
            out.push(Client {
                id: format!("steam:{appid}"),
                origin: Origin::Steam,
                label: app_label(&steamapps, &appid),
                prefix: pfx,
                game_dir,
                flavors,
            });
        }
    }
    out
}

/// Die Wurzeln plus alle Bibliotheken, die in `libraryfolders.vdf` eingetragen
/// sind. Fehlt die Datei, bleibt es bei den Wurzeln.
fn expand_libraries(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for root in roots {
        push_unique(&mut out, root.clone());
    }
    // Erst nach dem Sammeln der Wurzeln lesen: die VDF-Datei kann selbst auf
    // eine Wurzel verweisen, die schon in der Liste steht.
    for root in roots {
        let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
        let Ok(text) = std::fs::read_to_string(&vdf_path) else {
            continue;
        };
        for path in vdf::library_paths(&text) {
            push_unique(&mut out, path);
        }
    }
    out
}

fn push_unique(out: &mut Vec<PathBuf>, path: PathBuf) {
    let same = |a: &Path, b: &Path| -> bool {
        match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(ca), Ok(cb)) => ca == cb,
            _ => a == b,
        }
    };
    if !out.iter().any(|existing| same(existing, &path)) {
        out.push(path);
    }
}

/// Anzeigename aus `appmanifest_<appid>.acf`; ohne Datei bleibt die AppID.
/// Der Name ist das, was in der Oberflaeche steht, und `Steam 1664280` sagt
/// niemandem etwas.
fn app_label(steamapps: &Path, appid: &str) -> String {
    let manifest = steamapps.join(format!("appmanifest_{appid}.acf"));
    if let Ok(text) = std::fs::read_to_string(&manifest) {
        if let Some(name) = vdf::value_of(&text, "name") {
            if !name.trim().is_empty() {
                return name;
            }
        }
    }
    format!("Steam {appid}")
}
