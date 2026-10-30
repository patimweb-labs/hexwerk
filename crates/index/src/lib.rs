//! Lokaler Index: Clients, Katalog und installierte Addons in einer SQLite-Datei.
//!
//! Die Oberflaeche fragt ausschliesslich diesen Index, nie das Netz (PLAN §6).
//!
//! - Oeffnen: [`Index::open`] auf [`Index::default_path`]
//!   (`~/.local/share/hexwerk/index.db`), [`Index::open_in_memory`] fuer Tests.
//! - Schreiben: [`Index::sync_clients`] (Dateisystem),
//!   [`Index::sync_catalogue`] (Quellen), [`Index::sync`] (beides).
//! - Lesen: [`Index::search`], [`Index::addon`], [`Index::clients`],
//!   [`Index::installed`], [`Index::updates`], [`Index::stats`].
//!
//! Fehler sind `Result<_, String>`: der Text ist fuer die Anzeige gedacht
//! („Katalog schreiben: database is locked“). Es gibt keinen Panik-Pfad in
//! Bibliotheksfunktionen — fehlende Ordner, kaputte JSON-Spalten und
//! unbekannte Quellenkennungen werden behandelt, nicht angenommen.

pub mod queries;
pub mod remote;
pub mod schema;
pub mod sync;
pub mod version;

mod util;

pub use queries::{
    source_from_key, source_key, CatalogueItem, ClientRow, Filters, InstalledRow, ReleaseRow,
    SourceRelease, Stats, Update, UpdateHint,
};
pub use remote::{Manifest, RemoteCatalog, SyncOutcome};
pub use schema::{Index, SCHEMA_VERSION};
pub use sync::SyncReport;
pub use version::compare as compare_versions;
