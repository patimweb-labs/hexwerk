//! Herkünfte außerhalb von Linux: Windows und macOS.
//!
//! Beide brauchen keine neue Suchlogik, nur andere Wurzeln:
//!
//! * **Windows** — das Spiel liegt in einem Programmordner
//!   (`C:\Program Files (x86)\World of Warcraft`). Die Wurzel verhält sich wie
//!   ein `drive_c`: ein Verzeichnis, unter dem `World of Warcraft` liegt.
//! * **CrossOver unter macOS** — Flaschen sind Wine-Präfixe, also dieselbe
//!   Prüfung wie bei Faugus und Bottles.
//! * **macOS native** — der Blizzard-Launcher legt das Spiel nach
//!   `/Applications/World of Warcraft`; wieder eine Wurzel wie bei Windows.
//!
//! Die Wurzeln sind Parameter, nicht aus der Umgebung gelesen — nur so lässt
//! sich jede Herkunft gegen einen künstlichen Baum prüfen, ohne dass der Rechner
//! des Entwicklers das Ergebnis beeinflusst.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::super::{describe, Client, Origin};
use super::wine;

/// Programmordner unter Windows.
///
/// Die Variablen heißen je nach Prozess-Breite anders: ein 32-Bit-Programm sieht
/// `ProgramFiles` als `Program Files (x86)`, ein 64-Bit-Programm als
/// `Program Files`. Deshalb alle drei Namen, doppelte Einträge fliegen raus.
pub fn windows_roots(home: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for name in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
        if let Some(wert) = std::env::var_os(name) {
            let pfad = PathBuf::from(wert);
            if !out.contains(&pfad) {
                out.push(pfad);
            }
        }
    }
    // Unter Wine/CI gesetzt niemand die Variablen; dann bleibt der übliche Ort.
    for rel in ["Program Files (x86)", "Program Files"] {
        let pfad = home.join(rel);
        if pfad.is_dir() && !out.contains(&pfad) {
            out.push(pfad);
        }
    }
    out
}

/// Flaschen von CrossOver und Whisky.
pub fn crossover_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Library/Application Support/CrossOver/Bottles"),
        // Whisky ist der andere verbreitete Wine-Aufsatz für Apple Silicon.
        home.join("Library/Application Support/com.isaacmarovitz.Whisky/Bottles"),
    ]
}

/// Native Installationen des Blizzard-Launchers.
///
/// `/Applications` ist systemweit und existiert nur unter macOS; der Eintrag
/// schadet auf Linux nicht, weil dort niemand ein `World of Warcraft` darin hat.
pub fn mac_native_roots(home: &Path) -> Vec<PathBuf> {
    vec![PathBuf::from("/Applications"), home.join("Applications")]
}

/// Herkünfte, deren Wurzel ein Verzeichnis **über** dem Spiel ist.
pub fn find_direct_in(origin: Origin, roots: &[PathBuf]) -> Vec<Client> {
    let mut out: Vec<Client> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for root in roots {
        // Die Wurzel darf selbst der Spielordner sein (Nutzer zeigt direkt
        // darauf), sonst wird darunter gesucht.
        let kandidaten: Vec<PathBuf> = if !describe(root).is_empty() {
            vec![root.clone()]
        } else {
            wine::find_all_wow(root)
        };
        for game_dir in kandidaten {
            let key = std::fs::canonicalize(&game_dir).unwrap_or_else(|_| game_dir.clone());
            if !seen.insert(key) {
                continue;
            }
            let flavors = describe(&game_dir);
            if flavors.is_empty() {
                continue;
            }
            out.push(Client {
                id: format!("{}:{}", origin.slug(), game_dir.display()),
                origin,
                label: label_for_game(&game_dir),
                // Kein Wine-Präfix: der Spielordner ist der Pfad, den der
                // Nutzer im Explorer bzw. Finder sieht.
                prefix: game_dir.clone(),
                game_dir,
                flavors,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// CrossOver-Flaschen sind Wine-Präfixe — dieselbe Prüfung wie bei Bottles.
pub fn find_crossover_in(roots: &[PathBuf]) -> Vec<Client> {
    let mut out: Vec<Client> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for bottle in roots_flattened(roots) {
        let key = std::fs::canonicalize(&bottle).unwrap_or_else(|_| bottle.clone());
        if !seen.insert(key) {
            continue;
        }
        let Some((game_dir, flavors)) = wine::resolve_game(&bottle) else {
            continue;
        };
        out.push(Client {
            id: format!("{}:{}", Origin::CrossOver.slug(), bottle.display()),
            origin: Origin::CrossOver,
            label: label_for_prefix(&bottle),
            prefix: bottle,
            game_dir,
            flavors,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Eine Wurzel darf selbst eine Flasche sein oder die Flaschen enthalten.
fn roots_flattened(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if wine::looks_like_prefix(root) {
            out.push(root.clone());
        }
        let Ok(eintraege) = std::fs::read_dir(root) else {
            continue;
        };
        let mut kinder: Vec<PathBuf> = eintraege.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        kinder.sort();
        out.extend(kinder);
    }
    out
}

/// Anzeigename einer Flasche: das ist der Name, den der Nutzer in CrossOver
/// sieht (`WoW`, `Steam`).
fn label_for_prefix(prefix: &Path) -> String {
    prefix
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Flasche")
        .to_string()
}

/// Anzeigename einer direkten Installation: „World of Warcraft" steht bei
/// Windows und macOS immer im Ordnernamen, deshalb unterscheidet der
/// Elternordner zwei Installationen.
fn label_for_game(game_dir: &Path) -> String {
    game_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != "Applications" && *s != "Program Files" && *s != "Program Files (x86)")
        .map(|s| format!("{s} · World of Warcraft"))
        .unwrap_or_else(|| "World of Warcraft".to_string())
}
