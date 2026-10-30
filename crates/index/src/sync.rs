//! Abgleich Index <-> Dateisystem <-> Quellen.

/// Ergebnis eines Synchronlaufs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub clients: usize,
    pub installed: usize,
    pub catalogue: usize,
    pub duration_ms: u128,
}
