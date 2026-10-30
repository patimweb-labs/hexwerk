//! Den ganzen Addon-Ordner eines Clients leeren.
//!
//! Bewusst ohne zusätzliche ZIP-Sicherung: bei einem vollen Classic-Ordner sind
//! das mehrere Gigabyte, eine Sicherung würde den Platz verdoppeln. Der
//! Papierkorb ist der Rückweg, und der liegt auf demselben Datenträger, wird
//! also nicht kopiert, sondern verschoben.
//!
//! Was hier nicht passiert: der WTF-Ordner bleibt unangetastet. Dort liegen die
//! Einstellungen der Addons, die will nach dem Leeren weiterhaben.

use std::path::{Path, PathBuf};

use crate::trash::{trash_addon, TrashEntry};
use crate::{InstallError, Paths};

/// Ergebnis eines Leerlaufs.
#[derive(Debug, Clone)]
pub struct ClearOutcome {
    /// Anzahl der verschobenen Ordner.
    pub folders: usize,
    /// Belegte Bytes vor dem Verschieben.
    pub bytes: u64,
    /// Papierkorb-Einträge dieses Laufs, für "Rückgängig".
    pub trashed: Vec<TrashEntry>,
}

impl ClearOutcome {
    pub fn trash_dirs(&self) -> Vec<PathBuf> {
        self.trashed.iter().map(|t| t.path()).collect()
    }
}

/// Verschiebt jeden Ordner unter `addons_dir` in den Papierkorb.
///
/// `keep` nennt Ordnernamen, die bleiben sollen (etwa weil eine Auswahl
/// versehentlich alles erwischt hätte). Ein einzelner Fehler bricht den Lauf
/// ab: was schon verschoben ist, steht im Papierkorb und lässt sich
/// zurückholen, ein halb geleerter Ordner wäre dagegen ein Rätsel.
pub fn clear_addons(
    addons_dir: &Path,
    paths: &Paths,
    keep: &[String],
) -> Result<ClearOutcome, InstallError> {
    // Nur der Addon-Ordner selbst, nie sein Elternverzeichnis.
    let canonical = std::fs::canonicalize(addons_dir)
        .map_err(|e| InstallError::NotFound(format!("{}: {e}", addons_dir.display())))?;
    if !canonical.is_dir() {
        return Err(InstallError::NotFound(canonical.display().to_string()));
    }

    let mut names: Vec<String> = std::fs::read_dir(&canonical)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| !keep.iter().any(|k| k == name))
        .collect();
    names.sort();

    let mut bytes = 0;
    for name in &names {
        bytes += crate::folder_size(&canonical.join(name));
    }

    let mut trashed = Vec::with_capacity(names.len());
    for name in &names {
        match trash_addon(&canonical, name, paths) {
            Ok(entry) => trashed.push(entry),
            Err(err) => {
                // Was bis hierher verschoben wurde, bleibt im Papierkorb; der
                // Fehler nennt den Ordner, an dem es hakte.
                return Err(match err {
                    InstallError::Io(io) => {
                        InstallError::Io(std::io::Error::new(io.kind(), format!("{name}: {io}")))
                    }
                    other => other,
                });
            }
        }
    }

    Ok(ClearOutcome {
        folders: trashed.len(),
        bytes,
        trashed,
    })
}

/// Papierkorb-Einträge eines Laufs zurückholen.
pub fn restore_all(entries: &[TrashEntry]) -> Result<Vec<PathBuf>, InstallError> {
    let mut restored = Vec::with_capacity(entries.len());
    for entry in entries {
        restored.push(crate::restore_from_trash(entry)?);
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn werkbank() -> (PathBuf, Paths) {
        let base = std::env::temp_dir().join(format!(
            "hexwerk-clear-{}-{}",
            std::process::id(),
            crate::unique_token()
        ));
        let addons = base.join("AddOns");
        std::fs::create_dir_all(&addons).unwrap();
        let paths = Paths::at(base.join("daten"));
        (addons, paths)
    }

    fn addon(dir: &Path, name: &str, bytes: usize) {
        let p = dir.join(name);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("x.toc"), vec![b'a'; bytes]).unwrap();
    }

    #[test]
    fn leert_alles_und_zaehlt() {
        let (addons, paths) = werkbank();
        addon(&addons, "A", 100);
        addon(&addons, "B", 200);
        let out = clear_addons(&addons, &paths, &[]).unwrap();
        assert_eq!(out.folders, 2);
        assert!(out.bytes >= 300);
        assert_eq!(std::fs::read_dir(&addons).unwrap().count(), 0);
        assert_eq!(out.trashed.len(), 2);
    }

    #[test]
    fn behaelt_ausnahmen() {
        let (addons, paths) = werkbank();
        addon(&addons, "A", 10);
        addon(&addons, "Bleibt", 10);
        let out = clear_addons(&addons, &paths, &["Bleibt".to_string()]).unwrap();
        assert_eq!(out.folders, 1);
        assert!(addons.join("Bleibt").is_dir());
    }

    #[test]
    fn rueckgaengig_stellt_wieder_her() {
        let (addons, paths) = werkbank();
        addon(&addons, "A", 10);
        let out = clear_addons(&addons, &paths, &[]).unwrap();
        restore_all(&out.trashed).unwrap();
        assert!(addons.join("A").join("x.toc").is_file());
    }
}
