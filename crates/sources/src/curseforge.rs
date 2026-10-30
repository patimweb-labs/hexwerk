//! CurseForge: `api.curseforge.com/v1`, Header `x-api-key`. Ohne Schlüssel aus.
//!
//! Gemessen: ohne `x-api-key` antwortet der Endpunkt mit 403. Ein Schlüssel wird
//! von Overwolf je Entwickler vergeben und ist nicht übertragbar (PLAN.md §7) —
//! deshalb gibt es hier keinen Vorgabewert und die Quelle meldet ohne Schlüssel
//! [`SourceError::Disabled`], nicht einen Netzfehler.

use std::sync::Arc;

use serde::Deserialize;

use crate::cache::{Cache, TTL_SEARCH};
use crate::http::{Http, Request};
use crate::keys::Secret;
use crate::merge::{id_key, Entry, Release};
use crate::{AddonSource, Source, SourceError, SourceHealth};

const API: &str = "https://api.curseforge.com/v1";
/// `gameId` 1 = World of Warcraft in der CurseForge-API.
const GAME_WOW: u32 = 1;
const PAGE_SIZE: u32 = 25;

pub struct CurseForgeSource {
    http: Arc<dyn Http>,
    cache: Cache,
    key: Option<Secret>,
    /// Optionaler `classId`-Filter (4546 = Addons bei WoW). Vorgabe: keiner,
    /// damit ein falscher Wert nicht stillschweigend alles ausblendet.
    class_id: Option<u32>,
}

impl CurseForgeSource {
    pub fn new(http: Arc<dyn Http>, cache: Cache, key: Option<Secret>) -> Self {
        Self {
            http,
            cache,
            key,
            class_id: None,
        }
    }

    pub fn with_class_id(mut self, class_id: u32) -> Self {
        self.class_id = Some(class_id);
        self
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    fn url(&self, query: &str, page: u32) -> String {
        let mut url = format!(
            "{API}/mods/search?gameId={GAME_WOW}&searchFilter={}&pageSize={PAGE_SIZE}&index={}&sortField=6&sortOrder=desc",
            crate::http::urlencode(query.trim()),
            page * PAGE_SIZE
        );
        if let Some(class_id) = self.class_id {
            url.push_str(&format!("&classId={class_id}"));
        }
        url
    }

    fn authed(&self, request: Request) -> Result<Request, SourceError> {
        let key = self.key.as_ref().ok_or(SourceError::Disabled)?;
        Ok(request
            .header("accept", "application/json")
            .header("x-api-key", key.expose()))
    }
}

impl AddonSource for CurseForgeSource {
    fn id(&self) -> Source {
        Source::CurseForge
    }

    fn health(&self) -> SourceHealth {
        match &self.key {
            // `ok: None` heisst „nicht geprüft"; ein Request in `health()` wäre
            // Netzverkehr aus der UI heraus, den niemand angefordert hat.
            Some(key) => SourceHealth {
                source: Source::CurseForge,
                ok: None,
                detail: format!("Schlüssel gesetzt {}, nicht geprüft", key.masked()),
            },
            None => SourceHealth {
                source: Source::CurseForge,
                ok: Some(false),
                detail: "kein Schlüssel — abgeschaltet (HEXWERK_CURSEFORGE_KEY)".to_string(),
            },
        }
    }

    fn search(&self, query: &str, page: u32) -> Result<Vec<Entry>, SourceError> {
        if self.key.is_none() {
            return Err(SourceError::Disabled);
        }
        let url = self.url(query, page);
        let cache_key = format!("curse:search:{}:{}", query.trim(), page);
        let body = self.cache.cached(&cache_key, TTL_SEARCH, || {
            let request = self.authed(Request::get(&url))?;
            let response = self.http.request(&request)?;
            if response.status == 403 {
                // 403 kann „Schlüssel ungültig" oder „Schlüssel fehlt" heißen;
                // der Klartext der Antwort nennt es, der Schlüssel steht dort nie.
                return Err(SourceError::Http {
                    status: 403,
                    url: API.to_string(),
                });
            }
            if let Some(err) = response.error(API) {
                return Err(err);
            }
            Ok(response.body)
        })?;
        let response: ModSearch =
            serde_json::from_slice(&body).map_err(|e| SourceError::Parse(e.to_string()))?;
        Ok(response.data.into_iter().map(entry_from_mod).collect())
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

#[derive(Debug, Deserialize)]
struct ModSearch {
    #[serde(default)]
    data: Vec<Mod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    categories: Vec<Category>,
    #[serde(default)]
    links: Links,
    #[serde(default)]
    latest_files: Vec<ModFile>,
}

#[derive(Debug, Deserialize)]
struct Author {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Category {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct Links {
    website_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModFile {
    id: u64,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    file_date: Option<String>,
    #[serde(default)]
    file_length: Option<u64>,
    #[serde(default)]
    download_url: Option<String>,
}

fn entry_from_mod(mod_: Mod) -> Entry {
    let author = mod_
        .authors
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_default();
    let homepage = mod_.links.website_url.clone().or_else(|| {
        (!mod_.slug.is_empty())
            .then(|| format!("https://www.curseforge.com/wow/addons/{}", mod_.slug))
    });
    let category = mod_.categories.first().map(|c| c.name.clone());
    let release = mod_.latest_files.first().map(|file| Release {
        source: Source::CurseForge,
        external_id: mod_.id.to_string(),
        version: version_of(file),
        released_at: file.file_date.clone(),
        download_url: download_url_of(file),
        size_bytes: file.file_length,
        // `gameVersions` der API sind Spielversionen („11.0.2"), keine
        // Interface-Nummern; der `.toc` im ZIP liefert die echten.
        interface_numbers: Vec::new(),
    });
    Entry {
        key: id_key(Source::CurseForge, &mod_.id.to_string()),
        name: mod_.name,
        author,
        summary: mod_.summary,
        homepage,
        category,
        releases: release.into_iter().collect(),
    }
}

fn version_of(file: &ModFile) -> String {
    if let Some(display) = &file.display_name {
        if !display.trim().is_empty() {
            return display.trim().to_string();
        }
    }
    file.file_name
        .trim_end_matches(".zip")
        .trim_end_matches(".ZIP")
        .to_string()
}

/// ZIP-Adresse: `downloadUrl`, sonst der CDN-Pfad, den CurseForge aus der
/// Datei-ID bildet (`id/1000` und `id%1000`, drei Stellen).
///
/// WARUM der Rückfall nötig ist: die API liefert `downloadUrl` für Addons oft
/// als `null`, weil die Distribution über den eigenen Launcher läuft.
fn download_url_of(file: &ModFile) -> String {
    if let Some(url) = &file.download_url {
        if !url.trim().is_empty() {
            return url.clone();
        }
    }
    format!(
        "https://edge.forgecdn.net/files/{}/{:03}/{}",
        file.id / 1000,
        file.id % 1000,
        crate::http::urlencode(&file.file_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_wird_zu_eintrag() {
        let value = serde_json::json!({
            "id": 238222,
            "name": "Example Addon",
            "slug": "example-addon",
            "summary": "macht etwas",
            "authors": [{"name": "Autor"}],
            "categories": [{"name": "Buffs"}],
            "links": {"websiteUrl": "https://www.curseforge.com/wow/addons/example-addon"},
            "latestFiles": [{
                "id": 4567890,
                "displayName": "Example 1.2.3",
                "fileName": "Example-1.2.3.zip",
                "fileDate": "2026-10-01T10:00:00Z",
                "fileLength": 12345,
                "gameVersions": ["11.0.2"]
            }]
        });
        let mod_: Mod = serde_json::from_value(value).unwrap();
        let entry = entry_from_mod(mod_);
        assert_eq!(entry.key, "curseforge:238222");
        assert_eq!(entry.author, "Autor");
        assert_eq!(entry.category.as_deref(), Some("Buffs"));
        assert_eq!(entry.releases[0].version, "Example 1.2.3");
        // Ohne `downloadUrl` wird der CDN-Pfad gebaut.
        assert_eq!(
            entry.releases[0].download_url,
            "https://edge.forgecdn.net/files/4567/890/Example-1.2.3.zip"
        );
    }
}
