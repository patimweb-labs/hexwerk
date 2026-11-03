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
    /// Adressen der Bilder, die die Quelle zu diesem Eintrag nennt: erstes
    /// Element ist das Logo/Icon, danach Screenshots. Leer, wenn die Quelle
    /// keine Bilder liefert (GitHub und Tukui tun das nicht).
    pub screenshots: Vec<String>,
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
/// Rangfolge der Quellen: je kleiner der Rang, desto lieber wird die Quelle
/// fuer Version, Bild und Kurztext genommen.
///
/// Konfigurierbar (Reihenfolge in `source_priority` der Einstellungen); die
/// Vorgabe ist CurseForge → WoWInterface → Tukui → GitHub → Wago. Die Rangfolge
/// entscheidet nur, **wer gewinnt**, wenn mehrere Quellen dasselbe Addon
/// tragen — sie blendet keine Quelle aus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Priority(Vec<Source>);

impl Default for Priority {
    fn default() -> Self {
        Self(vec![
            Source::CurseForge,
            Source::WowInterface,
            Source::Wago,
            Source::Local,
        ])
    }
}

impl Priority {
    /// Aus einer Liste von Quellen; unbekannte bleiben aussen vor, fehlende
    /// landen hinten in ihrer Standardreihenfolge. Damit ist auch eine
    /// unvollstaendige Einstellung benutzbar.
    pub fn new(order: Vec<Source>) -> Self {
        let mut out: Vec<Source> = Vec::new();
        for source in order {
            if !out.contains(&source) {
                out.push(source);
            }
        }
        for source in Self::default().0 {
            if !out.contains(&source) {
                out.push(source);
            }
        }
        Self(out)
    }

    /// Rang einer Quelle (klein = wichtig). Unbekannte nach hinten.
    pub fn rank(&self, source: Source) -> usize {
        self.0
            .iter()
            .position(|known| *known == source)
            .unwrap_or(self.0.len())
    }

    /// Die wichtigste Quelle unter denen, die tatsaechlich etwas liefern.
    pub fn best<'a>(&self, sources: impl Iterator<Item = &'a Source>) -> Option<Source> {
        sources.min_by_key(|source| self.rank(**source)).copied()
    }
}

/// Vereinigt Eintraege zu je einem Eintrag pro Addon.
///
/// Zwei Schritte:
/// 1. gleicher `key` — dasselbe Addon aus derselben Quelle;
/// 2. gleicher normalisierter Name **und** vertraeglicher Autor — dasselbe
///    Addon aus verschiedenen Quellen, die je eine eigene Kennung vergeben
///    (`curseforge:638542` gegen `wowinterface:15151`).
///
/// Der Autor muss dazu passen: „OneBag3" von zwei verschiedenen Autoren bleibt
/// getrennt, sonst wuerden fremde Addons verschmolzen. Ein leerer Autor gilt als
/// „keine Aussage" und wird akzeptiert.
pub fn merge(entries: impl IntoIterator<Item = Entry>) -> Vec<Entry> {
    merge_with(entries, &Priority::default())
}

/// Wie [`merge`], aber mit waehlbarer Rangfolge der Quellen.
pub fn merge_with(entries: impl IntoIterator<Item = Entry>, priority: &Priority) -> Vec<Entry> {
    // Schritt 1: je Schluessel ein Eintrag.
    let mut by_key: BTreeMap<String, Entry> = BTreeMap::new();
    for e in entries {
        match by_key.get_mut(&e.key) {
            Some(existing) => absorb(existing, e),
            None => {
                by_key.insert(e.key.clone(), e);
            }
        }
    }

    // Schritt 2: ueber den Namen zusammenlegen. `BTreeMap` haelt die
    // Reihenfolge stabil, damit zwei Laeufe dasselbe Ergebnis ergeben.
    let mut by_name: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for (_, entry) in by_key {
        by_name.entry(slug(&entry.name)).or_default().push(entry);
    }

    let mut out: Vec<Entry> = Vec::new();
    for (_, group) in by_name {
        let mut group = group;
        // Innerhalb einer Namensgruppe die Autor-Klassen trennen.
        let mut classes: Vec<Vec<Entry>> = Vec::new();
        for entry in group.drain(..) {
            let target = classes.iter_mut().find(|class| {
                class
                    .first()
                    .is_some_and(|first| authors_fit(&first.author, &entry.author))
            });
            match target {
                Some(class) => class.push(entry),
                None => classes.push(vec![entry]),
            }
        }
        for mut class in classes {
            // Der wichtigste Eintrag gibt Name, Kurztext und Bilder vor.
            class.sort_by_key(|entry| {
                let best = priority
                    .best(entry.releases.iter().map(|release| &release.source))
                    .unwrap_or(Source::Local);
                priority.rank(best)
            });
            let mut head = class.remove(0);
            for other in class {
                absorb(&mut head, other);
            }
            out.push(head);
        }
    }

    for e in &mut out {
        e.releases
            .sort_by(|a, b| a.source.cmp(&b.source).then(b.version.cmp(&a.version)));
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Nimmt `donor` in `target` auf: Releases ergaenzen, Luecken fuellen, Bilder
/// vereinigen. Vorhandene Angaben werden nicht ueberschrieben.
fn absorb(target: &mut Entry, donor: Entry) {
    for release in donor.releases {
        if !target
            .releases
            .iter()
            .any(|known| known.source == release.source && known.version == release.version)
        {
            target.releases.push(release);
        }
    }
    if target.summary.is_empty() {
        target.summary = donor.summary;
    }
    if target.homepage.is_none() {
        target.homepage = donor.homepage;
    }
    if target.category.is_none() {
        target.category = donor.category;
    }
    if target.author.is_empty() {
        target.author = donor.author;
    }
    for url in donor.screenshots {
        if !target.screenshots.contains(&url) {
            target.screenshots.push(url);
        }
    }
}

/// Passen zwei Autorenangaben zu demselben Addon?
///
/// Gleicher normalisierter Text oder eine Seite ohne Angabe. Bewusst streng:
/// ein falsches Zusammenlegen ist schlimmer als zwei Karten.
fn authors_fit(a: &str, b: &str) -> bool {
    let (a, b) = (slug(a), slug(b));
    a.is_empty() || b.is_empty() || a == b
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
    fn gleicher_name_aus_zwei_quellen_wird_ein_eintrag() {
        // OneBag3 liegt bei CurseForge und WoWInterface unter verschiedenen
        // Kennungen. Bisher ergab das zwei Karten im Katalog.
        let cur = Entry {
            screenshots: vec!["https://bild/logo.png".into()],
            key: "curseforge:638542".into(),
            name: "OneBag3".into(),
            author: "Kaelten".into(),
            summary: "Sammelt alle Taschen".into(),
            homepage: None,
            category: Some("Miscellaneous".into()),
            releases: vec![rel(Source::CurseForge, "3.11.1")],
        };
        let wowi = Entry {
            screenshots: Vec::new(),
            key: "wowinterface:15151".into(),
            name: "OneBag3".into(),
            author: "kaelten".into(),
            summary: String::new(),
            homepage: Some("https://wowinterface.example/15151".into()),
            category: None,
            releases: vec![rel(Source::WowInterface, "3.11.1")],
        };
        let merged = merge(vec![wowi, cur]);
        assert_eq!(merged.len(), 1, "ein Eintrag, nicht zwei");
        assert_eq!(merged[0].releases.len(), 2, "beide Quellen bleiben");
        assert_eq!(merged[0].summary, "Sammelt alle Taschen");
        assert_eq!(merged[0].screenshots.len(), 1);
    }

    #[test]
    fn rangfolge_bestimmt_den_kopf() {
        let wowi = Entry {
            screenshots: Vec::new(),
            key: "wowinterface:15151".into(),
            name: "OneBag3".into(),
            author: "Kaelten".into(),
            summary: "Text von WoWI".into(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::WowInterface, "3.11.1")],
        };
        let cur = Entry {
            screenshots: Vec::new(),
            key: "curseforge:638542".into(),
            name: "OneBag3".into(),
            author: "Kaelten".into(),
            summary: "Text von CurseForge".into(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::CurseForge, "3.11.1")],
        };
        // WoWI zuerst in der Liste, CurseForge hat aber Vorrang.
        let merged = merge(vec![wowi, cur]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].summary, "Text von CurseForge");

        // Umgedrehte Rangfolge dreht auch den Kopf.
        let priority = Priority::new(vec![Source::WowInterface, Source::CurseForge]);
        let entries = vec![
            Entry {
                screenshots: Vec::new(),
                key: "wowinterface:15151".into(),
                name: "OneBag3".into(),
                author: "Kaelten".into(),
                summary: "Text von WoWI".into(),
                homepage: None,
                category: None,
                releases: vec![rel(Source::WowInterface, "3.11.1")],
            },
            Entry {
                screenshots: Vec::new(),
                key: "curseforge:638542".into(),
                name: "OneBag3".into(),
                author: "Kaelten".into(),
                summary: "Text von CurseForge".into(),
                homepage: None,
                category: None,
                releases: vec![rel(Source::CurseForge, "3.11.1")],
            },
        ];
        let merged = merge_with(entries, &priority);
        assert_eq!(merged[0].summary, "Text von WoWI");
    }

    #[test]
    fn gleicher_name_anderer_autor_bleibt_getrennt() {
        let a = Entry {
            screenshots: Vec::new(),
            key: "curseforge:1".into(),
            name: "OneBag3".into(),
            author: "Kaelten".into(),
            summary: String::new(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::CurseForge, "1.0")],
        };
        let b = Entry {
            screenshots: Vec::new(),
            key: "curseforge:2".into(),
            name: "OneBag3".into(),
            author: "Jemand anderes".into(),
            summary: String::new(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::WowInterface, "9.9")],
        };
        assert_eq!(merge(vec![a, b]).len(), 2);
    }

    #[test]
    fn gleicher_schluessel_wird_ein_eintrag() {
        let a = Entry {
            screenshots: Vec::new(),
            key: "weakauras".into(),
            name: "WeakAuras".into(),
            author: "WA".into(),
            summary: "".into(),
            homepage: None,
            category: None,
            releases: vec![rel(Source::GitHub, "5.19.2")],
        };
        let b = Entry {
            screenshots: Vec::new(),
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
