//! Quellen gegen echte, gekürzte Antworten — ohne Netz.
//!
//! Jede Quelle muss hier beweisen, dass sie aus der gemessenen Form das Richtige
//! liest, dass sie den Cache benutzt und dass sie ohne Schlüssel sauber
//! abschaltet. Ein Test, der ins Netz greift, wäre in der CI unbrauchbar und
//! würde fremde Rate-Limits verbrauchen.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use hexwerk_sources::cache::Cache;
use hexwerk_sources::curseforge::CurseForgeSource;
use hexwerk_sources::fingerprint::HubClient;
use hexwerk_sources::github::GitHubSource;
use hexwerk_sources::http::{Http, HttpResponse, Method, Request};
use hexwerk_sources::keys::Secret;
use hexwerk_sources::merge::Release;
use hexwerk_sources::registry::Registry;
use hexwerk_sources::tukui::TukuiSource;
use hexwerk_sources::wago::WagoSource;
use hexwerk_sources::wowinterface::WowInterfaceSource;
use hexwerk_sources::{AddonSource, Source, SourceError};

// ---------------------------------------------------------------- Testdoppel --

#[derive(Debug, Clone)]
struct Call {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

/// Antwortet mit Fixtures, deren URL mit dem eingetragenen Präfix beginnt.
struct Fake {
    routes: Vec<(String, HttpResponse)>,
    calls: Mutex<Vec<Call>>,
}

impl Fake {
    fn new(routes: Vec<(&str, Vec<u8>)>) -> Self {
        Self {
            routes: routes
                .into_iter()
                .map(|(prefix, body)| {
                    (
                        prefix.to_string(),
                        HttpResponse::new(200, body).with_content_type("application/json"),
                    )
                })
                .collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn with_response(prefix: &str, status: u16, body: Vec<u8>, content_type: &str) -> Self {
        Self {
            routes: vec![(
                prefix.to_string(),
                HttpResponse::new(status, body).with_content_type(content_type),
            )],
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    fn header(&self, index: usize, name: &str) -> Option<String> {
        let calls = self.calls.lock().unwrap();
        calls.get(index).and_then(|call| {
            call.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        })
    }
}

impl Http for Fake {
    fn request(&self, request: &Request) -> Result<HttpResponse, SourceError> {
        self.calls.lock().unwrap().push(Call {
            method: request.method,
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
        });
        for (prefix, response) in &self.routes {
            if request.url.starts_with(prefix.as_str()) {
                return Ok(response.clone());
            }
        }
        // Ein fehlendes Fixture ist im Test ein Netzfehler — genau das, was eine
        // ausgefallene Quelle im Betrieb liefert.
        Err(SourceError::Network(format!(
            "kein Fixture für {}",
            request.url
        )))
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Fixture {} fehlt: {e}", path.display()))
}

fn temp_cache(tag: &str) -> Cache {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("hexwerk-it-{}-{tag}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Cache::new(root)
}

fn fake_http(routes: Vec<(&str, Vec<u8>)>) -> (Arc<Fake>, Arc<dyn Http>) {
    let fake = Arc::new(Fake::new(routes));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    (fake, http)
}

// -------------------------------------------------------------------- Tukui --

#[test]
fn tukui_liest_gesamtliste_und_filtert() {
    let (fake, http) = fake_http(vec![(
        "https://api.tukui.org/v1/addons",
        fixture("tukui_addons.json"),
    )]);
    let source = TukuiSource::new(http, temp_cache("tukui"));

    let entries = source.search("elv", 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "tukui:elvui");
    assert_eq!(entries[0].name, "ElvUI");
    assert_eq!(entries[0].author, "Elv");
    assert_eq!(entries[0].releases.len(), 1);
    assert_eq!(entries[0].releases[0].version, "16.00");
    assert_eq!(
        entries[0].releases[0].released_at.as_deref(),
        Some("2026-10-13")
    );
    assert!(entries[0].releases[0]
        .download_url
        .starts_with("https://api.tukui.org/v1/download/elvui/"));

    // Zweite Suche kommt aus dem Cache: Tukui hätte sonst eine Anfrage je
    // Tastendruck.
    let _ = source.search("tukui", 0).unwrap();
    assert_eq!(fake.call_count(), 1, "Cache muss die zweite Suche tragen");

    // Ohne Suchbegriff kommt die ganze (kleine) Liste.
    assert_eq!(source.search("", 0).unwrap().len(), 2);
    // Seite 1 ist bei zwei Einträgen leer.
    assert!(source.search("", 1).unwrap().is_empty());
}

#[test]
fn tukui_release_ist_downloadbar() {
    let (fake, http) = fake_http(vec![
        (
            "https://api.tukui.org/v1/addons",
            fixture("tukui_addons.json"),
        ),
        (
            "https://api.tukui.org/v1/download/",
            b"PK\x03\x04elvui".to_vec(),
        ),
    ]);
    let source = TukuiSource::new(http, temp_cache("tukui-dl"));
    let entry = source.search("elvui", 0).unwrap().remove(0);
    let bytes = source.download(&entry.releases[0]).unwrap();
    assert!(bytes.starts_with(b"PK"));
    assert_eq!(fake.call_count(), 2);
}

// ------------------------------------------------------------- WoWInterface --

#[test]
fn wowinterface_liest_liste_und_details() {
    let (fake, http) = fake_http(vec![
        (
            "https://api.mmoui.com/v4/game/WOW/filelist.json",
            fixture("wowi_filelist.json"),
        ),
        (
            "https://api.mmoui.com/v3/game/WOW/filedetails/3826.json",
            fixture("wowi_details_3826.json"),
        ),
        (
            "https://cdn.wowinterface.com/downloads/",
            b"PK\x03\x04ctmod".to_vec(),
        ),
    ]);
    let source = WowInterfaceSource::new(http, temp_cache("wowi"));

    let entries = source.search("ctmod", 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "wowinterface:3826");
    assert_eq!(entries[0].name, "CTMod");
    assert_eq!(entries[0].author, "DahkCeles");
    let release = &entries[0].releases[0];
    assert_eq!(release.version, "11.0.2.3");
    // Millisekunden aus der Liste werden zum ISO-Datum.
    assert_eq!(release.released_at.as_deref(), Some("2024-08-19"));

    // Volltextsuche über Titel und Autor.
    assert_eq!(source.search("atlas", 0).unwrap().len(), 1);
    assert!(source.search("gibtsnicht", 0).unwrap().is_empty());

    // Der Download holt erst die Detailantwort und dann das ZIP.
    let bytes = source.download(release).unwrap();
    assert!(bytes.starts_with(b"PK"));
    let calls = fake.calls();
    assert!(calls
        .iter()
        .any(|c| c.url.contains("/filedetails/3826.json")));
    assert!(calls
        .iter()
        .any(|c| c.url.starts_with("https://cdn.wowinterface.com/")));
}

#[test]
fn wowinterface_leere_detailantwort_ist_parsefehler() {
    let (_fake, http) = fake_http(vec![
        (
            "https://api.mmoui.com/v4/game/WOW/filelist.json",
            fixture("wowi_filelist.json"),
        ),
        (
            "https://api.mmoui.com/v3/game/WOW/filedetails/3826.json",
            b"[]".to_vec(),
        ),
    ]);
    let source = WowInterfaceSource::new(http, temp_cache("wowi-empty"));
    let err = source.details("3826").unwrap_err();
    assert!(matches!(err, SourceError::Parse(_)));
}

// ------------------------------------------------------------------- GitHub --

#[test]
fn github_sucht_und_holt_nur_begrenzt_releases() {
    let (fake, http) = fake_http(vec![
        (
            "https://api.github.com/search/repositories",
            fixture("gh_search.json"),
        ),
        (
            "https://api.github.com/repos/WeakAuras/WeakAuras2/releases/latest",
            fixture("gh_release.json"),
        ),
    ]);
    let source = GitHubSource::new(http, temp_cache("gh")).with_release_lookups(1);
    let entries = source.search("weakauras", 0).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "gh:weakauras/weakauras2");
    assert_eq!(entries[0].name, "WeakAuras2");
    assert_eq!(entries[0].author, "WeakAuras");
    let release = &entries[0].releases[0];
    assert_eq!(release.version, "5.22.1");
    assert_eq!(release.external_id, "WeakAuras/WeakAuras2");
    assert!(release.download_url.ends_with("WeakAuras-5.22.1.zip"));
    assert_eq!(release.size_bytes, Some(10_557_265));
    // Der zweite Treffer bleibt ohne Release, damit 60 Anfragen/Stunde reichen.
    assert!(entries[1].releases.is_empty());
    assert_eq!(fake.call_count(), 2);
}

#[test]
fn github_ohne_token_schickt_api_version_und_mit_token_bearer() {
    let (fake, http) = fake_http(vec![(
        "https://api.github.com/search/repositories",
        fixture("gh_search.json"),
    )]);
    let source = GitHubSource::new(http, temp_cache("gh-headers")).with_release_lookups(0);
    source.search("weakauras", 0).unwrap();
    assert_eq!(
        fake.header(0, "x-github-api-version").as_deref(),
        Some("2022-11-28")
    );
    assert!(fake.header(0, "authorization").is_none());

    let (fake, http) = fake_http(vec![(
        "https://api.github.com/search/repositories",
        fixture("gh_search.json"),
    )]);
    let source = GitHubSource::new(http, temp_cache("gh-token"))
        .with_token(Secret::new("ghp_beispiel1234567890"))
        .with_release_lookups(0);
    source.search("weakauras", 0).unwrap();
    assert_eq!(
        fake.header(0, "authorization").as_deref(),
        Some("Bearer ghp_beispiel1234567890")
    );
}

#[test]
fn github_403_mit_rate_limit_wird_ratelimited() {
    let fake = Arc::new(Fake::with_response(
        "https://api.github.com/search/repositories",
        403,
        br#"{"message":"API rate limit exceeded for 1.2.3.4."}"#.to_vec(),
        "application/json",
    ));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    let source = GitHubSource::new(http, temp_cache("gh-ratelimit"));
    let err = source.search("weakauras", 0).unwrap_err();
    assert_eq!(err, SourceError::RateLimited);
}

#[test]
fn github_403_ohne_rate_limit_bleibt_httpfehler() {
    let fake = Arc::new(Fake::with_response(
        "https://api.github.com/search/repositories",
        403,
        br#"{"message":"Forbidden"}"#.to_vec(),
        "application/json",
    ));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    let source = GitHubSource::new(http, temp_cache("gh-forbidden"));
    match source.search("weakauras", 0).unwrap_err() {
        SourceError::Http { status, .. } => assert_eq!(status, 403),
        other => panic!("erwartet Http(403), bekommen {other:?}"),
    }
}

// -------------------------------------------------------------- CurseForge --

#[test]
fn curseforge_ohne_schluessel_ist_abgeschaltet() {
    let (fake, http) = fake_http(vec![]);
    let source = CurseForgeSource::new(http, temp_cache("cf-off"), None);
    assert_eq!(source.search("x", 0).unwrap_err(), SourceError::Disabled);
    assert_eq!(fake.call_count(), 0, "abgeschaltet heißt: kein Netzverkehr");
    let health = source.health();
    assert_eq!(health.ok, Some(false));
    assert!(health.detail.contains("kein Schlüssel"));
}

#[test]
fn curseforge_mit_schluessel_schickt_header_und_liest_antwort() {
    let (fake, http) = fake_http(vec![(
        "https://api.curseforge.com/v1/mods/search",
        fixture("curseforge_search.json"),
    )]);
    let source = CurseForgeSource::new(
        http,
        temp_cache("cf-on"),
        Some(Secret::new("curse-key-abcdef1234")),
    );
    let entries = source.search("tooltip", 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "curseforge:238222");
    assert_eq!(entries[0].author, "Beispielautor");
    assert_eq!(entries[0].releases[0].version, "ItemPriceTooltip 2.4.1");
    assert_eq!(
        entries[0].releases[0].download_url,
        "https://edge.forgecdn.net/files/4567/890/ItemPriceTooltip-2.4.1.zip"
    );
    assert_eq!(
        fake.header(0, "x-api-key").as_deref(),
        Some("curse-key-abcdef1234")
    );
    // Der Zustand zeigt nur die Endung, nie den Schlüssel.
    let health = source.health();
    assert!(health.detail.contains("…1234"));
    assert!(!health.detail.contains("curse-key-abcdef"));
}

#[test]
fn curseforge_erkennt_ungueltigen_schluessel_als_403() {
    let fake = Arc::new(Fake::with_response(
        "https://api.curseforge.com/v1/mods/search",
        403,
        br#"{"error":"invalid api key"}"#.to_vec(),
        "application/json",
    ));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    let source = CurseForgeSource::new(
        http,
        temp_cache("cf-403"),
        Some(Secret::new("falsch-123456")),
    );
    match source.search("x", 0).unwrap_err() {
        SourceError::Http { status, .. } => assert_eq!(status, 403),
        other => panic!("erwartet Http(403), bekommen {other:?}"),
    }
}

// --------------------------------------------------------------------- Wago --

#[test]
fn wago_ohne_token_ist_abgeschaltet() {
    let (fake, http) = fake_http(vec![]);
    let source = WagoSource::new(http, temp_cache("wago-off"), None);
    assert_eq!(source.search("x", 0).unwrap_err(), SourceError::Disabled);
    assert_eq!(fake.call_count(), 0);
    assert_eq!(source.health().ok, Some(false));
}

#[test]
fn wago_mit_token_schickt_bearer_und_liest_antwort() {
    let (fake, http) = fake_http(vec![(
        "https://addons.wago.io/api/addons",
        fixture("wago_search.json"),
    )]);
    let source = WagoSource::new(
        http,
        temp_cache("wago-on"),
        Some(Secret::new("wago-token-abcdef1234")),
    );
    let entries = source.search("example", 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "wago:example-wago-addon");
    assert_eq!(entries[0].releases[0].version, "1.4.0");
    assert_eq!(
        fake.header(0, "authorization").as_deref(),
        Some("Bearer wago-token-abcdef1234")
    );
}

#[test]
fn wago_html_statt_json_wird_als_parsefehler_gemeldet() {
    // Ohne gültigen Token leitet der Host auf die Website um; ureq folgt und
    // liefert HTML mit Status 200.
    let fake = Arc::new(Fake::with_response(
        "https://addons.wago.io/api/addons",
        200,
        b"<!doctype html><html></html>".to_vec(),
        "text/html; charset=utf-8",
    ));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    let source = WagoSource::new(
        http,
        temp_cache("wago-html"),
        Some(Secret::new("abgelaufen-1234567")),
    );
    let err = source.search("x", 0).unwrap_err();
    assert!(matches!(err, SourceError::Parse(_)), "{err:?}");
}

// ----------------------------------------------------------------- Hub --

#[test]
fn hub_fingerprint_abgleich_schickt_hashliste() {
    let fake = Arc::new(Fake::with_response(
        "https://hub.wowup.io/addons/fingerprint",
        200,
        fixture("hub_fingerprint.json"),
        "application/json",
    ));
    let http: Arc<dyn Http> = Arc::clone(&fake) as Arc<dyn Http>;
    let hub = HubClient::new(http);
    let matches = hub.match_many(&["a".repeat(64), "b".repeat(64)]).unwrap();
    assert!(matches.is_empty());
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, Method::Post);
    let body = calls[0].body.clone().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["fingerprints"].as_array().unwrap().len(), 2);
}

#[test]
fn hub_ausfall_blockiert_nicht() {
    let (_fake, http) = fake_http(vec![]);
    let hub = HubClient::new(http);
    // `lookup_or_empty` ist der Pfad für den Betrieb: der Hub hat kein SLA.
    assert!(hub.lookup_or_empty(&["c".repeat(64)]).is_empty());
}

// --------------------------------------------------------------- Registry --

#[test]
fn registry_sammelt_fehler_je_quelle_statt_abzubrechen() {
    let (_fake, http) = fake_http(vec![(
        "https://api.tukui.org/v1/addons",
        fixture("tukui_addons.json"),
    )]);
    let cache = temp_cache("reg");
    let registry = Registry::new(vec![
        Box::new(TukuiSource::new(Arc::clone(&http), cache.clone())),
        // Für WoWInterface gibt es kein Fixture: die Quelle fällt aus.
        Box::new(WowInterfaceSource::new(http, cache)),
    ]);
    let (entries, errors) = registry.search("elvui", 0);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "ElvUI");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, Source::WowInterface);
    assert!(errors[0].1.contains("Netzfehler"), "{}", errors[0].1);
}

#[test]
fn registry_probe_meldet_je_quelle_einen_zustand() {
    let (_fake, http) = fake_http(vec![(
        "https://api.tukui.org/v1/addons",
        fixture("tukui_addons.json"),
    )]);
    let cache = temp_cache("reg-probe");
    let registry = Registry::new(vec![
        Box::new(TukuiSource::new(Arc::clone(&http), cache.clone())),
        // Kein Fixture: die Probe muss den Ausfall melden, nicht panisch werden.
        Box::new(WowInterfaceSource::new(http, cache)),
    ]);
    let report = registry.probe_all();
    // GitHub und Tukui sind entfernt; im Bericht stehen die laufenden Quellen
    // (hier: Tukui als eingesetzte Quelle) plus die drei bekannten ohne
    // Schluessel.
    assert_eq!(report.len(), 4, "laufende plus eingetragene Quellen");
    let by = |s: Source| report.iter().find(|h| h.source == s).unwrap();
    assert_eq!(by(Source::Tukui).ok, Some(true));
    assert!(by(Source::Tukui).detail.contains("HTTP 200"));
    assert_eq!(by(Source::WowInterface).ok, Some(false));
    assert_eq!(by(Source::CurseForge).ok, Some(false));
    assert!(by(Source::CurseForge).detail.contains("kein Schlüssel"));
}

#[test]
fn registry_download_geht_an_die_richtige_quelle() {
    let (_fake, http) = fake_http(vec![(
        "https://api.tukui.org/v1/download/",
        b"PK\x03\x04".to_vec(),
    )]);
    let registry = Registry::new(vec![Box::new(TukuiSource::new(
        Arc::clone(&http),
        temp_cache("reg-dl"),
    ))]);
    let release = Release {
        source: Source::Tukui,
        external_id: "elvui".into(),
        version: "16.00".into(),
        released_at: None,
        download_url: "https://api.tukui.org/v1/download/elvui/token".into(),
        size_bytes: None,
        interface_numbers: vec![],
    };
    assert!(registry.download(&release).unwrap().starts_with(b"PK"));

    // Quelle nicht eingetragen (kein Schlüssel) → Disabled, kein Panik.
    let mut missing = release.clone();
    missing.source = Source::CurseForge;
    assert_eq!(
        registry.download(&missing).unwrap_err(),
        SourceError::Disabled
    );
}
