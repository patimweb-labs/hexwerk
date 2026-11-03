//! `discover_at`: Zusammenfuehrung der Sucher, Notizen, Deduplizierung und die
//! vom Nutzer gewaehlten Ordner.

mod common;

use common::Tree;
use hexwerk_core::{
    discover_at, ist_wow_ordner, wow_ordner, Flavor, NoteStatus, Origin, SearchRoots,
};

const WOW: &str = "drive_c/Program Files (x86)/World of Warcraft";

fn roots(home: &Tree) -> SearchRoots {
    // Ein echtes Home-Verzeichnis, aber ein kuenstliches: genau der Trick, den
    // `SearchRoots::from_home` moeglich macht.
    SearchRoots::from_home(home.root())
}

#[test]
fn leere_wurzeln_ergeben_keine_clients_und_sieben_notizen() {
    let d = discover_at(&SearchRoots::default(), &[]);
    assert!(d.clients.is_empty());
    // je Herkunft eine Notiz plus eine fuer die manuellen Ordner
    assert_eq!(d.notes.len(), Origin::all().len() + 1);
    assert!(d.notes.iter().all(|n| n.status == NoteStatus::Skip));
    assert!(d.notes.iter().any(|n| n.step == "Manuell"));
}

#[test]
fn notizen_unterscheiden_nicht_installiert_von_leerem_praefix() {
    let t = Tree::new("notizen");
    let mut r = roots(&t);
    r.faugus = vec![t.dir("Faugus/ohne_wow")];
    // Steam-Wurzel zeigt ins Leere.
    r.steam = vec![t.path("kein_steam")];

    let d = discover_at(&r, &[]);
    let faugus = d
        .notes
        .iter()
        .find(|n| n.step == "Faugus")
        .expect("Notiz fuer Faugus");
    let steam = d
        .notes
        .iter()
        .find(|n| n.step == "Steam")
        .expect("Notiz fuer Steam");
    assert_eq!(faugus.status, NoteStatus::Skip);
    assert!(faugus.detail.contains("kein Praefix"), "{}", faugus.detail);
    assert_eq!(steam.status, NoteStatus::Skip);
    assert!(
        steam.detail.contains("keine WoW-Installation"),
        "{}",
        steam.detail
    );
}

#[test]
fn notiz_zaehlt_die_gefundenen_installationen() {
    let t = Tree::new("notiz_zaehlt");
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(
        &format!("Faugus/zweite/{WOW}/_classic_"),
        "Questie",
        "11502",
    );

    let d = discover_at(&roots(&t), &[]);
    let faugus = d.notes.iter().find(|n| n.step == "Faugus").unwrap();
    assert_eq!(faugus.status, NoteStatus::Ok);
    assert!(faugus.detail.contains('2'), "{}", faugus.detail);
    assert_eq!(d.clients.len(), 2);
}

#[test]
fn derselbe_spielordner_ueber_zwei_herkuenfte_zaehlt_einmal() {
    let t = Tree::new("dedup");
    // Praefix liegt in einem Ordner, den zwei Herkuenfte als Wurzel bekommen
    // (etwa `~/Games` fuer Lutris und ein Praefix darunter fuer Heroic).
    t.addon(&format!("Games/battlenet/{WOW}/_retail_"), "ElvUI", "11509");
    let mut r = roots(&t);
    r.faugus = vec![t.path("Games")];
    r.lutris = vec![t.path("Games")];

    let d = discover_at(&r, &[]);
    assert_eq!(d.clients.len(), 1, "gezaehlt wird der echte Spielordner");
    // Bei Gleichstand gewinnt die zuerst gelaufene Herkunft.
    assert_eq!(d.clients[0].origin, Origin::Faugus);
}

#[test]
fn manueller_ordner_wird_erkannt() {
    let t = Tree::new("manuell");
    t.addon("wow/_retail_", "ElvUI", "11509");
    t.addon("wow/_classic_era_", "Questie", "11502");

    let d = discover_at(&SearchRoots::default(), &[t.path("wow")]);
    assert_eq!(d.clients.len(), 1);
    let c = &d.clients[0];
    assert_eq!(c.origin, Origin::Manual);
    assert_eq!(c.label, "wow");
    assert_eq!(c.prefix, t.path("wow"));
    assert_eq!(c.game_dir, t.path("wow"));
    assert_eq!(c.flavors.len(), 2);
    let note = d.notes.iter().find(|n| n.step == "Manuell").unwrap();
    assert_eq!(note.status, NoteStatus::Ok);
}

#[test]
fn manueller_ordner_ohne_client_wird_verworfen() {
    let t = Tree::new("manuell_leer");
    t.dir("leer/Unterordner");
    // Ein Ordner mit `Interface/AddOns`, aber ohne `_…_`-Ebene dazwischen.
    t.dir("falsch/Interface/AddOns");

    let d = discover_at(&SearchRoots::default(), &[t.path("leer"), t.path("falsch")]);
    assert!(d.clients.is_empty());
    let note = d.notes.iter().find(|n| n.step == "Manuell").unwrap();
    assert_eq!(note.status, NoteStatus::Skip);
}

#[test]
fn derselbe_manuelle_ordner_zaehlt_einmal() {
    let t = Tree::new("manuell_doppelt");
    t.addon("wow/_retail_", "ElvUI", "11509");

    let d = discover_at(
        &SearchRoots::default(),
        &[t.path("wow"), t.path("wow"), t.path("wow/.")],
    );
    assert_eq!(d.clients.len(), 1);
}

#[test]
fn manueller_ordner_gewinnt_gegen_eine_herkunft() {
    // Derselbe Ordner ist ueber Steam erreichbar und steht zusaetzlich in den
    // Einstellungen. Bleibt der Steam-Eintrag stehen, ist der selbst angegebene
    // Pfad nicht mehr zu entfernen.
    let t = Tree::new("manuell_gewinnt");
    let game_dir =
        t.path("Steam/steamapps/compatdata/7/pfx/drive_c/Program Files/World of Warcraft");
    t.addon(
        "Steam/steamapps/compatdata/7/pfx/drive_c/Program Files/World of Warcraft/_retail_",
        "ElvUI",
        "11509",
    );

    let mut r = roots(&t);
    r.steam = vec![t.path("Steam")];
    let d = discover_at(&r, &[game_dir.clone()]);

    assert_eq!(d.clients.len(), 1, "ein Ordner, nicht zwei");
    assert_eq!(d.clients[0].origin, Origin::Manual);
    assert_eq!(d.clients[0].prefix, game_dir);
}

#[test]
fn ist_wow_ordner_erkennt_nur_spielordner() {
    let t = Tree::new("ist_wow_ordner");
    t.addon("wow/_retail_", "ElvUI", "11509");
    t.dir("leer");
    // Der Addon-Ordner selbst ist kein Spielordner: die `_…_`-Ebene fehlt.
    t.dir("wow/_retail_/Interface/AddOns");

    assert!(ist_wow_ordner(&t.path("wow")));
    assert!(!ist_wow_ordner(&t.path("leer")));
    assert!(!ist_wow_ordner(&t.path("gibt_es_nicht")));
}

#[test]
fn wow_ordner_nimmt_bei_bedarf_die_ebene_darueber() {
    // Wer aus dem Addon-Ordner kommt, zeigt auf `…/_retail_`. Gespeichert wird
    // der Ordner darueber, denn dort sucht der Scan.
    let t = Tree::new("wow_ordner");
    t.addon("wow/_retail_", "ElvUI", "11509");
    let retail = t.path("wow/_retail_");
    let erwartet = retail
        .canonicalize()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    assert_eq!(wow_ordner(&retail), Some(erwartet.clone()));
    assert_eq!(wow_ordner(&t.path("wow")), Some(erwartet));
    assert_eq!(wow_ordner(&t.dir("leer")), None);
    // Zwei Ebenen zu tief gibt nichts her: dort liegen keine `_…_`-Ordner.
    assert_eq!(wow_ordner(&t.path("wow/_retail_/Interface")), None);
}

#[test]
fn flavor_ohne_addons_aber_mit_binary_gilt_als_client() {
    let t = Tree::new("frisch");
    t.fresh_flavor("wow/_retail_", "Wow.exe");

    let d = discover_at(&SearchRoots::default(), &[t.path("wow")]);
    assert_eq!(d.clients.len(), 1);
    assert_eq!(d.clients[0].flavors.len(), 1);
    assert_eq!(d.clients[0].flavors[0].flavor, Flavor::Retail);
    assert!(d.clients[0].flavors[0].interface_numbers.is_empty());
    // Kein `Interface/AddOns`, also auch keine Addons - aber der Pfad steht.
    assert_eq!(
        d.clients[0].flavors[0].addons_dir,
        t.path("wow/_retail_/Interface/AddOns")
    );
}

#[test]
fn flavor_ohne_addons_und_ohne_binary_wird_verworfen() {
    let t = Tree::new("ohne_alles");
    t.dir("wow/_retail_/Logs");
    t.dir("wow/nicht_ein_client");

    let d = discover_at(&SearchRoots::default(), &[t.path("wow")]);
    assert!(d.clients.is_empty(), "leerer Ordner ist kein Client");
}

#[test]
fn interface_nummern_werden_ueber_addons_gesammelt() {
    let t = Tree::new("interface");
    t.addon("wow/_classic_era_", "ElvUI", "11509, 20506, 16001");
    t.addon("wow/_classic_era_", "Questie", "11509");
    t.addon("wow/_classic_era_", "Details", "50500");

    let d = discover_at(&SearchRoots::default(), &[t.path("wow")]);
    let f = &d.clients[0].flavors[0];
    assert_eq!(f.interface_numbers, vec![11509, 16001, 20506, 50500]);
    assert!(f.accepts(20506));
    assert!(!f.accepts(11510));
}

#[test]
fn wiederholter_lauf_liefert_dasselbe() {
    let t = Tree::new("wiederholbar");
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(
        &format!("Faugus/zweite/pfx/{WOW}/_classic_"),
        "Questie",
        "11502",
    );
    t.addon(
        "Steam/steamapps/compatdata/7/pfx/drive_c/Program Files/World of Warcraft/_retail_",
        "ElvUI",
        "11509",
    );
    t.addon("wow/_retail_", "ElvUI", "11509");

    let mut r = roots(&t);
    r.steam = vec![t.path("Steam")];
    let extra = [t.path("wow")];
    let a = discover_at(&r, &extra);
    let b = discover_at(&r, &extra);
    assert_eq!(a, b);
    assert_eq!(a.clients.len(), 4);
}

#[test]
fn clients_sind_nach_herkunft_sortiert() {
    let t = Tree::new("sortierung");
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(
        "Steam/steamapps/compatdata/7/pfx/drive_c/Program Files/World of Warcraft/_retail_",
        "ElvUI",
        "11509",
    );
    t.addon("wow/_retail_", "ElvUI", "11509");

    let mut r = roots(&t);
    r.steam = vec![t.path("Steam")];
    let d = discover_at(&r, &[t.path("wow")]);
    let labels: Vec<&str> = d.clients.iter().map(|c| c.origin.label()).collect();
    assert_eq!(labels, vec!["Faugus", "Manuell", "Steam"]);
}
