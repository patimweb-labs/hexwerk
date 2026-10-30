//! Datenmodell der Oberflaeche und der Weg vom Dateisystem in den Hauptthread.
//!
//! Der Scan liest Wine-Praefixe und `.toc`-Dateien; das dauert auf einem
//! gefuellten `Interface/AddOns` (mehrere tausend Dateien) leicht Sekunden.
//! Deshalb laeuft `hexwerk::discover` in einem eigenen Thread und schickt das
//! Ergebnis ueber einen Kanal zurueck. Der Hauptthread fasst dabei keine
//! Festplatte an - nur so bleibt das Fenster waehrend des Scans bedienbar.
//!
//! Was hier liegt, ist bewusst *abgeschrieben* vom Kern: die Oberflaeche haelt
//! eigene, `Clone`-faehige Strukturen, damit sie Daten zwischen Bildschirmen
//! weiterreichen kann, ohne den Kern zu zwingen, teuer zu rechnen.

use std::path::PathBuf;
use std::time::SystemTime;

use hexwerk::{Client, Discovery, Flavor, InstalledAddon, Note, Origin};

/// Eine Client-Variante (`_retail_`, `_classic_era_` …) mit dem, was beim
/// Erfassen sowieso gelesen wurde.
pub struct FlavorData {
    pub flavor: Flavor,
    pub dir_name: String,
    pub addons_dir: PathBuf,
    pub interface_numbers: Vec<u32>,
    /// Spielversion dieses Client-Ordners aus `.build.info` (`1.15.9`), sofern
    /// vorhanden. Wird im Katalog als Versionsfilter angeboten.
    pub game_version: Option<String>,
    /// Einmal eingeordnet: die Tabelle zeigt diese Liste, ohne erneut zu lesen.
    pub addons: Vec<InstalledAddon>,
    pub size_bytes: u64,
}

impl FlavorData {
    pub fn addon_count(&self) -> usize {
        self.addons.len()
    }

    /// Hoechste Interface-Nummer — die Fassung, auf der der Client laeuft.
    pub fn interface_number(&self) -> Option<u32> {
        self.interface_numbers.last().copied()
    }

    /// Was im Filter steht: `Classic Era 1.15.9`, sonst `Classic Era 11509`,
    /// sonst nur `Classic Era`.
    pub fn version_label(&self) -> String {
        let label = flavor_label(&self.flavor);
        match (&self.game_version, self.interface_number()) {
            (Some(version), _) => format!("{label} {version}"),
            (None, Some(number)) => format!("{label} {number}"),
            (None, None) => label,
        }
    }
}

/// Anzeigename einer Client-Variante.
///
/// Bekannte Varianten liefern ihren Namen aus dem Kern (`Classic Era`). Neue,
/// die es bei Erscheinen dieses Programms noch nicht gab, heissen im Kern
/// `Other("_classic_beta_")` — deren Ordnername wird hier lesbar gemacht
/// (`_classic_beta_` → `Classic Beta`), statt roh in der Oberflaeche zu stehen.
fn flavor_label(flavor: &Flavor) -> String {
    match flavor {
        Flavor::Other(raw) => raw
            .trim_matches('_')
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        other => other.label().to_string(),
    }
}

/// Ein gefundener Client-Ordner (ein Praefix plus Spielverzeichnis).
pub struct ClientData {
    pub id: String,
    pub label: String,
    pub origin: Origin,
    pub prefix: PathBuf,
    pub game_dir: PathBuf,
    pub flavors: Vec<FlavorData>,
}

impl ClientData {
    pub fn addon_count(&self) -> usize {
        self.flavors.iter().map(|f| f.addon_count()).sum()
    }
}

/// Alles, was ein Scan hervorgebracht hat.
pub struct ScanData {
    pub clients: Vec<ClientData>,
    pub notes: Vec<Note>,
    pub finished: SystemTime,
}

impl ScanData {
    pub fn addon_count(&self) -> usize {
        self.clients.iter().map(|c| c.addon_count()).sum()
    }
}

/// Was der Arbeits-Thread zur Oberflaeche schickt.
pub enum ScanMsg {
    Done(ScanData),
}

/// Startet den Scan-Thread. Der Kanal wird geschlossen, indem der Thread endet;
/// die UI-Seite liest, bis `recv` mit einem Fehler zurueckkommt.
pub fn spawn_scan(tx: async_channel::Sender<ScanMsg>) {
    std::thread::Builder::new()
        .name("hexwerk-scan".to_string())
        .spawn(move || {
            let data = scan_once();
            // Empfaenger weg heisst: Fenster zu. Dann ist der Aufwand egal.
            let _ = tx.send_blocking(ScanMsg::Done(data));
        })
        // Ein nicht startbarer Thread ist ein echter Fehler, aber keinen Panic
        // wert: die UI bleibt dann im "laeuft"-Zustand und der Knopf hilft.
        .ok();
}

/// Der eigentliche Lauf. Nur hier wird gelesen.
fn scan_once() -> ScanData {
    let extra: Vec<PathBuf> = Vec::new();
    let Discovery { clients, notes } = hexwerk::discover(&extra);
    ScanData {
        clients: clients.into_iter().map(prepare_client).collect(),
        notes,
        finished: SystemTime::now(),
    }
}

fn prepare_client(client: Client) -> ClientData {
    let Client {
        id,
        origin,
        label,
        prefix,
        game_dir,
        flavors,
    } = client;
    ClientData {
        id,
        label,
        origin,
        prefix,
        game_dir,
        flavors: flavors
            .into_iter()
            .map(|flavor_dir| {
                let addons = hexwerk::scan_addons(&flavor_dir.addons_dir);
                let size_bytes = addons.iter().map(|a| a.size_bytes).sum();
                FlavorData {
                    dir_name: flavor_dir.flavor.dir_name(),
                    flavor: flavor_dir.flavor,
                    addons_dir: flavor_dir.addons_dir,
                    interface_numbers: flavor_dir.interface_numbers,
                    game_version: flavor_dir.game_version,
                    addons,
                    size_bytes,
                }
            })
            .collect(),
    }
}

/// Deutsche Beschriftung eines Notiz-Status und die CSS-Klasse, die ihn
/// einfaerbt. Beides an einer Stelle, damit Liste und Zaehler nicht
/// auseinanderlaufen.
pub fn note_mark(status: hexwerk::NoteStatus) -> (&'static str, &'static str) {
    match status {
        hexwerk::NoteStatus::Ok => ("✓", "sig"),
        hexwerk::NoteStatus::Skip => ("○", "off"),
        hexwerk::NoteStatus::Error => ("!", "bad"),
        hexwerk::NoteStatus::Running => ("›", "warn"),
    }
}

/// Quell-Kennungen aus einer `.toc`, als Paare (Anzeigename, Kennung). Die
/// Reihenfolge ist die der Quellen im Plan; sie entscheidet auch, welche Quelle
/// in der Tabelle als "die" Quelle gilt.
pub fn source_rows(addon: &InstalledAddon) -> Vec<(String, String)> {
    let Some(toc) = addon.toc.as_ref() else {
        return Vec::new();
    };
    let ids = &toc.source_ids;
    let mut rows = Vec::new();
    if let Some(v) = &ids.github {
        rows.push(("GitHub".to_string(), v.clone()));
    }
    if let Some(v) = &ids.tukui {
        rows.push(("Tukui".to_string(), v.clone()));
    }
    if let Some(v) = &ids.wowinterface {
        rows.push(("WoWInterface".to_string(), v.clone()));
    }
    if let Some(v) = &ids.wago {
        rows.push(("Wago".to_string(), v.clone()));
    }
    if let Some(v) = &ids.curseforge {
        rows.push(("CurseForge".to_string(), v.clone()));
    }
    // Der Kern setzt `github`, wenn die Website auf github.com zeigt. Dann
    // waere eine zweite Zeile "Website" dieselbe Angabe zweimal.
    if let Some(v) = &ids.website {
        let already = rows.iter().any(|(name, id)| name == "GitHub" && id == v);
        if !already {
            rows.push(("Website".to_string(), v.clone()));
        }
    }
    rows
}

/// Beschreibungstexte aus den `## Notes-<sprache>`-Feldern der `.toc`.
/// Deutsch zuerst, weil die Oberflaeche deutsch ist; danach die Reihenfolge,
/// die der Kern geliefert hat.
pub fn note_texts(addon: &InstalledAddon) -> Vec<(String, String)> {
    let Some(toc) = addon.toc.as_ref() else {
        return Vec::new();
    };
    let mut rows: Vec<(String, String)> = toc
        .notes
        .iter()
        .map(|(lang, text)| (lang.clone(), text.clone()))
        .collect();
    rows.sort_by_key(|(lang, _)| {
        let rank = match lang.to_ascii_lowercase().as_str() {
            "dede" => 0,
            "enus" => 1,
            _ => 2,
        };
        (rank, lang.clone())
    });
    rows
}

/// Sprachkuerzel aus der `.toc` ansehnlich machen. Der Kern legt die Schluessel
/// klein ab (`## Notes-deDE` wird zu `dede`); angezeigt wird die Schreibweise,
/// die auch Blizzard benutzt.
pub fn locale_label(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "dede" => "deDE".to_string(),
        "enus" => "enUS".to_string(),
        "engb" => "enGB".to_string(),
        "eses" => "esES".to_string(),
        "esmx" => "esMX".to_string(),
        "frfr" => "frFR".to_string(),
        "itit" => "itIT".to_string(),
        "ptbr" => "ptBR".to_string(),
        "ruru" => "ruRU".to_string(),
        "kokr" => "koKR".to_string(),
        "zhcn" => "zhCN".to_string(),
        "zhtw" => "zhTW".to_string(),
        "" => "ohne Sprachangabe".to_string(),
        _ => key.to_string(),
    }
}
