//! Adressen der Quellen — mit optionaler Ueberschreibung aus den Einstellungen.
//!
//! Jede Quelle hat eine feste Basisadresse. Wer sie aendern will (eigener
//! Proxy, Spiegel, Testserver), traegt sie im Reiter „Einstellungen" ein; sie
//! landet in `~/.config/hexwerk/settings.json` unter `"urls"`. Diese Datei
//! liest hier dieselbe Stelle, damit die Oberflaeche nicht zwei Orte pflegen
//! muss.
//!
//! Regel: fehlt der Eintrag oder ist er leer, gilt die Vorgabe. Eine falsch
//! geschriebene Adresse fuehrt zu einem Netzfehler mit klarem Text, nicht zu
//! einem Absturz.

use crate::Source;

/// Vorgabe je Quelle.
pub fn default_base(source: Source) -> &'static str {
    match source {
        Source::CurseForge => "https://api.curseforge.com/v1",
        Source::GitHub => "https://api.github.com",
        Source::Tukui => "https://api.tukui.org/v1",
        Source::WowInterface => "https://api.mmoui.com",
        Source::Wago => "https://addons.wago.io/api",
        Source::Local => "",
    }
}

/// Kennung der Quelle in der Einstellungsdatei.
pub fn key_of(source: Source) -> &'static str {
    match source {
        Source::CurseForge => "curseforge",
        Source::GitHub => "github",
        Source::Tukui => "tukui",
        Source::WowInterface => "wowi",
        Source::Wago => "wago",
        Source::Local => "local",
    }
}

/// Basisadresse zum Arbeiten: Einstellung, sonst Vorgabe.
pub fn base(source: Source) -> String {
    let trimmed = override_for(key_of(source)).unwrap_or_default();
    if trimmed.trim().is_empty() {
        default_base(source).to_string()
    } else {
        trimmed.trim().trim_end_matches('/').to_string()
    }
}

/// Der eingetragene Wert (auch leer), `None` wenn kein Eintrag existiert.
fn override_for(key: &str) -> Option<String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    let text = std::fs::read_to_string(base.join("hexwerk").join("settings.json")).ok()?;
    let at = text.find("\"urls\"")?;
    let rest = &text[at..];
    let open = rest.find('{')?;
    let close = rest[open..].find('}')? + open;
    for pair in rest[open + 1..close].split(',') {
        let mut parts = pair.splitn(2, ':');
        let name = parts.next()?.trim().trim_matches('"').trim();
        let value = parts.next()?.trim().trim_matches('"').trim();
        if name == key {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorgaben_sind_gesetzt() {
        assert_eq!(base(Source::CurseForge), "https://api.curseforge.com/v1");
        assert_eq!(base(Source::GitHub), "https://api.github.com");
        assert!(!base(Source::Tukui).is_empty());
    }

    #[test]
    fn kennungen_passen_zu_den_einstellungen() {
        assert_eq!(key_of(Source::WowInterface), "wowi");
        assert_eq!(key_of(Source::CurseForge), "curseforge");
    }
}
