//! Lokaler Index: Clients, Katalog und installierte Addons in einer SQLite-Datei.
//!
//! Die Oberflaeche fragt ausschliesslich diesen Index, nie das Netz.

pub mod queries;
pub mod schema;
pub mod sync;

pub use queries::Filters;
pub use schema::Index;
