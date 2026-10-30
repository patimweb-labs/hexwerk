//! Praefix-Herkuenfte ausser Steam. Der Kern dieser Tests ist die
//! Faugus-Eigenheit aus dem README: `drive_c` und `pfx/drive_c` liegen auf
//! derselben Ebene.

mod common;

use common::Tree;
use hexwerk_core::clients::finders::generic;
use hexwerk_core::{Flavor, Origin};

const WOW: &str = "drive_c/Program Files (x86)/World of Warcraft";

#[test]
fn faugus_findet_die_pfx_variante() {
    let t = Tree::new("faugus_pfx");
    // Genau der gemessene Fall: das Spiel liegt in `pfx/drive_c`, daneben steht
    // eine leere `drive_c`.
    t.dir("Faugus/battlenet/drive_c");
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_classic_era_"),
        "ElvUI",
        "11509",
    );

    let found = generic::find_faugus_in(&[t.path("Faugus")]);
    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.origin, Origin::Faugus);
    assert_eq!(
        c.id,
        format!("faugus:{}", t.path("Faugus/battlenet").display())
    );
    assert_eq!(c.label, "battlenet");
    assert_eq!(c.prefix, t.path("Faugus/battlenet"));
    assert_eq!(c.game_dir, t.path(format!("Faugus/battlenet/pfx/{WOW}")));
    assert_eq!(c.flavors[0].flavor, Flavor::ClassicEra);
}

#[test]
fn faugus_findet_die_flache_variante() {
    let t = Tree::new("faugus_flach");
    t.addon(
        &format!("Faugus/battlenet/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );

    let found = generic::find_faugus_in(&[t.path("Faugus")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].game_dir, t.path(format!("Faugus/battlenet/{WOW}")));
}

#[test]
fn faugus_bevorzugt_die_ergiebigere_fundstelle() {
    let t = Tree::new("faugus_beide");
    // Beide Baeume enthalten ein Spiel. Gewinnen muss der mit mehr
    // Client-Varianten - sonst zeigt die Oberflaeche je nach Dateisystem-
    // Reihenfolge mal den einen, mal den anderen Baum.
    t.addon(
        &format!("Faugus/battlenet/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(
        &format!("Faugus/battlenet/pfx/{WOW}/_classic_"),
        "Questie",
        "11502",
    );

    let found = generic::find_faugus_in(&[t.path("Faugus")]);
    assert_eq!(found.len(), 1, "ein Praefix ist ein Client");
    assert_eq!(
        found[0].game_dir,
        t.path(format!("Faugus/battlenet/pfx/{WOW}"))
    );
    assert_eq!(found[0].flavors.len(), 2);
}

#[test]
fn faugus_ohne_wow_ist_leer() {
    let t = Tree::new("faugus_ohne_wow");
    t.dir("Faugus/battlenet/pfx/drive_c/Program Files (x86)/IrgendeinSpiel");
    t.dir("Faugus/andere/drive_c");

    assert!(generic::find_faugus_in(&[t.path("Faugus")]).is_empty());
    // Wurzel fehlt voellig: kein Fehler, nur nichts.
    assert!(generic::find_faugus_in(&[t.path("gibt-es-nicht")]).is_empty());
}

#[test]
fn jede_herkunft_findet_ihren_ordner() {
    let t = Tree::new("herkuenfte");
    t.addon(&format!("Heroic/Divinity/{WOW}/_retail_"), "ElvUI", "11509");
    t.addon(
        &format!("Lutris/satisfactory/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );
    t.addon(&format!("Bottles/gaming/{WOW}/_retail_"), "ElvUI", "11509");
    t.addon(&format!("PortWine/PP/{WOW}/_retail_"), "ElvUI", "11509");

    let heroic = generic::find_heroic_in(&[t.path("Heroic")]);
    assert_eq!(heroic.len(), 1);
    assert_eq!(heroic[0].origin, Origin::Heroic);
    assert_eq!(heroic[0].label, "Divinity");
    assert!(heroic[0].id.starts_with("heroic:"));

    let lutris = generic::find_lutris_in(&[t.path("Lutris")]);
    assert_eq!(lutris.len(), 1);
    assert_eq!(lutris[0].origin, Origin::Lutris);
    assert!(lutris[0].id.starts_with("lutris:"));

    let bottles = generic::find_bottles_in(&[t.path("Bottles")]);
    assert_eq!(bottles.len(), 1);
    assert_eq!(bottles[0].origin, Origin::Bottles);

    let portproton = generic::find_portproton_in(&[t.path("PortWine")]);
    assert_eq!(portproton.len(), 1);
    assert_eq!(portproton[0].origin, Origin::PortProton);
    assert_eq!(portproton[0].label, "PP");
}

#[test]
fn ueberlappende_wurzeln_ergeben_einen_client() {
    let t = Tree::new("ueberlappung");
    // `~/Games` enthaelt `~/Games/Heroic/Prefixes`; wer beide als Wurzel
    // uebergibt, darf den Client nicht doppelt bekommen.
    t.addon(
        &format!("Games/Heroic/Prefixes/x/{WOW}/_retail_"),
        "ElvUI",
        "11509",
    );

    let found = generic::find_heroic_in(&[t.path("Games"), t.path("Games/Heroic/Prefixes")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].prefix, t.path("Games/Heroic/Prefixes/x"));
}

#[test]
fn die_wurzel_darf_selbst_ein_praefix_sein() {
    let t = Tree::new("wurzel_ist_praefix");
    // Nutzer zeigen haeufig direkt auf den konkreten Praefix-Ordner.
    t.addon(&format!("mein_praefix/{WOW}/_retail_"), "ElvUI", "11509");

    let found = generic::find_lutris_in(&[t.path("mein_praefix")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].prefix, t.path("mein_praefix"));
}

#[test]
fn sortiert_nach_id() {
    let t = Tree::new("sortierung");
    for name in ["zeta", "alpha", "mitte"] {
        t.addon(&format!("Heroic/{name}/{WOW}/_retail_"), "ElvUI", "11509");
    }

    let found = generic::find_heroic_in(&[t.path("Heroic")]);
    let names: Vec<&str> = found.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mitte", "zeta"]);
}
