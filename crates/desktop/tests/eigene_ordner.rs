//! Eigene Spielordner: pruefen, aufnehmen, austragen.
//!
//! Geprueft wird ohne Fenster und ohne Konfigurationsdatei — genau die zwei
//! Schritte, die schiefgehen koennen, sind hier erreichbar. Der Pfad zur
//! Einstellungsdatei wird nicht gebraucht; `$HOME` bleibt unberuehrt.

use std::path::{Path, PathBuf};

use hexwerk_desktop::commands::{aufnehmen, austragen};
use hexwerk_desktop::settings::Settings;

struct Baum {
    root: PathBuf,
}

impl Baum {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("eigene-ordner")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("Testbaum anlegen");
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn dir(&self, rel: &str) -> PathBuf {
        let p = self.path(rel);
        std::fs::create_dir_all(&p).expect("Ordner anlegen");
        p
    }

    /// Ein Client-Ordner mit `_retail_/Interface/AddOns`.
    fn client(&self, rel: &str) -> PathBuf {
        let p = self.path(rel);
        std::fs::create_dir_all(p.join("_retail_").join("Interface").join("AddOns"))
            .expect("Client anlegen");
        p
    }
}

fn pfad_text(p: &Path) -> String {
    p.canonicalize().expect("kanonisch").display().to_string()
}

#[test]
fn spielordner_wird_aufgenommen() {
    let t = Baum::new("aufnehmen");
    let wow = t.client("wow");
    let mut s = Settings::default();

    assert!(aufnehmen(&mut s, &wow.display().to_string()).expect("annehmen"));
    assert_eq!(s.manual_paths, vec![pfad_text(&wow)]);
}

#[test]
fn leerzeichen_am_rand_stoeren_nicht() {
    let t = Baum::new("leerzeichen");
    let wow = t.client("wow");
    let mut s = Settings::default();

    aufnehmen(&mut s, &format!("  {}  ", wow.display())).expect("annehmen");
    assert_eq!(s.manual_paths, vec![pfad_text(&wow)]);
}

#[test]
fn der_spielordner_selbst_wird_angenommen() {
    // Der haeufigste Fall: der Nutzer zeigt auf `…/_retail_`, weil dort die
    // Addons liegen. Gespeichert wird der Ordner darueber.
    let t = Baum::new("spielordner");
    let wow = t.client("wow");
    let retail = wow.join("_retail_");
    let mut s = Settings::default();

    aufnehmen(&mut s, &retail.display().to_string()).expect("annehmen");
    assert_eq!(s.manual_paths, vec![pfad_text(&wow)]);
}

#[test]
fn ordner_ohne_wow_wird_abgelehnt() {
    let t = Baum::new("ablehnen");
    let leer = t.dir("leer");
    let mut s = Settings::default();

    let fehler = aufnehmen(&mut s, &leer.display().to_string()).expect_err("ablehnen");
    assert!(fehler.contains("Kein World-of-Warcraft-Ordner"), "{fehler}");
    assert!(s.manual_paths.is_empty());
}

#[test]
fn unbekannter_pfad_wird_abgelehnt() {
    let t = Baum::new("unbekannt");
    let mut s = Settings::default();

    let fehler =
        aufnehmen(&mut s, &t.path("gibt_es_nicht").display().to_string()).expect_err("ablehnen");
    assert!(fehler.contains("nicht gefunden"), "{fehler}");
    assert!(s.manual_paths.is_empty());
}

#[test]
fn derselbe_ordner_wird_nicht_zweimal_aufgenommen() {
    let t = Baum::new("zweimal");
    let wow = t.client("wow");
    let mut s = Settings::default();

    assert!(aufnehmen(&mut s, &wow.display().to_string()).expect("annehmen"));
    assert!(!aufnehmen(&mut s, &wow.display().to_string()).expect("annehmen"));
    assert!(!aufnehmen(&mut s, &format!("{}/.", wow.display())).expect("annehmen"));
    assert_eq!(s.manual_paths.len(), 1);
}

#[test]
fn austragen_trifft_auch_andere_schreibweisen() {
    // Sonst bliebe die Karte in der Oberflaeche stehen: der Scan entdoppelt
    // ueber den echten Pfad, das Austragen muss dasselbe tun.
    let t = Baum::new("austragen");
    let wow = t.client("wow");
    let mut s = Settings::default();
    aufnehmen(&mut s, &wow.display().to_string()).expect("annehmen");

    assert!(austragen(&mut s, &format!("{}/.", wow.display())));
    assert!(s.manual_paths.is_empty());
}

#[test]
fn austragen_laesst_andere_ordner_stehen() {
    let t = Baum::new("austragen_andere");
    let eins = t.client("wow");
    let zwei = t.client("wow2");
    let mut s = Settings::default();
    aufnehmen(&mut s, &eins.display().to_string()).expect("annehmen");
    aufnehmen(&mut s, &zwei.display().to_string()).expect("annehmen");

    assert!(austragen(&mut s, &eins.display().to_string()));
    assert_eq!(s.manual_paths, vec![pfad_text(&zwei)]);
    // Was nicht drinsteht, aendert nichts - und meldet das auch.
    assert!(!austragen(&mut s, &eins.display().to_string()));
}
