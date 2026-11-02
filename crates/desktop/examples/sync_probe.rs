// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

//! Probelauf fuer den Katalog-Sync: eine Seite je Quelle, Ergebnis auf stdout.
//!
//! Zweck: pruefen, ob die Quellen erreichbar sind und ob der Index danach mehr
//! Eintraege hat — ohne das Fenster zu oeffnen.
//!
//!     cargo run --release --example sync_probe

fn main() {
    let lauf = tauri::async_runtime::block_on(hexwerk_desktop::commands::catalog_sync(Some(1)));
    match lauf {
        Ok(ergebnis) => {
            println!("zusammengefuehrt: {} Eintraege", ergebnis.eintraege);
            for (quelle, text) in ergebnis.quellen {
                println!("  {quelle:12} {text}");
            }
        }
        Err(fehler) => {
            eprintln!("Sync fehlgeschlagen: {fehler}");
            std::process::exit(1);
        }
    }
}
