//! Suchabfragen, Filter und alles, was die Oberflaeche aus dem Index liest.
//!
//! Grundsatz: jede Frage der Oberflaeche ist eine Methode hier. Wird etwas
//! gebraucht, das fehlt, kommt es hierher — nicht als direkter Netzaufruf in
//! die UI (PLAN §6).

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use hexwerk_sources::Source;
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::schema::Index;
use crate::util;
use crate::version;

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

impl Filters {
    /// Filter mit Suchtext, sonst nichts.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }
}

/// Kennung einer Quelle im Index (`addons.source`). Text statt Zahl, damit die
/// Datei auch mit `sqlite3` lesbar bleibt.
pub fn source_key(source: Source) -> &'static str {
    match source {
        Source::GitHub => "github",
        Source::Tukui => "tukui",
        Source::WowInterface => "wowi",
        Source::Wago => "wago",
        Source::CurseForge => "curseforge",
        Source::Local => "local",
    }
}

/// Rueckweg von [`source_key`]. Unbekannte Kennungen (aeltere Datei, Tippfehler
/// von Hand) liefern `None`, statt zu raten.
pub fn source_from_key(key: &str) -> Option<Source> {
    match key {
        "github" => Some(Source::GitHub),
        "tukui" => Some(Source::Tukui),
        "wowi" => Some(Source::WowInterface),
        "wago" => Some(Source::Wago),
        "curseforge" => Some(Source::CurseForge),
        "local" => Some(Source::Local),
        _ => None,
    }
}

/// Ein Release einer Quelle in der Anzeige: je Quelle die hoechste Version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRelease {
    pub source: Source,
    /// Projekt-ID bei dieser Quelle (fuer den Link zur Projektseite).
    pub external_id: String,
    pub version: String,
    pub released_at: Option<String>,
    pub download_url: String,
    pub size_bytes: Option<u64>,
    /// Interface-Nummern dieses Releases, falls bekannt.
    pub interfaces: Vec<u32>,
}

/// Eine Zeile aus `releases` — die Versionshistorie fuer den Tab „Dateien“.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRow {
    pub source: Source,
    pub version: String,
    pub interfaces: Vec<u32>,
    pub url: String,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub fetched_at: Option<String>,
}

/// Ein Katalogeintrag, wie ihn Katalogliste und Detailansicht brauchen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueItem {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub author: String,
    pub summary: String,
    pub homepage: Option<String>,
    pub icon_path: Option<String>,
    pub category: Option<String>,
    /// Je Quelle die hoechste Version, nach Quelle sortiert.
    pub sources: Vec<SourceRelease>,
    /// Anzeige-Version: die hoechste ueber alle Quellen (PLAN §5).
    pub latest: Option<SourceRelease>,
    /// Alle bekannten Versionen; im Katalog meist leer gelassen, in der
    /// Detailansicht gefuellt.
    pub releases: Vec<ReleaseRow>,
    /// Installierte Fassungen dieses Addons, eine je Client. Leer, wenn es
    /// nirgends installiert ist.
    pub installed: Vec<InstalledRow>,
}

impl CatalogueItem {
    /// Passt der Eintrag zu einer Interface-Nummer? `None` = unbekannt, dann
    /// gilt er als passend — ein Addon ohne `.toc`-Angabe soll nicht
    /// unsichtbar werden.
    pub fn accepts_interface(&self, number: u32) -> bool {
        let known: Vec<u32> = self
            .sources
            .iter()
            .flat_map(|s| s.interfaces.iter().copied())
            .collect();
        known.is_empty() || known.contains(&number)
    }
}

/// Ein Client samt Anzahl installierter Addons — die Karten in „Clients“.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRow {
    pub id: String,
    /// Kennung der Herkunft (`steam`, `faugus`, …), passend zu
    /// [`hexwerk::Origin`].
    pub origin: String,
    pub origin_label: String,
    /// Name aus dem Launcher, z. B. „Battle.net“.
    pub label: String,
    pub prefix: PathBuf,
    pub game_dir: PathBuf,
    /// Ordnername der Variante, z. B. `_retail_`.
    pub flavor: String,
    /// Alle Interface-Nummern, die im Ordner vorkommen.
    pub interfaces: Vec<u32>,
    pub addons_dir: PathBuf,
    pub installed_count: usize,
    pub seen_at: String,
}

/// Eine Zeile in „Installiert“.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRow {
    pub client_id: String,
    pub client_label: String,
    pub flavor: String,
    pub folder: String,
    /// Katalogeintrag, falls das Addon zugeordnet werden konnte.
    pub addon_id: Option<i64>,
    /// Anzeigename: Katalogname, sonst der Name aus dem `.toc`, sonst der
    /// Ordnername.
    pub name: String,
    pub version: Option<String>,
    pub interfaces: Vec<u32>,
    pub size_bytes: u64,
    pub fingerprint: Option<String>,
    /// Externe Kennungen aus dem `.toc` (`X-WoWI-ID`, `X-Tukui-Project-ID`, …).
    pub foreign_keys: BTreeMap<String, String>,
    /// Neuere Fassung vorhanden?
    pub update: Option<UpdateHint>,
}

/// Kurzhinweis „dafuer gibt es Version X bei Quelle Y“.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateHint {
    pub source: Source,
    pub version: String,
    pub download_url: String,
    pub released_at: Option<String>,
}

/// Ein faelliges Update. Eine Zeile je (Client, Addon, Quelle): dieselbe
/// installierte Fassung kann bei zwei Quellen veraltet sein, und die Wahl der
/// Quelle entscheidet ueber die Download-URL (PLAN §13.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub client_id: String,
    pub client_label: String,
    pub folder: String,
    pub addon_id: i64,
    pub addon_name: String,
    pub installed_version: String,
    pub source: Source,
    pub available_version: String,
    pub download_url: String,
    pub released_at: Option<String>,
}

/// Zahlen fuer die Fusszeile und den leeren Zustand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub catalogue: usize,
    pub clients: usize,
    pub installed: usize,
    pub updates: usize,
    /// Je Quelle die Zahl der Katalogeintraege (Filter-Chips).
    pub per_source: BTreeMap<&'static str, usize>,
}

/// Rohe Katalogzeile, bevor Quellen, Versionen und Installationen dazukommen.
struct RawAddon {
    id: i64,
    key: String,
    name: String,
    author: String,
    summary: String,
    homepage: Option<String>,
    icon_path: Option<String>,
    category: Option<String>,
}

impl Index {
    /// Katalogsuche.
    ///
    /// Leerer Suchtext liefert alle Eintraege nach Namen; sonst laeuft FTS5
    /// mit Praefix je Wort und Sortierung nach Relevanz (PLAN §6). Filter
    /// kommen als zusaetzliche `WHERE`-Bedingungen dazu, damit sie auch ohne
    /// Suchtext wirken.
    pub fn search(
        &self,
        filters: &Filters,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<CatalogueItem>, String> {
        // `updates_only` braucht den Versionsvergleich aus Rust; die IDs
        // werden vorab bestimmt und dann als `IN (…)` eingesetzt, statt die
        // halbe Update-Logik in SQL nachzubauen (dort fehlt der
        // Versionsvergleich).
        let update_ids = if filters.updates_only {
            self.updated_addon_ids()?
        } else {
            Vec::new()
        };
        let (sql, params) = search_sql(filters, &update_ids, limit, offset);
        let raws = self.query_raw(&sql, &params)?;
        self.assemble(raws, false)
    }

    /// Alle Suchtreffer, ohne Limit — fuer „alles markieren“ und Export.
    /// Obergrenze `u32::MAX`, damit ein Aufrufer nicht versehentlich in eine
    /// Endlosschleife laeuft.
    pub fn search_all(&self, filters: &Filters) -> Result<Vec<CatalogueItem>, String> {
        self.search(filters, u32::MAX, 0)
    }

    /// Ein Eintrag in voller Tiefe: alle Versionen, alle installierten
    /// Fassungen. `None`, wenn die ID nicht (mehr) existiert.
    pub fn addon(&self, id: i64) -> Result<Option<CatalogueItem>, String> {
        let raws = self.query_raw(
            "SELECT a.id, a.key, a.name, a.author, a.summary, a.homepage, a.icon_path, a.category
             FROM addons a WHERE a.id = ?",
            &[Value::Integer(id)],
        )?;
        let mut items = self.assemble(raws, true)?;
        Ok(items.pop())
    }

    fn query_raw(&self, sql: &str, params: &[Value]) -> Result<Vec<RawAddon>, String> {
        let mut stmt = self
            .conn()
            .prepare(sql)
            .map_err(|e| format!("Suche vorbereiten: {e}"))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(RawAddon {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    name: row.get(2)?,
                    author: row.get(3)?,
                    summary: row.get(4)?,
                    homepage: row.get(5)?,
                    icon_path: row.get(6)?,
                    category: row.get(7)?,
                })
            })
            .map_err(|e| format!("Suche ausfuehren: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("Treffer lesen: {e}"))?);
        }
        Ok(out)
    }

    /// Haengt Quellen, Versionshistorie und Installationen an die rohen
    /// Katalogzeilen. Zwei Sammelabfragen statt einer je Treffer: eine
    /// Katalogseite hat 50 Zeilen, 150 Abfragen waeren unnoetig.
    fn assemble(
        &self,
        raws: Vec<RawAddon>,
        with_releases: bool,
    ) -> Result<Vec<CatalogueItem>, String> {
        let ids: Vec<i64> = raws.iter().map(|r| r.id).collect();
        let mut sources = self.sources_for(&ids)?;
        let mut releases = if with_releases {
            self.releases_for(&ids)?
        } else {
            HashMap::new()
        };
        let mut installed = self.installed_for_addons(&ids)?;
        let mut out = Vec::with_capacity(raws.len());
        for raw in raws {
            let per_source = sources.remove(&raw.id).unwrap_or_default();
            // `latest` = hoechste Version ueber alle Quellen (PLAN §5). Bei
            // Gleichstand gewinnt das neuere Releasedatum, dann die Quelle in
            // fester Reihenfolge, damit die Anzeige nicht springt.
            let latest = per_source.iter().cloned().max_by(|a, b| {
                version::compare(&a.version, &b.version).then_with(|| {
                    a.released_at
                        .cmp(&b.released_at)
                        .then_with(|| b.source.cmp(&a.source))
                })
            });
            out.push(CatalogueItem {
                id: raw.id,
                key: raw.key,
                name: raw.name,
                author: raw.author,
                summary: raw.summary,
                homepage: raw.homepage,
                icon_path: raw.icon_path,
                category: raw.category,
                sources: per_source,
                latest,
                releases: releases.remove(&raw.id).unwrap_or_default(),
                installed: installed.remove(&raw.id).unwrap_or_default(),
            });
        }
        Ok(out)
    }
}

/// Baut die SQL-Abfrage der Suche. Getrennt von [`Index::search`], weil sich
/// das nur so ohne Datenbank testen laesst.
fn search_sql(
    filters: &Filters,
    update_ids: &[i64],
    limit: u32,
    offset: u32,
) -> (String, Vec<Value>) {
    let fts = fts_query(&filters.query);
    let mut params: Vec<Value> = Vec::new();
    let mut sql = String::new();
    if let Some(q) = &fts {
        // Treffer zuerst in einem CTE: bm25 und `MATCH` bleiben damit in
        // einer Abfrage, die FTS5 sicher als solches erkennt, und die
        // Relevanz steht fuer die Sortierung bereit.
        sql.push_str(
            "WITH hits AS (SELECT rowid AS id, bm25(addons_fts) AS score
                           FROM addons_fts WHERE addons_fts MATCH ?)\n",
        );
        params.push(Value::Text(q.clone()));
    }
    sql.push_str(
        "SELECT a.id, a.key, a.name, a.author, a.summary, a.homepage, a.icon_path, a.category
         FROM addons a",
    );
    if fts.is_some() {
        sql.push_str("\nJOIN hits ON hits.id = a.id");
    }

    let mut conds: Vec<String> = Vec::new();
    if !filters.sources.is_empty() {
        let marks = vec!["?"; filters.sources.len()].join(", ");
        conds.push(format!(
            "EXISTS (SELECT 1 FROM addon_sources s WHERE s.addon_id = a.id AND s.source IN ({marks}))"
        ));
        for source in &filters.sources {
            params.push(Value::Text(source_key(*source).to_string()));
        }
    }
    if let Some(iface) = filters.interface_number {
        // `json_each` liefert eine Zeile je Nummer der Liste — der einzige
        // Weg, „passt zu diesem Client“ ohne Zusatztabelle zu fragen. Der Join
        // auf `addon_sources` beschraenkt die Pruefung auf die Version, die
        // gerade angeboten wird: ein Release von 2019 fuer Classic darf ein
        // Addon nicht im Retail-Filter auftauchen lassen.
        conds.push(
            "EXISTS (SELECT 1
                     FROM addon_sources s
                     JOIN releases r ON r.addon_id = s.addon_id
                                    AND r.source = s.source
                                    AND r.version = s.version
                     JOIN json_each(r.interface_json) j
                     WHERE s.addon_id = a.id AND j.value = ?)"
                .to_string(),
        );
        params.push(Value::Integer(i64::from(iface)));
    }
    if filters.updates_only {
        if update_ids.is_empty() {
            // Ohne Updates muss die Bedingung etwas Unwahres sein; ein
            // weggelassener Filter waere ein stiller Fehler.
            conds.push("0".to_string());
        } else {
            let marks = vec!["?"; update_ids.len()].join(", ");
            conds.push(format!("a.id IN ({marks})"));
            for id in update_ids {
                params.push(Value::Integer(*id));
            }
        }
    }
    if !conds.is_empty() {
        sql.push_str("\nWHERE ");
        sql.push_str(&conds.join("\n  AND "));
    }
    if fts.is_some() {
        // bm25: kleiner ist besser.
        sql.push_str("\nORDER BY hits.score, a.name COLLATE NOCASE");
    } else {
        sql.push_str("\nORDER BY a.name COLLATE NOCASE");
    }
    sql.push_str("\nLIMIT ? OFFSET ?");
    params.push(Value::Integer(i64::from(limit)));
    params.push(Value::Integer(i64::from(offset)));
    (sql, params)
}

/// Uebersetzt freien Text in eine FTS5-Abfrage.
///
/// Zerlegt an allem, was kein Buchstabe, keine Ziffer und kein Unterstrich
/// ist; jedes Wort wird ein Praefix-Term, alle zusammen mit UND verknuepft
/// (Vorgabe von FTS5). Damit kann eine Eingabe die Abfrage nicht zerlegen:
/// Anfuehrungszeichen und Operatoren kommen nie in einen Term hinein.
///
/// `None` = kein Suchtext (dann sortiert der Katalog nach Namen).
fn fts_query(raw: &str) -> Option<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in raw.chars() {
        if c.is_alphanumeric() || c == '_' {
            current.push(c);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    if words.is_empty() {
        return None;
    }
    Some(
        words
            .iter()
            .map(|w| format!("\"{w}\"*"))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// `?, ?, ?` fuer `IN (…)`-Listen.
fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// „Battle.net · Retail“ — Variante aus dem Ordnernamen uebersetzt, damit die
/// UI nicht selbst `_retail_`-Namen auseinandernehmen muss.
fn flavor_label(dir_name: &str) -> String {
    hexwerk::Flavor::from_dir_name(dir_name).label().to_string()
}

impl InstalledRow {
    /// Anzeigename des Clients samt Variante.
    pub fn client_display(&self) -> String {
        format!("{} · {}", self.client_label, flavor_label(&self.flavor))
    }
}

/// Die Quellen, die eine neuere Fassung anbieten als die installierte.
///
/// Ohne installierte Version gibt es keinen Vergleich — dann bleibt die Liste
/// leer, statt „alles ist veraltet“ zu behaupten: eine fehlende
/// `## Version` im `.toc` ist kein Update.
fn newer_sources<'a>(installed: &str, sources: &'a [SourceRelease]) -> Vec<&'a SourceRelease> {
    if installed.trim().is_empty() {
        return Vec::new();
    }
    sources
        .iter()
        .filter(|s| {
            !s.version.trim().is_empty()
                && version::compare(&s.version, installed) == Ordering::Greater
        })
        .collect()
}

impl Index {
    /// Je Addon die Quellenzeilen mit ihrer hoechsten Version.
    ///
    /// Die Interface-Liste kommt aus genau dem Release, das in
    /// `addon_sources` eingetragen ist; der `LEFT JOIN` haelt den Fall
    /// „Version ohne Release-Zeile“ am Leben, statt die Quelle zu verlieren.
    fn sources_for(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<SourceRelease>>, String> {
        let mut out: HashMap<i64, Vec<SourceRelease>> = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        let sql = format!(
            "SELECT s.addon_id, s.source, s.external_id, s.version, s.released_at,
                    s.download_url, s.size_bytes, COALESCE(r.interface_json, '[]')
             FROM addon_sources s
             LEFT JOIN releases r
                    ON r.addon_id = s.addon_id
                   AND r.source = s.source
                   AND r.version = s.version
             WHERE s.addon_id IN ({})
             ORDER BY s.addon_id, s.source",
            placeholders(ids.len())
        );
        let params: Vec<Value> = ids.iter().map(|id| Value::Integer(*id)).collect();
        let mut stmt = self
            .conn()
            .prepare(&sql)
            .map_err(|e| format!("Quellen lesen: {e}"))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| format!("Quellen lesen: {e}"))?;
        for row in rows {
            let (addon_id, source, external_id, version, released_at, download_url, size, ifaces) =
                row.map_err(|e| format!("Quelle lesen: {e}"))?;
            // Unbekannte Quellenkennung: ueberspringen statt raten. Sie kann
            // nur aus einer aelteren Datei oder einem Eingriff von Hand kommen.
            let Some(source) = source_from_key(&source) else {
                continue;
            };
            out.entry(addon_id).or_default().push(SourceRelease {
                source,
                external_id,
                version,
                released_at,
                download_url,
                size_bytes: size.and_then(|n| u64::try_from(n).ok()),
                interfaces: util::parse_u32_list(&ifaces),
            });
        }
        Ok(out)
    }

    /// Versionshistorie je Addon, je Quelle absteigend.
    fn releases_for(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<ReleaseRow>>, String> {
        let mut out: HashMap<i64, Vec<ReleaseRow>> = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        let sql = format!(
            "SELECT addon_id, source, version, interface_json, url, size_bytes, sha256, fetched_at
             FROM releases WHERE addon_id IN ({})",
            placeholders(ids.len())
        );
        let params: Vec<Value> = ids.iter().map(|id| Value::Integer(*id)).collect();
        let mut stmt = self
            .conn()
            .prepare(&sql)
            .map_err(|e| format!("Versionen lesen: {e}"))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| format!("Versionen lesen: {e}"))?;
        for row in rows {
            let (addon_id, source, version, ifaces, url, size, sha256, fetched_at) =
                row.map_err(|e| format!("Version lesen: {e}"))?;
            let Some(source) = source_from_key(&source) else {
                continue;
            };
            out.entry(addon_id).or_default().push(ReleaseRow {
                source,
                version,
                interfaces: util::parse_u32_list(&ifaces),
                url,
                size_bytes: size.and_then(|n| u64::try_from(n).ok()),
                sha256,
                fetched_at,
            });
        }
        for rows in out.values_mut() {
            // Neueste zuerst: so steht im Tab „Dateien“ oben, was man will.
            rows.sort_by(|a, b| {
                a.source
                    .cmp(&b.source)
                    .then_with(|| version::compare(&b.version, &a.version))
            });
        }
        Ok(out)
    }

    /// Installierte Addons, optional nur fuer einen Client.
    pub fn installed(&self, client_id: Option<&str>) -> Result<Vec<InstalledRow>, String> {
        self.installed_rows(client_id, None)
    }

    /// Installierte Addons je Katalogeintrag — fuer die Zeile „lokal
    /// installiert“ in der Detailansicht.
    fn installed_for_addons(
        &self,
        addon_ids: &[i64],
    ) -> Result<HashMap<i64, Vec<InstalledRow>>, String> {
        let mut out: HashMap<i64, Vec<InstalledRow>> = HashMap::new();
        for row in self.installed_rows(None, Some(addon_ids))? {
            if let Some(id) = row.addon_id {
                out.entry(id).or_default().push(row);
            }
        }
        Ok(out)
    }

    fn installed_rows(
        &self,
        client_id: Option<&str>,
        addon_ids: Option<&[i64]>,
    ) -> Result<Vec<InstalledRow>, String> {
        if addon_ids.is_some_and(<[i64]>::is_empty) {
            return Ok(Vec::new());
        }
        let mut sql = String::from(
            "SELECT i.client_id, c.label, c.flavor, i.folder, i.addon_id,
                    COALESCE(a.name, i.title, i.folder), i.version, i.interface_json,
                    i.size_bytes, i.fingerprint, i.foreign_keys_json
             FROM installed i
             JOIN clients c ON c.id = i.client_id
             LEFT JOIN addons a ON a.id = i.addon_id",
        );
        let mut params: Vec<Value> = Vec::new();
        let mut conds: Vec<String> = Vec::new();
        if let Some(client) = client_id {
            conds.push("i.client_id = ?".to_string());
            params.push(Value::Text(client.to_string()));
        }
        if let Some(ids) = addon_ids {
            conds.push(format!("i.addon_id IN ({})", placeholders(ids.len())));
            params.extend(ids.iter().map(|id| Value::Integer(*id)));
        }
        if !conds.is_empty() {
            sql.push_str("\nWHERE ");
            sql.push_str(&conds.join("\n  AND "));
        }
        // Herkunft zuerst: auf einer Karte steht „Steam · Battle.net“ vor der
        // Variante, und die Reihenfolge soll nicht von der Einfuegereihenfolge
        // abhaengen.
        sql.push_str("\nORDER BY c.origin_label, c.label, c.flavor, i.folder COLLATE NOCASE");

        let mut raw = Vec::new();
        {
            let mut stmt = self
                .conn()
                .prepare(&sql)
                .map_err(|e| format!("Installierte lesen: {e}"))?;
            let rows = stmt
                .query_map(params_from_iter(params.iter()), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                })
                .map_err(|e| format!("Installierte lesen: {e}"))?;
            for row in rows {
                raw.push(row.map_err(|e| format!("Installiertes Addon lesen: {e}"))?);
            }
        }

        // Update-Hinweise brauchen die Quellen aller beteiligten Addons; ein
        // Aufruf fuer die ganze Liste statt einer je Zeile.
        let ids: Vec<i64> = {
            let mut v: Vec<i64> = raw.iter().filter_map(|r| r.4).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let sources = self.sources_for(&ids)?;
        let mut out = Vec::with_capacity(raw.len());
        for (client_id, label, flavor, folder, addon_id, name, version, ifaces, size, fp, fks) in
            raw
        {
            // Die hoechste angebotene Fassung gewinnt den Hinweis; welche
            // Quelle im Einzelnen neu ist, steht in `updates()`. `get` statt
            // `remove`: dasselbe Addon kann bei mehreren Clients liegen, und
            // jede Zeile braucht ihren Hinweis.
            let update = match (&version, addon_id) {
                (Some(v), Some(id)) => {
                    let offered = sources.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                    offered
                        .iter()
                        .filter(|s| {
                            !s.version.trim().is_empty()
                                && version::compare(&s.version, v) == Ordering::Greater
                        })
                        .max_by(|a, b| version::compare(&a.version, &b.version))
                        .map(|s| UpdateHint {
                            source: s.source,
                            version: s.version.clone(),
                            download_url: s.download_url.clone(),
                            released_at: s.released_at.clone(),
                        })
                }
                _ => None,
            };
            out.push(InstalledRow {
                client_id,
                client_label: label,
                flavor,
                folder,
                addon_id,
                name,
                version,
                interfaces: util::parse_u32_list(&ifaces),
                size_bytes: u64::try_from(size).unwrap_or(0),
                fingerprint: fp,
                foreign_keys: util::parse_string_map(&fks),
                update,
            });
        }
        Ok(out)
    }

    /// Faellige Updates, eine Zeile je (Client, Addon, Quelle).
    ///
    /// Verglichen wird die installierte Version gegen die hoechste
    /// Release-Version **jeder** Quelle (PLAN §6): dieselbe Fassung kann bei
    /// einer Quelle aktuell und bei einer anderen veraltet sein, und die
    /// Quellenwahl entscheidet ueber die Download-URL.
    pub fn updates(&self) -> Result<Vec<Update>, String> {
        let mut raw = Vec::new();
        {
            let mut stmt = self
                .conn()
                .prepare(
                    "SELECT i.client_id, c.label, i.folder, i.addon_id, a.name, i.version
                     FROM installed i
                     JOIN clients c ON c.id = i.client_id
                     JOIN addons a ON a.id = i.addon_id
                     WHERE i.version IS NOT NULL AND TRIM(i.version) <> ''
                     ORDER BY a.name COLLATE NOCASE, c.origin_label, c.label, i.folder",
                )
                .map_err(|e| format!("Updates lesen: {e}"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })
                .map_err(|e| format!("Updates lesen: {e}"))?;
            for row in rows {
                raw.push(row.map_err(|e| format!("Update lesen: {e}"))?);
            }
        }
        let ids: Vec<i64> = {
            let mut v: Vec<i64> = raw.iter().map(|r| r.3).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let sources = self.sources_for(&ids)?;
        let mut out = Vec::new();
        for (client_id, client_label, folder, addon_id, addon_name, installed) in raw {
            let per_source = sources.get(&addon_id).map(Vec::as_slice).unwrap_or(&[]);
            for source in newer_sources(&installed, per_source) {
                out.push(Update {
                    client_id: client_id.clone(),
                    client_label: client_label.clone(),
                    folder: folder.clone(),
                    addon_id,
                    addon_name: addon_name.clone(),
                    installed_version: installed.clone(),
                    source: source.source,
                    available_version: source.version.clone(),
                    download_url: source.download_url.clone(),
                    released_at: source.released_at.clone(),
                });
            }
        }
        Ok(out)
    }

    /// IDs der Katalogeintraege mit mindestens einem faelligen Update.
    pub fn updated_addon_ids(&self) -> Result<Vec<i64>, String> {
        let mut ids: Vec<i64> = self.updates()?.into_iter().map(|u| u.addon_id).collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// Zahl der Addons (nicht der Update-Zeilen) mit faelligem Update.
    pub fn update_count(&self) -> Result<usize, String> {
        Ok(self.updated_addon_ids()?.len())
    }

    /// Alle Client-Varianten.
    pub fn clients(&self) -> Result<Vec<ClientRow>, String> {
        self.client_rows(None)
    }

    /// Eine Client-Variante per ID, `None` wenn sie verschwunden ist.
    pub fn client(&self, id: &str) -> Result<Option<ClientRow>, String> {
        let mut rows = self.client_rows(Some(id))?;
        Ok(rows.pop())
    }

    /// Client-Varianten, die eine Interface-Nummer akzeptieren — die Frage
    /// „wohin installiere ich dieses Addon?“ (PLAN §4).
    pub fn clients_accepting(&self, interface_no: u32) -> Result<Vec<ClientRow>, String> {
        Ok(self
            .client_rows(None)?
            .into_iter()
            .filter(|c| c.interfaces.is_empty() || c.interfaces.contains(&interface_no))
            .collect())
    }

    fn client_rows(&self, id: Option<&str>) -> Result<Vec<ClientRow>, String> {
        let mut sql = String::from(
            "SELECT c.id, c.origin, c.origin_label, c.label, c.prefix, c.game_dir, c.flavor,
                    c.interfaces_json, c.addons_dir, c.seen_at,
                    (SELECT COUNT(*) FROM installed i WHERE i.client_id = c.id)
             FROM clients c",
        );
        let mut params: Vec<Value> = Vec::new();
        if let Some(id) = id {
            sql.push_str("\nWHERE c.id = ?");
            params.push(Value::Text(id.to_string()));
        }
        sql.push_str("\nORDER BY c.origin_label, c.label, c.flavor");
        let mut stmt = self
            .conn()
            .prepare(&sql)
            .map_err(|e| format!("Clients lesen: {e}"))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                ))
            })
            .map_err(|e| format!("Clients lesen: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id,
                origin,
                origin_label,
                label,
                prefix,
                game_dir,
                flavor,
                ifaces,
                addons_dir,
                seen_at,
                installed_count,
            ) = row.map_err(|e| format!("Client lesen: {e}"))?;
            out.push(ClientRow {
                id,
                origin,
                origin_label,
                label,
                prefix: PathBuf::from(prefix),
                game_dir: PathBuf::from(game_dir),
                flavor,
                interfaces: util::parse_u32_list(&ifaces),
                addons_dir: PathBuf::from(addons_dir),
                installed_count: usize::try_from(installed_count).unwrap_or(0),
                seen_at,
            });
        }
        Ok(out)
    }

    /// Zahlen fuer Kopfzeile, Filter-Chips und leeren Zustand.
    pub fn stats(&self) -> Result<Stats, String> {
        Ok(Stats {
            catalogue: self.count("SELECT COUNT(*) FROM addons")?,
            clients: self.count("SELECT COUNT(*) FROM clients")?,
            installed: self.count("SELECT COUNT(*) FROM installed")?,
            updates: self.update_count()?,
            per_source: self.source_counts()?,
        })
    }

    /// Zahl der Katalogeintraege je Quelle (Filter-Chips im Katalog).
    pub fn source_counts(&self) -> Result<BTreeMap<&'static str, usize>, String> {
        let mut stmt = self
            .conn()
            .prepare("SELECT source, COUNT(*) FROM addon_sources GROUP BY source ORDER BY source")
            .map_err(|e| format!("Quellenzahlen lesen: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| format!("Quellenzahlen lesen: {e}"))?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (key, n) = row.map_err(|e| format!("Quellenzahl lesen: {e}"))?;
            if let Some(source) = source_from_key(&key) {
                out.insert(source_key(source), usize::try_from(n).unwrap_or(0));
            }
        }
        Ok(out)
    }

    fn count(&self, sql: &str) -> Result<usize, String> {
        let n: i64 = self
            .conn()
            .query_row(sql, [], |row| row.get(0))
            .map_err(|e| format!("zaehlen: {e}"))?;
        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Setzt den Fingerprint eines installierten Ordners.
    ///
    /// Getrennt vom Scan: Dateien hashen dauert, der Scan soll in Sekunden
    /// fertig sein (PLAN §6). Der Installer setzt den Wert nach dem Entpacken.
    pub fn set_fingerprint(
        &mut self,
        client_id: &str,
        folder: &str,
        fingerprint: &str,
    ) -> Result<(), String> {
        let changed = self
            .conn_mut()
            .execute(
                "UPDATE installed SET fingerprint = ?1 WHERE client_id = ?2 AND folder = ?3",
                rusqlite::params![fingerprint, client_id, folder],
            )
            .map_err(|e| format!("Fingerprint schreiben: {e}"))?;
        if changed == 0 {
            return Err(format!(
                "kein installiertes Addon {folder:?} bei Client {client_id:?}"
            ));
        }
        Ok(())
    }
}
