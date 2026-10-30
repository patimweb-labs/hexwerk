//! Installierte Addons in einem `Interface/AddOns`-Ordner erfassen.

use std::path::{Path, PathBuf};

use crate::toc::{parse_file, Toc};

/// Ein Addon-Ordner, wie er auf der Platte liegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAddon {
    pub folder: String,
    pub path: PathBuf,
    pub toc: Option<Toc>,
    /// Version aus dem `.toc`, falls vorhanden.
    pub version: Option<String>,
    /// Interface-Nummern aus dem `.toc`, falls vorhanden.
    pub interface_numbers: Vec<u32>,
    /// Belegte Bytes im Ordner (Summe der Dateigroessen).
    pub size_bytes: u64,
}

/// Liest alle Addon-Ordner. Ein Ordner zaehlt, wenn er mindestens eine
/// `.toc`-Datei enthaelt; die `.toc` mit dem Ordnernamen hat Vorrang.
pub fn scan_addons(addons_dir: &Path) -> Vec<InstalledAddon> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(addons_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder = match path.file_name().and_then(|s| s.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };
        let tocs: Vec<PathBuf> = std::fs::read_dir(&path)
            .map(|it| {
                it.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toc"))
                    .collect()
            })
            .unwrap_or_default();
        if tocs.is_empty() {
            continue;
        }
        let preferred = tocs
            .iter()
            .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(folder.as_str()))
            .cloned()
            .unwrap_or_else(|| tocs[0].clone());
        let toc = parse_file(&preferred).ok();
        out.push(InstalledAddon {
            folder,
            size_bytes: dir_size(&path),
            version: toc.as_ref().and_then(|t| t.version.clone()),
            interface_numbers: toc
                .as_ref()
                .map(|t| t.interface_numbers.clone())
                .unwrap_or_default(),
            toc,
            path,
        });
    }
    out.sort_by(|a, b| a.folder.to_lowercase().cmp(&b.folder.to_lowercase()));
    out
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        match entry.metadata() {
            Ok(md) if md.is_dir() => total += dir_size(&p),
            Ok(md) => total += md.len(),
            Err(_) => {}
        }
    }
    total
}
