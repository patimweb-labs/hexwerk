//! ZIP entpacken, oberste Ebene bestimmen, Ordnername aus dem `.toc`.
//!
//! Der Ablauf ist bewusst zweistufig: erst wird das Archiv nur *gelesen* und
//! vermessen ([`analyse`]), dann wird in einen Arbeitsordner entpackt und per
//! `rename` eingesetzt. Ein Fehler beim Lesen (Zip-Slip, Symlink, kaputtes
//! Archiv) kostet damit nie einen vorhandenen Addon-Ordner.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Cursor, ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use hexwerk::toc::{parse_str, Toc};
use zip::ZipArchive;

use crate::backup;
use crate::plan::Installed;
use crate::{InstallError, Paths};

/// Was ein ZIP ergibt, ohne dass etwas geschrieben wurde.
///
/// Fuer die Oberflaeche gedacht: sie kann vor dem Klick zeigen, welcher Ordner
/// entsteht und welche Version drinsteht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipLayout {
    /// Ordner, die im Addon-Verzeichnis entstehen. Bei einem Buendel (ElvUI und
    /// ElvUI_Libraries im selben ZIP) mehr als einer.
    pub folders: Vec<String>,
    /// `.toc` im ZIP, aus der die Version stammt (Pfad innerhalb des ZIP).
    pub toc_name: Option<String>,
    pub version: Option<String>,
}

/// Wie das Archiv eingesetzt wird.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Publish {
    /// Ein Ordner unter diesem Namen.
    One(String),
    /// Mehrere Geschwisterordner; WoW erwartet sie nebeneinander, nicht
    /// ineinander.
    Siblings(Vec<String>),
}

/// Ergebnis der Analyse: was zu tun ist und welche Eintraege wohin.
struct Plan {
    publish: Publish,
    /// ZIP-Index und Zielpfad relativ zum Arbeitsordner.
    entries: Vec<(usize, PathBuf)>,
    layout: ZipLayout,
}

/// Prueft, ob der Ordnername als ein Verzeichnisname taugt.
///
/// Ein Name mit `/` waere ein Pfad und koennte aus dem Addon-Verzeichnis
/// ausbrechen; `..` und fuehrende Punkte sind aus demselben Grund verboten.
pub(crate) fn sanitize_folder(name: &str) -> Result<String, InstallError> {
    let name = name.trim();
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0');
    if bad {
        return Err(InstallError::InvalidFolder(name.to_string()));
    }
    Ok(name.to_string())
}

/// Vermisst ein ZIP, ohne etwas zu schreiben.
pub fn inspect_zip(data: &[u8], folder: Option<&str>) -> Result<ZipLayout, InstallError> {
    Ok(analyse(data, folder)?.layout)
}

/// Entpackt `data` nach `addons_dir/<name>`.
///
/// `folder` ueberschreibt den aus dem Archiv abgeleiteten Namen (`None` = aus
/// dem ZIP ableiten). Vorhandenes wird vorher unter `paths.backup_dir` gesichert.
pub fn install_zip(
    data: &[u8],
    addons_dir: &Path,
    folder: Option<&str>,
    paths: &Paths,
) -> Result<Installed, InstallError> {
    let plan = analyse(data, folder)?;
    fs::create_dir_all(addons_dir)?;

    // Arbeitsordner im Ziel: `rename` bleibt damit auf einem Dateisystem
    // (geraeteuebergreifend scheitert es mit EXDEV).
    let mut staging = Staging::create(addons_dir)?;
    extract(data, &staging.path, &plan.entries)?;

    // Erst sichern, dann ersetzen: scheitert die Sicherung (Platte voll),
    // bleibt der alte Stand unangetastet statt halb ersetzt.
    match &plan.publish {
        Publish::One(name) => {
            if addons_dir.join(name).exists() {
                backup::backup_folder(addons_dir, name, paths)?;
            }
        }
        Publish::Siblings(dirs) => {
            for dir in dirs {
                if addons_dir.join(dir).exists() {
                    backup::backup_folder(addons_dir, dir, paths)?;
                }
            }
        }
    }

    let installed = match &plan.publish {
        Publish::One(name) => {
            let target = addons_dir.join(name);
            remove_existing(&target)?;
            fs::rename(&staging.path, &target)?;
            staging.disarm();
            target
        }
        Publish::Siblings(dirs) => {
            for dir in dirs {
                let target = addons_dir.join(dir);
                remove_existing(&target)?;
                fs::rename(staging.path.join(dir), &target)?;
            }
            // Der Arbeitsordner bleibt bis hierhin bestehen; was danach noch
            // drinliegt (Beidateien auf der Archivwurzel), raeumt sein Drop weg.
            let primary = plan.layout.folders.first().cloned().unwrap_or_default();
            addons_dir.join(primary)
        }
    };

    Ok(Installed {
        folder: installed,
        version: plan.layout.version,
    })
}

fn open(data: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, InstallError> {
    ZipArchive::new(Cursor::new(data)).map_err(InstallError::from)
}

fn analyse(data: &[u8], requested: Option<&str>) -> Result<Plan, InstallError> {
    let requested = match requested {
        Some(name) => Some(sanitize_folder(name)?),
        None => None,
    };

    let mut archive = open(data)?;
    // (ZIP-Index, relativer Pfad, ist Verzeichnis)
    let mut raw: Vec<(usize, PathBuf, bool)> = Vec::new();
    // Erste Pfadteile: einmal die, die Ordner sind (tiefere Eintraege oder
    // ausdrueckliche Ordner-Eintraege), einmal die Dateien auf der Wurzel.
    let mut top_dirs: BTreeSet<String> = BTreeSet::new();
    let mut top_files: BTreeSet<String> = BTreeSet::new();

    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name()?.to_string();
        let is_dir = entry.is_dir();

        // Symlinks zuerst ablehnen: beim Entpacken entstuende ein Link auf ein
        // Ziel ausserhalb des Addon-Ordners, den wir nicht kontrollieren.
        if entry.is_symlink() {
            return Err(InstallError::UnsafeEntry(name));
        }

        // Ungueltige Namen sind ein Angriff oder ein kaputtes Archiv. Ablehnen
        // statt reparieren - nur macOS-Reste gelten als erlaubter Sonderfall.
        let Some(rel) = safe_relative(&name) else {
            return Err(InstallError::UnsafeEntry(name));
        };
        if is_macos_junk(&rel) {
            continue;
        }

        if let Some(Component::Normal(first)) = rel.components().next() {
            let first = first.to_string_lossy().to_string();
            if rel.components().count() > 1 || is_dir {
                top_dirs.insert(first);
            } else {
                top_files.insert(first);
            }
        }
        raw.push((index, rel, is_dir));
    }

    if raw.is_empty() {
        return Err(InstallError::EmptyArchive);
    }

    // Oberste Ebene bestimmen.
    //
    // Eine `.toc` auf der Wurzel heisst "flaches ZIP": das Addon liegt direkt
    // dort, Unterordner sind Beiwerk (`lib`, `media`). Fehlt sie, ist die
    // Wurzel nur Verpackung und wird entfernt.
    let toc_on_root = top_files.iter().any(|f| is_toc(Path::new(f)));

    // Fall A: alles liegt unter genau einem Ordner -> der ist das Addon.
    // Fall B: daneben liegen Beidateien (README, LICENSE) - der Ordner gewinnt;
    //         Beidateien sind kein Addon-Inhalt und werden verworfen.
    let strip: Option<String> = if top_dirs.len() == 1 && !toc_on_root {
        top_dirs.iter().next().cloned().and_then(|root| {
            let dir_has_toc = raw.iter().any(|(_, p, _)| {
                p.starts_with(&root) && p.as_path() != Path::new(&root) && is_toc(p)
            });
            // Nur strippen, wenn darunter wirklich ein Addon liegt - sonst
            // waere `lib/` der Ordnername.
            if top_files.is_empty() || dir_has_toc {
                Some(root)
            } else {
                None
            }
        })
    } else {
        None
    };

    let mut entries: Vec<(usize, PathBuf)> = Vec::new();
    for (index, rel, _) in &raw {
        let rel = match &strip {
            Some(root) => match rel.strip_prefix(root) {
                Ok(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => continue, // der Wurzelordner selbst
            },
            None => rel.clone(),
        };
        entries.push((*index, rel));
    }

    // Die `.toc` mit dem kuerzesten Pfad ist das Haupt-Addon; bei mehreren auf
    // derselben Ebene entscheidet der Name, damit das Ergebnis reproduzierbar
    // bleibt.
    let toc_entry = entries
        .iter()
        .filter(|(_, p)| is_toc(p))
        .min_by(|(_, a), (_, b)| {
            a.components()
                .count()
                .cmp(&b.components().count())
                .then_with(|| a.as_os_str().cmp(b.as_os_str()))
        })
        .cloned();

    let publish = if let Some(root) = &strip {
        Publish::One(requested.clone().unwrap_or_else(|| root.clone()))
    } else if top_dirs.len() >= 2 && !toc_on_root {
        // Buendel: jeder obere Ordner wird ein eigener Addon-Ordner. Ein
        // Wunschname ist hier sinnlos und wird deshalb ignoriert.
        Publish::Siblings(top_dirs.iter().cloned().collect())
    } else {
        // Flaches ZIP (PLAN §13.4): der Ordnername kommt aus der `.toc`.
        let from_toc = toc_entry
            .as_ref()
            .and_then(|(_, p)| p.file_stem())
            .and_then(|s| s.to_str())
            .map(str::to_string);
        let name = requested
            .clone()
            .or(from_toc)
            .ok_or_else(|| InstallError::Toc("keine .toc, Ordnername nicht ableitbar".into()))?;
        Publish::One(name)
    };

    let folders = match &publish {
        Publish::One(name) => vec![name.clone()],
        Publish::Siblings(dirs) => {
            // Der Ordner mit der Haupt-`.toc` zuerst, damit `Installed.folder`
            // auf das eigentliche Addon zeigt und nicht auf eine Bibliothek.
            let mut ordered = dirs.clone();
            if let Some((_, toc_path)) = &toc_entry {
                if let Some(Component::Normal(first)) = toc_path.components().next() {
                    let first = first.to_string_lossy().to_string();
                    if let Some(pos) = ordered.iter().position(|d| *d == first) {
                        ordered.swap(0, pos);
                    }
                }
            }
            ordered
        }
    };

    let (toc_name, version) = match &toc_entry {
        Some((index, path)) => {
            let text = read_entry(data, *index)?;
            let toc: Toc = parse_str(&text);
            (Some(path.to_string_lossy().into_owned()), toc.version)
        }
        None => (None, None),
    };

    Ok(Plan {
        publish,
        entries,
        layout: ZipLayout {
            folders,
            toc_name,
            version,
        },
    })
}

/// Liest einen Eintrag als Text. `.toc`-Dateien sind selten UTF-8 (CP1252 aus
/// alten Addons); ein verlustbehafteter String ist hier besser als ein Fehler,
/// weil nur `## Schluessel: Wert` gebraucht wird.
fn read_entry(data: &[u8], index: usize) -> Result<String, InstallError> {
    let mut archive = open(data)?;
    let mut entry = archive.by_index(index)?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn is_toc(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("toc"))
}

/// Macht aus einem ZIP-Namen einen relativen, sicheren Pfad.
///
/// `None` heisst: absolut, mit `..`, mit Laufwerksbuchstabe oder leer. Solche
/// Namen koennen das Ziel verlassen (Zip-Slip) und werden nicht "repariert",
/// weil eine Reparatur den Angriff nur verschleiert.
fn safe_relative(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }
    // Manche Packer schreiben Windows-Trenner; die sind kein Pfad-Ausbruch,
    // sondern nur eine andere Schreibweise.
    let normalized = raw.replace('\\', "/");
    let mut out = PathBuf::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(part) => {
                // "C:foo" waere unter Windows ein Laufwerk.
                if out.as_os_str().is_empty() && part.to_string_lossy().ends_with(':') {
                    return None;
                }
                out.push(part);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// macOS-Reste: `__MACOSX`, versteckte Dateien (`.DS_Store`) und die
/// Resource-Forks (`._Foo`), die beim Zippen unter macOS entstehen.
fn is_macos_junk(rel: &Path) -> bool {
    rel.components().any(|c| match c {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            name == "__MACOSX" || name.starts_with('.')
        }
        _ => false,
    })
}

fn extract(data: &[u8], dest: &Path, entries: &[(usize, PathBuf)]) -> Result<(), InstallError> {
    let mut archive = open(data)?;
    for (index, rel) in entries {
        let mut entry = archive.by_index(*index)?;
        let target = dest.join(rel);
        // Zweite Pruefung, obwohl der Pfad oben schon validiert wurde: `join`
        // darf das Ziel nie verlassen (Verteidigung in der Tiefe).
        if !target.starts_with(dest) {
            return Err(InstallError::UnsafeEntry(rel.display().to_string()));
        }
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&target)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn remove_existing(path: &Path) -> Result<(), InstallError> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Arbeitsordner, der sich beim Verlassen selbst aufraeumt.
///
/// Ohne das blieben bei jedem Fehler `.hexwerk-tmp-*`-Ordner im Addon-Verzeichnis
/// liegen - die sieht der Nutzer dann als kaputte Addons im Spiel.
struct Staging {
    path: PathBuf,
    armed: bool,
}

impl Staging {
    fn create(addons_dir: &Path) -> Result<Self, InstallError> {
        for _ in 0..16 {
            let path = addons_dir.join(format!(".hexwerk-tmp-{}", crate::unique_token()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path, armed: true }),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(InstallError::Io(io::Error::new(
            ErrorKind::AlreadyExists,
            "kein freier Arbeitsordner im Addon-Verzeichnis",
        )))
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{temp_dir, zip_of, FakeEntry};

    fn install(data: &[u8], addons: &Path, paths: &Paths) -> Result<Installed, InstallError> {
        install_zip(data, addons, None, paths)
    }

    fn setup(tag: &str) -> (PathBuf, Paths) {
        let base = temp_dir(tag);
        let addons = base.join("Wow/_retail_/Interface/AddOns");
        fs::create_dir_all(&addons).unwrap();
        (addons, Paths::at(base.join("data")))
    }

    #[test]
    fn flaches_zip_nimmt_ordner_aus_toc() {
        let (addons, paths) = setup("flach");
        let data = zip_of(&[
            FakeEntry::text("MyAddon.toc", "## Version: 1.2.3\n## Interface: 11509\n"),
            FakeEntry::text("core.lua", "-- tu was\n"),
            FakeEntry::text("lib/helper.lua", "-- hilf\n"),
        ]);

        let done = install(&data, &addons, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("1.2.3"));
        assert_eq!(done.folder, addons.join("MyAddon"));
        assert!(addons.join("MyAddon/MyAddon.toc").is_file());
        assert!(addons.join("MyAddon/lib/helper.lua").is_file());
        // Kein Arbeitsordner zurueckgeblieben.
        let leftovers: Vec<_> = fs::read_dir(&addons)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".hexwerk-tmp-"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn zip_mit_wurzelordner_behaelt_namen() {
        let (addons, paths) = setup("wurzel");
        let data = zip_of(&[
            FakeEntry::text("CoolAddon/CoolAddon.toc", "## Version: 4.0\n"),
            FakeEntry::text("CoolAddon/main.lua", ""),
        ]);

        let done = install(&data, &addons, &paths).unwrap();

        assert_eq!(done.folder, addons.join("CoolAddon"));
        assert!(addons.join("CoolAddon/main.lua").is_file());
        // Kein doppeltes CoolAddon/CoolAddon.
        assert!(!addons.join("CoolAddon/CoolAddon").exists());
    }

    #[test]
    fn ordner_wunsch_gewinnt() {
        let (addons, paths) = setup("wunsch");
        let data = zip_of(&[
            FakeEntry::text("CoolAddon/CoolAddon.toc", "## Version: 4.0\n"),
            FakeEntry::text("CoolAddon/main.lua", ""),
        ]);

        let done = install_zip(&data, &addons, Some("Umbenannt"), &paths).unwrap();

        assert_eq!(done.folder, addons.join("Umbenannt"));
        assert!(addons.join("Umbenannt/main.lua").is_file());
        assert!(!addons.join("CoolAddon").exists());
    }

    #[test]
    fn macos_reste_werden_ignoriert() {
        let (addons, paths) = setup("macos");
        let data = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 2\n"),
            FakeEntry::text("Tidy/core.lua", ""),
            FakeEntry::text("__MACOSX/Tidy/._Tidy.toc", "junk"),
            FakeEntry::text("Tidy/.DS_Store", "junk"),
            FakeEntry::text("Tidy/._core.lua", "junk"),
        ]);

        let done = install(&data, &addons, &paths).unwrap();

        assert_eq!(done.folder, addons.join("Tidy"));
        assert!(addons.join("Tidy/core.lua").is_file());
        assert!(!addons.join("Tidy/.DS_Store").exists());
        assert!(!addons.join("Tidy/._core.lua").exists());
        assert!(!addons.join("__MACOSX").exists());
    }

    #[test]
    fn zip_slip_wird_abgewiesen() {
        let (addons, paths) = setup("slip");
        for bad in ["../evil.toc", "/etc/evil.toc", "Tidy/../../evil.toc"] {
            let data = zip_of(&[
                FakeEntry::text("Tidy/Tidy.toc", "## Version: 1\n"),
                FakeEntry::text(bad, "boese"),
            ]);
            match install(&data, &addons, &paths) {
                Err(InstallError::UnsafeEntry(p)) => assert_eq!(p, bad),
                other => panic!("{bad} wurde nicht abgewiesen: {other:?}"),
            }
            assert!(!addons.join("Tidy").exists());
            assert!(!addons
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("evil.toc")
                .exists());
        }
    }

    #[test]
    fn symlink_wird_abgewiesen() {
        let (addons, paths) = setup("symlink");
        let data = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 1\n"),
            FakeEntry::with_mode("Tidy/passwd", "/etc/passwd", 0o120_777),
        ]);

        match install(&data, &addons, &paths) {
            Err(InstallError::UnsafeEntry(p)) => assert_eq!(p, "Tidy/passwd"),
            other => panic!("Symlink wurde nicht abgewiesen: {other:?}"),
        }
        assert!(!addons.join("Tidy").exists());
    }

    #[test]
    fn buendel_landet_als_geschwister() {
        let (addons, paths) = setup("buendel");
        let data = zip_of(&[
            FakeEntry::text("ElvUI/ElvUI.toc", "## Version: 13.5\n"),
            FakeEntry::text("ElvUI/core.lua", ""),
            FakeEntry::text("ElvUI_Libraries/Libs.toc", "## Version: 1\n"),
        ]);

        let done = install(&data, &addons, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("13.5"));
        assert_eq!(done.folder, addons.join("ElvUI"));
        assert!(addons.join("ElvUI/core.lua").is_file());
        assert!(addons.join("ElvUI_Libraries/Libs.toc").is_file());
    }

    #[test]
    fn vorhandener_ordner_wird_gesichert() {
        let (addons, paths) = setup("sichern");
        let alt = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 1\n"),
            FakeEntry::text("Tidy/alt.lua", "alt"),
        ]);
        install(&alt, &addons, &paths).unwrap();

        let neu = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 2\n"),
            FakeEntry::text("Tidy/neu.lua", "neu"),
        ]);
        let done = install(&neu, &addons, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("2"));
        assert!(addons.join("Tidy/neu.lua").is_file());
        assert!(!addons.join("Tidy/alt.lua").exists());

        let backups = backup::list_backups(&paths).unwrap();
        assert_eq!(
            backups.len(),
            1,
            "genau eine Sicherung erwartet: {backups:?}"
        );
        let name = backups[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(
            name.starts_with("Tidy-") && name.ends_with(".zip"),
            "{name}"
        );
    }

    #[test]
    fn beidateien_neben_dem_ordner_bleiben_draussen() {
        let (addons, paths) = setup("beidatei");
        let data = zip_of(&[
            FakeEntry::text("Tidy/Tidy.toc", "## Version: 7\n"),
            FakeEntry::text("Tidy/core.lua", ""),
            FakeEntry::text("README.md", "lies mich"),
            FakeEntry::text("LICENSE", "GPL"),
        ]);

        let done = install(&data, &addons, &paths).unwrap();

        assert_eq!(done.folder, addons.join("Tidy"));
        assert!(addons.join("Tidy/core.lua").is_file());
        assert!(!addons.join("README.md").exists());
        assert!(!addons.join("Tidy/README.md").exists());
    }

    #[test]
    fn zip_ohne_toc_auf_der_wurzel_scheitert() {
        // Ohne `.toc` laesst sich der Ordnername nicht ableiten; raten waere
        // schlimmer als abbrechen (PLAN §13.4).
        let (addons, paths) = setup("ohne-toc");
        let data = zip_of(&[
            FakeEntry::text("core.lua", ""),
            FakeEntry::text("lib/helper.lua", ""),
        ]);

        assert!(matches!(
            install(&data, &addons, &paths),
            Err(InstallError::Toc(_))
        ));
        assert!(fs::read_dir(&addons).unwrap().next().is_none());
    }

    #[test]
    fn leeres_zip_wird_abgewiesen() {
        let (addons, paths) = setup("leer");
        let data = zip_of(&[]);
        assert!(matches!(
            install(&data, &addons, &paths),
            Err(InstallError::EmptyArchive)
        ));
    }
}
