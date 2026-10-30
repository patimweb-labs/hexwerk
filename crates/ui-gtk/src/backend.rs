//! Anschluss der Oberflaeche an Index, Quellen und Installation.
//!
//! Warum ein eigenes Modul: der Hauptthread darf weder Dateisystem noch Netz
//! anfassen, waehrend das Fenster bedienbar bleiben soll (PLAN §6). Alles, was
//! laenger dauert als ein Klick, laeuft hier in `std::thread` und schickt sein
//! Ergebnis ueber `async_channel` zurueck. Die UI-Seite liest den Kanal in
//! `glib::spawn_future_local` und fasst dabei ausschliesslich Widgets an.
//!
//! Zwei Verbindungen auf denselben Index sind Absicht: der Hauptthread liest
//! (Suche, Detailseite), der Hintergrundlauf schreibt (Clients, Katalog). WAL
//! laesst beides parallel zu, ohne dass ein Leser auf einen Schreiber wartet.

use std::sync::Arc;

use hexwerk::{Client, Discovery, Flavor, FlavorDir};
use hexwerk_index::{Index, Stats, SyncReport};
use hexwerk_sources::{merge, Cache, Http, KeyProvider, Registry, Release, Source, UreqHttp};

use crate::state::{ClientData, ScanData};

/// Suchbegriff fuer den GitHub-Teil des Katalogs.
///
/// GitHub hat keinen Endpunkt „alle Repositories“: ohne Suchbegriff antwortet
/// die API mit HTTP 422. Tukui und WoWInterface liefern dagegen ihre
/// Gesamtliste, wenn der Begriff leer ist - deshalb bekommt nur GitHub einen
/// Begriff. Die Treffer selbst sind echte GitHub-Antworten, nichts erfunden.
const GITHUB_SEED: &str = "world of warcraft addon";

/// Wie eine Quelle in diesem Lauf abgeschnitten hat.
#[derive(Debug, Clone)]
pub struct SourceRun {
    pub source: Source,
    pub ok: bool,
    /// Menge oder Fehlertext - beides direkt anzeigbar.
    pub detail: String,
}

/// Was der Index-Thread zur Oberflaeche schickt.
pub enum IndexMsg {
    /// Clients und installierte Addons sind im Index.
    Clients(Result<SyncReport, String>),
    /// Je Quelle, ob sie geantwortet hat.
    Sources(Vec<SourceRun>),
    /// Der Katalog ist geschrieben.
    Catalogue(Result<SyncReport, String>),
    /// Kein Client gefunden - der Kataloglauf wurde uebersprungen (PLAN §5:
    /// ohne Client gibt es kein Ziel, also auch keinen sinnvollen Katalog).
    NoClients,
    /// Indexdatei nicht lesbar/oeffenbar.
    Error(String),
    /// Lauf zu Ende, gleich mit welchem Ausgang. Erst danach liest die UI neu,
    /// damit sie nicht auf halb geschriebene Transaktionen schaut.
    Finished,
}

/// Baut Clients und (optional) den Katalog im Hintergrund.
///
/// `run_sources == false` ist der Weg nach einer Installation: dann wird nur
/// das Dateisystem neu in den Index geschrieben. Ein Netzlauf nach jedem
/// entpackten ZIP waere gegenueber den Quellen unhoeflich und langsam.
pub fn spawn_pipeline(
    discovery: Discovery,
    run_sources: bool,
    tx: async_channel::Sender<IndexMsg>,
) {
    std::thread::Builder::new()
        .name("hexwerk-index".to_string())
        .spawn(move || {
            pipeline(&discovery, run_sources, &tx);
            // Immer, auch nach einem fruehen Abbruch: die UI haengt sonst im
            // Zustand „wird aufgebaut“.
            let _ = tx.send_blocking(IndexMsg::Finished);
        })
        .ok();
}

fn pipeline(discovery: &Discovery, run_sources: bool, tx: &async_channel::Sender<IndexMsg>) {
    let path = Index::default_path();
    let mut index = match Index::open(&path) {
        Ok(index) => index,
        Err(err) => {
            let _ = tx.send_blocking(IndexMsg::Error(format!("Index {}: {err}", path.display())));
            return;
        }
    };

    match index.sync_clients(discovery) {
        Ok(report) => {
            let _ = tx.send_blocking(IndexMsg::Clients(Ok(report)));
        }
        Err(err) => {
            let _ = tx.send_blocking(IndexMsg::Clients(Err(err)));
            return;
        }
    }

    if discovery.clients.is_empty() {
        let _ = tx.send_blocking(IndexMsg::NoClients);
        return;
    }
    if !run_sources {
        return;
    }

    let keys = KeyProvider::default_provider();
    let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
    let registry = Registry::build(&keys, http, Cache::open_default());

    let mut runs: Vec<SourceRun> = Vec::new();
    let mut entries = Vec::new();
    for source in registry.sources() {
        // Nur GitHub braucht einen Begriff; die beiden anderen Quellen listen
        // bei leerem Begriff ihren ganzen Bestand (siehe GITHUB_SEED).
        let query = match source.id() {
            Source::GitHub => GITHUB_SEED,
            _ => "",
        };
        match source.search(query, 0) {
            Ok(mut found) => {
                runs.push(SourceRun {
                    source: source.id(),
                    ok: true,
                    detail: format!("{} Einträge", found.len()),
                });
                entries.append(&mut found);
            }
            Err(err) => runs.push(SourceRun {
                source: source.id(),
                ok: false,
                detail: err.to_string(),
            }),
        }
    }
    // Abgeschaltete Quellen nennen den Grund, statt sie zu verschweigen: der
    // Katalog soll nicht so aussehen, als gaebe es sie nicht.
    for source in registry.absent() {
        runs.push(SourceRun {
            source,
            ok: false,
            detail: format!("nicht abgefragt, {}", keys.origin(source).label()),
        });
    }
    runs.sort_by_key(|run| run.source);
    let _ = tx.send_blocking(IndexMsg::Sources(runs));

    let merged = merge(entries);
    let report = index.sync_catalogue(&merged);
    let _ = tx.send_blocking(IndexMsg::Catalogue(report));
}

/// Auftrag an den Installations-Thread.
pub struct InstallJob {
    pub client: Client,
    pub flavor: Flavor,
    pub release: Release,
    /// Anzeigename fuer die Rueckmeldung; der Ordnername kommt aus dem ZIP.
    pub name: String,
}

/// Ergebnis einer Installation.
pub enum InstallMsg {
    Done {
        name: String,
        version: String,
        folder: String,
    },
    Failed(String),
}

/// Laedt und entpackt ein Release im Hintergrund.
///
/// Reihenfolge wie in der Aufgabe: Download bei der Quelle, `apply` ins
/// Ziel-Addons-Verzeichnis, danach meldet die UI den Erfolg. Das Neu-Einlesen
/// des Ordners in den Index macht der Aufrufer ueber einen neuen Scan - so
/// gibt es genau einen Weg, auf dem installierte Addons in den Index kommen.
pub fn spawn_install(job: InstallJob, tx: async_channel::Sender<InstallMsg>) {
    std::thread::Builder::new()
        .name("hexwerk-install".to_string())
        .spawn(move || {
            let msg = match install(&job) {
                Ok((version, folder)) => InstallMsg::Done {
                    name: job.name.clone(),
                    version,
                    folder,
                },
                Err(err) => InstallMsg::Failed(err),
            };
            let _ = tx.send_blocking(msg);
        })
        .ok();
}

fn install(job: &InstallJob) -> Result<(String, String), String> {
    if job.release.download_url.trim().is_empty() {
        return Err(format!(
            "{} nennt für {} keine Download-Adresse",
            job.release.source.label(),
            job.release.version
        ));
    }
    let keys = KeyProvider::default_provider();
    let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
    let registry = Registry::build(&keys, http, Cache::open_default());
    let bytes = registry
        .download(&job.release)
        .map_err(|err| format!("{}: {err}", job.release.source.label()))?;

    let request = hexwerk_install::InstallRequest {
        client: job.client.clone(),
        flavor: job.flavor.clone(),
        release: job.release.clone(),
        // Leer heisst: der Ordnername kommt aus dem `.toc` im ZIP. Ein Name
        // aus dem Katalog waere geraten - Ordnernamen sind nicht der Addon-Name.
        folder: None,
    };
    let done = hexwerk_install::apply(&request, &bytes).map_err(|err| err.to_string())?;
    let folder = done
        .folder
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| done.folder.display().to_string());
    Ok((
        done.version.unwrap_or_else(|| job.release.version.clone()),
        folder,
    ))
}

/// Index oeffnen (oder anlegen). Getrennt, damit die UI den Fehlertext anzeigen
/// kann, statt ihn zu schlucken.
pub fn open_index() -> Result<Index, String> {
    let path = Index::default_path();
    Index::open(&path).map_err(|err| format!("Index {}: {err}", path.display()))
}

/// Zahlen fuer Fusszeile und Filterchips.
pub fn load_stats(index: &Index) -> Result<Stats, String> {
    index.stats()
}

/// Baut aus dem, was der Scan in der UI abgelegt hat, die Kern-Struktur, die
/// `Index::sync_clients` erwartet.
///
/// WARUM die Umrechnung noetig ist: die UI haelt eigene, `Clone`-faehige
/// Kopien (state.rs), damit sie Daten zwischen Bildschirmen weiterreichen kann,
/// ohne den Kern zu zwingen, teuer zu rechnen. Der Index braucht aber die
/// Kern-Typen - also werden sie hier gebaut, aus genau den Werten, die der
/// Scan gelesen hat.
pub fn discovery_from(data: &ScanData) -> Discovery {
    Discovery {
        clients: data.clients.iter().map(core_client).collect(),
        notes: data.notes.clone(),
    }
}

/// Ein einzelner Client als Kern-Typ (`Client` ist `Clone`, also reicht eine
/// Kopie fuer den Installations-Thread).
pub fn core_client(data: &ClientData) -> Client {
    Client {
        id: data.id.clone(),
        origin: data.origin,
        label: data.label.clone(),
        prefix: data.prefix.clone(),
        game_dir: data.game_dir.clone(),
        flavors: data
            .flavors
            .iter()
            .map(|flavor| FlavorDir {
                flavor: flavor.flavor.clone(),
                // Der Varianten-Ordner liegt immer unter `game_dir`; der Scan
                // hat ihn dort gefunden, nur den Namen behalten.
                dir: data.game_dir.join(&flavor.dir_name),
                addons_dir: flavor.addons_dir.clone(),
                interface_numbers: flavor.interface_numbers.clone(),
            })
            .collect(),
    }
}

/// Anzeigezeile fuer die Seitenleiste: „312“ oder „HTTPS 429 …“.
pub fn source_state(runs: &[SourceRun], source: Source) -> Option<SourceRun> {
    runs.iter().find(|run| run.source == source).cloned()
}

/// Quellen, die der Katalog kennt. Reihenfolge wie in PLAN §5.
pub const CATALOGUE_SOURCES: [Source; 5] = [
    Source::GitHub,
    Source::Tukui,
    Source::WowInterface,
    Source::Wago,
    Source::CurseForge,
];
