//! Steam-Erkennung gegen kuenstliche Bibliotheksbaeume.

mod common;

use common::Tree;
use hexwerk_core::clients::finders::steam;
use hexwerk_core::{Flavor, Origin};

const PREFIX: &str = "Steam/steamapps/compatdata/1664280/pfx";
const GAME: &str =
    "Steam/steamapps/compatdata/1664280/pfx/drive_c/Program Files (x86)/World of Warcraft";

/// Baut ein Praefix mit einem Client-Ordner und einem Addon darin.
fn wow_prefix(t: &Tree, game_rel: &str, flavor: &str, folder: &str, interface: &str) {
    t.addon(&format!("{game_rel}/{flavor}"), folder, interface);
}

#[test]
fn findet_retail_im_compatdata_praefix() {
    let t = Tree::new("steam_retail");
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");
    t.file(
        "Steam/steamapps/appmanifest_1664280.acf",
        "\"AppState\"\n{\n\t\"appid\"\t\t\"1664280\"\n\t\"name\"\t\t\"World of Warcraft\"\n}\n",
    );

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.origin, Origin::Steam);
    assert_eq!(c.id, "steam:1664280");
    // Ohne Manifest waere das "Steam 1664280"; der Name kommt aus der ACF.
    assert_eq!(c.label, "World of Warcraft");
    assert_eq!(c.prefix, t.path(PREFIX));
    assert_eq!(c.game_dir, t.path(GAME));
    assert_eq!(c.flavors.len(), 1);
    assert_eq!(c.flavors[0].flavor, Flavor::Retail);
    assert_eq!(c.flavors[0].interface_numbers, vec![11509]);
    assert!(c.flavors[0].accepts(11509));
    assert!(!c.flavors[0].accepts(50500));
}

#[test]
fn ohne_manifest_bleibt_die_appid_als_name() {
    let t = Tree::new("steam_ohne_manifest");
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "Steam 1664280");
}

#[test]
fn liest_weitere_bibliothek_aus_neuer_vdf_form() {
    let t = Tree::new("steam_vdf_neu");
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");
    // Zweite Bibliothek auf einem anderen Datentraeger, nur ueber die VDF
    // erreichbar.
    let zweite = "Platte2/SteamLibrary/steamapps/compatdata/999/pfx/drive_c/Program Files (x86)/World of Warcraft";
    t.addon(&format!("{zweite}/_classic_"), "Questie", "11502");
    t.dir("Platte2/SteamLibrary/steamapps");
    t.file(
        "Steam/steamapps/libraryfolders.vdf",
        &format!(
            "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"1664280\"\t\t\"1\"\n\t\t}}\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
            t.path("Steam").display(),
            t.path("Platte2/SteamLibrary").display()
        ),
    );

    let found = steam::find_in(&[t.path("Steam")]);
    let ids: Vec<&str> = found.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["steam:1664280", "steam:999"]);
    let zweiter = &found[1];
    assert_eq!(zweiter.flavors.len(), 1);
    assert_eq!(zweiter.flavors[0].flavor, Flavor::Classic);
    assert_eq!(zweiter.flavors[0].interface_numbers, vec![11502]);
}

#[test]
fn liest_weitere_bibliothek_aus_alte_vdf_form() {
    let t = Tree::new("steam_vdf_alt");
    let zweite = "Platte2/SteamLibrary/steamapps/compatdata/999/pfx/drive_c/Program Files (x86)/World of Warcraft";
    t.addon(&format!("{zweite}/_retail_"), "ElvUI", "11509");
    // Alte Form: Bibliotheken als numerierte Schluessel ohne `path`-Feld.
    t.file(
        "Steam/steamapps/libraryfolders.vdf",
        &format!(
            "\"LibraryFolders\"\n{{\n\t\"TimeNextStatsReport\"\t\t\"1690000000\"\n\t\"1\"\t\t\"{}\"\n}}\n",
            t.path("Platte2/SteamLibrary").display()
        ),
    );

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "steam:999");
}

#[test]
fn vdf_app_bloecke_liefern_keine_bibliothek() {
    let t = Tree::new("steam_vdf_apps");
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");
    // Nur AppIDs, keine Pfade: daraus darf keine Bibliothek werden.
    t.file(
        "Steam/steamapps/libraryfolders.vdf",
        "\"libraryfolders\" { \"0\" { \"apps\" { \"1664280\" \"1\" \"228980\" \"2\" } } }",
    );

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
}

#[test]
fn ohne_compatdata_ist_die_liste_leer() {
    let t = Tree::new("steam_leer");
    t.dir("Steam/steamapps");
    assert!(steam::find_in(&[t.path("Steam")]).is_empty());
    // Wurzel existiert gar nicht: ebenfalls leer, aber ohne Panik.
    assert!(steam::find_in(&[t.path("gibt-es-nicht")]).is_empty());
    assert!(steam::find_in(&[]).is_empty());
}

#[test]
fn findet_auch_program_files_ohne_x86() {
    let t = Tree::new("steam_program_files");
    let game = "Steam/steamapps/compatdata/1/pfx/drive_c/Program Files/World of Warcraft";
    t.addon(&format!("{game}/_retail_"), "ElvUI", "11509");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].game_dir, t.path(game));
}

#[test]
fn findet_wow_unabhaengig_von_gross_klein() {
    let t = Tree::new("steam_case");
    let game = "Steam/steamapps/compatdata/1/pfx/drive_c/program files (x86)/world of warcraft";
    t.addon(&format!("{game}/_retail_"), "ElvUI", "11509");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1, "Wine laesst die Schreibweise stehen");
    assert_eq!(found[0].flavors[0].flavor, Flavor::Retail);
}

#[test]
fn findet_wow_in_tieferem_ordner() {
    let t = Tree::new("steam_tief");
    let game = "Steam/steamapps/compatdata/1/pfx/drive_c/Games/Rio/World of Warcraft";
    t.addon(&format!("{game}/_retail_"), "ElvUI", "11509");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].game_dir, t.path(game));
}

#[test]
fn ignoriert_zu_tiefe_ordner() {
    let t = Tree::new("steam_zu_tief");
    // Vier Ebenen unter drive_c: ausserhalb der Tiefenbegrenzung, damit ein
    // Scan nicht das ganze Praefix ablaufen muss.
    let game = "Steam/steamapps/compatdata/1/pfx/drive_c/a/b/c/World of Warcraft";
    t.addon(&format!("{game}/_retail_"), "ElvUI", "11509");

    assert!(steam::find_in(&[t.path("Steam")]).is_empty());
}

#[test]
fn leerer_namensgleicher_ordner_verdeckt_den_echten_nicht() {
    let t = Tree::new("steam_leerer_ordner");
    let pfx = "Steam/steamapps/compatdata/1/pfx";
    // Rest eines abgebrochenen Updates: Ordner heisst richtig, ist aber leer.
    t.dir(format!("{pfx}/drive_c/World of Warcraft"));
    let echt = format!("{pfx}/drive_c/Games/World of Warcraft");
    t.addon(&format!("{echt}/_retail_"), "ElvUI", "11509");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].game_dir, t.path(&echt));
}

#[test]
fn ein_praefix_mit_zwei_fundstellen_wird_ein_client() {
    let t = Tree::new("steam_zwei_fundstellen");
    // Dieselbe AppID in zwei Bibliotheken: der compatdata-Ordner ist je
    // Bibliothek ein eigener Pfad, gehoert aber zum selben Client.
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");
    let zweite = "Platte2/SteamLibrary/steamapps/compatdata/1664280/pfx/drive_c/Program Files (x86)/World of Warcraft";
    t.addon(&format!("{zweite}/_retail_"), "ElvUI", "11509");
    t.file(
        "Steam/steamapps/libraryfolders.vdf",
        &format!(
            "\"libraryfolders\" {{ \"1\" {{ \"path\" \"{}\" }} }}",
            t.path("Platte2/SteamLibrary").display()
        ),
    );

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1, "je AppID genau ein Client");
}

#[test]
fn mehrere_flavors_werden_einzeln_gemeldet() {
    let t = Tree::new("steam_flavors");
    wow_prefix(&t, GAME, "_retail_", "ElvUI", "11509");
    t.addon(&format!("{GAME}/_classic_era_"), "Questie", "11502, 11503");
    t.fresh_flavor(&format!("{GAME}/_ptr_"), "Wow.exe");

    let found = steam::find_in(&[t.path("Steam")]);
    assert_eq!(found.len(), 1);
    let flavors: Vec<&Flavor> = found[0].flavors.iter().map(|f| &f.flavor).collect();
    assert_eq!(
        flavors,
        vec![&Flavor::ClassicEra, &Flavor::RetailPtr, &Flavor::Retail],
        "sortiert nach Ordnername, damit der Scan wiederholbar bleibt"
    );
    // `## Interface` ist eine Menge, auch innerhalb eines Addons.
    assert_eq!(found[0].flavors[0].interface_numbers, vec![11502, 11503]);
    // Frische Installation: Binary da, AddOns noch nicht.
    assert!(found[0].flavors[1].interface_numbers.is_empty());
}
