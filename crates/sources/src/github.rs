//! GitHub: Repository-Suche und Releases über die REST-API.
//!
//! Hauptquelle laut PLAN.md §5, weil sie ohne Schlüssel läuft und die meisten
//! Addons ihr Release dort als ZIP bereitstellen.
//!
//! Grenze, die den Aufbau erklärt: unauthentifiziert erlaubt GitHub 60 Anfragen
//! pro Stunde. Eine Suche ist eine Anfrage, aber jede Suche will auch das letzte
//! Release sehen — das wären dreißig. Deshalb holt [`GitHubSource`] Releases nur
//! für die ersten [`GitHubSource::with_release_lookups`] Treffer und legt jede
//! Antwort sechs Stunden in den [`Cache`]. Treffer ohne geholtes Release stehen
//! trotzdem im Katalog, nur ohne Download-Adresse.

use std::sync::Arc;

use serde::Deserialize;

use crate::cache::{Cache, TTL_DETAILS, TTL_SEARCH};
use crate::http::{Http, Request};
use crate::keys::Secret;
use crate::merge::{Entry, Release};
use crate::{AddonSource, Source, SourceError, SourceHealth};

const API: &str = "https://api.github.com";
const PER_PAGE: u32 = 30;

pub struct GitHubSource {
    http: Arc<dyn Http>,
    cache: Cache,
    /// Optionaler Token (kein eigener Schlüsselbegriff im Plan, aber GitHub
    /// erlaubt 5000/h statt 60/h damit). Wird nie geloggt.
    token: Option<Secret>,
    release_lookups: usize,
    /// Zusatz zur Suchanfrage. Vorgabe: nur Name und Beschreibung durchsuchen,
    /// sonst liefert GitHub README-Treffer für jedes Wort.
    query_suffix: String,
}

impl GitHubSource {
    pub fn new(http: Arc<dyn Http>, cache: Cache) -> Self {
        Self {
            http,
            cache,
            token: None,
            release_lookups: 5,
            query_suffix: " in:name,description".to_string(),
        }
    }

    pub fn with_token(mut self, token: Secret) -> Self {
        self.token = Some(token);
        self
    }

    pub fn with_release_lookups(mut self, count: usize) -> Self {
        self.release_lookups = count;
        self
    }

    pub fn with_query_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.query_suffix = suffix.into();
        self
    }

    /// Letztes Release eines Repos (`owner/name`). Öffentlich, weil die
    /// Detailansicht genau eine Abfrage braucht und nicht die ganze Suche.
    pub fn latest_release(&self, full_name: &str) -> Result<Option<Release>, SourceError> {
        let url = format!("{API}/repos/{full_name}/releases/latest");
        let cache_key = format!("github:latest:{full_name}");
        let body = self.cache.cached(&cache_key, TTL_DETAILS, || {
            let request = self.authorized(Request::get(&url));
            self.get_json(request, &url)
        })?;
        let value: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| SourceError::Parse(e.to_string()))?;
        Ok(release_from_value(&value, full_name))
    }

    fn search_json(&self, query: &str, page: u32) -> Result<serde_json::Value, SourceError> {
        let q = format!("{}{}", query.trim(), self.query_suffix);
        let url = format!(
            "{API}/search/repositories?q={}&sort=stars&order=desc&per_page={PER_PAGE}&page={}",
            crate::http::urlencode(&q),
            page + 1
        );
        let cache_key = format!("github:search:{q}:{page}");
        let body = self.cache.cached(&cache_key, TTL_SEARCH, || {
            let request = self.authorized(Request::get(&url));
            self.get_json(request, &url)
        })?;
        serde_json::from_slice(&body).map_err(|e| SourceError::Parse(e.to_string()))
    }

    fn authorized(&self, request: Request) -> Request {
        match &self.token {
            Some(token) => request
                .bearer(token.expose())
                .header("accept", "application/vnd.github+json"),
            None => request
                .header("accept", "application/vnd.github+json")
                // Ohne Pin gilt die aktuelle API-Version; GitHub ändert
                // Verhalten sonst irgendwann stillschweigend.
                .header("x-github-api-version", "2022-11-28"),
        }
    }

    fn get_json(&self, request: Request, url: &str) -> Result<Vec<u8>, SourceError> {
        let response = self.http.request(&request)?;
        // 403 ist bei GitHub fast immer das Stundenlimit, nicht eine Sperre.
        // Die Unterscheidung entscheidet, ob die UI „später nochmal" sagt.
        if response.status == 403 || response.status == 429 {
            let text = response.text().to_ascii_lowercase();
            if text.contains("rate limit") || text.contains("abuse") {
                return Err(SourceError::RateLimited);
            }
        }
        if let Some(err) = response.error(url) {
            return Err(err);
        }
        Ok(response.body)
    }
}

impl AddonSource for GitHubSource {
    fn id(&self) -> Source {
        Source::GitHub
    }

    /// GitHub lehnt eine leere Suche mit HTTP 422 ab (`q= in:name`). Fuer die
    /// Gesundheitspruefung wird deshalb ein fester Begriff gesucht — das ist
    /// eine echte Anfrage gegen dieselbe Schnittstelle wie im Betrieb.
    fn probe(&self) -> SourceHealth {
        let source = self.id();
        match self.search("weakauras", 0) {
            Ok(entries) => SourceHealth {
                source,
                ok: Some(true),
                detail: format!("HTTP 200, {} Treffer", entries.len()),
            },
            Err(err) => SourceHealth {
                source,
                ok: Some(false),
                detail: err.to_string(),
            },
        }
    }

    fn health(&self) -> SourceHealth {
        SourceHealth {
            source: Source::GitHub,
            ok: None,
            detail: match &self.token {
                Some(token) => format!("bereit, Token {}", token.masked()),
                None => "bereit, ohne Token (60 Anfragen/Stunde)".to_string(),
            },
        }
    }

    fn search(&self, query: &str, page: u32) -> Result<Vec<Entry>, SourceError> {
        let value = self.search_json(query, page)?;
        let repos: Vec<Repo> = serde_json::from_value(value)
            .map(|r: RepoSearch| r.items)
            .map_err(|e| SourceError::Parse(e.to_string()))?;
        let mut entries = Vec::with_capacity(repos.len());
        for (index, repo) in repos.into_iter().enumerate() {
            // Ein fehlendes Release ist kein Fehler: das Repo ist trotzdem ein
            // Katalogeintrag, nur ohne ZIP. Deshalb `ok().flatten()`.
            let release = if index < self.release_lookups {
                self.latest_release(&repo.full_name).ok().flatten()
            } else {
                None
            };
            entries.push(entry_from_repo(repo, release));
        }
        Ok(entries)
    }

    fn download(&self, release: &Release) -> Result<Vec<u8>, SourceError> {
        let request = self
            .authorized(Request::get(&release.download_url))
            .header("accept", "application/octet-stream");
        let response = self.http.request(&request)?;
        if let Some(err) = response.error(&release.download_url) {
            return Err(err);
        }
        Ok(response.body)
    }
}

#[derive(Debug, Deserialize)]
struct RepoSearch {
    #[serde(default)]
    items: Vec<Repo>,
}

#[derive(Debug, Deserialize)]
struct Repo {
    name: String,
    full_name: String,
    owner: Owner,
    #[serde(default)]
    description: Option<String>,
    html_url: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    archived: bool,
}

#[derive(Debug, Deserialize)]
struct Owner {
    login: String,
}

fn entry_from_repo(repo: Repo, release: Option<Release>) -> Entry {
    let mut summary = repo.description.unwrap_or_default().trim().to_string();
    if repo.archived {
        // Ehrlich in der Anzeige: ein archiviertes Repo bekommt keine Updates,
        // und das ist für die Auswahl wichtiger als Sterne.
        summary = if summary.is_empty() {
            "archiviert (keine Updates mehr)".to_string()
        } else {
            format!("{summary} (archiviert)")
        };
    }
    Entry {
        // Name des Repos statt Name des Addons: das `.toc` im ZIP sagt erst beim
        // Herunterladen, wie das Addon wirklich heißt (PLAN.md §13.4).
        key: format!("gh:{}", repo.full_name.to_lowercase()),
        name: repo.name,
        author: repo.owner.login,
        summary,
        homepage: Some(repo.html_url),
        category: repo.topics.first().cloned(),
        releases: release.into_iter().collect(),
    }
}

/// Ein Release aus der GitHub-Antwort. Öffentlich für Tests und für Aufrufer,
/// die eine Antwort schon haben.
pub fn release_from_value(value: &serde_json::Value, full_name: &str) -> Option<Release> {
    let parsed: GhRelease = serde_json::from_value(value.clone()).ok()?;
    let tag = parsed.tag_name.trim();
    if tag.is_empty() {
        return None;
    }
    // ZIP-Assets bevorzugen: das `zipball` eines Repos enthält den ganzen Baum
    // inklusive Quellcode und wird von WoW nur mit korrigiertem Ordner geladen.
    let asset = parsed
        .assets
        .iter()
        .find(|a| a.name.to_ascii_lowercase().ends_with(".zip"));
    let (download_url, size_bytes) = match asset {
        Some(asset) => (asset.browser_download_url.clone(), asset.size),
        None => (parsed.zipball_url.clone()?, None),
    };
    Some(Release {
        source: Source::GitHub,
        external_id: full_name.to_string(),
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        released_at: parsed.published_at.clone(),
        download_url,
        size_bytes,
        // Interface-Nummern stehen im `.toc` im ZIP, nicht in der API.
        interface_numbers: Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
    #[serde(default)]
    zipball_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_bevorzugt_zip_asset() {
        let value = serde_json::json!({
            "tag_name": "v5.22.1",
            "published_at": "2026-10-01T10:00:00Z",
            "zipball_url": "https://api.github.com/repos/x/y/zipball/v5.22.1",
            "assets": [
                {"name": "release.json", "browser_download_url": "https://x/release.json", "size": 300},
                {"name": "WeakAuras-5.22.1.zip", "browser_download_url": "https://x/wa.zip", "size": 10557265}
            ]
        });
        let release = release_from_value(&value, "WeakAuras/WeakAuras2").unwrap();
        assert_eq!(release.version, "5.22.1");
        assert_eq!(release.download_url, "https://x/wa.zip");
        assert_eq!(release.size_bytes, Some(10557265));
    }

    #[test]
    fn ohne_zip_asset_wird_zipball_genommen() {
        let value = serde_json::json!({
            "tag_name": "1.0.0",
            "zipball_url": "https://api.github.com/repos/x/y/zipball/1.0.0",
            "assets": []
        });
        let release = release_from_value(&value, "x/y").unwrap();
        assert!(release.download_url.ends_with("/zipball/1.0.0"));
        assert_eq!(release.size_bytes, None);
    }
}
