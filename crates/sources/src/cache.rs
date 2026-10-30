//! Antworten zwischenspeichern: HTTP-Cache auf Platte unter `~/.cache/hexwerk`.
//!
//! WARUM überhaupt: GitHub erlaubt 60 Anfragen pro Stunde ohne Token, WoWInterface
//! liefert 2,7 MB für eine Gesamtliste. Ohne TTL-Cache wäre der Katalog nach zwei
//! Suchen gesperrt. WARUM auf Platte und nicht im Speicher: der Katalog wird auch
//! nach einem Neustart der App angezeigt, bevor das Netz geantwortet hat.
//!
//! Zweiter Zweck: Ausfallverhalten aus PLAN.md §5. [`Cache::cached`] liefert bei
//! Netzfehler die alte, abgelaufene Antwort weiter, damit eine tote Quelle nur den
//! Zustand verschlechtert und nicht den ganzen Katalog leert.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::SourceError;

/// Gesamtlisten (Tukui, WoWInterface): ändern sich höchstens stündlich.
pub const TTL_LIST: Duration = Duration::from_secs(6 * 60 * 60);
/// Je-Addon-Details (WoWInterface filedetails, GitHub releases/latest).
pub const TTL_DETAILS: Duration = Duration::from_secs(6 * 60 * 60);
/// Suchanfragen: kurz, damit neue Addons auftauchen, aber lang genug für 60/h.
pub const TTL_SEARCH: Duration = Duration::from_secs(15 * 60);

/// Verzeichnis-Cache mit einer Datei je Schlüssel. Der Schlüssel ist die URL
/// (inklusive Query), sein SHA-256 der Dateiname — so werden Dateinamen nie zu
/// lang und ein Schlüssel im Query-String landet nicht im Dateisystem.
#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$XDG_CACHE_HOME/hexwerk/http`, sonst `~/.cache/hexwerk/http`.
    pub fn default_root() -> PathBuf {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            // Ohne HOME (Flatpak-Sandbox mit kaputtem Umfeld) lieber ein
            // temporärer Cache als ein Absturz beim Start.
            .unwrap_or_else(|| std::env::temp_dir().join("hexwerk-cache"));
        base.join("hexwerk").join("http")
    }

    pub fn open_default() -> Self {
        Self::new(Self::default_root())
    }

    /// Nur für Anzeige und Prüfungen: der Schlüssel selbst wird nie gespeichert.
    pub fn path_for(&self, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let digest = hasher.finalize();
        let hex = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        self.root.join(format!("{hex}.body"))
    }

    /// Alter der gespeicherten Antwort, `None` wenn nichts da ist.
    pub fn age(&self, key: &str) -> Option<Duration> {
        let path = self.path_for(key);
        let modified = fs::metadata(&path).ok()?.modified().ok()?;
        SystemTime::now().duration_since(modified).ok()
    }

    /// Antwort ohne Rücksicht auf das Alter — Grundlage für den Rückfall.
    pub fn remember(&self, key: &str) -> Option<Vec<u8>> {
        fs::read(self.path_for(key)).ok()
    }

    pub fn fresh(&self, key: &str, ttl: Duration) -> Option<Vec<u8>> {
        match self.age(key) {
            // Streng kleiner: `TTL=0` heißt damit zuverlässig „nie aus dem
            // Cache", nicht „vielleicht, wenn die Uhr nicht getickt hat".
            Some(age) if age < ttl => self.remember(key),
            _ => None,
        }
    }

    /// Schreibt atomar: erst `.tmp`, dann `rename`. Sonst liest ein zweiter
    /// Thread eine halb geschriebene Datei und hält sie für JSON.
    pub fn put(&self, key: &str, body: &[u8]) -> std::io::Result<()> {
        if body.is_empty() {
            // Leere Antworten sind fast immer Fehlerseiten; sie zu behalten
            // würde den Fehler für die Dauer der TTL festschreiben.
            return Ok(());
        }
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(key);
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(body)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &path)
    }

    pub fn forget(&self, key: &str) -> std::io::Result<()> {
        match fs::remove_file(self.path_for(key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Frisch aus dem Cache oder holen; bei Fehler die abgelaufene Kopie.
    ///
    /// Der Rückfall ist der Grund, warum diese Funktion existiert: jede Quelle
    /// würde sonst ihre eigene Variante davon schreiben, und eine davon hätte
    /// irgendwann vergessen, den Fehler nicht zu verschlucken.
    pub fn cached<F>(&self, key: &str, ttl: Duration, fetch: F) -> Result<Vec<u8>, SourceError>
    where
        F: FnOnce() -> Result<Vec<u8>, SourceError>,
    {
        if let Some(body) = self.fresh(key, ttl) {
            return Ok(body);
        }
        match fetch() {
            Ok(body) => {
                // Cache-Schreibfehler dürfen die Suche nicht scheitern lassen.
                let _ = self.put(key, &body);
                Ok(body)
            }
            Err(err) => match self.remember(key) {
                Some(stale) => Ok(stale),
                None => Err(err),
            },
        }
    }

    /// Räumt Dateien weg, die älter als `max_age` sind. Wird vom Aufrufer
    /// angestoßen (nicht bei jedem Start), damit ein Start nie auf I/O wartet.
    pub fn prune(&self, max_age: Duration) -> usize {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return 0;
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            let Ok(age) = SystemTime::now().duration_since(modified) else {
                continue;
            };
            if age > max_age && fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache(name: &str) -> Cache {
        let root =
            std::env::temp_dir().join(format!("hexwerk-cache-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        Cache::new(root)
    }

    #[test]
    fn schluessel_wird_nicht_zum_dateinamen() {
        let cache = temp_cache("nohash");
        let path = cache.path_for("https://example.org/x?key=geheim1234");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(!name.contains("geheim"));
        assert!(name.ends_with(".body"));
    }

    #[test]
    fn frisch_getroffen_alter_verworfen() {
        let cache = temp_cache("ttl");
        let key = "https://example.org/list";
        cache.put(key, b"[1]").unwrap();
        assert_eq!(
            cache.fresh(key, Duration::from_secs(60)),
            Some(b"[1]".to_vec())
        );
        // TTL 0: alles ist abgelaufen, `remember` liefert trotzdem.
        assert_eq!(cache.fresh(key, Duration::ZERO), None);
        assert_eq!(cache.remember(key), Some(b"[1]".to_vec()));
    }

    #[test]
    fn fehler_faellt_auf_alte_antwort_zurueck() {
        let cache = temp_cache("stale");
        let key = "https://example.org/list";
        cache.put(key, b"alt").unwrap();
        let out = cache
            .cached(key, Duration::ZERO, || Err(SourceError::RateLimited))
            .unwrap();
        assert_eq!(out, b"alt".to_vec());
    }

    #[test]
    fn ohne_alte_antwort_bleibt_der_fehler() {
        let cache = temp_cache("nofallback");
        let err = cache
            .cached("https://example.org/neu", Duration::ZERO, || {
                Err(SourceError::NotFound)
            })
            .unwrap_err();
        assert_eq!(err, SourceError::NotFound);
    }

    #[test]
    fn leerer_body_wird_nicht_gespeichert() {
        let cache = temp_cache("empty");
        cache.put("k", b"").unwrap();
        assert!(cache.remember("k").is_none());
    }
}
