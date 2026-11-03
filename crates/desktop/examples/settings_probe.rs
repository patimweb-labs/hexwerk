//! Zeigt, welche Rangfolge die Einstellungen nach dem Laden haben.

fn main() {
    let s = hexwerk_desktop::settings::Settings::load();
    println!(
        "Sprache {} · Thema {} · Rangfolge {:?}",
        s.language, s.theme, s.source_priority
    );
    let pfad = hexwerk_desktop::settings::pfad();
    if let Some(p) = pfad {
        println!("Datei: {}", p.display());
        if let Ok(text) = std::fs::read_to_string(&p) {
            for zeile in text.lines().take(12) {
                println!("  {zeile}");
            }
        }
    }
}
