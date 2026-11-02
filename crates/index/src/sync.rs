//! Abgleich Index <-> Dateisystem <-> Quellen.
//!
//! Reihenfolge wie in PLAN §6: `scan` (Dateisystem) und `sync` (Quellen)
//! schreiben in den Index, die Oberflaeche liest nur. Beide Richtungen sind
//! wiederholbar — ein zweiter Lauf mit denselben Daten aendert nichts, ein
//! Lauf mit weniger Clients raeumt die verschwundenen weg.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use hexwerk::{scan_addons, Discovery, InstalledAddon, Origin};
use hexwerk_sources::{Entry, Source};
use rusqlite::{params, Transaction};

use crate::queries::source_key;
use crate::schema::{db_err, Index};
use crate::util;
use crate::version;

/// Ergebnis eines Synchronlaufs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Client-Varianten, die der Index jetzt kennt.
    pub clients: usize,
    /// Installierte Addons, die der Index jetzt kennt.
    pub installed: usize,
    /// Katalogeintraege im Index nach dem Lauf.
    pub catalogue: usize,
    /// Eintraege, die ohne Schluessel und ohne Namen kamen und deshalb nicht
    /// uebernommen wurden — sichtbar, statt still zu verschwinden.
    pub skipped: usize,
    pub duration_ms: u128,
}

impl Index {
    /// Vollstaendiger Lauf: Dateisystem und Quellen in einem Rutsch.
    ///
    /// `discovery` ist das Ergebnis von [`hexwerk::discover`], `entries` das
    /// gemergte Ergebnis von [`hexwerk_sources::Registry::search`].
    pub fn sync(&mut self, discovery: &Discovery, entries: &[Entry]) -> Result<SyncReport, String> {
        let start = Instant::now();
        let (clients, installed) = self.write_clients(discovery)?;
        let (_, skipped) = self.write_catalogue(entries)?;
        self.set_meta("last_sync", &util::now_utc())?;
        Ok(SyncReport {
            clients,
            installed,
            catalogue: self.stats()?.catalogue,
            skipped,
            duration_ms: start.elapsed().as_millis(),
        })
    }

    /// Nur Dateisystem -> Index (der „Scan“-Knopf).
    ///
    /// Die Zahl der Katalogeintraege im Bericht ist der Bestand, den der Lauf
    /// vorgefunden hat; dieser Aufruf aendert ihn nicht.
    pub fn sync_clients(&mut self, discovery: &Discovery) -> Result<SyncReport, String> {
        let start = Instant::now();
        let (clients, installed) = self.write_clients(discovery)?;
        let stats = self.stats()?;
        Ok(SyncReport {
            clients,
            installed,
            catalogue: stats.catalogue,
            skipped: 0,
            duration_ms: start.elapsed().as_millis(),
        })
    }

    /// Nur Quellen -> Index (der Hintergrundsync).
    pub fn sync_catalogue(&mut self, entries: &[Entry]) -> Result<SyncReport, String> {
        let start = Instant::now();
        let (_, skipped) = self.write_catalogue(entries)?;
        let stats = self.stats()?;
        Ok(SyncReport {
            clients: stats.clients,
            installed: stats.installed,
            catalogue: stats.catalogue,
            skipped,
            duration_ms: start.elapsed().as_millis(),
        })
    }

    /// Schreibt Clients und installierte Addons.
    ///
    /// Ein `seen_at`-Marker je Lauf ersetzt den Vergleich „was war vorher da“:
    /// was der Lauf nicht gesehen hat, ist verschwunden (Praefix geloescht,
    /// Addon von Hand entfernt) und faellt raus.
    fn write_clients(&mut self, discovery: &Discovery) -> Result<(usize, usize), String> {
        let marker = util::run_marker();
        let mut clients = 0usize;
        let mut installed = 0usize;
        {
            let tx = self.conn_mut().transaction().map_err(db_err("Scan"))?;
            for client in &discovery.clients {
                for flavor in &client.flavors {
                    // Eine Zeile je Variante: derselbe Praefix kann Retail und
                    // Classic tragen, und installiert wird immer je Client
                    // (PLAN §13.5).
                    let row_id = format!("{}::{}", client.id, flavor.flavor.dir_name());
                    let interfaces = &flavor.interface_numbers;
                    tx.execute(
                        "INSERT INTO clients (id, origin, origin_label, label, prefix, game_dir,
                                              flavor, interface_no, interfaces_json, addons_dir,
                                              seen_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                         ON CONFLICT(id) DO UPDATE SET
                             origin = excluded.origin,
                             origin_label = excluded.origin_label,
                             label = excluded.label,
                             prefix = excluded.prefix,
                             game_dir = excluded.game_dir,
                             flavor = excluded.flavor,
                             interface_no = excluded.interface_no,
                             interfaces_json = excluded.interfaces_json,
                             addons_dir = excluded.addons_dir,
                             seen_at = excluded.seen_at",
                        params![
                            row_id,
                            origin_key(client.origin),
                            client.origin.label(),
                            client.label,
                            client.prefix.to_string_lossy(),
                            client.game_dir.to_string_lossy(),
                            flavor.flavor.dir_name(),
                            interfaces.iter().max().copied(),
                            util::json_u32(interfaces),
                            flavor.addons_dir.to_string_lossy(),
                            marker,
                        ],
                    )
                    .map_err(|e| format!("Client {} schreiben: {e}", client.id))?;
                    clients += 1;
                    installed += write_installed(&tx, &row_id, &flavor.addons_dir, &marker)?;
                }
            }
            // Clients, die dieser Lauf nicht gesehen hat, sind weg; die
            // installierten Zeilen haengen per ON DELETE CASCADE daran.
            tx.execute("DELETE FROM clients WHERE seen_at <> ?1", params![marker])
                .map_err(db_err("veraltete Clients"))?;
            tx.commit().map_err(db_err("Scan/commit"))?;
        }
        // Ohne Zuordnung gaebe es keine Update-Pruefung und keine Zeile
        // „lokal installiert“ in der Detailansicht.
        self.link_installed()?;
        Ok((clients, installed))
    }

    /// Schreibt den Katalog: je Eintrag ein Addon, je Quelle die hoechste
    /// Version, dazu alle Releases.
    ///
    /// Additiv: was eine Quelle diesmal nicht geliefert hat, bleibt mit
    /// `checked_at`/`fetched_at` stehen. Eine ausgefallene Quelle soll den
    /// Katalog nicht leeren (PLAN §5, Ausfallverhalten).
    fn write_catalogue(&mut self, entries: &[Entry]) -> Result<(usize, usize), String> {
        let now = util::now_utc();
        let mut written = 0usize;
        let mut skipped = 0usize;
        {
            let tx = self
                .conn_mut()
                .transaction()
                .map_err(db_err("Katalog schreiben"))?;
            for entry in entries {
                // Der Schluessel ist Pflicht (merge() garantiert ihn). Fehlt
                // er, ist das ein Fehler der Quelle — dann lieber ueberspringen
                // und zaehlen, als den ganzen Katalog zu verlieren.
                let key = if entry.key.trim().is_empty() {
                    util::norm(&entry.name)
                } else {
                    entry.key.trim().to_string()
                };
                if key.is_empty() {
                    skipped += 1;
                    continue;
                }
                let name = if entry.name.trim().is_empty() {
                    key.clone()
                } else {
                    entry.name.trim().to_string()
                };

                // Bilder sammeln statt ersetzen: dasselbe Addon kommt aus
                // mehreren Quellen, und die neueste Fassung gewinnt zwar bei der
                // Version, aber Bilder einer aelteren Fassung oder einer
                // zweiten Quelle bleiben sehenswert. Neue zuerst, dann die
                // bekannten, ohne Doppel, hoechstens acht.
                let known: String = tx
                    .query_row(
                        "SELECT screenshots_json FROM addons WHERE key = ?1",
                        [&key],
                        |row| row.get(0),
                    )
                    .unwrap_or_else(|_| "[]".to_string());
                let merged_shots = merge_screenshots(&known, &entry.screenshots);

                // `RETURNING id`: ein Schreibvorgang statt „upsert, dann
                // nachfragen“.
                let addon_id: i64 = tx
                    .query_row(
                        "INSERT INTO addons (key, name, author, summary, homepage, category,
                                            screenshots_json, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(key) DO UPDATE SET
                             name = excluded.name,
                             author = CASE WHEN excluded.author <> ''
                                           THEN excluded.author ELSE addons.author END,
                             summary = CASE WHEN excluded.summary <> ''
                                            THEN excluded.summary ELSE addons.summary END,
                             homepage = COALESCE(excluded.homepage, addons.homepage),
                             category = COALESCE(excluded.category, addons.category),
                             -- Der Wert ist oben schon vereinigt worden (neu plus
                             -- bekannt); hier wird er nur uebernommen.
                             screenshots_json = excluded.screenshots_json,
                             updated_at = excluded.updated_at
                         RETURNING id",
                        params![
                            key,
                            name,
                            entry.author,
                            entry.summary,
                            entry.homepage,
                            entry.category,
                            serde_json::to_string(&merged_shots).unwrap_or_else(|_| "[]".into()),
                            now,
                        ],
                        |row| row.get(0),
                    )
                    .map_err(|e| format!("Addon {key} schreiben: {e}"))?;

                let mut best: HashMap<Source, &hexwerk_sources::Release> = HashMap::new();
                // Bestehende Anzeige-Version je Quelle: eine Quelle, die kurz
                // nur alte Releases ausliefert, darf die Anzeige nicht
                // zurueckdrehen — sonst verschwaende ein faelliges Update.
                let stored = stored_versions(&tx, addon_id)?;
                for release in &entry.releases {
                    tx.execute(
                        "INSERT INTO releases (addon_id, source, version, interface_json, url,
                                               sha256, size_bytes, fetched_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)
                         ON CONFLICT(addon_id, source, version) DO UPDATE SET
                             interface_json = excluded.interface_json,
                             url = excluded.url,
                             size_bytes = excluded.size_bytes,
                             fetched_at = excluded.fetched_at",
                        params![
                            addon_id,
                            source_key(release.source),
                            release.version,
                            util::json_u32(&release.interface_numbers),
                            release.download_url,
                            release.size_bytes.and_then(|n| i64::try_from(n).ok()),
                            now,
                        ],
                    )
                    .map_err(|e| format!("Release {} schreiben: {e}", release.version))?;

                    // Je Quelle nur die hoechste Version in `addon_sources`:
                    // das ist die Zeile, die Katalog und Update-Pruefung lesen.
                    // Gleichstand -> das neuere Releasedatum gewinnt, damit die
                    // Anzeige nicht zwischen zwei Laeufen springt.
                    match best.get(&release.source) {
                        Some(current) => {
                            let newer = version::compare(&release.version, &current.version)
                                .then_with(|| release.released_at.cmp(&current.released_at));
                            if newer == std::cmp::Ordering::Greater {
                                best.insert(release.source, release);
                            }
                        }
                        None => {
                            best.insert(release.source, release);
                        }
                    }
                }

                for (source, release) in &best {
                    // Rueckwaerts nie: was schon als hoehere Version
                    // gespeichert ist, bleibt stehen (siehe `stored`).
                    if let Some(old) = stored.get(source_key(*source)) {
                        if !old.trim().is_empty()
                            && version::compare(&release.version, old) == Ordering::Less
                        {
                            continue;
                        }
                    }
                    tx.execute(
                        "INSERT INTO addon_sources (addon_id, source, external_id, version,
                                                    released_at, download_url, size_bytes, checked_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(addon_id, source) DO UPDATE SET
                             external_id = excluded.external_id,
                             version = excluded.version,
                             released_at = excluded.released_at,
                             download_url = excluded.download_url,
                             size_bytes = excluded.size_bytes,
                             checked_at = excluded.checked_at",
                        params![
                            addon_id,
                            source_key(*source),
                            release.external_id,
                            release.version,
                            release.released_at,
                            release.download_url,
                            release.size_bytes.and_then(|n| i64::try_from(n).ok()),
                            now,
                        ],
                    )
                    .map_err(|e| format!("Quelle {} schreiben: {e}", source.label()))?;
                }
                written += 1;
            }
            tx.commit().map_err(db_err("Katalog/commit"))?;
        }
        // Der Katalog ist die Grundlage der Zuordnung: erst jetzt lassen sich
        // installierte Addons wiederfinden. Ohne diesen Schritt gaebe es nach
        // einem reinen Kataloglauf keine Updates.
        self.link_installed()?;
        Ok((written, skipped))
    }

    /// Verknuepft installierte Addons mit Katalogeintraegen.
    ///
    /// Reihenfolge der Signale: externe Kennung aus dem `.toc` (PLAN §5, das
    /// staerkste Signal), dann Name aus dem `.toc`, dann Ordnername. Ohne
    /// Treffer bleibt `addon_id` NULL — lieber unverknuepft als falsch
    /// verknuepft, denn daran haengt die Update-Pruefung.
    pub fn link_installed(&mut self) -> Result<usize, String> {
        let tx = self.conn_mut().transaction().map_err(db_err("Zuordnung"))?;

        let mut by_name: HashMap<String, i64> = HashMap::new();
        {
            let mut stmt = tx
                .prepare("SELECT id, key, name FROM addons ORDER BY id")
                .map_err(db_err("Zuordnung/Katalog"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(db_err("Zuordnung/Katalog"))?;
            for row in rows {
                let (id, key, name) = row.map_err(db_err("Zuordnung/Katalog"))?;
                // `or_insert`: bei Namensgleichheit gewinnt der aeltere
                // Eintrag, damit die Zuordnung stabil bleibt.
                by_name.entry(util::norm(&name)).or_insert(id);
                by_name.entry(util::norm(&key)).or_insert(id);
            }
        }

        let mut by_external: HashMap<String, i64> = HashMap::new();
        {
            let mut stmt = tx
                .prepare("SELECT addon_id, external_id FROM addon_sources WHERE external_id <> ''")
                .map_err(db_err("Zuordnung/Quellen"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(db_err("Zuordnung/Quellen"))?;
            for row in rows {
                let (id, external) = row.map_err(db_err("Zuordnung/Quellen"))?;
                by_external.entry(util::norm(&external)).or_insert(id);
            }
        }

        let mut pending = Vec::new();
        {
            let mut stmt = tx
                .prepare(
                    "SELECT client_id, folder, addon_id, title, foreign_keys_json FROM installed",
                )
                .map_err(db_err("Zuordnung/installiert"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(db_err("Zuordnung/installiert"))?;
            for row in rows {
                let (client_id, folder, current, title, fks) =
                    row.map_err(db_err("Zuordnung/installiert"))?;
                let target = match_installed(
                    &util::parse_string_map(&fks),
                    title.as_deref(),
                    &folder,
                    &by_name,
                    &by_external,
                );
                if target != current {
                    pending.push((target, client_id, folder));
                }
            }
        }

        let mut changed = 0usize;
        for (target, client_id, folder) in pending {
            changed += tx
                .execute(
                    "UPDATE installed SET addon_id = ?1 WHERE client_id = ?2 AND folder = ?3",
                    params![target, client_id, folder],
                )
                .map_err(db_err("Zuordnung/update"))?;
        }
        tx.commit().map_err(db_err("Zuordnung/commit"))?;
        Ok(changed)
    }
}

/// Sucht den Katalogeintrag zu einem installierten Ordner.
fn match_installed(
    foreign_keys: &std::collections::BTreeMap<String, String>,
    title: Option<&str>,
    folder: &str,
    by_name: &HashMap<String, i64>,
    by_external: &HashMap<String, i64>,
) -> Option<i64> {
    // Feste Reihenfolge: eine WoWInterface-ID ist eindeutiger als ein Name,
    // und der Ordnername ist die schwaechste Aussage (er wird beim Entpacken
    // notfalls aus dem `.toc` gebildet, PLAN §13.4).
    for key in ["wowinterface", "tukui", "curseforge", "wago"] {
        if let Some(value) = foreign_keys.get(key) {
            if let Some(id) = by_external.get(&util::norm(value)) {
                return Some(*id);
            }
        }
    }
    if let Some(title) = title {
        if let Some(id) = by_name.get(&util::norm(title)) {
            return Some(*id);
        }
    }
    by_name.get(&util::norm(folder)).copied()
}

/// Vereinigt bekannte und neue Bildadressen.
///
/// Reihenfolge: neue zuerst (die aktuelle Fassung ist der beste Beleg), dann die
/// schon bekannten; Duplikate fallen raus. Acht Adressen reichen fuer die
/// Detailseite und die Galerie, mehr waeren nur Ballast im Index.
fn merge_screenshots(known_json: &str, fresh: &[String]) -> Vec<String> {
    let known: Vec<String> = serde_json::from_str(known_json).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for url in fresh.iter().chain(known.iter()) {
        let url = url.trim();
        if url.is_empty() || out.iter().any(|seen| seen == url) {
            continue;
        }
        out.push(url.to_string());
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// Anzeige-Version je Quelle, wie sie gerade im Index steht.
fn stored_versions(tx: &Transaction<'_>, addon_id: i64) -> Result<HashMap<String, String>, String> {
    let mut stmt = tx
        .prepare("SELECT source, version FROM addon_sources WHERE addon_id = ?1")
        .map_err(db_err("Quellenstand lesen"))?;
    let rows = stmt
        .query_map([addon_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_err("Quellenstand lesen"))?;
    let mut out = HashMap::new();
    for row in rows {
        let (source, version) = row.map_err(db_err("Quellenstand lesen"))?;
        out.insert(source, version);
    }
    Ok(out)
}

/// Schreibt die installierten Addons eines Client-Ordners.
fn write_installed(
    tx: &Transaction<'_>,
    client_id: &str,
    addons_dir: &Path,
    marker: &str,
) -> Result<usize, String> {
    let addons = scan_addons(addons_dir);
    for addon in &addons {
        // `fingerprint` wird absichtlich nicht ueberschrieben: das Feld
        // gehoert dem Installer, und ein Scan hat die Dateien nicht gehasht.
        tx.execute(
            "INSERT INTO installed (client_id, folder, title, version, interface_json,
                                    foreign_keys_json, size_bytes, seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(client_id, folder) DO UPDATE SET
                 title = excluded.title,
                 version = excluded.version,
                 interface_json = excluded.interface_json,
                 foreign_keys_json = excluded.foreign_keys_json,
                 size_bytes = excluded.size_bytes,
                 seen_at = excluded.seen_at",
            params![
                client_id,
                addon.folder,
                addon.toc.as_ref().and_then(|t| t.title.clone()),
                addon.version,
                util::json_u32(&addon.interface_numbers),
                foreign_keys_json(addon),
                i64::try_from(addon.size_bytes).unwrap_or(i64::MAX),
                marker,
            ],
        )
        .map_err(|e| format!("installiertes Addon {} schreiben: {e}", addon.folder))?;
    }
    // Ordner, die es nicht mehr gibt, verschwinden auch aus dem Index.
    tx.execute(
        "DELETE FROM installed WHERE client_id = ?1 AND seen_at <> ?2",
        params![client_id, marker],
    )
    .map_err(db_err("veraltete Addons"))?;
    Ok(addons.len())
}

/// Herkunft als stabile Kennung im Index.
///
/// Delegiert an `Origin::slug()`, damit es nur eine Liste gibt: die Kopie hier
/// lief bei jeder neuen Herkunft dem Enum hinterher.
fn origin_key(origin: Origin) -> &'static str {
    origin.slug()
}

/// Externe Kennungen aus dem `.toc` als JSON.
///
/// Nur gesetzte Felder: ein `null` je Quelle waere Rauschen, und die
/// Zuordnung fragt gezielt nach einzelnen Schluesseln.
fn foreign_keys_json(addon: &InstalledAddon) -> String {
    let Some(ids) = addon.toc.as_ref().map(|toc| &toc.source_ids) else {
        return "{}".to_string();
    };
    let mut map = serde_json::Map::new();
    for (key, value) in [
        ("wowinterface", &ids.wowinterface),
        ("tukui", &ids.tukui),
        ("curseforge", &ids.curseforge),
        ("wago", &ids.wago),
        ("website", &ids.website),
        ("github", &ids.github),
    ] {
        if let Some(v) = value {
            if !v.trim().is_empty() {
                map.insert(key.to_string(), serde_json::Value::String(v.clone()));
            }
        }
    }
    serde_json::Value::Object(map).to_string()
}
