//! Kleine Formatierer fuer die Anzeige.
//!
//! Warum ein eigenes Modul: dieselbe Zahl erscheint an vier Stellen (Client-
//! Karte, Tabelle, Detail-Seite, Fusszeile) und soll ueberall gleich aussehen -
//! deutsche Schreibweise mit Komma, Pfade mit `~` statt `/home/<nutzer>`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Kuerzt das Home-Verzeichnis zu `~`. Lange Wine-Pfade passen sonst in keine
/// Spalte, und der Nutzer erkennt seinen eigenen Pfad trotzdem.
pub fn shorten_home(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home {
        Some(home) => match path.strip_prefix(&home) {
            Ok(rest) => {
                if rest.as_os_str().is_empty() {
                    "~".to_string()
                } else {
                    format!("~/{}", rest.display())
                }
            }
            Err(_) => path.display().to_string(),
        },
        None => path.display().to_string(),
    }
}

/// Groesse in B/KB/MB/GB mit einer Nachkommastelle ab KB. Basis 1024, weil
/// Dateigroessen so gezaehlt werden - und weil `du` es genauso macht.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit]).replace('.', ",")
}

/// Interface-Nummern als Liste. Ab vier Werten wird abgekuerzt, damit die
/// Zelle in der Tabelle nicht die Spaltenbreite bestimmt.
pub fn interface_list(numbers: &[u32]) -> String {
    if numbers.is_empty() {
        return "kein Interface".to_string();
    }
    let shown: Vec<String> = numbers.iter().take(3).map(|n| n.to_string()).collect();
    let mut out = shown.join(" · ");
    if numbers.len() > shown.len() {
        out.push_str(" …");
    }
    out
}

/// Datum als `YYYY-MM-DD` in UTC. Kein Datums-Crate: gebraucht wird ein
/// Kalenderdatum, und die Umrechnung ist zehn Zeilen (Howard Hinnant).
pub fn iso_date(time: SystemTime) -> String {
    let secs = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(_) => 0,
    };
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Schaltjahre relativ zum 1.3.0000 verschieben, dann in 400-Jahres-Zyklen
    // rechnen: so gibt es keine Sonderfaelle fuer Februar.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // Tag im 400-Jahres-Zyklus
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// Erster Buchstabe gross, Rest wie gegeben - fuer die Glyphen in den
/// Client-Karten.
pub fn initial(text: &str) -> String {
    text.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groessen_mit_komma() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2_600_000_000), "2,4 GB");
        assert!(human_size(1536).ends_with(" KB"));
    }

    #[test]
    fn intervalliste_kuerzt() {
        assert_eq!(interface_list(&[]), "kein Interface");
        assert_eq!(interface_list(&[11509, 20506]), "11509 · 20506");
        assert_eq!(interface_list(&[1, 2, 3, 4]), "1 · 2 · 3 …");
    }

    #[test]
    fn datum_ist_kalenderdatum() {
        // 2026-10-30 00:00:00 UTC = 1793318400
        assert_eq!(
            iso_date(UNIX_EPOCH + std::time::Duration::from_secs(1_793_318_400)),
            "2026-10-30"
        );
    }
}
