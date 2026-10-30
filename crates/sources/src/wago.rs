//! Wago: `addons.wago.io`, Token im Header. Ohne Token aus.
//!
//! Gemessen: ohne Token antwortet der Host mit 302 auf die Website. Deshalb
//! prüft die Quelle den Token **vor** der Anfrage und meldet
//! [`SourceError::Disabled`] — ein Redirect auf eine HTML-Seite ist kein
//! brauchbarer Fehler für die UI.
//!
//! Die Antwortform mit Token ist hier nicht gemessen (kein Token vorhanden).
//! Gelesen wird deshalb tolerant über [`serde_json::Value`] mit mehreren
//! bekannten Schreibweisen; was fehlt, wird zu `None` statt zum Absturz.

use std::sync::Arc;

use serde_json::Value;

use crate::cache::{Cache, TTL_SEARCH};
use crate::http::{Http, Request};
use crate::keys::Secret;
use crate::merge::{id_key, Entry, Release};
use crate::{AddonSource, Source, SourceError, SourceHealth};

const API: &str = "https://addons.wago.io/api";
const PAGE_SIZE: u32 = 25;

pub struct WagoSource {
    http: Arc<dyn Http>,
    cache: Cache,
    token: Option<Secret>,
}

impl WagoSource {
    pub fn new(http: Arc<dyn Http>, cache: Cache, token: Option<Secret>) -> Self {
        Self { http, cache, token }
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn authed(&self, request: Request) -> Result<Request, SourceError> {
        let token = self.token.as_ref().ok_or(SourceError::Disabled)?;
        Ok(request
            .bearer(token.expose())
            .header("accept", "application/json"))
    }
}

impl AddonSource for WagoSource {
    fn id(&self) -> Source {
        Source::Wago
    }

    fn health(&self) -> SourceHealth {
        match &self.token {
            Some(token) => SourceHealth {
                source: Source::Wago,
                ok: None,
                detail: format!("Token gesetzt {}, nicht geprüft", token.masked()),
            },
            None => SourceHealth {
                source: Source::Wago,
                ok: Some(false),
                detail: "kein Token — abgeschaltet (HEXWERK_WAGO_TOKEN)".to_string(),
            },
        }
    }

    fn search(&self, query: &str, page: u32) -> Result<Vec<Entry>, SourceError> {
        if self.token.is_none() {
            return Err(SourceError::Disabled);
        }
        let url = format!(
            "{API}/addons?search={}&page={}&limit={PAGE_SIZE}",
            crate::http::urlencode(query.trim()),
            page + 1
        );
        let cache_key = format!("wago:search:{}:{}", query.trim(), page);
        let body = self.cache.cached(&cache_key, TTL_SEARCH, || {
            let request = self.authed(Request::get(&url))?;
            let response = self.http.request(&request)?;
            // 401/403 = Token ungültig. Der Klartext bleibt beim Code, weil
            // Wago in der Antwort nichts Vertrauliches mitschickt.
            if let Some(err) = response.error(API) {
                return Err(err);
            }
            Ok(response.body)
        })?;
        let value: Value =
            serde_json::from_slice(&body).map_err(|e| SourceError::Parse(e.to_string()))?;
        Ok(entries_from_value(&value))
    }

    fn download(&self, release: &Release) -> Result<Vec<u8>, SourceError> {
        if release.download_url.is_empty() {
            return Err(SourceError::NotFound);
        }
        let request = self.authed(Request::get(&release.download_url))?;
        let response = self.http.request(&request)?;
        if let Some(err) = response.error(&release.download_url) {
            return Err(err);
        }
        Ok(response.body)
    }
}

/// Liest Array oder `{ "data": [ … ] }` und überspringt alles Unbrauchbare.
fn entries_from_value(value: &Value) -> Vec<Entry> {
    let items = value
        .as_array()
        .cloned()
        .or_else(|| value.get("data").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    items.iter().filter_map(entry_from_item).collect()
}

fn entry_from_item(item: &Value) -> Option<Entry> {
    let addon = item.get("addon").unwrap_or(item);
    let slug = text(addon, &["slug", "id"])?;
    let name = text(addon, &["name", "title"]).unwrap_or_else(|| slug.clone());
    let author = text(addon, &["author", "authorName"])
        .or_else(|| {
            addon
                .get("author")
                .and_then(|a| a.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    let release = addon
        .get("latestRelease")
        .or_else(|| addon.get("latest_release"))
        .and_then(|r| release_from_value(r, &slug));
    Some(Entry {
        key: id_key(Source::Wago, &slug),
        name,
        author,
        summary: text(addon, &["summary", "description"]).unwrap_or_default(),
        homepage: Some(format!("https://addons.wago.io/addons/{slug}")),
        category: text(addon, &["category", "categoryName"]),
        releases: release.into_iter().collect(),
    })
}

fn release_from_value(value: &Value, slug: &str) -> Option<Release> {
    let version = text(value, &["label", "version", "name"])?;
    let url = text(
        value,
        &["downloadUri", "download_uri", "downloadUrl", "url"],
    )
    .unwrap_or_default();
    Some(Release {
        source: Source::Wago,
        external_id: slug.to_string(),
        version,
        released_at: text(value, &["createdAt", "created_at", "date"]),
        download_url: url,
        size_bytes: value.get("size").and_then(Value::as_u64),
        interface_numbers: Vec::new(),
    })
}

fn text(value: &Value, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(s) = value.get(*name).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liest_array_und_data_objekt() {
        let item = serde_json::json!({
            "slug": "example",
            "name": "Example",
            "author": "Autor",
            "summary": "macht etwas",
            "latestRelease": {"label": "1.2.3", "createdAt": "2026-10-01T10:00:00Z", "downloadUri": "https://x/y.zip"}
        });
        let array = serde_json::json!([item.clone()]);
        let wrapped = serde_json::json!({"data": [item]});
        for value in [array, wrapped] {
            let entries = entries_from_value(&value);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].key, "wago:example");
            assert_eq!(entries[0].releases[0].version, "1.2.3");
            assert_eq!(entries[0].releases[0].download_url, "https://x/y.zip");
        }
    }

    #[test]
    fn ohne_slug_faellt_der_eintrag_weg() {
        assert!(entries_from_value(&serde_json::json!([{"name": "ohne slug"}])).is_empty());
    }
}
