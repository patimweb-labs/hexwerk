//! Windows und macOS: das Spiel liegt entweder direkt in einem Programmordner
//! oder in einer CrossOver-Flasche. Beides wird gegen künstliche Bäume geprüft,
//! nie gegen das echte System.

mod common;

use common::Tree;
use hexwerk_core::clients::finders::direct;
use hexwerk_core::{Flavor, Origin};

#[test]
fn windows_findet_das_spiel_in_den_programmdateien() {
    let t = Tree::new("windows_programme");
    t.addon(
        "Program Files (x86)/World of Warcraft/_retail_/Interface/AddOns",
        "Details!",
        "110105",
    );

    let found = direct::find_direct_in(Origin::Windows, &[t.path("Program Files (x86)")]);

    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.origin, Origin::Windows);
    assert_eq!(
        c.game_dir,
        t.path("Program Files (x86)/World of Warcraft"),
        "der Spielordner ist das Verzeichnis mit den _retail_-artigen Kindern"
    );
    assert_eq!(c.flavors[0].flavor, Flavor::Retail);
    assert!(c.id.starts_with("windows:"));
    assert_eq!(c.label, "World of Warcraft");
}

#[test]
fn windows_nimmt_auch_die_wurzel_selbst_als_spielordner() {
    let t = Tree::new("windows_direkt");
    t.addon("wow/_classic_era_/Interface/AddOns", "ElvUI", "11509");

    // Der Nutzer zeigt im Explorer direkt auf den Spielordner.
    let found = direct::find_direct_in(Origin::Windows, &[t.path("wow")]);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].flavors[0].flavor, Flavor::ClassicEra);
}

#[test]
fn macos_native_installation() {
    let t = Tree::new("macos_applications");
    t.addon(
        "Applications/World of Warcraft/_retail_/Interface/AddOns",
        "WeakAuras",
        "110107",
    );

    let found = direct::find_direct_in(Origin::MacNative, &[t.path("Applications")]);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].origin, Origin::MacNative);
    assert_eq!(found[0].game_dir, t.path("Applications/World of Warcraft"));
    assert!(found[0].id.starts_with("macos:"));
}

#[test]
fn crossover_flasche_ist_ein_wine_praefix() {
    let t = Tree::new("crossover_bottle");
    t.addon(
        "Bottles/WoW/drive_c/Program Files (x86)/World of Warcraft/_retail_/Interface/AddOns",
        "Details!",
        "110105",
    );

    let found = direct::find_crossover_in(&[t.path("Bottles")]);

    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.origin, Origin::CrossOver);
    assert_eq!(c.label, "WoW", "der Flaschenname ist das, was der Nutzer sieht");
    assert_eq!(c.prefix, t.path("Bottles/WoW"), "Praefix ist die Flasche");
    assert_eq!(c.flavors[0].flavor, Flavor::Retail);
}

#[test]
fn zwei_installationen_unter_einer_wurzel_werden_beide_gefunden() {
    let t = Tree::new("zwei_installationen");
    t.addon(
        "Applications/Erste/World of Warcraft/_retail_/Interface/AddOns",
        "Details!",
        "110105",
    );
    t.addon(
        "Applications/Zweite/World of Warcraft/_classic_era_/Interface/AddOns",
        "ElvUI",
        "11509",
    );

    let found = direct::find_direct_in(Origin::MacNative, &[t.path("Applications")]);

    assert_eq!(found.len(), 2);
    let labels: Vec<&str> = found.iter().map(|c| c.label.as_str()).collect();
    assert!(labels.contains(&"Erste · World of Warcraft"));
    assert!(labels.contains(&"Zweite · World of Warcraft"));
}

#[test]
fn ein_leerer_spielordner_zaehlt_nicht() {
    let t = Tree::new("windows_leer");
    t.dir("Program Files (x86)/World of Warcraft");

    let found = direct::find_direct_in(Origin::Windows, &[t.path("Program Files (x86)")]);
    assert!(
        found.is_empty(),
        "ein World-of-Warcraft-Ordner ohne Client-Varianten ist kein Client"
    );
}

#[test]
fn alle_herkuenfte_kennt_die_neuen() {
    for origin in [Origin::Windows, Origin::CrossOver, Origin::MacNative] {
        assert!(
            Origin::all().contains(&origin),
            "{} fehlt in Origin::all() und wuerde nie gesucht",
            origin.label()
        );
        assert!(!origin.label().is_empty());
        assert!(origin.slug().is_ascii());
    }
}
