//! Registry: welche Quellen laufen, wie gesund sind sie, Suche über alle.
//!
//! Ausfallverhalten aus PLAN.md §5: fällt eine Quelle aus, bleibt der Rest
//! nutzbar. [`Registry::search`] sammelt die Fehler je Quelle ein und bricht
//! nicht ab; die UI zeigt daraus „Tukui: nicht erreichbar" statt eines leeren
//! Bildschirms. Quellen ohne Schlüssel werden gar nicht erst eingetragen, stehen
//! aber in [`Registry::health`] als abgeschaltet.

use std::sync::Arc;

use crate::cache::Cache;
use crate::curseforge::CurseForgeSource;
use crate::github::GitHubSource;
use crate::http::Http;
use crate::keys::{KeyProvider, Secret};
use crate::merge::{Entry, Release};
use crate::tukui::TukuiSource;
use crate::wago::WagoSource;
use crate::wowinterface::WowInterfaceSource;
use crate::{AddonSource, Source, SourceError, SourceHealth};

/// Sammlung aktiver Quellen. Quellen ohne Schlüssel werden nicht eingetragen,
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

    /// Baut die Registry aus Schlüsseln und Netzzugang: GitHub, Tukui und
    /// WoWInterface laufen immer, Wago und CurseForge nur mit Schlüssel.
    ///
    /// Der Aufruf macht **keinen** Netzverkehr; die Quellen holen erst bei der
    /// ersten Suche. Deshalb ist die Funktion auch in der UI beim Start nutzbar.
    pub fn build(keys: &KeyProvider, http: Arc<dyn Http>, cache: Cache) -> Self {
        Self::build_with_github_token(keys, http, cache, None)
    }

    pub fn build_with_github_token(
        keys: &KeyProvider,
        http: Arc<dyn Http>,
        cache: Cache,
        github_token: Option<Secret>,
    ) -> Self {
        let mut sources: Vec<Box<dyn AddonSource>> = Vec::new();

        let mut github = GitHubSource::new(Arc::clone(&http), cache.clone());
        if let Some(token) = github_token {
            github = github.with_token(token);
        }
        sources.push(Box::new(github));
        sources.push(Box::new(TukuiSource::new(Arc::clone(&http), cache.clone())));
        sources.push(Box::new(WowInterfaceSource::new(
            Arc::clone(&http),
            cache.clone(),
        )));
        if let Some(token) = keys.key(Source::Wago) {
            sources.push(Box::new(WagoSource::new(
                Arc::clone(&http),
                cache.clone(),
                Some(token),
            )));
        }
        if let Some(key) = keys.key(Source::CurseForge) {
            sources.push(Box::new(CurseForgeSource::new(
                Arc::clone(&http),
                cache.clone(),
                Some(key),
            )));
        }
        Self::new(sources)
    }

    pub fn sources(&self) -> &[Box<dyn AddonSource>] {
        &self.sources
    }

    pub fn source(&self, source: Source) -> Option<&dyn AddonSource> {
        self.sources
            .iter()
            .find(|s| s.id() == source)
            .map(|s| s.as_ref())
    }

    /// Zustand jeder eingetragenen Quelle.
    pub fn health(&self) -> Vec<SourceHealth> {
        self.sources.iter().map(|s| s.health()).collect()
    }

    /// Prüft jede Quelle mit einer echten Anfrage und meldet den Ausgang —
    /// für `hexwerk keys test`. Nicht eingetragene Quellen (kein Schlüssel)
    /// stehen mit `ok: Some(false)` dabei, damit der Bericht vollständig ist.
    pub fn probe_all(&self) -> Vec<SourceHealth> {
        let mut out: Vec<SourceHealth> = self.sources.iter().map(|s| s.probe()).collect();
        for source in self.absent() {
            out.push(SourceHealth {
                source,
                ok: Some(false),
                detail: "nicht eingetragen (kein Schlüssel)".to_string(),
            });
        }
        out.sort_by_key(|h| h.source);
        out
    }

    /// Quellen, die der Katalog kennt, die aber nicht laufen — für die Anzeige
    /// „CurseForge: kein Schlüssel".
    pub fn absent(&self) -> Vec<Source> {
        [
            Source::GitHub,
            Source::Tukui,
            Source::WowInterface,
            Source::Wago,
            Source::CurseForge,
        ]
        .into_iter()
        .filter(|s| self.source(*s).is_none())
        .collect()
    }

    /// Suche über alle Quellen, Fehler je Quelle gesammelt statt abgebrochen.
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

    /// Lädt ein Release bei der Quelle, aus der es stammt.
    ///
    /// WARUM hier und nicht beim Aufrufer: die Quelle ist im [`Release`]
    /// vermerkt; ein `match` in jedem Aufrufer wäre dieselbe Verteilung, nur
    /// mehrfach geschrieben.
    pub fn download(&self, release: &Release) -> Result<Vec<u8>, SourceError> {
        match self.source(release.source) {
            Some(source) => source.download(release),
            None => Err(SourceError::Disabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_schluessel_laufen_drei_quellen() {
        let dir = std::env::temp_dir().join(format!("hexwerk-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let keys = KeyProvider::new(&dir).without_env().without_keyring();
        let http: Arc<dyn Http> = Arc::new(crate::http::UreqHttp::new());
        // Kein Request: `build` ist reine Konstruktion.
        let registry = Registry::build(&keys, http, Cache::new(dir.join("cache")));
        let ids: Vec<Source> = registry.sources().iter().map(|s| s.id()).collect();
        assert_eq!(
            ids,
            vec![Source::GitHub, Source::Tukui, Source::WowInterface]
        );
        assert_eq!(registry.absent(), vec![Source::Wago, Source::CurseForge]);
        let health = registry.health();
        assert_eq!(health.len(), 3);
    }
}
