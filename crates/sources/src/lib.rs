//! Quellen fuer Addons: suchen, Releases holen, zu einem Katalog mergen.
//!
//! Jede Quelle ist ein eigener Typ hinter dem Trait [`AddonSource`]. Die
//! Kette in [`registry`] entscheidet, welche Quellen laufen (Schluessel
//! vorhanden?), und [`merge`] fasst dasselbe Addon aus mehreren Quellen zu
//! einem Eintrag zusammen.

pub mod cache;
pub mod curseforge;
pub mod fingerprint;
pub mod github;
pub mod http;
pub mod keys;
pub mod merge;
pub mod registry;
pub mod tukui;
pub mod wago;
pub mod wowinterface;

pub use cache::Cache;
pub use http::{Http, HttpResponse, Request, UreqHttp};
pub use keys::{KeyOrigin, KeyProvider, Secret};
pub use merge::{merge, Entry, Release};
pub use registry::Registry;

/// Kennung einer Quelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    GitHub,
    Tukui,
    WowInterface,
    Wago,
    CurseForge,
    Local,
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::GitHub => "GitHub",
            Source::Tukui => "Tukui",
            Source::WowInterface => "WoWInterface",
            Source::Wago => "Wago",
            Source::CurseForge => "CurseForge",
            Source::Local => "Lokal",
        }
    }
    /// Braucht diese Quelle einen Schluessel?
    pub fn needs_key(&self) -> bool {
        matches!(self, Source::Wago | Source::CurseForge)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHealth {
    pub source: Source,
    /// `None` = nicht geprueft.
    pub ok: Option<bool>,
    /// HTTP-Code oder Klartextgrund.
    pub detail: String,
}

impl SourceHealth {
    pub fn unchecked(source: Source) -> Self {
        Self {
            source,
            ok: None,
            detail: "nicht geprueft".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// Quelle ist aus (kein Schluessel, absichtlich abgeschaltet).
    Disabled,
    Network(String),
    Http {
        status: u16,
        url: String,
    },
    Parse(String),
    RateLimited,
    NotFound,
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::Disabled => write!(f, "Quelle ist abgeschaltet"),
            SourceError::Network(e) => write!(f, "Netzfehler: {e}"),
            SourceError::Http { status, url } => write!(f, "HTTP {status} bei {url}"),
            SourceError::Parse(e) => write!(f, "Antwort nicht lesbar: {e}"),
            SourceError::RateLimited => write!(f, "Anfragen gedrosselt"),
            SourceError::NotFound => write!(f, "nicht gefunden"),
        }
    }
}

impl std::error::Error for SourceError {}

/// Eine Quelle. `Send + Sync`, weil sie im Hintergrundthread laeuft.
pub trait AddonSource: Send + Sync {
    fn id(&self) -> Source;
    fn health(&self) -> SourceHealth;
    /// Freitextsuche. `page` beginnt bei 0.
    fn search(&self, query: &str, page: u32) -> Result<Vec<merge::Entry>, SourceError>;
    /// Laedt das ZIP eines Releases.
    fn download(&self, release: &merge::Release) -> Result<Vec<u8>, SourceError>;

    /// Ein einziger echter Testaufruf je Quelle — die Grundlage fuer
    /// `hexwerk keys test` (PLAN.md §7: „prueft jede Quelle mit einer Anfrage und
    /// meldet den HTTP-Code").
    ///
    /// WARUM als Vorgabe im Trait: jede Quelle kann ihre Suche als Probe
    /// benutzen; nur wer eine billigere Anfrage hat, ueberschreibt das.
    fn probe(&self) -> SourceHealth {
        let source = self.id();
        match self.search("", 0) {
            Ok(entries) => SourceHealth {
                source,
                ok: Some(true),
                detail: format!("HTTP 200, {} Einträge", entries.len()),
            },
            Err(SourceError::Disabled) => SourceHealth {
                source,
                ok: Some(false),
                detail: "kein Schlüssel — abgeschaltet".to_string(),
            },
            Err(SourceError::RateLimited) => SourceHealth {
                source,
                ok: Some(false),
                detail: "HTTP 429 — gedrosselt".to_string(),
            },
            Err(err) => SourceHealth {
                source,
                ok: Some(false),
                detail: err.to_string(),
            },
        }
    }
}
