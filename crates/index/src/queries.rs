//! Suchabfragen und Filter.

/// Filter fuer die Katalogsuche.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    pub query: String,
    /// Nur diese Quellen.
    pub sources: Vec<hexwerk_sources::Source>,
    /// Nur Addons, die zu dieser Interface-Nummer passen.
    pub interface_number: Option<u32>,
    /// Nur Eintraege, fuer die es ein Update gibt.
    pub updates_only: bool,
}
