//! Tukui: `api.tukui.org/v1/addons` (ohne Schlüssel).
//!
//! Die Gesamtliste ist winzig (gemessen 3,7 KB, zwei Addons: ElvUI und Tukui),
//! also wird sie einmal geholt, sechs Stunden behalten und im Speicher gefiltert —
//! eine Suche pro Tastendruck ans Netz wäre für zwei Einträge absurd.
//!
//! Download-Adresse: Tukui liefert sie fertig als `url` mit, ein eigener
//! Release-Endpunkt ist nicht nötig.

use std::sync::Arc;

use serde::Deserialize;

use crate::cache::{Cache, TTL_LIST};
use crate::http::{Http, Request};
use crate::merge::{id_key, Entry, Release};
use crate::{AddonSource, Source, SourceError, SourceHealth};

const LIST_URL: &str = "https://api.tukui.org/v1/addons";
const PAGE_SIZE: usize = 25;

/// Ein Eintrag der Gesamtliste, so wie Tukui ihn liefert.
#[derive(Debug, Clone, Deserialize)]
pub struct TukuiAddon {
    /// Projekt-ID; bei ElvUI negativ (gemessen `-2`), deshalb `i64`.
    #[serde(default)]
    pub id: Option<i64>,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    /// Fertige ZIP-Adresse.
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub small_desc: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub git_url: Option<String>,
    /// Datum `YYYY-MM-DD` des letzten Uploads.
    #[serde(default)]
    pub last_update: Option<String>,
}

impl TukuiAddon {
    fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        self.name.to_ascii_lowercase().contains(&needle)
            || self.slug.to_ascii_lowercase().contains(&needle)
            || self.author.to_ascii_lowercase().contains(&needle)
            || self.small_desc.to_ascii_lowercase().contains(&needle)
    }

    fn into_entry(self) -> Entry {
        let key = id_key(Source::Tukui, &self.slug);
        let homepage = self.web_url.clone().or_else(|| {
            (!self.slug.is_empty()).then(|| format!("https://tukui.org/{}", self.slug))
        });
        let release = Release {
            source: Source::Tukui,
            external_id: self.slug.clone(),
            version: self.version.clone(),
            released_at: self.last_update.clone(),
            download_url: self.url.clone(),
            size_bytes: None,
            interface_numbers: Vec::new(),
        };
        Entry {
            key,
            name: self.name,
            author: self.author,
            summary: self.small_desc,
            homepage,
            category: None,
            releases: if release.download_url.is_empty() {
                Vec::new()
            } else {
                vec![release]
            },
        }
    }
}

pub struct TukuiSource {
    http: Arc<dyn Http>,
    cache: Cache,
}

impl TukuiSource {
    pub fn new(http: Arc<dyn Http>, cache: Cache) -> Self {
        Self { http, cache }
    }

    /// Gesamtliste, aus dem Cache oder frisch.
    pub fn addons(&self) -> Result<Vec<TukuiAddon>, SourceError> {
        let body = self.cache.cached("tukui:addons", TTL_LIST, || {
            let response = self.http.request(&Request::get(LIST_URL))?;
            if let Some(err) = response.error(LIST_URL) {
                return Err(err);
            }
            Ok(response.body)
        })?;
        serde_json::from_slice(&body).map_err(|e| SourceError::Parse(e.to_string()))
    }

    /// Ein Addon anhand seines Slugs (für die Detailansicht).
    pub fn by_slug(&self, slug: &str) -> Result<Option<Entry>, SourceError> {
        let slug = slug.to_ascii_lowercase();
        Ok(self
            .addons()?
            .into_iter()
            .find(|a| a.slug.to_ascii_lowercase() == slug)
            .map(TukuiAddon::into_entry))
    }
}

impl AddonSource for TukuiSource {
    fn id(&self) -> Source {
        Source::Tukui
    }

    fn health(&self) -> SourceHealth {
        SourceHealth {
            source: Source::Tukui,
            ok: None,
            detail: "bereit, ohne Schlüssel".to_string(),
        }
    }

    fn search(&self, query: &str, page: u32) -> Result<Vec<Entry>, SourceError> {
        let query = query.trim();
        let hits: Vec<TukuiAddon> = self
            .addons()?
            .into_iter()
            .filter(|a| query.is_empty() || a.matches(query))
            .collect();
        Ok(hits
            .into_iter()
            .skip(page as usize * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(TukuiAddon::into_entry)
            .collect())
    }

    fn download(&self, release: &Release) -> Result<Vec<u8>, SourceError> {
        if release.download_url.is_empty() {
            return Err(SourceError::NotFound);
        }
        let response = self.http.request(&Request::get(&release.download_url))?;
        if let Some(err) = response.error(&release.download_url) {
            return Err(err);
        }
        Ok(response.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suche_ignoriert_grossschreibung() {
        let addon = TukuiAddon {
            id: Some(-2),
            slug: "elvui".into(),
            name: "ElvUI".into(),
            author: "Elv".into(),
            version: "16.00".into(),
            url: "https://api.tukui.org/v1/download/elvui/x".into(),
            small_desc: "A user interface replacement".into(),
            web_url: Some("https://tukui.org/elvui".into()),
            git_url: None,
            last_update: Some("2026-10-13".into()),
        };
        assert!(addon.matches("ELV"));
        assert!(addon.matches("interface"));
        assert!(!addon.matches("detail"));
        let entry = addon.into_entry();
        assert_eq!(entry.key, "tukui:elvui");
        assert_eq!(entry.releases.len(), 1);
        assert_eq!(entry.releases[0].version, "16.00");
    }
}
