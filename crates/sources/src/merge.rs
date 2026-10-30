//! Zusammenfuehren: ein Addon, mehrere Quellen.

use std::collections::BTreeMap;

use crate::Source;

/// Ein Release aus einer Quelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub source: Source,
    pub external_id: String,
    pub version: String,
    pub released_at: Option<String>,
    pub download_url: String,
    pub size_bytes: Option<u64>,
    /// Interface-Nummern aus dem `.toc` dieses Releases, falls bekannt.
    pub interface_numbers: Vec<u32>,
}

/// Ein Katalogeintrag, der mehrere Quellen traegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Stabiler Schluessel: externe Projekt-ID, sonst normalisierter Name.
    pub key: String,
    pub name: String,
    pub author: String,
    pub summary: String,
    pub homepage: Option<String>,
    pub category: Option<String>,
    pub releases: Vec<Release>,
}

impl Entry {
    /// Hoechste Version im Eintrag (nach Releasedatum, sonst lexikalisch).
    pub fn newest(&self) -> Option<&Release> {
        self.releases.iter().max_by(|a, b| {
            a.released_at
                .cmp(&b.released_at)
                .then(a.version.cmp(&b.version))
        })
    }
}

/// Normalisiert einen Text zu einem Schlüsselteil: klein, ASCII, ein `-` je
/// Trenner. Umlaute und Akzente werden umgeschrieben, damit derselbe Addon-Name
/// aus zwei Quellen denselben Schlüssel ergibt.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_dash = true;
    for ch in text.chars() {
        let mapped: Option<char> = match ch {
            'ä' | 'Ä' => Some('a'),
            'ö' | 'Ö' => Some('o'),
            'ü' | 'Ü' => Some('u'),
            'ß' => Some('s'),
            'é' | 'è' | 'ê' | 'É' => Some('e'),
            'á' | 'à' | 'â' | 'Á' => Some('a'),
            'í' | 'ì' | 'î' => Some('i'),
            'ó' | 'ò' | 'ô' => Some('o'),
            'ú' | 'ù' | 'û' => Some('u'),
            'ç' => Some('c'),
            _ => None,
        };
        let ch = mapped.unwrap_or(ch);
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Schlüssel aus einer externen Projekt-ID (PLAN.md §5, erster Fall).
///
/// WARUM mit Quellenpräfix: `X-Curse-Project-ID: 1` und `X-WoWI-ID: 1` sind
/// verschiedene Addons; ohne Präfix kollidierten sie zu einem Eintrag.
pub fn id_key(source: Source, external_id: &str) -> String {
    format!("{}:{}", slug(source.label()), external_id.trim())
}

/// Schlüssel ohne externe ID (PLAN.md §5, zweiter Fall).
///
/// Autor ist Teil des Schlüssels, weil zwei gleichnamige Addons verschiedener
/// Autoren sonst verschmölzen — der Preis ist, dass derselbe Autor in zwei
/// Quellen unterschiedlich geschrieben sein muss, um zu treffen.
pub fn name_key(name: &str, author: &str) -> String {
    let name = slug(name);
    let author = slug(author);
    if author.is_empty() {
        name
    } else {
        format!("{name}-{author}")
    }
}

/// Fasst Eintraege zusammen. Regel: gleicher `key` = gleiches Addon.
pub fn merge(entries: impl IntoIterator<Item = Entry>) -> Vec<Entry> {
    let mut map: BTreeMap<String, Entry> = BTreeMap::new();
    for e in entries {
        match map.get_mut(&e.key) {
            Some(existing) => {
                for r in e.releases {
                    if !existing
                        .releases
                        .iter()
                        .any(|x| x.source == r.source && x.version == r.version)
                    {
                        existing.releases.push(r);
                    }
                }
                if existing.summary.is_empty() {
                    existing.summary = e.summary
                }
                if existing.homepage.is_none() {
                    existing.homepage = e.homepage
                }
            }
            None => {
                map.insert(e.key.clone(), e);
            }
        }
    }
    let mut out: Vec<Entry> = map.into_values().collect();
    for e in &mut out {
        e.releases
            .sort_by(|a, b| a.source.cmp(&b.source).then(b.version.cmp(&a.version)));
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(src: Source, v: &str) -> Release {
        Release {
            source: src,
            external_id: "1".into(),
            version: v.into(),
            released_at: None,
            download_url: "https://example.invalid/x.zip".into(),
            size_bytes: None,
            interface_numbers: vec![],
        }
    }

    #[test]
    fn gleicher_schluessel_wird_ein_eintrag() {
        let a = Entry {
            key: "weakauras".into(),
            name: "WeakAuras".into(),
            author: "WA".into(),
            summary: "".into(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::GitHub, "5.19.2")],
        };
        let b = Entry {
            key: "weakauras".into(),
            name: "WeakAuras".into(),
            author: "WA".into(),
            summary: "".into(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::Tukui, "5.19.2")],
        };
        let merged = merge(vec![a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].releases.len(), 2);
    }
}
