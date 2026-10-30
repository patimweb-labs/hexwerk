//! Integrationstests gegen die oeffentliche Schnittstelle.
//!
//! Aufbau je Test: echte Ordner mit echten `.toc`-Dateien in einem
//! temporaeren Verzeichnis, damit der Scan-Pfad (Dateisystem lesen) und der
//! Index-Pfad (SQLite schreiben) gemeinsam geprueft werden — die Fehler, die
//! zaehlen, sitzen genau an dieser Naht.

use std::path::{Path, PathBuf};

use hexwerk::{Client, Discovery, Flavor, FlavorDir, Origin};
use hexwerk_index::{Filters, Index};
use hexwerk_sources::{Entry, Release, Source};
use tempfile::TempDir;

/// Ein Client samt angelegten Addon-Ordnern.
fn client_fixture(
    prefix_root: &Path,
    origin: Origin,
    id: &str,
    flavor: &str,
    addons: &[(&str, &str)],
) -> Client {
    let game_dir = prefix_root.join("World of Warcraft");
    let dir = game_dir.join(flavor);
    let addons_dir = dir.join("Interface").join("AddOns");
    std::fs::create_dir_all(&addons_dir).expect("Addon-Ordner");
    for (folder, toc) in addons {
        let addon_dir = addons_dir.join(folder);
        std::fs::create_dir_all(&addon_dir).expect("Addon-Ordner");
        // Der Ordnername muss zur .toc passen, sonst nimmt scan_addons eine
        // andere Datei im Ordner.
        std::fs::write(addon_dir.join(format!("{folder}.toc")), toc).expect(".toc");
    }
    Client {
        id: id.to_string(),
        origin,
        label: "Battle.net".to_string(),
        prefix: prefix_root.join("pfx"),
        game_dir,
        flavors: vec![FlavorDir {
            flavor: Flavor::from_dir_name(flavor),
            dir,
            addons_dir,
            interface_numbers: vec![11509],
        }],
    }
}

fn discovery(clients: Vec<Client>) -> Discovery {
    Discovery {
        clients,
        notes: Vec::new(),
    }
}

fn release(source: Source, version: &str, interfaces: &[u32]) -> Release {
    Release {
        source,
        external_id: format!("{}-{version}", source.label()),
        version: version.to_string(),
        released_at: Some("2026-10-01T00:00:00Z".to_string()),
        download_url: format!("https://example.invalid/{version}.zip"),
        size_bytes: Some(1_024),
        interface_numbers: interfaces.to_vec(),
    }
}

fn entry(key: &str, name: &str, author: &str, releases: Vec<Release>) -> Entry {
    Entry {
        key: key.to_string(),
        name: name.to_string(),
        author: author.to_string(),
        summary: "Beschreibung".to_string(),
        homepage: Some("https://example.invalid".to_string()),
        category: Some("Combat".to_string()),
        releases,
    }
}

/// Standardkatalog: zwei Addons aus zwei Quellen.
fn catalogue() -> Vec<Entry> {
    vec![
        entry(
            "weakauras",
            "WeakAuras",
            "WeakAuras Team",
            vec![release(Source::GitHub, "5.19.2", &[11509])],
        ),
        entry(
            "dbm",
            "Deadly Boss Mods",
            "MysticalOS",
            vec![release(Source::WowInterface, "11.0.5", &[11509, 20506])],
        ),
    ]
}

const TOC_WEAKAURAS: &str =
    "## Interface: 11509\n## Version: 5.19.2\n## Title: WeakAuras\n## Author: WeakAuras Team\n";
const TOC_DBM: &str =
    "## Interface: 11509\n## Version: 11.0.0\n## Title: Deadly Boss Mods\n## X-WoWI-ID: 1234\n";

fn path_ends_with(path: &Path, tail: &str) -> bool {
    path.to_string_lossy().ends_with(tail)
}

#[test]
fn vorgabepfad_zeigt_nach_hexwerk() {
    let path = Index::default_path();
    assert!(
        path_ends_with(&path, "hexwerk/index.db"),
        "unerwarteter Pfad: {}",
        path.display()
    );
}

#[test]
fn leerer_index_liefert_leere_listen() {
    let index = Index::open_in_memory().expect("Index");
    assert!(index
        .search(&Filters::new("irgendwas"), 50, 0)
        .expect("Suche")
        .is_empty());
    assert!(index.clients().expect("Clients").is_empty());
    assert!(index.installed(None).expect("Installiert").is_empty());
    assert!(index.updates().expect("Updates").is_empty());
    assert!(index.addon(42).expect("Detail").is_none());
    let stats = index.stats().expect("Zahlen");
    assert_eq!(stats.catalogue, 0);
    assert_eq!(stats.clients, 0);
}

#[test]
fn suche_trifft_praefix_und_autor() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let hits = index.search(&Filters::new("weak"), 50, 0).expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "WeakAuras");
    assert_eq!(
        hits[0].latest.as_ref().map(|r| r.version.as_str()),
        Some("5.19.2")
    );

    // Praefix: „weakau“ muss treffen, nicht nur das ganze Wort.
    let hits = index.search(&Filters::new("weakau"), 50, 0).expect("Suche");
    assert_eq!(hits.len(), 1);

    // Autor ist eine eigene FTS-Spalte, also durchsuchbar.
    let hits = index
        .search(&Filters::new("mysticalos"), 50, 0)
        .expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Deadly Boss Mods");
}

#[test]
fn leere_suche_sortiert_nach_namen() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let hits = index.search(&Filters::default(), 50, 0).expect("Suche");
    let names: Vec<&str> = hits.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, vec!["Deadly Boss Mods", "WeakAuras"]);
}

#[test]
fn fts_syntax_in_der_eingabe_bricht_nichts() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    // Anfuehrungszeichen, Operator und Klammer sind fuer FTS5 Syntax; die
    // Eingabe darf die Abfrage nicht zerlegen (sonst waere es ein Fehler statt
    // einer leeren Trefferliste).
    for raw in ["\"weak* OR name:", "weak AND", "(((", "***", "-weak", "a:b"] {
        let hits = index
            .search(&Filters::new(raw), 50, 0)
            .unwrap_or_else(|e| panic!("Eingabe {raw:?} brach die Suche: {e}"));
        assert!(hits.len() <= 2, "Eingabe {raw:?} lieferte {hits:?}");
    }
}

#[test]
fn umlaute_werden_ohne_diakritika_gefunden() {
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_catalogue(&[entry(
            "atzend",
            "Ätzende Auren",
            "Jemand",
            vec![release(Source::Local, "1.0", &[])],
        )])
        .expect("Katalog");

    assert_eq!(
        index
            .search(&Filters::new("atzende"), 10, 0)
            .expect("Suche")
            .len(),
        1,
        "remove_diacritics muss „Ä“ wie „A“ behandeln"
    );
    assert_eq!(
        index
            .search(&Filters::new("Ätzende"), 10, 0)
            .expect("Suche")
            .len(),
        1
    );
}

#[test]
fn filter_nach_quelle_und_interface() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let mut filters = Filters {
        sources: vec![Source::GitHub],
        ..Filters::default()
    };
    let hits = index.search(&filters, 50, 0).expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "WeakAuras");

    // Zwei Quellen in einem Filter sind ein ODER.
    filters.sources = vec![Source::GitHub, Source::WowInterface];
    assert_eq!(index.search(&filters, 50, 0).expect("Suche").len(), 2);

    filters.sources = vec![Source::Tukui];
    assert!(index.search(&filters, 50, 0).expect("Suche").is_empty());

    // Interface 20506 kennt nur DBM, 11509 beide.
    let mut filters = Filters {
        interface_number: Some(20_506),
        ..Filters::default()
    };
    let hits = index.search(&filters, 50, 0).expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Deadly Boss Mods");

    filters.interface_number = Some(11_509);
    assert_eq!(index.search(&filters, 50, 0).expect("Suche").len(), 2);

    filters.interface_number = Some(99_999);
    assert!(index.search(&filters, 50, 0).expect("Suche").is_empty());
}

#[test]
fn seitenweise_ausgabe() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let first = index.search(&Filters::default(), 1, 0).expect("Seite 1");
    let second = index.search(&Filters::default(), 1, 1).expect("Seite 2");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].name, second[0].name);
    assert!(index
        .search(&Filters::default(), 1, 5)
        .expect("Seite 6")
        .is_empty());
    // `search_all` ignoriert die Seitengroesse.
    assert_eq!(
        index.search_all(&Filters::default()).expect("alle").len(),
        2
    );
}

/// Ein Steam-Retail-Client mit zwei installierten Addons.
fn retail_client(root: &Path) -> Client {
    // Zwei Clients im selben Test brauchen verschiedene IDs; im Betrieb
    // liefert `discover` sie aus dem Praefixpfad.
    let id = format!("steam:{}", root.display());
    client_fixture(
        root,
        Origin::Steam,
        &id,
        "_retail_",
        &[("WeakAuras", TOC_WEAKAURAS), ("DBM-Core", TOC_DBM)],
    )
}

#[test]
fn scan_schreibt_clients_und_addons() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");

    let report = index
        .sync_clients(&discovery(vec![retail_client(dir.path())]))
        .expect("Scan");
    assert_eq!(report.clients, 1);
    assert_eq!(report.installed, 2);

    let clients = index.clients().expect("Clients");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].origin, "steam");
    assert_eq!(clients[0].origin_label, "Steam");
    assert_eq!(clients[0].flavor, "_retail_");
    assert_eq!(clients[0].interfaces, vec![11_509]);
    assert_eq!(clients[0].installed_count, 2);
    assert!(clients[0].addons_dir.ends_with("Interface/AddOns"));
    assert!(!clients[0].seen_at.is_empty());

    let installed = index.installed(None).expect("Installiert");
    let names: Vec<&str> = installed.iter().map(|r| r.name.as_str()).collect();
    // Ohne Katalog kommt der Name aus dem `.toc`.
    assert_eq!(names, vec!["Deadly Boss Mods", "WeakAuras"]);
    // Noch nicht verknuepft - der Katalog fehlt ja.
    assert!(installed.iter().all(|r| r.addon_id.is_none()));
    assert_eq!(
        installed[0]
            .foreign_keys
            .get("wowinterface")
            .map(String::as_str),
        Some("1234")
    );

    // Client-Filter der Tabelle.
    assert_eq!(
        index
            .installed(Some(&clients[0].id))
            .expect("gefiltert")
            .len(),
        2
    );
    assert!(index
        .installed(Some("gibt-es-nicht"))
        .expect("gefiltert")
        .is_empty());
}

#[test]
fn zweiter_scan_raeumt_verschwundenes_weg() {
    let dir_a = TempDir::new().expect("tempdir");
    let dir_b = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_clients(&discovery(vec![
            retail_client(dir_a.path()),
            retail_client(dir_b.path()),
        ]))
        .expect("Scan");
    assert_eq!(index.clients().expect("Clients").len(), 2);
    assert_eq!(index.installed(None).expect("Installiert").len(), 4);

    // Zweiter Lauf ohne den zweiten Client: sein Praefix ist geloescht.
    index
        .sync_clients(&discovery(vec![retail_client(dir_a.path())]))
        .expect("Scan");
    assert_eq!(index.clients().expect("Clients").len(), 1);
    assert_eq!(index.installed(None).expect("Installiert").len(), 2);
}

#[test]
fn update_gegen_hoechste_version_je_quelle() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_clients(&discovery(vec![retail_client(dir.path())]))
        .expect("Scan");
    index
        .sync_catalogue(&[
            // Installiert 5.19.2, Quelle hat dieselbe Version: kein Update.
            entry(
                "weakauras",
                "WeakAuras",
                "WeakAuras Team",
                vec![release(Source::GitHub, "5.19.2", &[11_509])],
            ),
            // Installiert 11.0.0: WoWInterface ist weiter, Tukui nicht.
            entry(
                "dbm",
                "Deadly Boss Mods",
                "MysticalOS",
                vec![
                    release(Source::WowInterface, "11.0.5", &[11_509]),
                    release(Source::Tukui, "11.0.0", &[11_509]),
                ],
            ),
        ])
        .expect("Katalog");

    let updates = index.updates().expect("Updates");
    assert_eq!(updates.len(), 1, "unerwartet: {updates:?}");
    let update = &updates[0];
    assert_eq!(update.addon_name, "Deadly Boss Mods");
    assert_eq!(update.installed_version, "11.0.0");
    assert_eq!(update.available_version, "11.0.5");
    assert_eq!(update.source, Source::WowInterface);
    assert_eq!(update.folder, "DBM-Core");
    assert!(update.download_url.ends_with("11.0.5.zip"));

    // Die Tabelle „Installiert“ traegt denselben Hinweis.
    let installed = index.installed(None).expect("Installiert");
    let dbm = installed
        .iter()
        .find(|r| r.folder == "DBM-Core")
        .expect("DBM");
    assert_eq!(
        dbm.update.as_ref().map(|u| u.version.as_str()),
        Some("11.0.5")
    );
    let weakauras = installed
        .iter()
        .find(|r| r.folder == "WeakAuras")
        .expect("WA");
    assert!(weakauras.update.is_none());

    assert_eq!(index.update_count().expect("Zahl"), 1);
    assert_eq!(index.stats().expect("Zahlen").updates, 1);

    // Filter „nur mit Update“ liefert genau den einen Eintrag.
    let filters = Filters {
        updates_only: true,
        ..Filters::default()
    };
    let hits = index.search(&filters, 50, 0).expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Deadly Boss Mods");

    // Und mit Suchtext zusammen.
    let mut filters = Filters::new("weak");
    filters.updates_only = true;
    assert!(index.search(&filters, 50, 0).expect("Suche").is_empty());
}

#[test]
fn verknuepfung_ueber_externe_kennung() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    // Ordnername und Titel passen zu keinem Katalognamen; nur die
    // WoWInterface-ID ist eindeutig (PLAN §5).
    let client = client_fixture(
        dir.path(),
        Origin::Faugus,
        "faugus:/pfx",
        "_retail_",
        &[(
            "AElectron",
            "## Interface: 11509\n## Version: 2.0\n## Title: Alles Elektro\n## X-WoWI-ID: 7777\n",
        )],
    );
    index.sync_clients(&discovery(vec![client])).expect("Scan");

    let mut katalog = release(Source::WowInterface, "2.1", &[11_509]);
    katalog.external_id = "7777".to_string();
    index
        .sync_catalogue(&[entry(
            "alleselektro",
            "Alles Elektro",
            "Jemand",
            vec![katalog],
        )])
        .expect("Katalog");

    let installed = index.installed(None).expect("Installiert");
    assert_eq!(installed.len(), 1);
    assert!(
        installed[0].addon_id.is_some(),
        "Kennung 7777 muss verknuepfen"
    );
    assert_eq!(installed[0].name, "Alles Elektro");
    assert_eq!(index.updates().expect("Updates").len(), 1);
}

#[test]
fn detail_enthaelt_versionen_und_installationen() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_clients(&discovery(vec![retail_client(dir.path())]))
        .expect("Scan");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let dbm = index
        .search(&Filters::new("deadly"), 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("DBM im Katalog");

    let detail = index.addon(dbm.id).expect("Detail").expect("vorhanden");
    assert_eq!(detail.releases.len(), 1);
    assert_eq!(detail.releases[0].version, "11.0.5");
    assert_eq!(detail.releases[0].interfaces, vec![11_509, 20_506]);
    assert_eq!(detail.installed.len(), 1);
    assert_eq!(detail.installed[0].folder, "DBM-Core");
    assert_eq!(detail.installed[0].client_display(), "Battle.net · Retail");
    assert!(detail.accepts_interface(11_509));
    assert!(!detail.accepts_interface(9_999));
    assert_eq!(detail.sources.len(), 1);
    assert_eq!(detail.sources[0].external_id, "WoWInterface-11.0.5");

    // Addon, das installiert ist: die lokale Fassung steht am Eintrag.
    let wa = index
        .search(&Filters::new("weakauras"), 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("WA");
    let detail = index.addon(wa.id).expect("Detail").expect("vorhanden");
    assert_eq!(detail.installed.len(), 1);
    assert_eq!(detail.installed[0].version.as_deref(), Some("5.19.2"));

    // Und eines, das nirgends liegt.
    index
        .sync_catalogue(&[entry(
            "nichtinstalliert",
            "Nicht Installiert",
            "Jemand",
            vec![release(Source::Local, "1.0", &[])],
        )])
        .expect("Katalog");
    let none = index
        .search(&Filters::new("nicht installiert"), 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("Eintrag");
    let detail = index.addon(none.id).expect("Detail").expect("vorhanden");
    assert!(detail.installed.is_empty());
}

#[test]
fn fingerprint_ueberlebt_den_scan() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_clients(&discovery(vec![retail_client(dir.path())]))
        .expect("Scan");
    let client_id = index.clients().expect("Clients")[0].id.clone();
    index
        .set_fingerprint(&client_id, "DBM-Core", "sha256:abcd")
        .expect("Fingerprint setzen");

    // Der Scan hat die Dateien nicht gehasht und darf den Wert deshalb nicht
    // ueberschreiben.
    index
        .sync_clients(&discovery(vec![retail_client(dir.path())]))
        .expect("Scan");
    let installed = index.installed(None).expect("Installiert");
    let dbm = installed
        .iter()
        .find(|r| r.folder == "DBM-Core")
        .expect("DBM");
    assert_eq!(dbm.fingerprint.as_deref(), Some("sha256:abcd"));

    // Falscher Ordner ist ein Fehler, kein stiller Nichtsnutz.
    assert!(index
        .set_fingerprint(&client_id, "gibt-es-nicht", "x")
        .is_err());
}

#[test]
fn fts_folgt_einer_namensaenderung() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");
    assert_eq!(
        index
            .search(&Filters::new("weakauras"), 10, 0)
            .expect("Suche")
            .len(),
        1
    );

    // Kleine Addons benennen sich um; der Trigger muss den alten Namen
    // entfernen, sonst findet die Suche Leichen.
    index
        .sync_catalogue(&[entry(
            "weakauras",
            "Auras Neu",
            "Frisches Team",
            vec![release(Source::GitHub, "6.0.0", &[11_509])],
        )])
        .expect("Katalog");
    assert!(index
        .search(&Filters::new("weakauras"), 10, 0)
        .expect("Suche")
        .is_empty());
    let hits = index.search(&Filters::new("auras"), 10, 0).expect("Suche");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "Auras Neu");
    // Gleicher Schluessel: der alte Eintrag wurde aktualisiert, nicht doppelt.
    assert_eq!(index.stats().expect("Zahlen").catalogue, 2);
}

#[test]
fn eintrag_ohne_schluessel_wird_gezaehlt_statt_zu_verschwinden() {
    let mut index = Index::open_in_memory().expect("Index");
    let report = index
        .sync_catalogue(&[
            entry("", "", "Niemand", vec![]),
            entry("gueltig", "Gueltig", "Jemand", vec![]),
        ])
        .expect("Katalog");
    assert_eq!(report.skipped, 1);
    assert_eq!(report.catalogue, 1);
    assert_eq!(
        index.search_all(&Filters::default()).expect("alle").len(),
        1
    );
}

#[test]
fn ausgefallene_quelle_leert_den_katalog_nicht() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");
    // Ein Lauf, in dem eine Quelle nichts geliefert hat: der Bestand bleibt
    // stehen (PLAN §5, Ausfallverhalten) und wird nicht als „weg“ gedeutet.
    index.sync_catalogue(&[]).expect("leerer Lauf");
    assert_eq!(index.stats().expect("Zahlen").catalogue, 2);
    assert_eq!(
        index
            .search(&Filters::new("deadly"), 10, 0)
            .expect("Suche")
            .len(),
        1
    );
}

#[test]
fn quellenzahlen_und_meta() {
    let mut index = Index::open_in_memory().expect("Index");
    index.sync_catalogue(&catalogue()).expect("Katalog");
    let counts = index.source_counts().expect("Quellenzahlen");
    assert_eq!(counts.get("github"), Some(&1));
    assert_eq!(counts.get("wowi"), Some(&1));

    index
        .set_meta("last_sync", "2026-10-30T09:00:00.000Z")
        .expect("meta");
    assert_eq!(
        index.meta("last_sync").expect("meta").as_deref(),
        Some("2026-10-30T09:00:00.000Z")
    );
    assert!(index.meta("gibt-es-nicht").expect("meta").is_none());
}

#[test]
fn sync_schreibt_beides_in_einem_lauf() {
    let dir = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    let report = index
        .sync(&discovery(vec![retail_client(dir.path())]), &catalogue())
        .expect("Sync");
    assert_eq!(report.clients, 1);
    assert_eq!(report.installed, 2);
    assert_eq!(report.catalogue, 2);
    assert_eq!(report.skipped, 0);
    assert!(index.meta("last_sync").expect("meta").is_some());

    // Alles, was die Oberflaeche braucht, steht jetzt im Index.
    let stats = index.stats().expect("Zahlen");
    assert_eq!(stats.catalogue, 2);
    assert_eq!(stats.clients, 1);
    assert_eq!(stats.installed, 2);
    assert_eq!(stats.updates, 1);

    // Datei-Index: derselbe Lauf auf Platte, danach neu oeffnen.
    let path: PathBuf = dir.path().join("index.db");
    {
        let mut file_index = Index::open(&path).expect("Datei");
        file_index
            .sync(&discovery(vec![retail_client(dir.path())]), &catalogue())
            .expect("Sync");
    }
    let reopened = Index::open(&path).expect("erneut oeffnen");
    assert_eq!(reopened.stats().expect("Zahlen").catalogue, 2);
    assert_eq!(reopened.updates().expect("Updates").len(), 1);
}

#[test]
fn eine_quelle_darf_die_anzeige_nicht_zurueckdrehen() {
    let mut index = Index::open_in_memory().expect("Index");
    let with = |version: &str| {
        vec![entry(
            "weakauras",
            "WeakAuras",
            "Team",
            vec![release(Source::GitHub, version, &[11_509])],
        )]
    };
    index.sync_catalogue(&with("6.0.0")).expect("Katalog");

    // Ein Lauf, in dem dieselbe Quelle nur ein altes Release liefert
    // (Hiccup, zwischengespeicherte Antwort): die Anzeige bleibt bei 6.0.0,
    // sonst verschwaende ein faelliges Update stillschweigend.
    index.sync_catalogue(&with("5.0.0")).expect("Katalog");

    let hit = index
        .search(&Filters::new("weakauras"), 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("Eintrag");
    assert_eq!(
        hit.latest.as_ref().map(|r| r.version.as_str()),
        Some("6.0.0")
    );
    // Die Version selbst bleibt in der Historie (Tab „Dateien“).
    let detail = index.addon(hit.id).expect("Detail").expect("vorhanden");
    let versions: Vec<&str> = detail.releases.iter().map(|r| r.version.as_str()).collect();
    assert_eq!(versions, vec!["6.0.0", "5.0.0"]);
    // Und ein hoeheres Release kommt weiterhin an.
    index.sync_catalogue(&with("7.1.0")).expect("Katalog");
    let hit = index
        .search(&Filters::new("weakauras"), 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("Eintrag");
    assert_eq!(
        hit.latest.as_ref().map(|r| r.version.as_str()),
        Some("7.1.0")
    );
}

#[test]
fn clear_leert_alles_und_der_index_bleibt_benutzbar() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("index.db");
    let mut index = Index::open(&path).expect("Index");
    index
        .sync(&discovery(vec![retail_client(dir.path())]), &catalogue())
        .expect("Sync");
    index.clear().expect("leeren");

    let stats = index.stats().expect("Zahlen");
    assert_eq!(stats.catalogue, 0);
    assert_eq!(stats.clients, 0);
    assert_eq!(stats.installed, 0);
    assert!(index
        .search_all(&Filters::default())
        .expect("Suche")
        .is_empty());

    // Danach muss derselbe Index wieder Daten aufnehmen.
    index.sync_catalogue(&catalogue()).expect("Katalog");
    assert_eq!(index.stats().expect("Zahlen").catalogue, 2);
    drop(index);

    // Und die Datei muss sich erneut oeffnen lassen (Schemaversion stand noch).
    let reopened = Index::open(&path).expect("erneut oeffnen");
    assert_eq!(reopened.stats().expect("Zahlen").catalogue, 2);
}

#[test]
fn dasselbe_addon_bei_zwei_clients_hat_zwei_hinweise() {
    let dir_a = TempDir::new().expect("tempdir");
    let dir_b = TempDir::new().expect("tempdir");
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_clients(&discovery(vec![
            retail_client(dir_a.path()),
            retail_client(dir_b.path()),
        ]))
        .expect("Scan");
    index.sync_catalogue(&catalogue()).expect("Katalog");

    let installed = index.installed(None).expect("Installiert");
    assert_eq!(installed.len(), 4);
    // Jede Zeile braucht ihren eigenen Hinweis, nicht nur die erste des
    // Addons: die Oberflaeche zeigt die Tabelle je Client.
    for row in &installed {
        if row.folder == "DBM-Core" {
            assert!(row.update.is_some(), "Hinweis fehlt fuer {}", row.client_id);
        } else {
            assert!(row.update.is_none(), "unerwartet fuer {}", row.client_id);
        }
    }
    // Ein Update je Client - Installation ist client-scharf, nie „fuer alle“.
    let updates = index.updates().expect("Updates");
    assert_eq!(updates.len(), 2);
    let clients: std::collections::BTreeSet<&str> =
        updates.iter().map(|u| u.client_id.as_str()).collect();
    assert_eq!(clients.len(), 2);
}

#[test]
fn interface_filter_sieht_nur_die_angebotene_version() {
    let mut index = Index::open_in_memory().expect("Index");
    index
        .sync_catalogue(&[entry(
            "weakauras",
            "WeakAuras",
            "Team",
            vec![
                // Alte Fassung war fuer Classic, die aktuelle ist fuer Retail.
                release(Source::GitHub, "1.0", &[20_506]),
                release(Source::GitHub, "2.0", &[11_509]),
            ],
        )])
        .expect("Katalog");

    let mut filters = Filters {
        interface_number: Some(20_506),
        ..Filters::default()
    };
    assert!(
        index.search(&filters, 10, 0).expect("Suche").is_empty(),
        "ein Classic-Release von 2019 darf im Retail-Filter nicht auftauchen"
    );
    filters.interface_number = Some(11_509);
    assert_eq!(index.search(&filters, 10, 0).expect("Suche").len(), 1);
    // Die Historie bleibt trotzdem vollstaendig (Tab „Dateien“).
    let hit = index
        .search(&filters, 10, 0)
        .expect("Suche")
        .into_iter()
        .next()
        .expect("Eintrag");
    let detail = index.addon(hit.id).expect("Detail").expect("vorhanden");
    assert_eq!(detail.releases.len(), 2);
}
