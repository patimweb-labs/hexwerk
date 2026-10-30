//! Client-Varianten von World of Warcraft und ihre Ordner im Spielverzeichnis.

use std::path::PathBuf;

/// Eine Client-Variante. Unbekannte Ordner (`_irgendwas_`) bleiben erhalten,
/// statt verworfen zu werden - Blizzard legt regelmaessig neue an
/// (gemessen 2026-10-30: `_forever_`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flavor {
    Retail,
    Classic,
    ClassicEra,
    Anniversary,
    Forever,
    RetailPtr,
    ClassicPtr,
    Other(String),
}

impl Flavor {
    /// Ordner-Name im Spielverzeichnis, z. B. `_classic_era_`.
    pub fn dir_name(&self) -> String {
        match self {
            Flavor::Retail => "_retail_".into(),
            Flavor::Classic => "_classic_".into(),
            Flavor::ClassicEra => "_classic_era_".into(),
            Flavor::Anniversary => "_anniversary_".into(),
            Flavor::Forever => "_forever_".into(),
            Flavor::RetailPtr => "_ptr_".into(),
            Flavor::ClassicPtr => "_classic_ptr_".into(),
            Flavor::Other(s) => s.clone(),
        }
    }

    pub fn from_dir_name(name: &str) -> Self {
        match name {
            "_retail_" => Flavor::Retail,
            "_classic_" => Flavor::Classic,
            "_classic_era_" => Flavor::ClassicEra,
            "_anniversary_" => Flavor::Anniversary,
            "_forever_" => Flavor::Forever,
            "_ptr_" => Flavor::RetailPtr,
            "_classic_ptr_" => Flavor::ClassicPtr,
            other => Flavor::Other(other.to_string()),
        }
    }

    /// Anzeigename fuer die Oberflaeche.
    pub fn label(&self) -> &str {
        match self {
            Flavor::Retail => "Retail",
            Flavor::Classic => "Classic",
            Flavor::ClassicEra => "Classic Era",
            Flavor::Anniversary => "Anniversary",
            Flavor::Forever => "Forever",
            Flavor::RetailPtr => "Retail PTR",
            Flavor::ClassicPtr => "Classic PTR",
            Flavor::Other(s) => s,
        }
    }
}

/// Ein gefundener Client-Ordner mit seinem Addon-Verzeichnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlavorDir {
    pub flavor: Flavor,
    pub dir: PathBuf,
    pub addons_dir: PathBuf,
    /// `## Interface` aller `.toc` im Ordner, entdoppelt und sortiert.
    /// Gemessen: mehrere Werte je Addon (`11509, 20506, 16001`).
    pub interface_numbers: Vec<u32>,
    /// Spielversion aus `.build.info`, kurz (`11.2.0`, `1.15.9`).
    ///
    /// Getrennt von `interface_numbers`, weil beides verschiedene Dinge sagt:
    /// die Version gilt fuer den Client, die Interface-Nummern stammen aus den
    /// `.toc` der Addons und koennen veraltet sein. `None`, wenn der Ordner
    /// keine `.flavor.info`/`.build.info` hat (z. B. ein Testbaum).
    pub game_version: Option<String>,
}

impl FlavorDir {
    /// Passt dieser Client zu einer Interface-Nummer aus einem Addon?
    pub fn accepts(&self, interface_number: u32) -> bool {
        self.interface_numbers.is_empty() || self.interface_numbers.contains(&interface_number)
    }

    /// Hoechste bekannte Interface-Nummer - die, auf der der Client laeuft.
    pub fn interface_number(&self) -> Option<u32> {
        self.interface_numbers.last().copied()
    }
}
