//! Was soll wohin: Auftrag, Zielordner, Ergebnis.
//!
//! [`apply`] ist der einzige Weg, an dem ein ZIP in ein Spielverzeichnis kommt.
//! Ziel ist immer das `addons_dir` genau der [`hexwerk::FlavorDir`], die im
//! Auftrag steht - ein Addon fuer "_retail_" und "_classic_" sind verschiedene
//! Builds, und ein Schreibzugriff in den falschen Ordner faellt im Spiel als
//! Ladezeitfehler auf, nicht als Installationsfehler (PLAN §13.5).

use std::path::PathBuf;

use hexwerk::{Client, Flavor};
use hexwerk_sources::Release;

use crate::{zip_install, InstallError, Paths};

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub client: Client,
    pub flavor: Flavor,
    pub release: Release,
    /// Ordnername im Addon-Verzeichnis; leer = aus dem ZIP ableiten.
    pub folder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Vollstaendiger Pfad des Ordners, der im Addon-Verzeichnis entstanden ist.
    pub folder: PathBuf,
    /// Version aus der `.toc`, falls das ZIP eine enthielt.
    pub version: Option<String>,
}

/// Installiert oder aktualisiert `data` im Client des Auftrags.
///
/// Sicherungen und Papierkorb landen unter `~/.local/share/hexwerk`.
pub fn apply(req: &InstallRequest, data: &[u8]) -> Result<Installed, InstallError> {
    apply_with(req, data, &Paths::system())
}

/// Wie [`apply`], aber mit waehlbaren Ablageorten (Tests, Flatpak-Pfade).
pub fn apply_with(
    req: &InstallRequest,
    data: &[u8],
    paths: &Paths,
) -> Result<Installed, InstallError> {
    // Nur die Variante, die im Auftrag steht: kein "alle Clients", kein
    // Rueckfall auf die erste gefundene.
    let flavor = req
        .client
        .flavors
        .iter()
        .find(|f| f.flavor == req.flavor)
        .ok_or(InstallError::NoTarget)?;

    // Ein Client kann erkannt sein, waehrend `Interface/AddOns` noch fehlt
    // (frisch angelegtes Praefix); dann legt die Installation es an.
    std::fs::create_dir_all(&flavor.addons_dir)?;

    zip_install::install_zip(data, &flavor.addons_dir, req.folder.as_deref(), paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{temp_dir, zip_of, FakeEntry};
    use hexwerk::{FlavorDir, Origin};

    fn client(base: &std::path::Path) -> Client {
        let game_dir = base.join("Wow");
        let retail = game_dir.join("_retail_/Interface/AddOns");
        let classic = game_dir.join("_classic_/Interface/AddOns");
        std::fs::create_dir_all(&retail).unwrap();
        std::fs::create_dir_all(&classic).unwrap();
        Client {
            id: "manuell:test".into(),
            origin: Origin::Manual,
            label: "Test".into(),
            prefix: base.join("pfx"),
            game_dir: game_dir.clone(),
            flavors: vec![
                FlavorDir {
                    flavor: Flavor::Retail,
                    dir: game_dir.join("_retail_"),
                    addons_dir: retail,
                    interface_numbers: vec![11509],
                },
                FlavorDir {
                    flavor: Flavor::Classic,
                    dir: game_dir.join("_classic_"),
                    addons_dir: classic,
                    interface_numbers: vec![20506],
                },
            ],
        }
    }

    fn request(base: &std::path::Path, flavor: Flavor) -> InstallRequest {
        InstallRequest {
            client: client(base),
            flavor,
            release: Release {
                source: hexwerk_sources::Source::GitHub,
                external_id: "1".into(),
                version: "1.0".into(),
                released_at: None,
                download_url: "https://example.invalid/x.zip".into(),
                size_bytes: None,
                interface_numbers: vec![11509],
            },
            folder: None,
        }
    }

    #[test]
    fn apply_installiert_nur_in_den_gewaehlten_client() {
        let base = temp_dir("plan-client");
        let paths = Paths::at(base.join("data"));
        let req = request(&base, Flavor::Retail);
        let data = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 3.1\n"),
            FakeEntry::text("Tidy/core.lua", ""),
        ]);

        let done = apply_with(&req, &data, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("3.1"));
        assert!(base
            .join("Wow/_retail_/Interface/AddOns/Tidy/core.lua")
            .is_file());
        // Der Classic-Client bekommt nichts - auch keinen leeren Ordner.
        assert!(!base.join("Wow/_classic_/Interface/AddOns/Tidy").exists());
    }

    #[test]
    fn apply_ohne_passende_variante_ist_ein_fehler() {
        let base = temp_dir("plan-ohne");
        let paths = Paths::at(base.join("data"));
        let mut req = request(&base, Flavor::Retail);
        req.flavor = Flavor::ClassicEra;

        let data = zip_of(&[FakeEntry::text("Tidy/Tidy.toc", "## Version: 3.1\n")]);

        assert!(matches!(
            apply_with(&req, &data, &paths),
            Err(InstallError::NoTarget)
        ));
    }

    #[test]
    fn update_ersetzt_und_sichert() {
        let base = temp_dir("plan-update");
        let paths = Paths::at(base.join("data"));
        let req = request(&base, Flavor::Retail);
        let addons = base.join("Wow/_retail_/Interface/AddOns");

        let alt = zip_of(&[FakeEntry::text("Tidy/Tidy.toc", "## Version: 1.0\n")]);
        apply_with(&req, &alt, &paths).unwrap();

        let neu = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 2.0\n"),
            FakeEntry::text("Tidy/neu.lua", ""),
        ]);
        let done = apply_with(&req, &neu, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("2.0"));
        assert!(addons.join("Tidy/neu.lua").is_file());
        let backups = crate::list_backups(&paths).unwrap();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn fehlendes_addons_verzeichnis_wird_angelegt() {
        let base = temp_dir("plan-anlegen");
        let paths = Paths::at(base.join("data"));
        let mut req = request(&base, Flavor::Retail);
        let fresh = base.join("frisch");
        req.client.flavors[0].addons_dir = fresh.join("Interface/AddOns");

        let data = zip_of(&[FakeEntry::text("Tidy/Tidy.toc", "## Version: 1\n")]);
        apply_with(&req, &data, &paths).unwrap();

        assert!(fresh.join("Interface/AddOns/Tidy/Tidy.toc").is_file());
    }
}
