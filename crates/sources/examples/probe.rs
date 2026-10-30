//! Einmaliger Netztest der Quellen (nicht Teil von `cargo test`).
//!
//! Aufruf: `cargo run -p hexwerk-sources --example probe`.
//! Vorsicht: verbraucht Rate-Limit (GitHub 60/h ohne Token) und lädt bei
//! WoWInterface ein echtes ZIP. Deshalb ein Beispiel und kein Test.

use std::sync::Arc;

use hexwerk_sources::cache::Cache;
use hexwerk_sources::curseforge::CurseForgeSource;
use hexwerk_sources::fingerprint::HubClient;
use hexwerk_sources::github::GitHubSource;
use hexwerk_sources::http::{Http, UreqHttp};
use hexwerk_sources::tukui::TukuiSource;
use hexwerk_sources::wago::WagoSource;
use hexwerk_sources::wowinterface::WowInterfaceSource;
use hexwerk_sources::AddonSource;

fn main() {
    let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
    let cache = Cache::open_default();

    let tukui = TukuiSource::new(Arc::clone(&http), cache.clone());
    match tukui.search("elv", 0) {
        Ok(e) => println!(
            "Tukui: {} Treffer, erster {} {:?}",
            e.len(),
            e.first().map(|x| x.name.clone()).unwrap_or_default(),
            e.first()
                .and_then(|x| x.releases.first().map(|r| r.version.clone()))
        ),
        Err(err) => println!("Tukui: FEHLER {err}"),
    }

    let wowi = WowInterfaceSource::new(Arc::clone(&http), cache.clone());
    match wowi.search("ctmod", 0) {
        Ok(e) => println!(
            "WoWInterface: {} Treffer, erster {} {:?}",
            e.len(),
            e.first().map(|x| x.name.clone()).unwrap_or_default(),
            e.first()
                .and_then(|x| x.releases.first().map(|r| r.released_at.clone()))
        ),
        Err(err) => println!("WoWInterface: FEHLER {err}"),
    }

    if let Ok(entries) = wowi.search("ctmod", 0) {
        if let Some(release) = entries.first().and_then(|e| e.releases.first()) {
            match wowi.download(release) {
                Ok(bytes) => println!(
                    "WoWInterface-Download: {} Byte, ZIP-Kopf {}",
                    bytes.len(),
                    bytes.starts_with(b"PK")
                ),
                Err(err) => println!("WoWInterface-Download: FEHLER {err}"),
            }
        }
    }

    let github = GitHubSource::new(Arc::clone(&http), cache.clone()).with_release_lookups(1);
    match github.search("weakauras", 0) {
        Ok(e) => println!(
            "GitHub: {} Treffer, erster {} {:?}",
            e.len(),
            e.first().map(|x| x.name.clone()).unwrap_or_default(),
            e.first()
                .and_then(|x| x.releases.first().map(|r| r.version.clone()))
        ),
        Err(err) => println!("GitHub: FEHLER {err}"),
    }

    let wago = WagoSource::new(Arc::clone(&http), cache.clone(), None);
    println!("Wago ohne Token: {:?}", wago.search("x", 0));

    let curse = CurseForgeSource::new(Arc::clone(&http), cache.clone(), None);
    println!("CurseForge ohne Schlüssel: {:?}", curse.search("x", 0));

    let hub = HubClient::new(Arc::clone(&http));
    println!(
        "Hub-Abgleich leer: {:?}",
        hub.match_many(&["0".repeat(64)]).map(|m| m.len())
    );
}
