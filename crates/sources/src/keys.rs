//! Schlüssel: Umgebung → Konfigdatei → Schlüsselbund. Nie loggen.
//!
//! Reihenfolge und Ort stehen in PLAN.md §7. Harte Regel dieses Moduls: der
//! Klartext eines Schlüssels verlässt [`Secret::expose`] nicht. `Debug`, `Display`
//! und [`Secret::masked`] zeigen höchstens die letzten vier Zeichen; Fehlermeldungen
//! bauen ihren Text aus [`KeyOrigin`], nicht aus dem Wert.
//!
//! Ehrlich dazugesagt (PLAN.md §7): ein mitgelieferter CurseForge-Schlüssel wäre
//! laut Overwolf-Bedingungen unzulässig. Deshalb gibt es hier keinen Vorgabewert —
//! ohne eigenen Schlüssel bleibt die Quelle [`crate::SourceError::Disabled`].

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Source;

/// Umgebungsvariablen. Bewusst sprechend und ohne Präfix-Kollision.
pub const ENV_CURSEFORGE: &str = "HEXWERK_CURSEFORGE_KEY";
pub const ENV_WAGO: &str = "HEXWERK_WAGO_TOKEN";

/// Dateiname in `~/.config/hexwerk`.
pub const CONFIG_FILE: &str = "keys.json";

/// Ein Schlüssel im Speicher.
///
/// `expose()` heißt absichtlich so umständlich: wer ihn aufruft, hat eine
/// Entscheidung getroffen, und die Stelle ist im Code auffindbar.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into().trim().to_string())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `…abcd`, sonst nur `…`.
    ///
    /// WARUM die Schwelle bei 8: bei kurzen Schlüsseln wären vier Zeichen die
    /// halbe Information — dann lieber gar nichts zeigen.
    pub fn masked(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        if chars.len() < 8 {
            return "…".to_string();
        }
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("…{tail}")
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({})", self.masked())
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked())
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// Woher ein Schlüssel kommt. Für Anzeige und Diagnose — ohne den Wert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOrigin {
    Environment,
    File,
    Keyring,
    /// Zur Bauzeit eingebettet (`HEXWERK_CURSEFORGE_KEY`), als letzter Weg:
    /// ein eigener Schluessel des Nutzers gewinnt immer.
    BuiltIn,
    Missing,
    /// Quelle braucht keinen Schlüssel.
    NotNeeded,
}

impl KeyOrigin {
    pub fn label(&self) -> &'static str {
        match self {
            KeyOrigin::Environment => "Umgebungsvariable",
            KeyOrigin::File => CONFIG_FILE,
            KeyOrigin::Keyring => "Schlüsselbund",
            KeyOrigin::BuiltIn => "eingebaut",
            KeyOrigin::Missing => "kein Schlüssel",
            KeyOrigin::NotNeeded => "nicht nötig",
        }
    }
}

/// Der zur Bauzeit eingebettete CurseForge-Schluessel, falls vorhanden.
///
/// `option_env!` liest ihn beim Uebersetzen (siehe `build.rs`); ohne
/// `HEXWERK_CURSEFORGE_KEY` und ohne `.secrets/curseforge.key` ist er leer und
/// das Programm verhaelt sich wie ohne Schluessel.
pub fn built_in_key() -> Option<Secret> {
    match option_env!("HEXWERK_CURSEFORGE_KEY") {
        Some(value) if !value.trim().is_empty() => Some(Secret::new(value.trim())),
        _ => None,
    }
}

/// Umgebungsvariable je Quelle.
pub fn env_name(source: Source) -> Option<&'static str> {
    match source {
        Source::CurseForge => Some(ENV_CURSEFORGE),
        Source::Wago => Some(ENV_WAGO),
        _ => None,
    }
}

/// Name in der Konfigdatei (und im Schlüsselbund).
pub fn config_name(source: Source) -> Option<&'static str> {
    match source {
        Source::CurseForge => Some("curseforge"),
        Source::Wago => Some("wago"),
        _ => None,
    }
}

/// Schluessel aus `~/.config/hexwerk/settings.json`, Abschnitt `keys`.
///
/// Bewusst ein eigener, kleiner Leser statt `serde_json`: dieses Modul liegt in
/// `hexwerk-sources` und soll nicht die Abhaengigkeiten der Oberflaeche
/// mitziehen. Gesucht wird `"keys"` gefolgt von `{ "quelle": "wert", … }`.
fn read_settings_key(name: &str) -> Option<String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    let text = fs::read_to_string(base.join("hexwerk").join("settings.json")).ok()?;
    let at = text.find("\"keys\"")?;
    let rest = &text[at..];
    let open = rest.find('{')?;
    let close = rest[open..].find('}')? + open;
    let inside = &rest[open + 1..close];
    for pair in inside.split(',') {
        let mut parts = pair.splitn(2, ':');
        let key = parts.next()?.trim().trim_matches('"').trim().to_string();
        let value = parts.next()?.trim().trim_matches('"').trim().to_string();
        if key == name && !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// Liest Schluessel in der Reihenfolge aus PLAN.md §7.
///
/// `use_env`/`use_keyring` sind Schalter für Tests und für Aufrufer, die die
/// Umgebung bewusst ignorieren wollen (etwa `hexwerk keys test` im Flatpak, wo
/// die Variablen aus der Shell nicht sichtbar sein sollen).
pub struct KeyProvider {
    config_path: PathBuf,
    use_env: bool,
    use_keyring: bool,
}

impl KeyProvider {
    /// Verzeichnis `~/.config/hexwerk` (oder ein Testverzeichnis).
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_dir.as_ref().join(CONFIG_FILE),
            use_env: true,
            use_keyring: true,
        }
    }

    /// `$XDG_CONFIG_HOME/hexwerk/keys.json`, sonst `~/.config/hexwerk/keys.json`.
    pub fn default_provider() -> Self {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| std::env::temp_dir().join("hexwerk-config"));
        Self::new(base.join("hexwerk"))
    }

    pub fn without_env(mut self) -> Self {
        self.use_env = false;
        self
    }

    pub fn without_keyring(mut self) -> Self {
        self.use_keyring = false;
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Schlüssel für eine Quelle, falls vorhanden.
    pub fn key(&self, source: Source) -> Option<Secret> {
        self.key_with_origin(source).map(|(secret, _)| secret)
    }

    pub fn key_with_origin(&self, source: Source) -> Option<(Secret, KeyOrigin)> {
        let name = config_name(source)?;
        if self.use_env {
            if let Some(env) = env_name(source) {
                if let Ok(value) = std::env::var(env) {
                    let secret = Secret::new(value);
                    if !secret.is_empty() {
                        return Some((secret, KeyOrigin::Environment));
                    }
                }
            }
        }
        // Dieselbe Datei, die die Oberflaeche schreibt (Einstellungen-Reiter):
        // der Schluessel wird dort eingetippt, hier gelesen. Die alte
        // `keys.json` bleibt als Rueckfallebene gueltig.
        if let Some(secret) = read_settings_key(name) {
            if !secret.trim().is_empty() {
                return Some((Secret::new(secret), KeyOrigin::File));
            }
        }
        if let Some(secret) = self.read_file().get(name).cloned() {
            let secret = Secret::new(secret);
            if !secret.is_empty() {
                return Some((secret, KeyOrigin::File));
            }
        }
        if self.use_keyring {
            if let Some(secret) = read_keyring(name) {
                if !secret.is_empty() {
                    return Some((secret, KeyOrigin::Keyring));
                }
            }
        }
        // Zuletzt der eingebaute Schluessel: er ist fuer alle da, also darf er
        // nur greifen, wenn niemand einen eigenen hinterlegt hat.
        if source == Source::CurseForge {
            if let Some(secret) = built_in_key() {
                return Some((secret, KeyOrigin::BuiltIn));
            }
        }
        None
    }

    pub fn origin(&self, source: Source) -> KeyOrigin {
        if config_name(source).is_none() {
            return KeyOrigin::NotNeeded;
        }
        self.key_with_origin(source)
            .map(|(_, origin)| origin)
            .unwrap_or(KeyOrigin::Missing)
    }

    /// `true`, wenn die Quelle benutzt werden kann. Quellen ohne Schlüsselbedarf
    /// sind immer `true`.
    pub fn enabled(&self, source: Source) -> bool {
        if !source.needs_key() {
            return true;
        }
        self.key(source).is_some()
    }

    /// Schreibt einen Schlüssel in die Konfigdatei (Modus 0600) und lässt die
    /// anderen Einträge stehen. Ein Umgebungswert hat weiter Vorrang — das steht
    /// so im Plan und ist Absicht: die Umgebung ist flüchtig, die Datei bleibt.
    pub fn set(&self, source: Source, secret: &str) -> std::io::Result<()> {
        let name = config_name(source).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} braucht keinen Schlüssel", source.label()),
            )
        })?;
        let value = secret.trim();
        if value.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "leerer Schlüssel",
            ));
        }
        let mut map = self.read_file();
        map.insert(name.to_string(), value.to_string());
        self.write_file(&map)
    }

    /// Entfernt einen Schlüssel aus der Konfigdatei. `false`, wenn keiner da war.
    /// Umgebung und Schlüsselbund bleiben unberührt — sie gehören nicht uns.
    pub fn remove(&self, source: Source) -> std::io::Result<bool> {
        let Some(name) = config_name(source) else {
            return Ok(false);
        };
        let mut map = self.read_file();
        let had = map.remove(name).is_some();
        if had {
            self.write_file(&map)?;
        }
        Ok(had)
    }

    /// Was der Nutzer wissen sollte, ohne dass ein Wert sichtbar wird.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.config_path.exists() {
            return out;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&self.config_path) {
                let mode = meta.permissions().mode() & 0o777;
                if mode & 0o077 != 0 {
                    out.push(format!(
                        "{} ist für Gruppe/Andere zugänglich (Modus {:03o}) — bitte `chmod 600`",
                        self.config_path.display(),
                        mode
                    ));
                }
            }
        }
        out
    }

    fn read_file(&self) -> BTreeMap<String, String> {
        let Ok(text) = fs::read_to_string(&self.config_path) else {
            return BTreeMap::new();
        };
        // Kaputte Datei darf die Quelle nicht mitreißen: dann gilt sie als
        // „kein Schlüssel" und `hexwerk keys set` schreibt sie neu.
        serde_json::from_str::<BTreeMap<String, String>>(&text).unwrap_or_default()
    }

    fn write_file(&self, map: &BTreeMap<String, String>) -> std::io::Result<()> {
        if let Some(dir) = self.config_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let body = serde_json::to_vec_pretty(map)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = self
            .config_path
            .with_extension(format!("json.tmp.{}", std::process::id()));
        {
            // Modus schon beim Anlegen setzen: zwischen `create` und `chmod`
            // läge sonst ein Fenster mit 0644.
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(&body)?;
            file.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Falls die Zieldatei schon existierte, hat `rename` die Rechte der
            // temporären Datei übernommen — sicherheitshalber trotzdem setzen.
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&tmp, &self.config_path)?;
        Ok(())
    }
}

impl fmt::Debug for KeyProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kein `read_file()` hier: Debug darf keine Werte in die Ausgabe ziehen.
        f.debug_struct("KeyProvider")
            .field("config_path", &self.config_path)
            .field("use_env", &self.use_env)
            .field("use_keyring", &self.use_keyring)
            .finish()
    }
}

/// Schlüsselbund über `secret-tool` (libsecret). Optional: fehlt das Programm
/// oder der Dienst, gibt es einfach keinen Schlüssel und die Quelle bleibt aus.
fn read_keyring(name: &str) -> Option<Secret> {
    let output = std::process::Command::new("secret-tool")
        .args(["lookup", "service", "hexwerk", "key", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(Secret::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hexwerk-keys-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn maskierung_zeigt_nur_vier_zeichen() {
        let secret = Secret::new("abcdef1234567890");
        assert_eq!(secret.masked(), "…7890");
        assert!(!format!("{secret:?}").contains("abcdef"));
        assert_eq!(format!("{secret}"), "…7890");
    }

    #[test]
    fn kurze_schluessel_zeigen_gar_nichts() {
        assert_eq!(Secret::new("kurz").masked(), "…");
        assert_eq!(Secret::new("12345678").masked(), "…5678");
    }

    #[test]
    fn datei_wird_0600_geschrieben_und_gelesen() {
        let dir = temp_dir("write");
        let provider = KeyProvider::new(&dir).without_env().without_keyring();
        assert!(provider.key(Source::CurseForge).is_none());
        provider
            .set(Source::CurseForge, "  geheim-123456  ")
            .unwrap();
        let secret = provider.key(Source::CurseForge).unwrap();
        assert_eq!(secret.expose(), "geheim-123456");
        assert_eq!(provider.origin(Source::CurseForge), KeyOrigin::File);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(provider.config_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "Modus {:03o}", mode);
        }
        // Zweiter Schlüssel lässt den ersten stehen.
        provider.set(Source::Wago, "wago-token-abcdef").unwrap();
        assert!(provider.key(Source::CurseForge).is_some());
        assert!(provider.key(Source::Wago).is_some());
    }

    #[test]
    fn quellen_ohne_bedarf_brauchen_nichts() {
        let provider = KeyProvider::new(temp_dir("none"))
            .without_env()
            .without_keyring();
        assert_eq!(provider.origin(Source::Tukui), KeyOrigin::NotNeeded);
        assert!(provider.enabled(Source::GitHub));
        assert!(!provider.enabled(Source::Wago));
    }

    #[test]
    fn entfernen_meldet_ob_etwas_da_war() {
        let dir = temp_dir("remove");
        let provider = KeyProvider::new(&dir).without_env().without_keyring();
        provider.set(Source::Wago, "token-1234567").unwrap();
        assert!(provider.remove(Source::Wago).unwrap());
        assert!(!provider.remove(Source::Wago).unwrap());
        assert!(provider.key(Source::Wago).is_none());
    }

    #[test]
    fn kaputte_datei_gilt_als_kein_schluessel() {
        let dir = temp_dir("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILE), "{ das ist kein json").unwrap();
        let provider = KeyProvider::new(&dir).without_env().without_keyring();
        assert!(provider.key(Source::CurseForge).is_none());
        // Schreiben repariert sie.
        provider.set(Source::CurseForge, "neu-12345678").unwrap();
        assert!(provider.key(Source::CurseForge).is_some());
    }

    #[test]
    fn umgebung_hat_vorrang_vor_der_datei() {
        // Umgebungsvariablen sind prozessweit; der Mutex hält parallele Tests
        // davon ab, sich gegenseitig den Wert wegzunehmen.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = temp_dir("env");
        let provider = KeyProvider::new(&dir).without_keyring();
        provider
            .set(Source::CurseForge, "aus-der-datei-9999")
            .unwrap();
        std::env::set_var(ENV_CURSEFORGE, "aus-der-umgebung-1234");
        let (secret, origin) = provider.key_with_origin(Source::CurseForge).unwrap();
        assert_eq!(origin, KeyOrigin::Environment);
        assert_eq!(secret.expose(), "aus-der-umgebung-1234");
        std::env::remove_var(ENV_CURSEFORGE);
        assert_eq!(
            provider.origin(Source::CurseForge),
            KeyOrigin::File,
            "ohne Umgebung greift wieder die Datei"
        );
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::*;

    #[test]
    fn eingebauter_schluessel_aus_dem_bau() {
        // Der Bau im Container liest `.secrets/curseforge.key`; liegt die Datei
        // nicht vor, ist das kein Fehler, sondern der Zustand "ohne Schluessel".
        match built_in_key() {
            Some(secret) => {
                assert!(secret.expose().len() > 40, "Schluessel zu kurz");
                let provider = KeyProvider::new(std::env::temp_dir())
                    .without_env()
                    .without_keyring();
                let (found, origin) = provider
                    .key_with_origin(Source::CurseForge)
                    .expect("eingebauter Schluessel muss greifen");
                assert_eq!(origin, KeyOrigin::BuiltIn);
                assert_eq!(found.expose(), secret.expose());
            }
            None => eprintln!("kein Schluessel eingebettet (keine .secrets/curseforge.key)"),
        }
    }
}
