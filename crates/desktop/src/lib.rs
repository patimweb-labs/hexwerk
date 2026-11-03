// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

//! Hexwerk als Fensteranwendung.
//!
//! Das Fenster ist eine Web-Anwendung (Vue, in `ui/`). Diese Bibliothek startet
//! es und stellt die Befehle bereit; die Logik liegt in den vier Crates
//! `core`, `sources`, `index` und `install`.

pub mod commands;
pub mod settings;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::catalog_stats,
            commands::catalog_search,
            commands::install,
            commands::settings_get,
            commands::settings_set,
            commands::ordner_hinzufuegen,
            commands::ordner_entfernen,
            commands::catalog_sync,
            commands::app_info,
            commands::fetch_images,
            commands::oeffne_url,
            commands::zurueckholen,
            commands::installiert_in,
            commands::katalog_leeren,
            commands::deinstalliere,
        ])
        .run(tauri::generate_context!())
        .expect("Hexwerk konnte das Fenster nicht starten");
}
