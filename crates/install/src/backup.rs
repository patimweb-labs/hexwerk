//! Sicherung vor dem Ueberschreiben (ZIP unter `~/.local/share/hexwerk/backup`).
//!
//! Gesichert wird der ganze Addon-Ordner als ZIP, nicht als Kopie: ein ZIP
//! braucht nur eine Datei, laesst sich mit denselben Pruefungen
//! ([`crate::zip_install`]) wieder einsetzen und waechst beim Aufraeumen nicht
//! unbegrenzt in der Tiefe.
//!
//! Namen: `<ordner>-<zeitstempel>.zip`, bei Kollision `-2`, `-3`, … Zwei
//! Installationen derselben Minute ueberschreiben einander also nicht.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::plan::Installed;
use crate::zip_install::{install_zip, sanitize_folder};
use crate::{unique_path, InstallError, Paths};

/// Sichert `<addons_dir>/<folder>` als ZIP in `paths.backup_dir`.
///
/// Rueckgabe ist der Pfad der Sicherung, damit die Oberflaeche ihn zeigen oder
/// wiederherstellen kann.
pub fn backup_folder(
    addons_dir: &Path,
    folder: &str,
    paths: &Paths,
) -> Result<PathBuf, InstallError> {
    let folder = sanitize_folder(folder)?;
    let source = addons_dir.join(&folder);
    if !source.is_dir() {
        return Err(InstallError::NotFound(source.display().to_string()));
    }

    fs::create_dir_all(&paths.backup_dir)?;
    let dest = unique_path(
        &paths.backup_dir,
        &format!("{folder}-{}", crate::timestamp_now()),
        Some("zip"),
    );

    let file = fs::File::create(&dest)?;
    let mut zip = ZipWriter::new(file);
    // Rechte wie im Spiel: Ordner 755, Dateien 644 - sonst meckert Wine nicht,
    // aber ein `git`-Vergleich umso mehr.
    let dir_options = SimpleFileOptions::default().unix_permissions(0o755);
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    if let Err(e) = write_tree(
        &mut zip,
        &source,
        Path::new(&folder),
        dir_options,
        file_options,
    ) {
        // Kein halbes Archiv liegen lassen: es waere als Sicherung
        // unbrauchbar und wuerde beim Wiederherstellen nur scheitern.
        drop(zip);
        let _ = fs::remove_file(&dest);
        return Err(e);
    }
    zip.finish()?;
    Ok(dest)
}

/// Alle vorhandenen Sicherungen, aelteste zuerst.
pub fn list_backups(paths: &Paths) -> Result<Vec<PathBuf>, InstallError> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(&paths.backup_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_zip(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Stellt eine Sicherung wieder her.
///
/// Der Ordnername kommt aus dem ZIP (die Sicherung enthaelt genau einen
/// Wurzelordner). Ein schon vorhandener Ordner wird dabei erneut gesichert -
/// "wiederherstellen" darf nie der einzige Weg sein, den neuen Stand zu
/// verlieren.
pub fn restore_backup(
    archive: &Path,
    addons_dir: &Path,
    paths: &Paths,
) -> Result<Installed, InstallError> {
    if !archive.is_file() {
        return Err(InstallError::NotFound(archive.display().to_string()));
    }
    let data = fs::read(archive)?;
    install_zip(&data, addons_dir, None, paths)
}

fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

fn write_tree(
    zip: &mut ZipWriter<fs::File>,
    source: &Path,
    prefix: &Path,
    dir_options: SimpleFileOptions,
    file_options: SimpleFileOptions,
) -> Result<(), InstallError> {
    zip.add_directory(zip_name(prefix)?, dir_options)?;

    // Sortiert, damit zwei Sicherungen desselben Ordners vergleichbar bleiben.
    let mut entries: Vec<_> = fs::read_dir(source)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        // `symlink_metadata`: ein Symlink im Addon-Ordner wird nicht verfolgt,
        // sonst landete fremder Inhalt in der Sicherung.
        let md = fs::symlink_metadata(&path)?;
        let rel = prefix.join(entry.file_name());
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            write_tree(zip, &path, &rel, dir_options, file_options)?;
        } else if md.is_file() {
            zip.start_file(zip_name(&rel)?, file_options)?;
            let mut file = fs::File::open(&path)?;
            io::copy(&mut file, zip)?;
        }
    }
    Ok(())
}

fn zip_name(path: &Path) -> Result<String, InstallError> {
    path.to_str()
        .map(|s| s.replace('\\', "/"))
        .ok_or_else(|| InstallError::InvalidFolder(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn addons_with_file(tag: &str) -> (PathBuf, Paths, PathBuf) {
        let base = temp_dir(tag);
        let addons = base.join("Wow/_retail_/Interface/AddOns");
        let folder = addons.join("Tidy");
        fs::create_dir_all(folder.join("lib")).unwrap();
        fs::write(folder.join("Tidy.toc"), "## Version: 1.0\n").unwrap();
        fs::write(folder.join("lib/a.lua"), "inhalt").unwrap();
        (addons, Paths::at(base.join("data")), folder)
    }

    #[test]
    fn sicherung_enthaelt_den_ganzen_ordner() {
        let (addons, paths, _folder) = addons_with_file("sicherung-inhalt");

        let archive = backup_folder(&addons, "Tidy", &paths).unwrap();

        assert!(archive.is_file());
        assert!(archive.starts_with(&paths.backup_dir));
        let name = archive.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("Tidy-"), "{name}");
        assert!(name.ends_with(".zip"), "{name}");

        let data = fs::read(&archive).unwrap();
        let mut zip = zip::ZipArchive::new(io::Cursor::new(data)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert!(names.contains(&"Tidy/Tidy.toc".to_string()), "{names:?}");
        assert!(names.contains(&"Tidy/lib/a.lua".to_string()), "{names:?}");
        assert_eq!(list_backups(&paths).unwrap(), vec![archive]);
    }

    #[test]
    fn sicherung_und_wiederherstellung() {
        let (addons, paths, folder) = addons_with_file("sicherung-rundlauf");

        let archive = backup_folder(&addons, "Tidy", &paths).unwrap();
        fs::remove_dir_all(&folder).unwrap();
        assert!(!folder.exists());

        let done = restore_backup(&archive, &addons, &paths).unwrap();

        assert_eq!(done.folder, folder);
        assert_eq!(done.version.as_deref(), Some("1.0"));
        assert_eq!(
            fs::read_to_string(folder.join("lib/a.lua")).unwrap(),
            "inhalt"
        );
    }

    #[test]
    fn zweite_sicherung_ueberschreibt_nicht() {
        let (addons, paths, _folder) = addons_with_file("sicherung-zwei");

        let first = backup_folder(&addons, "Tidy", &paths).unwrap();
        let second = backup_folder(&addons, "Tidy", &paths).unwrap();

        assert_ne!(first, second);
        assert_eq!(list_backups(&paths).unwrap().len(), 2);
        // Beide tragen denselben Inhalt, keine ist verloren.
        for archive in [&first, &second] {
            let data = fs::read(archive).unwrap();
            assert!(zip::ZipArchive::new(io::Cursor::new(data)).unwrap().len() >= 3);
        }
    }

    #[test]
    fn sicherung_eines_fehlenden_ordners_scheitert() {
        let (addons, paths, _folder) = addons_with_file("sicherung-fehlt");
        assert!(matches!(
            backup_folder(&addons, "GibtsNicht", &paths),
            Err(InstallError::NotFound(_))
        ));
    }

    #[test]
    fn wiederherstellung_sichert_den_aktuellen_stand() {
        let (addons, paths, folder) = addons_with_file("sicherung-neu");
        let archive = backup_folder(&addons, "Tidy", &paths).unwrap();

        // Neuer Stand auf der Platte, dann zurueck auf die Sicherung.
        fs::write(folder.join("Tidy.toc"), "## Version: 2.0\n").unwrap();
        assert_eq!(list_backups(&paths).unwrap().len(), 1);

        let done = restore_backup(&archive, &addons, &paths).unwrap();

        assert_eq!(done.version.as_deref(), Some("1.0"));
        let names: Vec<String> = list_backups(&paths)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names.len(),
            2,
            "Stand 2.0 haette gesichert werden muessen: {names:?}"
        );
    }

    #[test]
    fn kaputtes_archiv_laesst_nichts_liegen() {
        let (addons, paths, _folder) = addons_with_file("sicherung-kaputt");

        let broken = paths.backup_dir.join("kaputt.zip");
        fs::create_dir_all(&paths.backup_dir).unwrap();
        fs::write(&broken, b"kein zip").unwrap();

        assert!(matches!(
            restore_backup(&broken, &addons, &paths),
            Err(InstallError::Zip(_))
        ));
        assert!(
            addons.join("Tidy/Tidy.toc").is_file(),
            "alter Stand unangetastet"
        );
    }
}
