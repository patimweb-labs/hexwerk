//! `.toc`-Dateien lesen: das Manifest eines Addons.
//!
//! Aufbau: Zeilen `## Schluessel: Wert`, danach optional eine Liste von
//! Dateien. Mehrsprachige Felder tragen das Sprachkuerzel im Schluessel
//! (`## Notes-deDE`). `## Interface` kann eine Komma-Liste sein.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Externe Kennungen, mit denen sich ein installiertes Addon ohne Raten einer
/// Quelle zuordnen laesst. WoWUp nutzt dieselben Felder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TocSourceIds {
    pub wowinterface: Option<String>,
    pub tukui: Option<String>,
    pub curseforge: Option<String>,
    pub wago: Option<String>,
    pub website: Option<String>,
    pub github: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Toc {
    pub path: Option<PathBuf>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub interface_numbers: Vec<u32>,
    pub source_ids: TocSourceIds,
    pub saved_variables: Vec<String>,
    /// Alle `## X-…`-Felder, unveraendert.
    pub extra: BTreeMap<String, String>,
    /// Beschreibungen je Sprache (`## Notes`, `## Notes-deDE`, …), Schluessel
    /// klein geschrieben; `""` ist die Vorgabesprache.
    pub notes: BTreeMap<String, String>,
}

/// Liest eine `.toc` aus dem Speicher.
pub fn parse_str(text: &str) -> Toc {
    let mut toc = Toc::default();
    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        let Some(rest) = line.strip_prefix("## ") else {
            continue;
        };
        let Some((key, value)) = rest.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().to_string();
        let lower = key.to_ascii_lowercase();
        match lower.as_str() {
            "title" => toc.title = Some(value),
            "version" => toc.version = Some(value),
            "author" => toc.author = Some(value),
            "category" | "x-category" => toc.category = Some(value),
            "interface" => {
                toc.interface_numbers = value
                    .split(',')
                    .filter_map(|n| n.trim().parse::<u32>().ok())
                    .collect();
                toc.interface_numbers.sort_unstable();
                toc.interface_numbers.dedup();
            }
            "savedvariables" => {
                toc.saved_variables = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            _ if lower.starts_with("notes") => {
                let lang = key[5..].trim_start_matches('-').to_ascii_lowercase();
                toc.notes.insert(lang, value);
            }
            _ if lower.starts_with("x-") => {
                match lower.as_str() {
                    "x-wowi-id" => toc.source_ids.wowinterface = Some(value.clone()),
                    "x-tukui-project-id" => toc.source_ids.tukui = Some(value.clone()),
                    "x-curse-project-id" => toc.source_ids.curseforge = Some(value.clone()),
                    "x-wago-id" => toc.source_ids.wago = Some(value.clone()),
                    "x-website" => toc.source_ids.website = Some(value.clone()),
                    _ => {}
                }
                toc.extra.insert(key.to_string(), value);
            }
            _ => {}
        }
    }
    if let Some(w) = toc.source_ids.website.clone() {
        if w.to_ascii_lowercase().contains("github.com") {
            toc.source_ids.github = Some(w);
        }
    }
    toc
}

/// Liest eine `.toc` von der Platte.
pub fn parse_file(path: &Path) -> std::io::Result<Toc> {
    let text = std::fs::read_to_string(path)?;
    let mut toc = parse_str(&text);
    toc.path = Some(path.to_path_buf());
    Ok(toc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_ist_eine_liste() {
        // Reihenfolge ist Teil der Zusage nicht: `## Interface` ist eine Menge
        // (PLAN.md §4), der Parser sortiert und entdoppelt. Frueher stand hier
        // die Reihenfolge der Datei, was nur zufaellig gleich war.
        let t = parse_str("## Interface: 11509, 20506, 16001\n## Version: 3.5.1\n");
        assert_eq!(t.interface_numbers, vec![11509, 16001, 20506]);
        assert_eq!(t.version.as_deref(), Some("3.5.1"));
    }

    #[test]
    fn interface_wird_entdoppelt() {
        let t = parse_str("## Interface: 11509, 11509, 20506\n");
        assert_eq!(t.interface_numbers, vec![11509, 20506]);
    }

    #[test]
    fn notizen_je_sprache() {
        let t = parse_str("## Notes: hi\n## Notes-deDE: hallo\n");
        assert_eq!(t.notes.get("").map(String::as_str), Some("hi"));
        assert_eq!(t.notes.get("dede").map(String::as_str), Some("hallo"));
    }
}
