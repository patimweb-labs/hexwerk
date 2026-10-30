//! WoWInterface: `api.mmoui.com` (ohne Schlüssel).
//!
//! Zwei Endpunkte, zwei Formen — und das ist der Grund für die zwei Parser hier:
//!
//! * `GET /v4/game/WOW/filelist.json` — die Gesamtliste, gemessen 2.744.891 Byte,
//!   7.766 Einträge. Felder in v4: `id`, `title`, `author`, `version`,
//!   `lastUpdate` (Millisekunden), `checksum` (MD5), `fileInfoUri`, `gameVersions`.
//!   Die vom Plan notierten Namen `UID`/`UIName`/… gehören zur v3-Form; ältere
//!   Antworten und der Detail-Endpunkt benutzen sie weiterhin, deshalb liest
//!   [`pick`] beide Schreibweisen.
//! * `GET /v3/game/WOW/filedetails/<id>.json` — je Addon: `UIDownload` (die
//!   echte ZIP-Adresse), `UIFileName`, `UIDescription`, `UIChangeLog`.
//!
//! WARUM die Detailabfrage nicht in der Suche steckt: die Liste hat 7.766
//! Einträge, eine Suche nach „a" liefert hunderte Treffer, und jede Detailabfrage
//! ist eine Anfrage. Deshalb zeigt die Suche die Seite aus der Liste (ohne ZIP),
//! und [`AddonSource::download`] holt die Detailantwort einmal — zwölf Stunden
//! aus dem Cache.

use std::sync::Arc;

use serde_json::Value;

use crate::cache::{Cache, TTL_DETAILS, TTL_LIST};
use crate::http::{Http, Request};
use crate::merge::{id_key, Entry, Release};
use crate::{AddonSource, Source, SourceError, SourceHealth};

const PAGE_SIZE: usize = 25;

/// Ein Eintrag der Gesamtliste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WowiFile {
    pub id: String,
    pub title: String,
    pub author: String,
    pub version: String,
    /// Letzter Upload als ISO-Datum (`YYYY-MM-DD`), damit die Sortierung im
    /// Merge mit GitHub-Datumsangaben vergleichbar bleibt.
    pub updated: Option<String>,
    pub checksum_md5: Option<String>,
    pub info_url: Option<String>,
    pub category: Option<String>,
    pub downloads: Option<u64>,
}

/// Die Detailantwort eines Addons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WowiDetails {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub updated: Option<String>,
    pub checksum_md5: Option<String>,
    pub file_name: Option<String>,
    pub download_url: Option<String>,
    pub description: Option<String>,
    pub changelog: Option<String>,
}

impl WowiFile {
    fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        self.title.to_ascii_lowercase().contains(&needle)
            || self.author.to_ascii_lowercase().contains(&needle)
    }

    fn into_entry(self) -> Entry {
        let release = Release {
            source: Source::WowInterface,
            external_id: self.id.clone(),
            version: self.version.clone(),
            released_at: self.updated.clone(),
            // Platzhalter: die Dateiseite. `download()` ersetzt sie durch die
            // CDN-Adresse aus der Detailantwort.
            download_url: self.info_url.clone().unwrap_or_default(),
            size_bytes: None,
            interface_numbers: Vec::new(),
        };
        Entry {
            screenshots: Vec::new(),
            key: id_key(Source::WowInterface, &self.id),
            name: self.title,
            author: self.author,
            summary: String::new(),
            homepage: self.info_url,
            category: self.category,
            releases: vec![release],
        }
    }
}

pub struct WowInterfaceSource {
    http: Arc<dyn Http>,
    cache: Cache,
}

impl WowInterfaceSource {
    pub fn new(http: Arc<dyn Http>, cache: Cache) -> Self {
        Self { http, cache }
    }

    /// Gesamtliste. Sechs Stunden TTL: 2,7 MB je Anfrage wären sonst teuer.
    pub fn files(&self) -> Result<Vec<WowiFile>, SourceError> {
        let body = self.cache.cached("wowi:filelist", TTL_LIST, || {
            let list_url = format!(
                "{}/v4/game/WOW/filelist.json",
                crate::endpoints::base(Source::WowInterface)
            );
            let response = self.http.request(&Request::get(&list_url))?;
            if let Some(err) = response.error(&list_url) {
                return Err(err);
            }
            Ok(response.body)
        })?;
        parse_file_list(&body)
    }

    /// Detailantwort eines Addons (ZIP-Adresse, Beschreibung, Changelog).
    pub fn details(&self, id: &str) -> Result<WowiDetails, SourceError> {
        let url = format!(
            "{}/v3/game/WOW/filedetails/{id}.json",
            crate::endpoints::base(Source::WowInterface)
        );
        let cache_key = format!("wowi:details:{id}");
        let body = self.cache.cached(&cache_key, TTL_DETAILS, || {
            let response = self.http.request(&Request::get(&url))?;
            if let Some(err) = response.error(&url) {
                return Err(err);
            }
            Ok(response.body)
        })?;
        parse_details(&body)
    }
}

impl AddonSource for WowInterfaceSource {
    fn id(&self) -> Source {
        Source::WowInterface
    }

    fn health(&self) -> SourceHealth {
        SourceHealth {
            source: Source::WowInterface,
            ok: None,
            detail: "bereit, ohne Schlüssel".to_string(),
        }
    }

    fn search(&self, query: &str, page: u32) -> Result<Vec<Entry>, SourceError> {
        let query = query.trim();
        let mut hits: Vec<WowiFile> = self
            .files()?
            .into_iter()
            .filter(|f| query.is_empty() || f.matches(query))
            .collect();
        // Meistgeladen zuerst: bei 7.766 Einträgen ist das die einzige Ordnung,
        // die ohne Ranking-Algorithmus brauchbar ist.
        hits.sort_by(|a, b| {
            b.downloads
                .unwrap_or(0)
                .cmp(&a.downloads.unwrap_or(0))
                .then(a.title.cmp(&b.title))
        });
        Ok(hits
            .into_iter()
            .skip(page as usize * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(WowiFile::into_entry)
            .collect())
    }

    fn download(&self, release: &Release) -> Result<Vec<u8>, SourceError> {
        self.download_with(release, &mut |_, _| {})
    }

    fn download_with(
        &self,
        release: &Release,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>, SourceError> {
        // Die Liste kennt nur die Dateiseite; die CDN-Adresse steht im Detail.
        let url = match self.details(&release.external_id) {
            Ok(details) => details
                .download_url
                .or_else(|| Some(release.download_url.clone()).filter(|u| !u.is_empty()))
                .ok_or(SourceError::NotFound)?,
            Err(err) => {
                // Ohne Detailantwort bleibt die Chance auf eine direkte Adresse.
                if release.download_url.is_empty() {
                    return Err(err);
                }
                release.download_url.clone()
            }
        };
        crate::download_via_http(self.http.as_ref(), &Request::get(&url), &url, progress)
    }
}

/// Liest die Gesamtliste. Tolerant, weil v2/v4 unterschiedliche Feldnamen haben.
pub fn parse_file_list(body: &[u8]) -> Result<Vec<WowiFile>, SourceError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let array = value
        .as_array()
        .ok_or_else(|| SourceError::Parse("erwartet ein JSON-Array".to_string()))?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        let Some(id) = pick(item, &["id", "UID"]) else {
            // Einträge ohne ID sind nicht adressierbar; überspringen statt die
            // ganze Liste scheitern zu lassen.
            continue;
        };
        out.push(WowiFile {
            id,
            title: pick(item, &["title", "UIName"]).unwrap_or_default(),
            author: pick(item, &["author", "UIAuthorName"]).unwrap_or_default(),
            version: pick(item, &["version", "UIVersion"]).unwrap_or_default(),
            updated: pick_number(item, &["lastUpdate", "UIDate"]).and_then(epoch_ms_to_date),
            checksum_md5: pick(item, &["checksum", "UIMD5"]),
            info_url: pick(item, &["fileInfoUri", "UIFileInfoURL", "UIDownload"]),
            category: pick(item, &["UICATID", "categoryId"]),
            downloads: pick_number(item, &["downloads", "UIHitCount"]),
        });
    }
    Ok(out)
}

/// Liest eine Detailantwort (v3 liefert ein Array mit einem Element).
pub fn parse_details(body: &[u8]) -> Result<WowiDetails, SourceError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let item = match &value {
        Value::Array(items) => items
            .first()
            .ok_or_else(|| SourceError::Parse("leere Detailantwort".to_string()))?,
        other => other,
    };
    let Some(id) = pick(item, &["UID", "id"]) else {
        return Err(SourceError::Parse("Detailantwort ohne UID".to_string()));
    };
    Ok(WowiDetails {
        id,
        name: pick(item, &["UIName", "title"]).unwrap_or_default(),
        author: pick(item, &["UIAuthorName", "author"]).unwrap_or_default(),
        version: pick(item, &["UIVersion", "version"]).unwrap_or_default(),
        updated: pick_number(item, &["UIDate", "lastUpdate"]).and_then(epoch_ms_to_date),
        checksum_md5: pick(item, &["UIMD5", "checksum"]),
        file_name: pick(item, &["UIFileName", "fileName"]),
        download_url: pick(item, &["UIDownload", "downloadUrl"]),
        description: pick(item, &["UIDescription", "description"]),
        changelog: pick(item, &["UIChangeLog", "changelog"]),
    })
}

/// Feldname aus einer Liste von Kandidaten, als String. Zahlen werden
/// mitgenommen, weil die IDs in v3 als String und in v4 als Zahl kommen.
fn pick(item: &Value, names: &[&str]) -> Option<String> {
    for name in names {
        match item.get(*name) {
            Some(Value::String(s)) if !s.trim().is_empty() => return Some(s.trim().to_string()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

fn pick_number(item: &Value, names: &[&str]) -> Option<u64> {
    for name in names {
        match item.get(*name) {
            Some(Value::Number(n)) => {
                if let Some(v) = n.as_u64() {
                    return Some(v);
                }
            }
            Some(Value::String(s)) => {
                if let Ok(v) = s.trim().parse::<u64>() {
                    return Some(v);
                }
            }
            _ => {}
        }
    }
    None
}

/// Millisekunden seit Epoche → `YYYY-MM-DD`.
///
/// WARUM von Hand: `chrono` für ein Datumsfeld wäre eine Abhängigkeit zu viel,
/// und die Rechnung ist der bekannte Zivil-Kalender-Algorithmus.
pub fn epoch_ms_to_date(ms: u64) -> Option<String> {
    let secs = i64::try_from(ms / 1000).ok()?;
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Tage seit 1970-01-01 → (Jahr, Monat, Tag). Nach Howard Hinnant.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // 0..=146096
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // 0..=399
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = doy - (153 * mp + 2) / 5 + 1; // 1..=31
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millisekunden_werden_zum_datum() {
        assert_eq!(
            epoch_ms_to_date(1_724_089_877_000).as_deref(),
            Some("2024-08-19")
        );
        assert_eq!(epoch_ms_to_date(0).as_deref(), Some("1970-01-01"));
    }

    #[test]
    fn v4_und_v3_namen_werden_gelesen() {
        let v4 = br#"[{"id":3826,"version":"11.0.2.3","lastUpdate":1724089877000,"title":"CTMod","author":"DahkCeles","fileInfoUri":"https://x/info3826.html","downloads":598613,"checksum":"abc"}]"#;
        let files = parse_file_list(v4).unwrap();
        assert_eq!(files[0].id, "3826");
        assert_eq!(files[0].title, "CTMod");
        assert_eq!(files[0].updated.as_deref(), Some("2024-08-19"));

        let v3 = br#"[{"UID":"3826","UIName":"CTMod","UIAuthorName":"DahkCeles","UIVersion":"1.0","UIDate":1724089877000,"UIMD5":"abc","UIDownload":"https://cdn/x.zip"}]"#;
        let details = parse_details(v3).unwrap();
        assert_eq!(details.id, "3826");
        assert_eq!(details.download_url.as_deref(), Some("https://cdn/x.zip"));
    }

    #[test]
    fn eintraege_ohne_id_werden_uebersprungen() {
        let body = br#"[{"title":"ohne id"},{"id":"7","title":"mit"}]"#;
        let files = parse_file_list(body).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "7");
    }
}
