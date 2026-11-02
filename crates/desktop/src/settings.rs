// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

//! Einstellungen: dieselbe Datei wie die GTK-Oberflaeche
//! (`~/.config/hexwerk/settings.json`), damit ein Wechsel der Oberflaeche
//! nichts zuruecksetzt.
//!
//! Gelesen wird mit `serde`, geschrieben als eingeruecktes JSON. Unbekannte
//! Felder bleiben erhalten (`flatten`), damit eine aeltere Fassung der
//! Anwendung die Datei nicht ausduennt.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn prioritaet() -> Vec<String> {
    ["curseforge", "wowi", "tukui", "github", "wago"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn sprache() -> String {
    "de".to_string()
}

fn thema() -> String {
    "dark".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Rangfolge der Quellen, wichtigste zuerst.
    #[serde(default = "prioritaet")]
    pub source_priority: Vec<String>,
    /// Sprachkuerzel: `de`, `en`, `es`, `fr`.
    #[serde(default = "sprache")]
    pub language: String,
    /// Leiste: `full` oder `compact`.
    #[serde(default)]
    pub nav: String,
    /// Schluessel je Quelle.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
    /// Basisadresse je Quelle; leer heisst: Vorgabe aus dem Code.
    #[serde(default)]
    pub urls: BTreeMap<String, String>,
    /// `dark`, `light` oder `mono`.
    #[serde(default = "thema")]
    pub theme: String,
    /// Alles, was diese Fassung nicht kennt, bleibt in der Datei.
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_priority: prioritaet(),
            language: sprache(),
            nav: String::new(),
            keys: BTreeMap::new(),
            urls: BTreeMap::new(),
            theme: thema(),
            rest: BTreeMap::new(),
        }
    }
}

pub fn pfad() -> Option<PathBuf> {
    let basis = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(basis.join("hexwerk").join("settings.json"))
}

impl Settings {
    /// Lesen; fehlt die Datei oder ist sie kaputt, gelten die Vorgaben.
    /// Eine kaputte Datei ist kein Grund, das Fenster nicht zu oeffnen.
    pub fn load() -> Self {
        let Some(pfad) = pfad() else { return Self::default() };
        let Ok(text) = std::fs::read_to_string(&pfad) else { return Self::default() };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(pfad) = pfad() else { return Err("kein Konfigurationsverzeichnis".into()) };
        if let Some(dir) = pfad.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&pfad, text + "\n").map_err(|e| e.to_string())
    }
}
