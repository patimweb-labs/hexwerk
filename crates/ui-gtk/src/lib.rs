//! Hexwerk-Oberflaeche: GTK4 + libadwaita.
//!
//! Fuenf Bildschirme (Scan, Clients, Installiert, Katalog, Addon-Detail) und
//! ein eigenes Theming aus `data/theme.css`. Die Oberflaeche haengt an allen
//! vier Paketen: der Kern liefert Clients, der Index die Suchantworten, die
//! Quellen die Downloads, die Installation das Entpacken. Was dort nicht steht,
//! steht hier nicht - lieber „keine Angabe“ als eine erfundene Zahl.

pub mod app;
pub mod backend;
pub mod catalog_view;
pub mod clients_view;
pub mod detail_view;
pub mod format;
pub mod installed_view;
pub mod scan_view;
pub mod state;
pub mod theme;

pub use app::{build_window, run};
