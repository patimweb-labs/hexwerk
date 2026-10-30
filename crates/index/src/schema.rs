//! Schema, Migrationen und Verbindung zum lokalen Index.
//!
//! Eine Datei, eine Schemaversion in `meta`. Der Index ist die einzige
//! Wahrheit fuer die Oberflaeche: Clients, Katalog und installierte Addons
//! muessen hier vollstaendig lesbar sein, ohne dass irgendwer das Netz oder
//! das Dateisystem nachfragt.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

/// Schemaversion, die dieser Code erwartet. Steigt mit jeder Migration in
/// [`MIGRATIONS`] um eins.
/// 2: `addons.screenshots_json` (Bilder je Eintrag). Ein Index mit Version 1
/// wird beim Oeffnen neu aufgebaut; der Katalog laedt sich beim naechsten Lauf
/// wieder nach.
pub const SCHEMA_VERSION: i64 = 2;

/// Migrationen in Reihenfolge; Index + 1 ist die Zielversion.
///
/// Eine veroeffentlichte Migration wird nie mehr geaendert — sonst haetten
/// bestehende Dateien nach dem Update ein anderes Schema als neu erzeugte,
/// ohne dass es jemand merkt. Aenderungen kommen als neue Migration dazu.
const MIGRATIONS: &[&str] = &[MIGRATION_1, MIGRATION_2];

/// Version 2: Bildadressen je Eintrag (`addons.screenshots_json`).
///
/// Als `ALTER TABLE`, damit bestehende Indexdateien die Spalte nachtraeglich
/// bekommen, ohne den Katalog neu laden zu muessen.
const MIGRATION_2: &str = r#"
ALTER TABLE addons ADD COLUMN screenshots_json TEXT NOT NULL DEFAULT '[]';
"#;

/// Version 1: das Schema aus PLAN §6, ergaenzt um Spalten, die die
/// Oberflaeche braucht, ohne den Katalog erneut zu befragen.
const MIGRATION_1: &str = r#"
-- Eine Zeile je Client-Variante: derselbe Praefix kann `_retail_` und
-- `_classic_era_` tragen, und Installation ist immer client-scharf (PLAN §13).
CREATE TABLE clients (
    id              TEXT PRIMARY KEY NOT NULL,  -- Client-Kennung + Variante
    origin          TEXT NOT NULL,              -- steam, faugus, heroic, ...
    origin_label    TEXT NOT NULL DEFAULT '',   -- Anzeigename der Herkunft
    label           TEXT NOT NULL DEFAULT '',   -- Name aus dem Launcher
    prefix          TEXT NOT NULL DEFAULT '',   -- Wurzel des Wine-Praefixes
    game_dir        TEXT NOT NULL DEFAULT '',
    flavor          TEXT NOT NULL,              -- Ordnername, z. B. _retail_
    interface_no    INTEGER,                    -- hoechste Nummer, NULL = unbekannt
    interfaces_json TEXT NOT NULL DEFAULT '[]', -- ganze Menge aus den .toc
    addons_dir      TEXT NOT NULL DEFAULT '',
    seen_at         TEXT NOT NULL               -- Kennung des letzten Scans
);
CREATE INDEX clients_addons_dir ON clients(addons_dir);
CREATE INDEX clients_interface ON clients(interface_no);

-- Ein Katalogeintrag, gemergt ueber alle Quellen. `key` ist der stabile
-- Schluessel aus hexwerk-sources::merge (externe Projekt-ID oder Name).
CREATE TABLE addons (
    id         INTEGER PRIMARY KEY,
    key        TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    author     TEXT NOT NULL DEFAULT '',
    summary    TEXT NOT NULL DEFAULT '',
    homepage   TEXT,
    icon_path  TEXT,
    category   TEXT,
    updated_at TEXT
);
CREATE INDEX addons_name ON addons(name COLLATE NOCASE);

-- Je Quelle die aktuell hoechste Version. Getrennt von `releases`, weil die
-- Oberflaeche im Katalog nur diese Zeile braucht (Quellen-Chips, Downloads)
-- und nicht die ganze Versionshistorie.
CREATE TABLE addon_sources (
    addon_id     INTEGER NOT NULL REFERENCES addons(id) ON DELETE CASCADE,
    source       TEXT NOT NULL,
    external_id  TEXT NOT NULL DEFAULT '',
    version      TEXT NOT NULL DEFAULT '',
    released_at  TEXT,
    download_url TEXT NOT NULL DEFAULT '',
    size_bytes   INTEGER,
    checked_at   TEXT,
    PRIMARY KEY (addon_id, source)
);

-- Alle bekannten Releases. `interface_json` ist die Liste der
-- Interface-Nummern aus dem `.toc` dieses Releases; der Filter
-- „passt zu Client X“ laeuft darueber.
CREATE TABLE releases (
    addon_id       INTEGER NOT NULL REFERENCES addons(id) ON DELETE CASCADE,
    source         TEXT NOT NULL,
    version        TEXT NOT NULL,
    interface_json TEXT NOT NULL DEFAULT '[]',
    url            TEXT NOT NULL DEFAULT '',
    sha256         TEXT,
    size_bytes     INTEGER,
    fetched_at     TEXT,
    PRIMARY KEY (addon_id, source, version)
);
CREATE INDEX releases_addon ON releases(addon_id);

-- Was auf der Platte liegt. `addon_id` bleibt NULL, solange das Addon nicht
-- im Katalog steht (manuell installiert, Quelle unbekannt).
CREATE TABLE installed (
    addon_id          INTEGER REFERENCES addons(id) ON DELETE SET NULL,
    client_id         TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    folder            TEXT NOT NULL,
    title             TEXT,                   -- Name aus dem .toc
    version           TEXT,
    interface_json    TEXT NOT NULL DEFAULT '[]',
    fingerprint       TEXT,                   -- Hash des Ordners, vom Installer
    foreign_keys_json TEXT NOT NULL DEFAULT '{}',
    size_bytes        INTEGER NOT NULL DEFAULT 0,
    seen_at           TEXT NOT NULL,
    PRIMARY KEY (client_id, folder)
);
CREATE INDEX installed_addon ON installed(addon_id);

-- Volltextsuche ueber den Katalog. External content: der Text liegt in
-- `addons`, FTS haelt nur den Index — keine doppelten Daten, und die Trigger
-- unten sorgen dafuer, dass er nicht auseinanderlaeuft.
-- `remove_diacritics 2` macht „Atzend“ auffindbar als „Ätzend“.
CREATE VIRTUAL TABLE addons_fts USING fts5(
    name,
    author,
    summary,
    content='addons',
    content_rowid='id',
    tokenize="unicode61 remove_diacritics 2"
);

CREATE TRIGGER addons_fts_ai AFTER INSERT ON addons BEGIN
    INSERT INTO addons_fts(rowid, name, author, summary)
    VALUES (new.id, new.name, new.author, new.summary);
END;

CREATE TRIGGER addons_fts_ad AFTER DELETE ON addons BEGIN
    INSERT INTO addons_fts(addons_fts, rowid, name, author, summary)
    VALUES ('delete', old.id, old.name, old.author, old.summary);
END;

CREATE TRIGGER addons_fts_au AFTER UPDATE ON addons BEGIN
    INSERT INTO addons_fts(addons_fts, rowid, name, author, summary)
    VALUES ('delete', old.id, old.name, old.author, old.summary);
    INSERT INTO addons_fts(rowid, name, author, summary)
    VALUES (new.id, new.name, new.author, new.summary);
END;
"#;

/// Der Index. Haelt die SQLite-Verbindung.
///
/// Nicht `Sync`: ein `Index` gehoert einem Thread (UI oder Hintergrundsync).
/// Fuer den Hintergrundlauf oeffnet der Aufrufer eine zweite Verbindung auf
/// dieselbe Datei; WAL laesst Lesen und Schreiben parallel zu.
pub struct Index {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Index {
    /// Oeffnet oder erzeugt den Index an `path`.
    ///
    /// Fehler sind Text, weil der Aufrufer sie nur anzeigt: der Index hat
    /// keinen Zustand, den ein Aufrufer programmatisch behandeln muesste.
    /// Fehlende Elternverzeichnisse werden angelegt, damit
    /// `~/.local/share/hexwerk/index.db` beim ersten Start einfach da ist.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Index-Ordner {} anlegen: {e}", parent.display()))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| format!("Index {}: {e}", path.display()))?;
        let mut index = Self {
            conn,
            path: Some(path.to_path_buf()),
        };
        index.configure()?;
        index.migrate()?;
        Ok(index)
    }

    /// Index im Arbeitsspeicher. Fuer Tests und fuer `hexwerk --dry-run`.
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("Index im Speicher: {e}"))?;
        let mut index = Self { conn, path: None };
        index.configure()?;
        index.migrate()?;
        Ok(index)
    }

    /// Vorgabepfad im Betrieb: `$XDG_DATA_HOME/hexwerk/index.db`, sonst
    /// `~/.local/share/hexwerk/index.db` (PLAN §6).
    pub fn default_path() -> PathBuf {
        // Eine Regel fuer alle: Kern, Index und Papierkorb muessen denselben
        // Ort benutzen, sonst findet die Kommandozeile auf dem Host nicht, was
        // die App im Flatpak abgelegt hat (siehe `hexwerk::data_root`).
        hexwerk::data_root().join("index.db")
    }

    /// Datei hinter diesem Index, `None` bei [`Index::open_in_memory`].
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Verbindung fuer die Abfragen in [`crate::queries`] und [`crate::sync`].
    ///
    /// `pub(crate)` mit Absicht: Aufrufer ausserhalb des Pakets sollen die
    /// Methoden benutzen. Eigenes SQL von aussen wuerde die Migrationen
    /// umgehen und beim naechsten Schemawechsel still brechen.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Wie [`Index::conn`], aber veraenderbar — Transaktionen brauchen das.
    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Schemaversion aus `meta`. `0` heisst: kein oder unlesbares `meta`
    /// (dann ist die Datei auch nicht von uns) — die Zahl ist nur zur Anzeige
    /// gedacht, nicht als Entscheidungsgrundlage.
    pub fn schema_version(&self) -> i64 {
        self.read_version().unwrap_or(0)
    }

    /// Eintrag aus `meta` (Schema-Version, letzter Vollsync, …).
    pub fn meta(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT v FROM meta WHERE k = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(|e| format!("meta/{key} lesen: {e}"))
    }

    /// Setzt einen `meta`-Eintrag.
    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta(k, v) VALUES (?1, ?2)
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                [key, value],
            )
            .map(|_| ())
            .map_err(|e| format!("meta/{key} schreiben: {e}"))
    }

    /// Leert den Index vollstaendig (Einstellungen: „Index neu aufbauen“).
    ///
    /// Die Schemaversion in `meta` bleibt stehen: ohne sie wuerde die Datei
    /// beim naechsten [`Index::open`] als leer gelten und die Migration ein
    /// zweites Mal laufen — was an „table already exists“ scheitert. Das
    /// `rebuild` raeumt die Schatten-Tabellen auf, die FTS5 anlegt.
    pub fn clear(&mut self) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(db_err("clear"))?;
        for table in [
            "installed",
            "releases",
            "addon_sources",
            "addons",
            "clients",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])
                .map_err(db_err("clear"))?;
        }
        tx.execute("DELETE FROM meta WHERE k <> 'schema_version'", [])
            .map_err(db_err("clear"))?;
        tx.execute("INSERT INTO addons_fts(addons_fts) VALUES('rebuild')", [])
            .map_err(db_err("clear/fts"))?;
        tx.commit().map_err(db_err("clear/commit"))
    }

    /// Baut den Volltextindex neu. Noetig nur nach Eingriffen von aussen
    /// (SQLite-Werkzeug, Kopie einer Datei); im Normalbetrieb halten die
    /// Trigger in der Migration ihn aktuell.
    /// Uebernimmt einen Katalog-Snapshot in diesen Index.
    ///
    /// Der Snapshot ist eine SQLite-Datei mit den Tabellen `addons`,
    /// `addon_sources` und `releases` (dasselbe Schema wie hier). Vorhandene
    /// Eintraege werden ergaenzt, nicht verworfen: was ein eigener Lauf ueber
    /// eine Quelle weiss, bleibt stehen, nur leere Felder werden gefuellt.
    ///
    /// Rueckgabe ist die Zahl der Addons, die der Snapshot mitbringt.
    pub fn merge_catalog(&mut self, snapshot: &Path) -> Result<usize, String> {
        let path = snapshot.to_string_lossy().to_string();
        self.conn
            .execute("ATTACH DATABASE ?1 AS snap", [&path])
            .map_err(|err| format!("Snapshot anhaengen: {err}"))?;
        let result = self.merge_attached_catalog();
        // Auch bei Fehlern loesen: sonst bleibt die Verbindung haengen und der
        // naechste Lauf scheitert an "database snap is already in use".
        let _ = self.conn.execute_batch("DETACH DATABASE snap;");
        result
    }

    fn merge_attached_catalog(&mut self) -> Result<usize, String> {
        let addons: i64 = self
            .conn
            .query_row("SELECT count(*) FROM snap.addons", [], |row| row.get(0))
            .unwrap_or(0);
        let statements = if self.has_column("addons", "screenshots_json")? {
            MERGE_WITH_SCREENSHOTS
        } else {
            MERGE_WITHOUT_SCREENSHOTS
        };
        self.conn
            .execute_batch(statements)
            .map_err(|err| format!("Katalog uebernehmen: {err}"))?;
        // Der Suchindex muss den neuen Bestand kennen; ein Fehler dabei ist
        // kein Grund, das Uebernehmen zu verwerfen: die Suche findet dann
        // weniger, die Zeilen sind aber da.
        if let Err(err) = self.rebuild_fts() {
            eprintln!("Suchindex nicht neu gebaut: {err}");
        }
        // Nach dem Import neu zuordnen: ein von Hand installiertes Addon
        // bekommt seinen Katalogeintrag erst, wenn der Katalog da ist. Ohne
        // diesen Schritt bleibt es fuer immer unverknuepft.
        if let Err(err) = self.link_installed() {
            eprintln!("Installierte Addons nicht zugeordnet: {err}");
        }
        Ok(addons as usize)
    }

    fn has_column(&self, table: &str, column: &str) -> Result<bool, String> {
        let mut statement = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|err| format!("Spalten lesen: {err}"))?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| format!("Spalten lesen: {err}"))?;
        for name in names {
            if name.map_err(|err| format!("Spalten lesen: {err}"))? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn rebuild_fts(&self) -> Result<(), String> {
        self.conn
            .execute("INSERT INTO addons_fts(addons_fts) VALUES('rebuild')", [])
            .map(|_| ())
            .map_err(db_err("FTS neu bauen"))
    }

    /// Setzt PRAGMAs. WAL, damit die Oberflaeche waehrend eines
    /// Hintergrundsyncs weiterlesen kann, ohne auf den Schreiber zu warten.
    fn configure(&self) -> Result<(), String> {
        // `journal_mode` liefert eine Zeile zurueck; `execute` wuerde daran
        // scheitern, also bewusst `query_row`. Eine Speicher-Datenbank
        // antwortet mit `memory` und nicht mit einem Fehler.
        let _mode: String = self
            .conn
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(|e| format!("PRAGMA journal_mode: {e}"))?;
        self.conn
            .execute_batch(
                "PRAGMA busy_timeout = 5000;
                 PRAGMA foreign_keys = ON;
                 PRAGMA synchronous = NORMAL;",
            )
            .map_err(|e| format!("PRAGMA: {e}"))
    }

    fn read_version(&self) -> Result<i64, String> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT v FROM meta WHERE k = 'schema_version'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| format!("Schema-Version lesen: {e}"))?;
        match raw {
            Some(text) => text
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("Schema-Version ist keine Zahl: {text:?}")),
            None => Ok(0),
        }
    }

    /// Bringt die Datei auf [`SCHEMA_VERSION`]. Jede Migration und ihre
    /// Versionsnummer stehen in derselben Transaktion: bricht der Prozess
    /// mittendrin ab, ist die Datei entweder alt oder neu, nie halb.
    fn migrate(&mut self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS meta (
                     k TEXT PRIMARY KEY NOT NULL,
                     v TEXT NOT NULL
                 );",
            )
            .map_err(|e| format!("meta anlegen: {e}"))?;

        let current = self.read_version()?;
        if current > SCHEMA_VERSION {
            // Neuere Datei als dieser Code kennt: nicht anfassen, sonst
            // schreiben wir Daten in ein Schema, das wir nicht verstehen.
            return Err(format!(
                "Index hat Schema-Version {current}, dieser Code kennt nur {SCHEMA_VERSION}. \
                 Hexwerk ist aelter als die Indexdatei."
            ));
        }

        for (i, migration) in MIGRATIONS.iter().enumerate() {
            let target = i as i64 + 1;
            if target <= current {
                continue;
            }
            let tx = self.conn.transaction().map_err(db_err("Migration"))?;
            tx.execute_batch(migration)
                .map_err(|e| format!("Migration auf Version {target}: {e}"))?;
            tx.execute(
                "INSERT INTO meta(k, v) VALUES ('schema_version', ?1)
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                [target.to_string()],
            )
            .map_err(db_err("Migration/Version"))?;
            tx.commit().map_err(db_err("Migration/commit"))?;
        }
        Ok(())
    }
}

/// Einheitliche Fehlertexte: was gerade geschah plus SQLite-Meldung.
pub(crate) fn db_err(what: &'static str) -> impl Fn(rusqlite::Error) -> String {
    move |e| format!("{what}: {e}")
}


/// Uebernehmen eines angehaengten Snapshots (`snap`) in den eigenen Index.
///
/// Vorhandene Werte gewinnen: ein eigener Lauf ueber eine Quelle kennt mehr
/// als der Dienst, etwa die Bilder, die beim Oeffnen der Detailseite geholt
/// wurden. Nur leere Felder werden gefuellt.
const MERGE_WITH_SCREENSHOTS: &str = r#"
INSERT INTO addons (key, name, author, summary, homepage, icon_path, category, updated_at, screenshots_json)
SELECT key, name, author, summary, homepage, icon_path, category, updated_at, screenshots_json
FROM snap.addons
WHERE true
ON CONFLICT(key) DO UPDATE SET
    name = excluded.name,
    author = CASE WHEN addons.author = '' THEN excluded.author ELSE addons.author END,
    summary = CASE WHEN addons.summary = '' THEN excluded.summary ELSE addons.summary END,
    homepage = COALESCE(addons.homepage, excluded.homepage),
    icon_path = COALESCE(addons.icon_path, excluded.icon_path),
    category = COALESCE(addons.category, excluded.category),
    updated_at = COALESCE(addons.updated_at, excluded.updated_at),
    screenshots_json = CASE
        WHEN addons.screenshots_json IN ('', '[]') THEN excluded.screenshots_json
        ELSE addons.screenshots_json END;

INSERT INTO addon_sources (addon_id, source, external_id, version, released_at, download_url, size_bytes)
SELECT a.id, s.source, s.external_id, s.version, s.released_at, s.download_url, s.size_bytes
FROM snap.addon_sources s
JOIN snap.addons sa ON sa.id = s.addon_id
JOIN addons a ON a.key = sa.key
WHERE true
ON CONFLICT(addon_id, source) DO UPDATE SET
    external_id = excluded.external_id,
    version = excluded.version,
    released_at = excluded.released_at,
    download_url = excluded.download_url,
    size_bytes = excluded.size_bytes;

INSERT INTO releases (addon_id, source, version, interface_json, url, size_bytes, fetched_at)
SELECT a.id, r.source, r.version, r.interface_json, r.url, r.size_bytes, r.fetched_at
FROM snap.releases r
JOIN snap.addons sa ON sa.id = r.addon_id
JOIN addons a ON a.key = sa.key
WHERE true
ON CONFLICT(addon_id, source, version) DO UPDATE SET
    interface_json = excluded.interface_json,
    url = excluded.url,
    size_bytes = excluded.size_bytes,
    fetched_at = excluded.fetched_at;
"#;

/// Dasselbe fuer Indexdateien ohne Bildspalte (Schema 1).
const MERGE_WITHOUT_SCREENSHOTS: &str = r#"
INSERT INTO addons (key, name, author, summary, homepage, icon_path, category, updated_at)
SELECT key, name, author, summary, homepage, icon_path, category, updated_at
FROM snap.addons
WHERE true
ON CONFLICT(key) DO UPDATE SET
    name = excluded.name,
    author = CASE WHEN addons.author = '' THEN excluded.author ELSE addons.author END,
    summary = CASE WHEN addons.summary = '' THEN excluded.summary ELSE addons.summary END,
    homepage = COALESCE(addons.homepage, excluded.homepage),
    icon_path = COALESCE(addons.icon_path, excluded.icon_path),
    category = COALESCE(addons.category, excluded.category),
    updated_at = COALESCE(addons.updated_at, excluded.updated_at);

INSERT INTO addon_sources (addon_id, source, external_id, version, released_at, download_url, size_bytes)
SELECT a.id, s.source, s.external_id, s.version, s.released_at, s.download_url, s.size_bytes
FROM snap.addon_sources s
JOIN snap.addons sa ON sa.id = s.addon_id
JOIN addons a ON a.key = sa.key
WHERE true
ON CONFLICT(addon_id, source) DO UPDATE SET
    external_id = excluded.external_id,
    version = excluded.version,
    released_at = excluded.released_at,
    download_url = excluded.download_url,
    size_bytes = excluded.size_bytes;

INSERT INTO releases (addon_id, source, version, interface_json, url, size_bytes, fetched_at)
SELECT a.id, r.source, r.version, r.interface_json, r.url, r.size_bytes, r.fetched_at
FROM snap.releases r
JOIN snap.addons sa ON sa.id = r.addon_id
JOIN addons a ON a.key = sa.key
WHERE true
ON CONFLICT(addon_id, source, version) DO UPDATE SET
    interface_json = excluded.interface_json,
    url = excluded.url,
    size_bytes = excluded.size_bytes,
    fetched_at = excluded.fetched_at;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legt_datei_und_ordner_an() {
        let dir = tempfile::tempdir().expect("temporaeres Verzeichnis");
        // Zwei Ebenen tief, damit `create_dir_all` wirklich laeuft.
        let path = dir.path().join("hexwerk").join("index.db");
        let index = Index::open(&path).expect("Index oeffnen");
        assert!(path.exists());
        assert_eq!(index.schema_version(), SCHEMA_VERSION);
        assert_eq!(index.path(), Some(path.as_path()));
    }

    #[test]
    fn migration_ist_wiederholbar() {
        let dir = tempfile::tempdir().expect("temporaeres Verzeichnis");
        let path = dir.path().join("index.db");
        {
            let mut index = Index::open(&path).expect("erster Start");
            index
                .set_meta("last_sync", "2026-10-30T00:00:00.000Z")
                .expect("meta");
        }
        // Zweiter Start darf nicht neu migrieren und nichts verlieren.
        let index = Index::open(&path).expect("zweiter Start");
        assert_eq!(index.schema_version(), SCHEMA_VERSION);
        assert_eq!(
            index.meta("last_sync").expect("meta lesen").as_deref(),
            Some("2026-10-30T00:00:00.000Z")
        );
    }

    #[test]
    fn wal_ist_aktiv() {
        let dir = tempfile::tempdir().expect("temporaeres Verzeichnis");
        let index = Index::open(&dir.path().join("index.db")).expect("Index oeffnen");
        let mode: String = index
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal_mode");
        assert_eq!(mode.to_lowercase(), "wal");
        // Fremdschluessel sind sonst per Vorgabe aus; ohne sie wuerde das
        // Aufraeumen verschwundener Clients installierte Zeilen stehen lassen.
        let fk: i64 = index
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys");
        assert_eq!(fk, 1);
    }

    #[test]
    fn fts5_ist_vorhanden() {
        let index = Index::open_in_memory().expect("Index im Speicher");
        let count: i64 = index
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'addons_fts'",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master");
        assert_eq!(count, 1, "FTS5-Tabelle fehlt - SQLite ohne FTS5 gebaut?");
    }

    #[test]
    fn neuere_datei_wird_abgelehnt() {
        let dir = tempfile::tempdir().expect("temporaeres Verzeichnis");
        let path = dir.path().join("index.db");
        {
            let mut index = Index::open(&path).expect("Index oeffnen");
            index
                .set_meta("schema_version", &(SCHEMA_VERSION + 7).to_string())
                .expect("meta");
        }
        let err = Index::open(&path).expect_err("neuere Version muss abgelehnt werden");
        assert!(err.contains("Schema-Version"), "unerwartet: {err}");
    }

    #[test]
    fn nach_clear_bleibt_der_index_benutzbar() {
        let dir = tempfile::tempdir().expect("temporaeres Verzeichnis");
        let path = dir.path().join("index.db");
        {
            let mut index = Index::open(&path).expect("Index oeffnen");
            index.clear().expect("leeren");
            // Die Schemaversion bleibt stehen, sonst wuerde ein erneutes
            // Oeffnen die Migration noch einmal fahren.
            assert_eq!(index.schema_version(), SCHEMA_VERSION);
            index.set_meta("hint", "ok").expect("meta");
            assert_eq!(index.meta("hint").expect("meta").as_deref(), Some("ok"));
            assert!(index.meta("last_sync").expect("meta").is_none());
            index.rebuild_fts().expect("FTS neu bauen");
        }
        // Genau das ist der Fall, den ein geleertes `meta` zerstoeren wuerde.
        let reopened = Index::open(&path).expect("nach clear erneut oeffnen");
        assert_eq!(reopened.schema_version(), SCHEMA_VERSION);
        assert_eq!(reopened.meta("hint").expect("meta").as_deref(), Some("ok"));
    }
}
