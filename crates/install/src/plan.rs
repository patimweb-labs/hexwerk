//! Was soll wohin. Wird von Arbeitspaket D gefuellt.

use std::path::PathBuf;

use hexwerk::{Client, Flavor};
use hexwerk_sources::Release;

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub client: Client,
    pub flavor: Flavor,
    pub release: Release,
    /// Ordnername im Addon-Verzeichnis; leer = aus dem ZIP ableiten.
    pub folder: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub folder: PathBuf,
    pub version: Option<String>,
}

pub fn apply(_req: &InstallRequest) -> Result<Installed, super::InstallError> {
    Err(super::InstallError::NoTarget)
}
