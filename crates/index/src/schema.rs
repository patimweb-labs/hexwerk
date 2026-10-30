//! Schema und Verbindung. Wird von Arbeitspaket C gefuellt.

/// Der Index. Haelt die SQLite-Verbindung.
pub struct Index {
    _priv: (),
}

impl Index {
    /// Oeffnet oder erzeugt den Index an `path`.
    pub fn open(_path: &std::path::Path) -> Result<Self, String> {
        Ok(Self { _priv: () })
    }

    pub fn schema_version(&self) -> i64 {
        0
    }
}
