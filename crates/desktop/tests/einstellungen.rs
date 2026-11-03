//! `Settings`: eigene Spielordner, unbekannte Felder, kaputte Dateien.
//!
//! Alles entsteht unter `CARGO_TARGET_TMPDIR`; `$HOME` und
//! `XDG_CONFIG_HOME` werden nie gelesen, sonst haenge das Ergebnis davon ab,
//! was auf dem Rechner des Entwicklers gerade eingestellt ist.

use std::path::{Path, PathBuf};

use hexwerk_desktop::settings::Settings;

fn datei(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("einstellungen");
    std::fs::create_dir_all(&dir).expect("Testverzeichnis");
    let pfad = dir.join(name);
    let _ = std::fs::remove_file(&pfad);
    pfad
}

#[test]
fn fehlende_datei_ergibt_vorgaben() {
    let s = Settings::load_at(&datei("fehlt.json"));
    assert!(s.manual_paths.is_empty());
    assert_eq!(s.theme, "dark");
    assert_eq!(s.source_priority, vec!["curseforge", "wowi", "wago"]);
}

#[test]
fn eigene_ordner_ueberstehen_schreiben_und_lesen() {
    let pfad = datei("ordner.json");
    let s = Settings {
        manual_paths: vec![
            "/mnt/games/wow".into(),
            "C:\\Games\\World of Warcraft".into(),
        ],
        ..Settings::default()
    };
    s.save_at(&pfad).expect("schreiben");

    let gelesen = Settings::load_at(&pfad);
    assert_eq!(gelesen.manual_paths, s.manual_paths);
    assert_eq!(gelesen.theme, s.theme);
}

#[test]
fn kaputte_datei_ergibt_vorgaben_statt_panik() {
    let pfad = datei("kaputt.json");
    std::fs::write(&pfad, "{ das ist kein JSON").expect("Testdatei");
    let s = Settings::load_at(&pfad);
    assert!(s.manual_paths.is_empty());
    assert_eq!(s.language, "de");
}

#[test]
fn unbekannte_felder_bleiben_in_der_datei() {
    // Eine aeltere Fassung darf die Datei einer neueren nicht ausduennen.
    let pfad = datei("fremd.json");
    std::fs::write(&pfad, r#"{ "manual_paths": ["/wow"], "neues_feld": 42 }"#).expect("Testdatei");

    let s = Settings::load_at(&pfad);
    assert_eq!(s.manual_paths, vec!["/wow".to_string()]);
    assert_eq!(s.rest.get("neues_feld"), Some(&serde_json::json!(42)));

    s.save_at(&pfad).expect("schreiben");
    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pfad).expect("lesen")).expect("JSON");
    assert_eq!(roh["neues_feld"], serde_json::json!(42));
    assert_eq!(roh["manual_paths"], serde_json::json!(["/wow"]));
}

#[test]
fn entfernte_quellen_fallen_beim_lesen_heraus() {
    let pfad = datei("quellen.json");
    std::fs::write(
        &pfad,
        r#"{ "source_priority": ["tukui", "curseforge", "github"] }"#,
    )
    .expect("Testdatei");

    let s = Settings::load_at(&pfad);
    assert_eq!(s.source_priority, vec!["curseforge"]);
}
