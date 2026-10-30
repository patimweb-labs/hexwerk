//! Spielversion eines Client-Ordners aus den Dateien, die Blizzard selbst hinlegt.
//!
//! Warum das hier steht: `## Interface` in einer `.toc` ist die einzige Nummer,
//! nach der sich ein Addon einem Client zuordnen laesst - aber sie existiert nur,
//! wenn schon ein Addon im Ordner liegt. Ein frisch installierter Client hat
//! einen leeren `Interface/AddOns`-Ordner und damit keine einzige Nummer; ohne
//! diesen Umweg bliebe der Versionsfilter im Katalog leer, obwohl das Spiel
//! installiert ist.
//!
//! Gelesen werden zwei Dateien, beide Klartext mit `|`-Trennern:
//!
//! - `<spiel>/_retail_/.flavor.info` nennt das Produkt (`wow`, `wow_classic_era` …),
//! - `<spiel>/.build.info` ist eine Tabelle mit einer Zeile je Produkt, darunter
//!   die Spalte `Version` (`11.2.0.63003`, `1.15.9.70576`).
//!
//! Beide sind reine Textdateien ohne Registry und ohne Wine-Besonderheit; ein
//! fehlendes oder unbekanntes Format ergibt `None` und keine geratene Nummer.

use std::path::Path;

/// Produkt und Version eines Client-Ordners, z. B. `wow_classic_era` / `1.15.9.70576`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    pub product: String,
    /// Volle Fassung inklusive Build, wie sie in `.build.info` steht.
    pub version: String,
}

impl GameVersion {
    /// `## Interface`-Nummer, die zu dieser Fassung gehoert.
    ///
    /// Regel: `Haupt * 10000 + Neben * 100 + Patch`, also `1.15.9` → `11509`
    /// und `11.2.0` → `110200`. Der Build hinten dran zaehlt nicht mit.
    pub fn interface_number(&self) -> Option<u32> {
        interface_of(&self.version)
    }

    /// Kurzform ohne Build: `1.15.9`.
    pub fn short_version(&self) -> &str {
        short_of(&self.version)
    }
}

/// Liest `.flavor.info` und `.build.info` eines Client-Ordners.
///
/// `game_dir` ist der Ordner, in dem die `_retail_`-artigen Verzeichnisse
/// liegen (dort steht `.build.info`); `flavor_dir` ist eines davon.
pub fn read(game_dir: &Path, flavor_dir: &Path) -> Option<GameVersion> {
    let product = std::fs::read_to_string(flavor_dir.join(".flavor.info"))
        .ok()
        .and_then(|text| parse_flavor_info(&text))?;
    let build = std::fs::read_to_string(game_dir.join(".build.info")).ok()?;
    let version = parse_build_info(&build)
        .into_iter()
        .find(|(name, _)| *name == product)?
        .1;
    Some(GameVersion { product, version })
}

/// Produktname aus `.flavor.info`: erste Zeile ist die Kopfzeile
/// (`Product Flavor!STRING:0`), zweite die Antwort.
pub fn parse_flavor_info(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .nth(1)
        .map(|line| line.to_string())
}

/// Zeilen aus `.build.info` als `(Produkt, Version)`.
///
/// Die Kopfzeile nennt die Spalten als `Name!TYP:Groesse`; gesucht werden
/// `Product` und `Version`. Fehlt eine der beiden, ist die Datei nicht die, die
/// wir kennen - dann lieber nichts als ein falsches Paar.
pub fn parse_build_info(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let columns: Vec<String> = header
        .split('|')
        .map(|name| name.split('!').next().unwrap_or(name).trim().to_string())
        .collect();
    let (Some(product_at), Some(version_at)) = (
        columns.iter().position(|c| c == "Product"),
        columns.iter().position(|c| c == "Version"),
    ) else {
        return Vec::new();
    };
    lines
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('|').collect();
            let product = fields.get(product_at)?.trim();
            let version = fields.get(version_at)?.trim();
            if product.is_empty() || version.is_empty() {
                return None;
            }
            Some((product.to_string(), version.to_string()))
        })
        .collect()
}

/// `1.15.9.70576` → `11509`.
pub fn interface_of(version: &str) -> Option<u32> {
    let mut parts = version.split('.').map(|part| part.trim().parse::<u32>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().and_then(Result::ok).unwrap_or(0);
    let patch = parts.next().and_then(Result::ok).unwrap_or(0);
    if minor > 99 || patch > 99 {
        return None;
    }
    Some(major * 10_000 + minor * 100 + patch)
}

/// `1.15.9.70576` → `1.15.9`.
pub fn short_of(version: &str) -> &str {
    let mut seen = 0;
    for (at, ch) in version.char_indices() {
        if ch == '.' {
            seen += 1;
            if seen == 3 {
                return &version[..at];
            }
        }
    }
    version
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kopfzeile wie in einer echten `.build.info` (gekuerzt auf zwei Spalten,
    /// die Reihenfolge ist Absicht: die Spalten werden ueber die Namen gesucht).
    const BUILD: &str = "Tags!STRING:0|Version!STRING:0|Product!STRING:0\n\
Windows x86_64|1.15.9.70576|wow_classic_era\n\
Windows x86_64|11.2.0.63003|wow\n";

    #[test]
    fn build_info_liest_produkt_und_version() {
        let rows = parse_build_info(BUILD);
        assert_eq!(
            rows,
            vec![
                ("wow_classic_era".to_string(), "1.15.9.70576".to_string()),
                ("wow".to_string(), "11.2.0.63003".to_string()),
            ]
        );
    }

    #[test]
    fn unbekannte_kopfzeile_ergibt_nichts() {
        assert!(parse_build_info("irgendwas|ohne|spalten\n1|2|3\n").is_empty());
    }

    #[test]
    fn interface_nummer_aus_version() {
        assert_eq!(interface_of("1.15.9.70576"), Some(11509));
        assert_eq!(interface_of("11.2.0.63003"), Some(110200));
        assert_eq!(interface_of("1.60.1.70685"), Some(16001));
        assert_eq!(interface_of("2.17.6"), Some(21706));
        // Unsinn wird nicht geraten.
        assert_eq!(interface_of(""), None);
        assert_eq!(interface_of("keine.1"), None);
    }

    #[test]
    fn kurzform_ohne_build() {
        assert_eq!(short_of("1.15.9.70576"), "1.15.9");
        assert_eq!(short_of("11.2.0"), "11.2.0");
        assert_eq!(short_of("ohne"), "ohne");
    }

    #[test]
    fn flavor_info_liefert_zweite_zeile() {
        assert_eq!(
            parse_flavor_info("Product Flavor!STRING:0\nwow_classic_era\n"),
            Some("wow_classic_era".to_string())
        );
        assert_eq!(parse_flavor_info(""), None);
    }

    #[test]
    fn read_verbindet_beide_dateien() {
        let base = std::env::temp_dir().join(format!("hexwerk-gv-{}", std::process::id()));
        let game = base.join("World of Warcraft");
        let era = game.join("_classic_era_");
        std::fs::create_dir_all(&era).unwrap();
        std::fs::write(game.join(".build.info"), BUILD).unwrap();
        std::fs::write(
            era.join(".flavor.info"),
            "Product Flavor!STRING:0\nwow_classic_era\n",
        )
        .unwrap();
        let found = read(&game, &era).expect("Version");
        assert_eq!(found.product, "wow_classic_era");
        assert_eq!(found.short_version(), "1.15.9");
        assert_eq!(found.interface_number(), Some(11509));
        std::fs::remove_dir_all(&base).ok();
    }
}
