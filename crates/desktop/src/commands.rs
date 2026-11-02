// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

//! Die Naht zwischen Fenster und Logik.
//!
//! Jeder Befehl hier ist eine Uebersetzung: Vue schickt etwas, die Logik-Crates
//! rechnen, das Ergebnis geht als Daten zurueck. In dieser Datei steht keine
//! Fachlogik — wenn hier gerechnet wird, gehoert das Rechnen in ein Crate.

use serde::Serialize;

/// Ein Addon so, wie die Oberflaeche es braucht.
#[derive(Serialize)]
pub struct AddonDto {
    pub folder: String,
    pub title: String,
    pub version: Option<String>,
    pub interface: Vec<u32>,
    pub size_bytes: u64,
}

/// Ein Spielordner (Einzelspieler, Classic, …) eines Clients.
#[derive(Serialize)]
pub struct FlavorDto {
    pub dir_name: String,
    pub label: String,
    pub addons_dir: String,
    pub interface: Vec<u32>,
    pub game_version: String,
    pub size_bytes: u64,
    pub addons: Vec<AddonDto>,
}

/// Ein gefundener Client: Steam, ein Wine-Prefix, ein Ordner.
#[derive(Serialize)]
pub struct ClientDto {
    pub id: String,
    pub label: String,
    pub origin: String,
    pub prefix: String,
    pub game_dir: String,
    pub flavors: Vec<FlavorDto>,
}

/// Ein Schritt des Scans, der nicht geklappt hat oder wichtig ist.
#[derive(Serialize)]
pub struct NoteDto {
    pub step: String,
    pub level: String,
    pub detail: String,
}

/// Das Ergebnis eines Scans.
#[derive(Serialize)]
pub struct ScanResult {
    pub clients: Vec<ClientDto>,
    pub notes: Vec<NoteDto>,
    pub addon_count: usize,
}

fn level(status: hexwerk::NoteStatus) -> &'static str {
    match status {
        hexwerk::NoteStatus::Ok => "ok",
        hexwerk::NoteStatus::Skip => "info",
        hexwerk::NoteStatus::Running => "info",
        hexwerk::NoteStatus::Error => "error",
    }
}

fn flavor_label(flavor: hexwerk::Flavor) -> String {
    flavor.label().to_string()
}

/// Alle Clients und ihre Addons einsammeln.
///
/// Das Lesen von der Platte dauert: bei einem gewachsenen Addon-Ordner sind es
/// einige tausend Dateien. Deshalb laeuft der Scan in einem eigenen Thread, und
/// die Oberflaeche zeigt so lange „laeuft" — der Befehl blockiert sie nicht.
#[tauri::command]
pub async fn scan() -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let discovered = hexwerk::discover(&[]);
        let mut addon_count = 0;

        let clients = discovered
            .clients
            .into_iter()
            .map(|client| {
                let flavors = client
                    .flavors
                    .into_iter()
                    .map(|flavor_dir| {
                        let addons: Vec<AddonDto> = hexwerk::scan_addons(&flavor_dir.addons_dir)
                            .into_iter()
                            .map(|addon| AddonDto {
                                // Titel kommt aus der .toc; fehlt er, ist der
                                // Ordnername die ehrlichste Auskunft.
                                title: addon
                                    .toc
                                    .as_ref()
                                    .and_then(|toc| toc.title.clone())
                                    .unwrap_or_else(|| addon.folder.clone()),
                                folder: addon.folder,
                                version: addon.version,
                                interface: addon.interface_numbers,
                                size_bytes: addon.size_bytes,
                            })
                            .collect();
                        let size_bytes = addons.iter().map(|a| a.size_bytes).sum();
                        addon_count += addons.len();
                        FlavorDto {
                            dir_name: flavor_dir.flavor.dir_name().to_string(),
                            label: flavor_label(flavor_dir.flavor),
                            addons_dir: flavor_dir.addons_dir.display().to_string(),
                            interface: flavor_dir.interface_numbers,
                            game_version: flavor_dir
                                .game_version
                                .as_ref()
                                .map(|v| v.clone())
                                .unwrap_or_default(),
                            size_bytes,
                            addons,
                        }
                    })
                    .collect();

                ClientDto {
                    id: client.id,
                    label: client.label,
                    origin: client.origin.label().to_string(),
                    prefix: client.prefix.display().to_string(),
                    game_dir: client.game_dir.display().to_string(),
                    flavors,
                }
            })
            .collect();

        let notes = discovered
            .notes
            .into_iter()
            .map(|note| NoteDto {
                step: note.step,
                level: level(note.status).to_string(),
                detail: note.detail,
            })
            .collect();

        // Der Scan ist der eine Weg, auf dem installierte Addons in den Index
        // kommen (wie `index.sync_clients` in der GTK-Fassung). Ohne diesen
        // Schritt weiss die Detailseite nichts von Installationen: der Katalog
        // fragt den Index, und der kennt nur, was hier hineingeschrieben wurde.
        if let Ok(mut idx) = hexwerk_index::Index::open(&hexwerk_index::Index::default_path()) {
            let _ = idx.sync_clients(&hexwerk::discover(&[]));
        }

        ScanResult {
            clients,
            notes,
            addon_count,
        }
    })
    .await
    .map_err(|e| e.to_string())
}


// ---------------------------------------------------------------- Katalog

/// Ein Release so, wie die Detailansicht es zeigt.
#[derive(Serialize)]
pub struct ReleaseDto {
    pub source: String,
    pub version: String,
    pub released_at: Option<String>,
    pub download_url: String,
    pub size_bytes: Option<u64>,
    pub interfaces: Vec<u32>,
}

/// Ein Katalogeintrag. Dieselbe Form fuer Liste und Detail — die Detailansicht
/// bekommt zusaetzlich `releases` gefuellt.
#[derive(Serialize)]
pub struct ItemDto {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub author: String,
    pub summary: String,
    pub category: Option<String>,
    pub homepage: Option<String>,
    pub screenshots: Vec<String>,
    /// Hoechste Fassung ueber alle Quellen.
    pub latest: Option<ReleaseDto>,
    /// Je Quelle die hoechste Fassung.
    pub sources: Vec<ReleaseDto>,
    /// Wo es installiert ist.
    pub installed: Vec<InstalledDto>,
    pub releases: Vec<ReleaseDto>,
}

#[derive(Serialize)]
pub struct InstalledDto {
    pub client_label: String,
    pub flavor: String,
    pub folder: String,
    pub name: String,
    pub version: Option<String>,
    pub size_bytes: u64,
    /// Version, die es anderswo gibt — `None`, wenn aktuell.
    pub update: Option<String>,
}

/// Zahlen fuer Fusszeile und Filter.
#[derive(Serialize)]
pub struct StatsDto {
    pub catalogue: usize,
    pub clients: usize,
    pub installed: usize,
    pub updates: usize,
    /// Je Quelle die Zahl der Eintraege, absteigend.
    pub per_source: Vec<(String, usize)>,
}

fn release_dto(r: &hexwerk_index::ReleaseRow) -> ReleaseDto {
    ReleaseDto {
        source: hexwerk_index::source_key(r.source).to_string(),
        version: r.version.clone(),
        released_at: r.fetched_at.clone(),
        download_url: r.url.clone(),
        size_bytes: r.size_bytes,
        interfaces: r.interfaces.clone(),
    }
}

fn source_release_dto(r: &hexwerk_index::SourceRelease) -> ReleaseDto {
    ReleaseDto {
        source: hexwerk_index::source_key(r.source).to_string(),
        version: r.version.clone(),
        released_at: r.released_at.clone(),
        download_url: r.download_url.clone(),
        size_bytes: r.size_bytes,
        interfaces: r.interfaces.clone(),
    }
}

fn item_dto(item: hexwerk_index::CatalogueItem) -> ItemDto {
    ItemDto {
        id: item.id,
        key: item.key,
        name: item.name,
        author: item.author,
        summary: item.summary,
        category: item.category,
        homepage: item.homepage,
        screenshots: item.screenshots,
        latest: item.latest.as_ref().map(source_release_dto),
        sources: item.sources.iter().map(source_release_dto).collect(),
        installed: item
            .installed
            .into_iter()
            .map(|i| InstalledDto {
                client_label: i.client_label,
                flavor: i.flavor,
                folder: i.folder,
                name: i.name,
                version: i.version,
                size_bytes: i.size_bytes,
                update: i.update.map(|u| u.version),
            })
            .collect(),
        releases: item.releases.iter().map(release_dto).collect(),
    }
}

fn index() -> Result<hexwerk_index::Index, String> {
    let pfad = hexwerk_index::Index::default_path();
    hexwerk_index::Index::open(&pfad).map_err(|e| format!("Index {}: {e}", pfad.display()))
}

/// Zahlen fuer Fusszeile und Quellen-Filter.
#[tauri::command]
pub fn catalog_stats() -> Result<StatsDto, String> {
    let idx = index()?;
    let stats = idx.stats()?;
    Ok(StatsDto {
        catalogue: stats.catalogue,
        clients: stats.clients,
        installed: stats.installed,
        updates: stats.updates,
        per_source: stats
            .per_source
            .into_iter()
            .map(|(k, n)| (k.to_string(), n))
            .collect(),
    })
}

/// Im Katalog suchen.
///
/// Der Index ist eine SQLite-Datei mit Volltextsuche; die Arbeit macht `search`.
/// Hier wird nur gefiltert, was die Oberflaeche als Eingabe schickt.
#[tauri::command]
pub fn catalog_search(query: String, limit: Option<u32>) -> Result<Vec<ItemDto>, String> {
    let idx = index()?;
    let filters = hexwerk_index::Filters::new(query);
    let treffer = idx.search(&filters, limit.unwrap_or(120), 0)?;
    Ok(treffer.into_iter().map(item_dto).collect())
}

// Eine eigene Detailabfrage gibt es nicht: `search` liefert je Eintrag schon
// Quellen, hoechste Fassung und die Installationen. Die Detailansicht arbeitet
// mit dem Eintrag, den die Liste geholt hat — eine zweite Abfrage waere eine
// zweite Wahrheit.


// ------------------------------------------------------------ Installieren

/// Ergebnis einer Installation.
#[derive(Serialize)]
pub struct InstallErgebnis {
    /// Ordner, der im Addon-Verzeichnis entstanden ist.
    pub folder: String,
    /// Version aus der `.toc`, falls das ZIP eine enthielt.
    pub version: Option<String>,
}

/// Ein Release laden und in ein Addon-Verzeichnis entpacken.
///
/// Der Ablauf ist der der GTK-Oberflaeche: laden, entpacken, fertig. Was dabei
/// passiert, macht `install_zip` — inklusive Sicherung des vorhandenen Ordners
/// und Papierkorb. Neu eingelesen wird danach vom Aufrufer (neuer Scan): so gibt
/// es genau einen Weg, auf dem installierte Addons in den Index kommen.
///
/// `addons_dir` ist der Zielordner aus dem Scan, nicht ein Client-Name: die
/// Oberflaeche weiss, wohin sie will, und muss hier nicht nachgeschlagen werden.
#[tauri::command]
pub async fn install(addons_dir: String, url: String) -> Result<InstallErgebnis, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // `request` kommt aus dem Trait, nicht aus dem Typ.
        use hexwerk_sources::Http as _;
        let http = hexwerk_sources::UreqHttp::new();
        let request = hexwerk_sources::Request::get(&url);
        let antwort = http.request(&request).map_err(|e| format!("Download: {e}"))?;
        if !antwort.is_success() {
            return Err(format!("Download: HTTP {}", antwort.status));
        }
        let ziel = std::path::Path::new(&addons_dir);
        let pfade = hexwerk_install::Paths::system();
        let installiert = hexwerk_install::install_zip(&antwort.body, ziel, None, &pfade)
            .map_err(|e| format!("Entpacken: {e}"))?;
        Ok(InstallErgebnis {
            folder: installiert.folder.display().to_string(),
            version: installiert.version,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}


// ------------------------------------------------------------- Einstellungen

/// Einstellungen lesen. Dieselbe Datei wie die GTK-Oberflaeche.
#[tauri::command]
pub fn settings_get() -> crate::settings::Settings {
    crate::settings::Settings::load()
}

/// Einstellungen schreiben.
#[tauri::command]
pub fn settings_set(settings: crate::settings::Settings) -> Result<(), String> {
    settings.save()
}


// ------------------------------------------------------------ Katalog holen

/// Der Katalogdienst. Fest eingebaut: die App hat keinen eigenen Zugang zu den
/// Maerkten, sondern fragt diesen Dienst. Eine einstellbare Adresse gab es
/// frueher — sie ist entfallen, weil es nur diesen einen Dienst gibt.
pub const CATALOG_URL: &str = "https://hexwerk-api.weppelmann.eu";
pub const CATALOG_KEY: &str = "3rp4y4t5zyexg8rcaej84tbpp44sx2bc";

/// Was ein Katalogabruf ergeben hat.
#[derive(Serialize)]
pub struct SyncErgebnis {
    /// Stand des Katalogs (Version aus dem Manifest).
    pub version: String,
    /// Eintraege im Index danach.
    pub eintraege: usize,
    /// Wurde etwas uebernommen oder war der Stand schon da?
    pub neu: bool,
}

/// Den Katalog vom Dienst holen.
///
/// Ablauf (siehe `hexwerk_index::remote`): Manifest lesen, mit der bekannten
/// Version vergleichen, sonst den gepackten Katalog laden, Pruefsumme pruefen
/// und die Zeilen in den lokalen Index uebernehmen. Was der Index aus eigenen
/// Laeufen weiss, bleibt erhalten.
#[tauri::command]
pub async fn catalog_sync() -> Result<SyncErgebnis, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pfad = hexwerk_index::Index::default_path();
        let mut idx = hexwerk_index::Index::open(&pfad)?;
        let cache = pfad
            .parent()
            .map(|d| d.join("catalog"))
            .unwrap_or_else(|| std::path::PathBuf::from("catalog"));

        let remote = hexwerk_index::RemoteCatalog::new(CATALOG_URL, CATALOG_KEY);
        let (version, neu) = match remote.sync(&mut idx, &cache, false)? {
            hexwerk_index::SyncOutcome::UpToDate { version } => (version, false),
            hexwerk_index::SyncOutcome::Imported { version, .. } => (version, true),
        };
        let eintraege = idx.stats().map(|s| s.catalogue).unwrap_or(0);
        Ok(SyncErgebnis { version, eintraege, neu })
    })
    .await
    .map_err(|e| e.to_string())?
}


// -------------------------------------------------------------- Programm-Info

/// Wo die Einstellungen liegen und welche Fassung laeuft.
#[derive(Serialize)]
pub struct Info {
    pub version: String,
    pub settings_pfad: String,
    /// Stand des Katalogs aus dem Index-Merker (leer, wenn nie geholt).
    pub catalog_version: String,
}

#[tauri::command]
pub fn app_info() -> Info {
    let catalog_version = index()
        .ok()
        .and_then(|idx| idx.meta("catalog_version").ok().flatten())
        .unwrap_or_default();
    Info {
        version: env!("CARGO_PKG_VERSION").to_string(),
        settings_pfad: crate::settings::pfad()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        catalog_version,
    }
}


// ------------------------------------------------------------------- Bilder

/// Ein Bild, wie es angekommen ist: Adresse und fertige data-URL.
#[derive(Serialize)]
pub struct BildDto {
    pub url: String,
    /// `data:image/…;base64,…` — die Oberflaeche setzt es direkt als src ein.
    pub data: String,
}

fn base64(daten: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut aus = String::with_capacity(daten.len().div_ceil(3) * 4);
    for stueck in daten.chunks(3) {
        let b = [stueck[0], *stueck.get(1).unwrap_or(&0), *stueck.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        aus.push(ALPHABET[(n >> 18) as usize & 63] as char);
        aus.push(ALPHABET[(n >> 12) as usize & 63] as char);
        aus.push(if stueck.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        aus.push(if stueck.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    aus
}

/// Bilder einer Detailseite laden.
///
/// Bilder sind Anzeige, nicht Kataloginhalt: im Index stehen nur die Adressen,
/// geholt wird erst, wenn jemand den Eintrag oeffnet. Sonst zoege ein
/// Kataloglauf zehntausend Bilder mit (so steht es auch in `spawn_image_fetch`).
#[tauri::command]
pub async fn fetch_images(urls: Vec<String>) -> Vec<BildDto> {
    tauri::async_runtime::spawn_blocking(move || {
        use hexwerk_sources::Http as _;
        let http = hexwerk_sources::UreqHttp::new();
        let mut aus = Vec::new();
        for url in urls {
            let anfrage = hexwerk_sources::Request::get(&url).header("accept", "image/*");
            if let Ok(antwort) = http.request(&anfrage) {
                if !antwort.is_success() || antwort.body.is_empty() {
                    continue;
                }
                // Typ aus dem Inhalt raten: Header sind nicht immer da.
                let typ = if antwort.body.starts_with(&[0x89, b'P', b'N', b'G']) {
                    "image/png"
                } else if antwort.body.starts_with(&[0xff, 0xd8]) {
                    "image/jpeg"
                } else if antwort.body.starts_with(b"GIF8") {
                    "image/gif"
                } else if antwort.body.starts_with(b"RIFF") {
                    "image/webp"
                } else {
                    "image/jpeg"
                };
                aus.push(BildDto {
                    url,
                    data: format!("data:{typ};base64,{}", base64(&antwort.body)),
                });
            }
        }
        aus
    })
    .await
    .unwrap_or_default()
}
