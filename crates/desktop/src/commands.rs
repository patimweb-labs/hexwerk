// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

//! Die Naht zwischen Fenster und Logik.
//!
//! Jeder Befehl hier ist eine Uebersetzung: Vue schickt etwas, die Logik-Crates
//! rechnen, das Ergebnis geht als Daten zurueck. In dieser Datei steht keine
//! Fachlogik — wenn hier gerechnet wird, gehoert das Rechnen in ein Crate.

use serde::Serialize;

/// Ein Addon so, wie die Oberflaeche es braucht.
#[derive(Serialize)]
pub struct AddonDto {
    pub folder: String,
    pub title: String,
    pub version: Option<String>,
    pub interface: Vec<u32>,
    pub size_bytes: u64,
}

/// Ein Spielordner (Einzelspieler, Classic, …) eines Clients.
#[derive(Serialize)]
pub struct FlavorDto {
    pub dir_name: String,
    pub label: String,
    pub addons_dir: String,
    pub interface: Vec<u32>,
    pub game_version: String,
    pub size_bytes: u64,
    pub addons: Vec<AddonDto>,
}

/// Ein gefundener Client: Steam, ein Wine-Prefix, ein Ordner.
#[derive(Serialize)]
pub struct ClientDto {
    pub id: String,
    pub label: String,
    pub origin: String,
    pub prefix: String,
    pub game_dir: String,
    pub flavors: Vec<FlavorDto>,
}

/// Ein Schritt des Scans, der nicht geklappt hat oder wichtig ist.
#[derive(Serialize)]
pub struct NoteDto {
    pub step: String,
    pub level: String,
    pub detail: String,
}

/// Das Ergebnis eines Scans.
#[derive(Serialize)]
pub struct ScanResult {
    pub clients: Vec<ClientDto>,
    pub notes: Vec<NoteDto>,
    pub addon_count: usize,
}

fn level(status: hexwerk::NoteStatus) -> &'static str {
    match status {
        hexwerk::NoteStatus::Ok => "ok",
        hexwerk::NoteStatus::Skip => "info",
        hexwerk::NoteStatus::Running => "info",
        hexwerk::NoteStatus::Error => "error",
    }
}

fn flavor_label(flavor: hexwerk::Flavor) -> String {
    flavor.label().to_string()
}

/// Alle Clients und ihre Addons einsammeln.
///
/// Das Lesen von der Platte dauert: bei einem gewachsenen Addon-Ordner sind es
/// einige tausend Dateien. Deshalb laeuft der Scan in einem eigenen Thread, und
/// die Oberflaeche zeigt so lange „laeuft" — der Befehl blockiert sie nicht.
#[tauri::command]
pub async fn scan() -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let discovered = hexwerk::discover(&[]);
        let mut addon_count = 0;

        let clients = discovered
            .clients
            .into_iter()
            .map(|client| {
                let flavors = client
                    .flavors
                    .into_iter()
                    .map(|flavor_dir| {
                        let addons: Vec<AddonDto> = hexwerk::scan_addons(&flavor_dir.addons_dir)
                            .into_iter()
                            .map(|addon| AddonDto {
                                // Titel kommt aus der .toc; fehlt er, ist der
                                // Ordnername die ehrlichste Auskunft.
                                title: addon
                                    .toc
                                    .as_ref()
                                    .and_then(|toc| toc.title.clone())
                                    .unwrap_or_else(|| addon.folder.clone()),
                                folder: addon.folder,
                                version: addon.version,
                                interface: addon.interface_numbers,
                                size_bytes: addon.size_bytes,
                            })
                            .collect();
                        let size_bytes = addons.iter().map(|a| a.size_bytes).sum();
                        addon_count += addons.len();
                        FlavorDto {
                            dir_name: flavor_dir.flavor.dir_name().to_string(),
                            label: flavor_label(flavor_dir.flavor),
                            addons_dir: flavor_dir.addons_dir.display().to_string(),
                            interface: flavor_dir.interface_numbers,
                            game_version: flavor_dir
                                .game_version
                                .as_ref()
                                .map(|v| v.clone())
                                .unwrap_or_default(),
                            size_bytes,
                            addons,
                        }
                    })
                    .collect();

                ClientDto {
                    id: client.id,
                    label: client.label,
                    origin: client.origin.label().to_string(),
                    prefix: client.prefix.display().to_string(),
                    game_dir: client.game_dir.display().to_string(),
                    flavors,
                }
            })
            .collect();

        let notes = discovered
            .notes
            .into_iter()
            .map(|note| NoteDto {
                step: note.step,
                level: level(note.status).to_string(),
                detail: note.detail,
            })
            .collect();

        ScanResult {
            clients,
            notes,
            addon_count,
        }
    })
    .await
    .map_err(|e| e.to_string())
}
