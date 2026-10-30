//! Fingerprint eines Addon-Ordners (Hash über sortierte Datei-Hashes) und Abgleich.
//!
//! Zweck (PLAN.md §5): der WowUp-Hub ist eine Abkürzung, um ein **installiertes**
//! Addon ohne `.toc`-Kennung einer Quelle zuzuordnen. Er ist ausdrücklich keine
//! Voraussetzung — schlägt der Hub fehl, bleibt die Zuordnung über
//! `X-WoWI-ID`/`X-Curse-Project-ID` aus dem `.toc`.
//!
//! Der Fingerprint selbst muss auch ohne Hub stimmen, weil er im Index steht und
//! einen Installationsstand wiedererkennbar macht (PLAN.md §6, `installed`).
//! Deshalb: deterministisch, unabhängig von Zeitstempeln und Verzeichnisreihenfolge.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::http::{Http, Request};
use crate::Source;
use crate::SourceError;

/// Endpunkt des WowUp-Hubs. Gemessen: `POST` mit `{"fingerprints":[…]}` → 200
/// und `{"exactMatches":[]}` bei unbekanntem Hash. Ohne SLA.
pub const HUB_URL: &str = "https://hub.wowup.io/addons/fingerprint";

/// Ein Treffer des Hubs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubMatch {
    pub fingerprint: String,
    /// Ordnername im `Interface/AddOns`-Verzeichnis, falls der Hub ihn nennt.
    pub folder_name: Option<String>,
    /// Quelle, sofern der Hub eine nennt, die wir kennen.
    pub source: Option<Source>,
    /// Projekt-ID bei der Quelle (Curse-`id`, Wago-Slug, …).
    pub external_id: String,
    pub addon_name: Option<String>,
}

/// Hash eines Addon-Ordners: SHA-256 über `pfad\0dateihash\n` aller Dateien,
/// sortiert nach Pfad.
///
/// WARUM der Pfad mit in den Hash geht: sonst hätten zwei Addons mit gleicher
/// Datei (etwa eine gemeinsame `LibStub`) denselben Fingerprint. WARUM sortiert:
/// `read_dir` liefert eine beliebige Reihenfolge, der Hash wäre sonst nicht
/// reproduzierbar.
pub fn folder_fingerprint(dir: &Path) -> std::io::Result<String> {
    let mut files = BTreeMap::new();
    collect_files(dir, "", &mut files)?;
    let mut hasher = Sha256::new();
    for (relative, hash) in &files {
        hasher.update(relative.as_bytes());
        hasher.update([0u8]);
        hasher.update(hash.as_bytes());
        hasher.update(b"\n");
    }
    Ok(hex(&hasher.finalize()))
}

/// SHA-256 einer einzelnen Datei.
pub fn file_fingerprint(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex(&hasher.finalize()))
}

fn collect_files(
    dir: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, String>,
) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };
        // Versteckte Einträge (`.git`, `.DS_Store`, Editor-Tauschdateien) sind
        // kein Addon-Inhalt; sie würden den Hash je nach Arbeitskopie ändern.
        if name.starts_with('.') {
            continue;
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            // Symlinks nicht verfolgen: Zyklen und Verweise aus dem Addon-Ordner
            // heraus würden den Hash fremder Verzeichnisse einbeziehen.
            continue;
        }
        if meta.is_dir() {
            collect_files(&path, &relative, out)?;
        } else if meta.is_file() {
            out.insert(relative, file_fingerprint(&path)?);
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Abgleich beim WowUp-Hub.
pub struct HubClient {
    http: Arc<dyn Http>,
}

impl HubClient {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http }
    }

    /// Fragt den Hub nach einer Liste von Fingerprints.
    ///
    /// Gibt `Err` weiter, damit der Aufrufer entscheiden kann. Für den reinen
    /// Beschleuniger-Pfad gibt es [`HubClient::lookup_or_empty`].
    pub fn match_many(&self, fingerprints: &[String]) -> Result<Vec<HubMatch>, SourceError> {
        if fingerprints.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fingerprints": fingerprints });
        let request = Request::post(HUB_URL)
            .header("accept", "application/json")
            .json_body(&body);
        let response = self.http.request(&request)?;
        if let Some(err) = response.error(HUB_URL) {
            return Err(err);
        }
        let value = response.json(HUB_URL)?;
        Ok(parse_matches(&value))
    }

    /// Wie [`HubClient::match_many`], aber ohne Fehler: der Hub ist laut Plan
    /// „offen, kein SLA" und darf die Installation nie blockieren.
    pub fn lookup_or_empty(&self, fingerprints: &[String]) -> Vec<HubMatch> {
        self.match_many(fingerprints).unwrap_or_default()
    }
}

/// Liest `{"exactMatches":[ … ]}`. Form nach der Hub-Dokumentation; gemessen ist
/// nur die leere Antwort, deshalb bleibt der Parser bei fehlenden Feldern ruhig.
pub fn parse_matches(value: &serde_json::Value) -> Vec<HubMatch> {
    let Some(items) = value.get("exactMatches").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let fingerprint = item
                .get("fingerprint")
                .and_then(|v| v.as_str())?
                .to_string();
            let addon = item.get("addon");
            let slug = addon
                .and_then(|a| a.get("slug"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let id = addon
                .and_then(|a| a.get("id"))
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .filter(|s| !s.is_empty())
                .unwrap_or(slug);
            Some(HubMatch {
                fingerprint,
                folder_name: item
                    .get("folderName")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                source: addon
                    .and_then(|a| a.get("source"))
                    .and_then(|v| v.as_str())
                    .and_then(source_from_hub),
                external_id: id,
                addon_name: addon
                    .and_then(|a| a.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect()
}

fn source_from_hub(name: &str) -> Option<Source> {
    match name.to_ascii_lowercase().as_str() {
        "curseforge" | "curse" => Some(Source::CurseForge),
        "wago" => Some(Source::Wago),
        "wowinterface" | "mmoui" => Some(Source::WowInterface),
        "tukui" => Some(Source::Tukui),
        "github" => Some(Source::GitHub),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_addon(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hexwerk-fp-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("Addon.toc"), "## Interface: 110002\n").unwrap();
        fs::write(dir.join("sub/core.lua"), "print('hi')\n").unwrap();
        dir
    }

    #[test]
    fn fingerprint_ist_reproduzierbar_und_inhaltsabhaengig() {
        let dir = temp_addon("stable");
        let a = folder_fingerprint(&dir).unwrap();
        let b = folder_fingerprint(&dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        fs::write(dir.join("sub/core.lua"), "print('anders')\n").unwrap();
        let c = folder_fingerprint(&dir).unwrap();
        assert_ne!(a, c, "anderer Inhalt muss anderen Hash geben");
    }

    #[test]
    fn versteckte_dateien_zaehlen_nicht() {
        let dir = temp_addon("hidden");
        let before = folder_fingerprint(&dir).unwrap();
        fs::write(dir.join(".git"), "irgendwas").unwrap();
        assert_eq!(before, folder_fingerprint(&dir).unwrap());
    }

    #[test]
    fn leere_hub_antwort_ergibt_keine_treffer() {
        assert!(parse_matches(&serde_json::json!({"exactMatches": []})).is_empty());
    }

    #[test]
    fn hub_treffer_werden_gelesen() {
        let value = serde_json::json!({
            "exactMatches": [{
                "fingerprint": "abc",
                "folderName": "WeakAuras",
                "addon": {"id": 12345, "slug": "weakauras", "name": "WeakAuras", "source": "curseforge"},
                "release": {"id": 1}
            }]
        });
        let matches = parse_matches(&value);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].source, Some(Source::CurseForge));
        assert_eq!(matches[0].external_id, "12345");
        assert_eq!(matches[0].folder_name.as_deref(), Some("WeakAuras"));
    }
}
