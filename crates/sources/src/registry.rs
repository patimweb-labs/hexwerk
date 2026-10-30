//! Registry: welche Quellen laufen, wie gesund sind sie, Suche ueber alle.

use crate::{AddonSource, Source, SourceHealth};

use crate::merge::Entry;

/// Sammlung aktiver Quellen. Quellen ohne Schluessel werden nicht eingetragen,
/// ihr Zustand steht trotzdem in [`Registry::health`].
pub struct Registry {
    sources: Vec<Box<dyn AddonSource>>,
}

impl Registry {
    pub fn new(sources: Vec<Box<dyn AddonSource>>) -> Self {
        Self { sources }
    }

    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn sources(&self) -> &[Box<dyn AddonSource>] {
        &self.sources
    }

    /// Zustand jeder eingetragenen Quelle.
    pub fn health(&self) -> Vec<SourceHealth> {
        self.sources.iter().map(|s| s.health()).collect()
    }

    /// Suche ueber alle Quellen, Fehler je Quelle gesammelt statt abgebrochen.
    pub fn search(&self, query: &str, page: u32) -> (Vec<Entry>, Vec<(Source, String)>) {
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        for s in &self.sources {
            match s.search(query, page) {
                Ok(mut e) => entries.append(&mut e),
                Err(err) => errors.push((s.id(), err.to_string())),
            }
        }
        (crate::merge(entries), errors)
    }
}
