//! Katalog vom Dienst holen und Ergebnis auf stdout zeigen.

fn main() {
    match tauri::async_runtime::block_on(hexwerk_desktop::commands::catalog_sync()) {
        Ok(e) => println!(
            "version {} · {} Eintraege · {}",
            e.version,
            e.eintraege,
            if e.neu {
                "neu uebernommen"
            } else {
                "war schon aktuell"
            }
        ),
        Err(fehler) => {
            eprintln!("Sync fehlgeschlagen: {fehler}");
            std::process::exit(1);
        }
    }
}
