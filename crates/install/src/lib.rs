//! Addons installieren, aktualisieren, sichern und entfernen.
//!
//! Drei Regeln bestimmen dieses Paket:
//!
//! 1. **Client-scharf.** Es wird immer in das `addons_dir` genau einer
//!    [`hexwerk::FlavorDir`] geschrieben. "Fuer alle Clients" gibt es nicht:
//!    dasselbe Addon liegt je Client in einem anderen Build (PLAN §13.5).
//! 2. **Atomar.** Entpackt wird in einen Arbeitsordner im Zielverzeichnis,
//!    danach wird umbenannt. Ein abgebrochener Download oder ein Fehler beim
//!    Entpacken hinterlaesst so nie einen halb gefuellten Addon-Ordner.
//! 3. **Nichts geht verloren.** Was ueberschrieben wird, liegt vorher als ZIP
//!    unter `backup/`; was entfernt wird, wandert in den Papierkorb unter
//!    `trash/` und laesst sich zurueckholen.
//!
//! Das Herunterladen ist Sache der Quellen (`hexwerk-sources`); hier kommt nur
//! der fertige ZIP-Inhalt an.

pub mod backup;
pub mod clear;
pub mod plan;
pub mod trash;
pub mod zip_install;

pub use backup::{backup_folder, list_backups, restore_backup};
pub use clear::{clear_addons, restore_all, ClearOutcome};
pub use plan::{apply, apply_with, InstallRequest, Installed};
pub use trash::{list_trash, remove_trash_entry, restore_from_trash, trash_addon, TrashEntry};
pub use zip_install::{inspect_zip, install_zip, ZipLayout};

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fehler beim Installieren.
#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Zip(String),
    Toc(String),
    NoTarget,
    /// Ein Pfad im ZIP ist absolut, enthaelt `..` oder ist ein Symlink.
    /// Ein solches Archiv wird komplett abgelehnt, nicht teilweise entpackt.
    UnsafeEntry(String),
    /// Der gewuenschte Ordnername taugt nicht als Verzeichnisname.
    InvalidFolder(String),
    /// Im ZIP steht nichts Verwertbares (keine Dateien, keine `.toc`).
    EmptyArchive,
    /// Erwarteter Ordner oder erwartete Datei fehlt.
    NotFound(String),
    /// Beim Wiederherstellen ist das Ziel schon belegt.
    AlreadyInstalled(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io(e) => write!(f, "Dateisystem: {e}"),
            InstallError::Zip(e) => write!(f, "ZIP: {e}"),
            InstallError::Toc(e) => write!(f, "TOC: {e}"),
            InstallError::NoTarget => write!(f, "kein Ziel-Client gewaehlt"),
            InstallError::UnsafeEntry(p) => write!(f, "unsicherer Pfad im ZIP: {p}"),
            InstallError::InvalidFolder(p) => write!(f, "untauglicher Ordnername: {p}"),
            InstallError::EmptyArchive => write!(f, "ZIP enthaelt kein Addon"),
            InstallError::NotFound(p) => write!(f, "nicht gefunden: {p}"),
            InstallError::AlreadyInstalled(p) => write!(f, "Ziel ist belegt: {p}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

impl From<zip::result::ZipError> for InstallError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            // Ein E/A-Fehler bleibt ein E/A-Fehler, damit der Aufrufer ihn
            // (Platte voll, Rechte) von einem kaputten Archiv unterscheiden kann.
            zip::result::ZipError::Io(io) => InstallError::Io(io),
            other => InstallError::Zip(other.to_string()),
        }
    }
}

/// Wo Sicherungen und Papierkorb liegen.
///
/// Eigener Typ statt globaler Konstante, damit Tests und ein abweichendes
/// `XDG_DATA_HOME` (Flatpak) denselben Code benutzen koennen, ohne dass eine
/// Funktion stillschweigend ins echte Home schreibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub backup_dir: PathBuf,
    pub trash_dir: PathBuf,
}

impl Paths {
    /// Vorgabe aus der Umgebung: `$XDG_DATA_HOME/hexwerk` bzw.
    /// `~/.local/share/hexwerk`.
    pub fn system() -> Self {
        Self::at(data_root())
    }

    /// Feste Datenwurzel, z. B. fuer Tests.
    pub fn at(data_root: impl Into<PathBuf>) -> Self {
        let base = data_root.into();
        Self {
            backup_dir: base.join("backup"),
            trash_dir: base.join("trash"),
        }
    }
}

/// Datenwurzel `~/.local/share/hexwerk` (bzw. `$XDG_DATA_HOME/hexwerk`).
pub fn data_root() -> PathBuf {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(".local").join("share"),
    };
    base.join("hexwerk")
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Zeitstempel `20261030T153012Z` (UTC) fuer Papierkorb und Sicherungen.
///
/// Von Hand statt mit `chrono`/`time`: ein Datumsformat ist die einzige
/// Kalenderrechnung, die dieses Paket braucht.
pub fn timestamp_now() -> String {
    format_stamp(SystemTime::now())
}

fn format_stamp(now: SystemTime) -> String {
    // Vor 1970 (oder eine Uhr, die rueckwaerts laeuft) wird als 0 behandelt -
    // ein falscher Stempel ist harmlos, ein Panic in der Installation nicht.
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Tage seit 1970-01-01 in (Jahr, Monat, Tag) - Howard Hinnants `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // 0..=146096
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // 0..=399
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if m <= 2 {
        y += 1;
    }
    (y, m, d as u32)
}

/// Kurzer Zufallsteil fuer Arbeitsordner.
///
/// Ohne `rand`: Nanosekunden, Prozess-ID und ein Zaehler unterscheiden sich
/// selbst bei parallelen Tests, und der Aufrufer versucht es ohnehin mehrfach.
/// Belegte Bytes eines Ordners, rekursiv. Symbolische Verweise zählen nicht,
/// sonst läuft ein Verweis auf sich selbst endlos.
pub(crate) fn folder_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        match entry.metadata() {
            Ok(md) if md.is_dir() => total += folder_size(&entry.path()),
            Ok(md) if md.file_type().is_symlink() => {}
            Ok(md) => total += md.len(),
            Err(_) => {}
        }
    }
    total
}

pub(crate) fn unique_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = u64::from(std::process::id());
    format!(
        "{:016x}",
        nanos ^ (pid << 32) ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    )
}

/// Freier Dateiname `<dir>/<stem>.<ext>`, notfalls mit `-2`, `-3`, …
///
/// Wird fuer Sicherungen und Papierkorb-Eintraege gebraucht: zweimal dasselbe
/// Addon am selben Tag darf die aeltere Sicherung nicht ueberschreiben.
pub(crate) fn unique_path(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    let name = |stem: &str, suffix: &str| match ext {
        Some(ext) => dir.join(format!("{stem}{suffix}.{ext}")),
        None => dir.join(format!("{stem}{suffix}")),
    };
    let first = name(stem, "");
    if !first.exists() {
        return first;
    }
    for n in 2..1_000u32 {
        let candidate = name(stem, &format!("-{n}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    name(stem, &format!("-{}", unique_token()))
}

#[cfg(test)]
mod testutil {
    //! Kleine Helfer fuer die Tests: ZIPs im Speicher bauen, Zielordner anlegen.
    //!
    //! Alles landet unter `CARGO_TARGET_TMPDIR`, damit Tests nie das echte Home
    //! anfassen - `XDG_DATA_HOME` wird nicht verbogen, weil das bei parallelen
    //! Tests prozessweit waere.

    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    use zip::write::SimpleFileOptions;

    /// Ein Eintrag im Test-ZIP.
    pub struct FakeEntry {
        pub name: String,
        pub data: Vec<u8>,
        pub mode: Option<u32>,
    }

    impl FakeEntry {
        pub fn text(name: &str, text: &str) -> Self {
            Self {
                name: name.to_string(),
                data: text.as_bytes().to_vec(),
                mode: None,
            }
        }

        /// Eintrag mit Unix-Modus, z. B. `0o120_777` fuer einen Symlink.
        ///
        /// `unix_permissions` der `zip`-Crate maskiert auf `0o777` und kann
        /// deshalb keinen Dateityp ausdruecken; der Modus muss ueber die
        /// externen Attribute in die oberen 16 Bit.
        pub fn with_mode(name: &str, data: &str, mode: u32) -> Self {
            Self {
                name: name.to_string(),
                data: data.as_bytes().to_vec(),
                mode: Some(mode),
            }
        }
    }

    /// Baut ein ZIP im Speicher.
    pub fn zip_of(entries: &[FakeEntry]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for entry in entries {
            let mut options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            if let Some(mode) = entry.mode {
                options = options
                    .system(zip::System::Unix)
                    .external_attributes(mode << 16);
            }
            writer.start_file(entry.name.clone(), options).unwrap();
            writer.write_all(&entry.data).unwrap();
        }
        let cursor = writer.finish().unwrap();
        cursor.into_inner()
    }

    /// Frischer, eindeutiger Zielordner unter `CARGO_TARGET_TMPDIR`.
    ///
    /// Die Variable setzt Cargo nur fuer Integrations-Tests zur Laufzeit; fuer
    /// Unit-Tests faellt der Pfad auf `<workspace>/target/tmp` zurueck, damit
    /// nichts im echten Home landet.
    pub fn temp_dir(tag: &str) -> PathBuf {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("CARGO_TARGET_TMPDIR").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp"));
        let path = base.join(format!("{tag}-{}", crate::unique_token()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeitstempel_ist_lesbar() {
        // Feste Sekunde: prueft die Kalenderrechnung, nicht die Uhr.
        let stamp = format_stamp(UNIX_EPOCH + std::time::Duration::from_secs(1_793_374_212));
        assert_eq!(stamp, "20261030T153012Z");
    }

    #[test]
    fn zivildatum_stimmt_an_randtagen() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // Schaltjahr
        assert_eq!(civil_from_days(20_727), (2026, 10, 1));
    }

    #[test]
    fn ungueltiger_ordnername_wird_abgelehnt() {
        assert!(zip_install::sanitize_folder("../raus").is_err());
        assert!(zip_install::sanitize_folder("a/b").is_err());
        assert!(zip_install::sanitize_folder(".versteckt").is_err());
        assert!(zip_install::sanitize_folder("Ok").is_ok());
    }
}
