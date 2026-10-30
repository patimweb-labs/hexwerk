//! Katalog aus dem Dienst holen.
//!
//! Die App haelt keinen CurseForge-Schluessel. Sie holt beim Start ein
//! Manifest und, wenn sich die Version geaendert hat, den gepackten Katalog.
//! Der Schluessel in den Einstellungen gilt nur gegenueber dem Dienst.
//!
//! Ablauf:
//!   1. `GET {base}/v1/catalog/manifest` mit `Authorization: Bearer <key>`.
//!   2. Ist `version` schon bekannt (steht in `meta`), ist nichts zu tun.
//!   3. Sonst `GET {base}/v1/catalog/snapshot`, Pruefsumme gegen das Manifest.
//!   4. Entpacken, als zweite Datenbank anhaengen und die Zeilen in den
//!      lokalen Index uebernehmen. Der Index behaelt alles, was er aus
//!      eigenen Laeufen weiss.

use crate::schema::Index;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Merker im Index, welcher Katalogstand zuletzt uebernommen wurde.
const META_VERSION: &str = "catalog_version";

#[derive(Debug, Clone)]
pub struct RemoteCatalog {
    pub base: String,
    pub key: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub entries: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Nichts zu tun, der Stand liegt schon im Index.
    UpToDate { version: String },
    /// Uebernommen.
    Imported {
        version: String,
        entries: i64,
        addons_written: usize,
    },
}

impl RemoteCatalog {
    pub fn new(base: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            key: key.into(),
        }
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .redirects(2)
            .build()
    }

    pub fn fetch_manifest(&self) -> Result<Manifest, String> {
        let url = format!("{}/v1/catalog/manifest", self.base);
        let response = self
            .agent()
            .get(&url)
            .set("Authorization", &format!("Bearer {}", self.key))
            .call()
            .map_err(|err| format!("Manifest nicht abrufbar ({url}): {err}"))?;
        response
            .into_json::<Manifest>()
            .map_err(|err| format!("Manifest nicht lesbar: {err}"))
    }

    /// Holt den Katalog und uebernimmt ihn in den Index.
    ///
    /// `force` ignoriert den bekannten Stand. Der Abruf laeuft im aufrufenden
    /// Thread; die Oberflaeche ruft das aus einem eigenen Faden.
    pub fn sync(
        &self,
        index: &mut Index,
        cache_dir: &Path,
        force: bool,
    ) -> Result<SyncOutcome, String> {
        let manifest = self.fetch_manifest()?;
        if !force {
            if let Some(known) = index.meta(META_VERSION).unwrap_or(None) {
                if known == manifest.version {
                    return Ok(SyncOutcome::UpToDate {
                        version: manifest.version,
                    });
                }
            }
        }

        std::fs::create_dir_all(cache_dir).map_err(|err| format!("Ablage: {err}"))?;
        let packed: PathBuf = cache_dir.join("catalog.sqlite.zst");
        self.download_snapshot(&packed)?;

        let bytes = std::fs::read(&packed).map_err(|err| format!("Paket lesen: {err}"))?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hex::encode(hasher.finalize());
        if !manifest.sha256.is_empty() && digest != manifest.sha256 {
            return Err(format!(
                "Pruefsumme stimmt nicht: erwartet {}, bekommen {}",
                manifest.sha256, digest
            ));
        }

        let raw = zstd::decode_all(std::io::Cursor::new(&bytes))
            .map_err(|err| format!("Entpacken: {err}"))?;
        let plain: PathBuf = cache_dir.join("catalog.sqlite");
        std::fs::write(&plain, &raw).map_err(|err| format!("Katalog schreiben: {err}"))?;

        let written = index
            .merge_catalog(&plain)
            .map_err(|err| format!("Uebernehmen: {err}"))?;
        index.set_meta(META_VERSION, &manifest.version)?;
        Ok(SyncOutcome::Imported {
            version: manifest.version,
            entries: manifest.entries,
            addons_written: written,
        })
    }

    fn download_snapshot(&self, target: &Path) -> Result<(), String> {
        let url = format!("{}/v1/catalog/snapshot", self.base);
        let response = self
            .agent()
            .get(&url)
            .timeout(std::time::Duration::from_secs(300))
            .set("Authorization", &format!("Bearer {}", self.key))
            .call()
            .map_err(|err| format!("Katalog nicht abrufbar ({url}): {err}"))?;
        let mut reader = response.into_reader();
        let mut file =
            std::fs::File::create(target).map_err(|err| format!("Zieldatei: {err}"))?;
        std::io::copy(&mut reader, &mut file).map_err(|err| format!("Schreiben: {err}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_ohne_schraegstrich() {
        let remote = RemoteCatalog::new("https://example.org/", "k");
        assert_eq!(remote.base, "https://example.org");
    }

    #[test]
    fn manifest_liest_json() {
        let raw = r#"{"version":"17","bytes":123,"sha256":"ab","entries":42}"#;
        let manifest: Manifest = serde_json::from_str(raw).unwrap();
        assert_eq!(manifest.version, "17");
        assert_eq!(manifest.entries, 42);
        assert_eq!(manifest.sha256, "ab");
    }
}
