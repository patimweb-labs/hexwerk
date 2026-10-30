//! Kleine Helfer, die keine eigene Abhaengigkeit rechtfertigen: Zeitstempel
//! und die JSON-Spalten des Schemas.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUN: AtomicU64 = AtomicU64::new(0);

/// Zeitstempel in UTC als RFC3339 mit Millisekunden.
///
/// SQLite kennt keinen Datumstyp; als Text in dieser Form sortiert er
/// lexikografisch wie chronologisch, was fuer `ORDER BY` und „zuletzt
/// gesehen“ reicht. `chrono` waere fuer einen Zeitstempel zu viel Gewicht.
pub(crate) fn now_utc() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        // Eine Uhr vor 1970 ist kein Grund, den Index zu verweigern.
        .unwrap_or_default();
    format_utc(now.as_secs(), now.subsec_millis())
}

/// Kennung eines Sync-Laufs.
///
/// Der Zaehler ist noetig, weil ein Lauf „alles, was diese Kennung nicht
/// traegt, ist verschwunden“ aufraeumt: zwei Laeufe in derselben Millisekunde
/// wuerden sich sonst gegenseitig die Zeilen loeschen.
pub(crate) fn run_marker() -> String {
    let n = RUN.fetch_add(1, Ordering::Relaxed);
    format!("{}#{n}", now_utc())
}

fn format_utc(secs: u64, millis: u32) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Tage seit 1970-01-01 -> (Jahr, Monat, Tag) im gregorianischen Kalender.
/// Algorithmus von Howard Hinnant (`civil_from_days`), damit wir ohne
/// Kalenderbibliothek auskommen.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

/// Interface-Nummern als JSON-Liste.
///
/// Immer `[]` statt `NULL`: die Filterabfrage geht mit `json_each` ueber die
/// Spalte, und die will gueltiges JSON sehen.
pub(crate) fn json_u32(values: &[u32]) -> String {
    // Vec<u32> kann nicht scheitern; der Fallback haelt die Funktion trotzdem
    // frei von einem Panik-Pfad.
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

/// Liest eine Interface-Liste. Kaputte Werte gelten als leer — ein
/// beschaedigtes Feld darf die Oberflaeche nicht ausfallen lassen.
pub(crate) fn parse_u32_list(text: &str) -> Vec<u32> {
    serde_json::from_str::<Vec<u32>>(text).unwrap_or_default()
}

/// Liest die externen Kennungen (`X-*` aus dem `.toc`) aus ihrer JSON-Spalte.
pub(crate) fn parse_string_map(text: &str) -> BTreeMap<String, String> {
    if text.trim().is_empty() {
        return BTreeMap::new();
    }
    serde_json::from_str::<BTreeMap<String, String>>(text).unwrap_or_default()
}

/// Vergleichsschluessel fuer Namen: klein geschrieben, nur Buchstaben und
/// Ziffern.
///
/// „WeakAuras“, „weakauras“ und „Weak Auras“ sind fuer den Menschen dasselbe
/// Addon; der Katalog kommt aus vier Quellen mit vier Schreibweisen, und ohne
/// diese Normalisierung bliebe ein installiertes Addon unverknuepft.
pub(crate) fn norm(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeitstempel_sieht_aus_wie_rfc3339() {
        // 1_700_000_000 = 2023-11-14T22:13:20Z
        assert_eq!(format_utc(1_700_000_000, 0), "2023-11-14T22:13:20.000Z");
        assert_eq!(format_utc(0, 0), "1970-01-01T00:00:00.000Z");
        let now = now_utc();
        assert_eq!(now.len(), 24, "unerwartetes Format: {now}");
        assert!(now.ends_with('Z'));
    }

    #[test]
    fn lauf_kennung_ist_eindeutig() {
        let a = run_marker();
        let b = run_marker();
        assert_ne!(a, b);
    }

    #[test]
    fn json_felder_ueberstehen_kaputte_werte() {
        assert_eq!(json_u32(&[11509, 20506]), "[11509,20506]");
        assert_eq!(parse_u32_list("[11509,20506]"), vec![11509, 20506]);
        assert!(parse_u32_list("kein json").is_empty());
        assert!(parse_string_map("").is_empty());
        assert_eq!(
            parse_string_map(r#"{"tukui":"123"}"#)
                .get("tukui")
                .map(String::as_str),
            Some("123")
        );
    }

    #[test]
    fn namen_werden_normalisiert() {
        assert_eq!(norm("Weak Auras"), "weakauras");
        assert_eq!(norm("DBM-Core"), "dbmcore");
        assert_eq!(norm("Ätzend"), "ätzend");
    }
}
