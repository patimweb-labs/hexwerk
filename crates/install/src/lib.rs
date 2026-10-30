//! Addons installieren, aktualisieren, sichern und entfernen.

pub mod backup;
pub mod plan;
pub mod trash;
pub mod zip_install;

pub use plan::{apply, InstallRequest, Installed};

/// Fehler beim Installieren.
#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Zip(String),
    Toc(String),
    NoTarget,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io(e) => write!(f, "Dateisystem: {e}"),
            InstallError::Zip(e) => write!(f, "ZIP: {e}"),
            InstallError::Toc(e) => write!(f, "TOC: {e}"),
            InstallError::NoTarget => write!(f, "kein Ziel-Client gewaehlt"),
        }
    }
}

impl std::error::Error for InstallError {}
