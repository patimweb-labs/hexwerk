//! Papierkorb statt Loeschen (`~/.local/share/hexwerk/trash`).
//!
//! Ein Eintrag liegt unter `trash/<zeitstempel>-<ordner>/` und enthaelt:
//!
//! ```text
//! <zeitstempel>-<ordner>/meta          Ziel-Client als Text (folder=…, addons_dir=…)
//! <zeitstempel>-<ordner>/tree/<ordner> der entfernte Ordner, unveraendert
//! ```
//!
//! Das `meta` ist der Grund fuer die eine Ebene mehr: "rueckgaengig" muss auch
//! nach einem Neustart der Oberflaeche funktionieren, und ohne den Zielpfad
//! waere nicht bekannt, in welchen Client zurueckzulegen ist - dieselbe Falle
//! wie bei "installiere fuer alle".
//!
//! Der zurueckgegebene Pfad ist der Eintragsordner, nicht der Addon-Ordner:
//! damit kann die Oberflaeche den Eintrag spaeter gezielt loeschen.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::zip_install::sanitize_folder;
use crate::{unique_path, InstallError, Paths};

/// Ein Eintrag im Papierkorb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashEntry {
    /// Ordnername im Addon-Verzeichnis.
    pub folder: String,
    /// Ablage im Papierkorb (enthaelt `meta` und `tree/`).
    pub path: PathBuf,
    /// Vollstaendiger Pfad, an dem der Ordner vorher lag.
    pub original: PathBuf,
    /// Addon-Verzeichnis des Clients, aus dem entfernt wurde.
    pub addons_dir: PathBuf,
}

/// Verschiebt `<addons_dir>/<folder>` in den Papierkorb.
///
/// Der Ordner verschwindet damit aus dem Spiel, bleibt aber vollstaendig
/// erhalten. `paths.trash_dir` wird bei Bedarf angelegt.
pub fn trash_addon(
    addons_dir: &Path,
    folder: &str,
    paths: &Paths,
) -> Result<TrashEntry, InstallError> {
    let folder = sanitize_folder(folder)?;
    let source = addons_dir.join(&folder);
    if !source.is_dir() {
        return Err(InstallError::NotFound(source.display().to_string()));
    }

    fs::create_dir_all(&paths.trash_dir)?;
    let entry_dir = unique_path(
        &paths.trash_dir,
        &format!("{}-{folder}", crate::timestamp_now()),
        None,
    );
    fs::create_dir(&entry_dir)?;

    let entry = TrashEntry {
        folder: folder.clone(),
        path: entry_dir.clone(),
        original: source.clone(),
        addons_dir: addons_dir.to_path_buf(),
    };

    if let Err(e) = write_meta(&entry).and_then(|()| {
        fs::create_dir_all(entry_dir.join("tree"))?;
        move_dir(&source, &entry_dir.join("tree").join(&folder))
    }) {
        // Kein halber Eintrag: sonst stuende das Addon im Papierkorb, ohne
        // dass "rueckgaengig" es findet.
        let _ = fs::remove_dir_all(&entry_dir);
        return Err(e);
    }
    Ok(entry)
}

/// Alle Papierkorb-Eintraege, neueste zuerst.
///
/// Eintraege ohne lesbares `meta` werden uebersprungen: sie sind mit fremder
/// Hand entstanden und nicht sicher zuzuordnen.
pub fn list_trash(paths: &Paths) -> Result<Vec<TrashEntry>, InstallError> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(&paths.trash_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(trash) = read_meta(&path) {
            out.push(trash);
        }
    }
    out.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(out)
}

/// Holt einen Eintrag zurueck an seinen alten Platz.
///
/// Der Zielname ist der gespeicherte Ordnername, nicht der Eintragsname -
/// `20261030T153012Z-Tidy` enthaelt den Stempel nur fuer die Sortierung.
pub fn restore_from_trash(entry: &TrashEntry) -> Result<PathBuf, InstallError> {
    let source = entry.path.join("tree").join(&entry.folder);
    if !source.is_dir() {
        return Err(InstallError::NotFound(source.display().to_string()));
    }
    let target = entry.addons_dir.join(&entry.folder);
    if target.exists() {
        // Nicht ueberschreiben: der Nutzer hat inzwischen etwas anderes dort.
        return Err(InstallError::AlreadyInstalled(target.display().to_string()));
    }

    fs::create_dir_all(&entry.addons_dir)?;
    move_dir(&source, &target)?;
    // Der Eintrag ist damit leer und hat keinen Zweck mehr.
    let _ = fs::remove_dir_all(&entry.path);
    Ok(target)
}

/// Loescht einen Papierkorb-Eintrag endgueltig.
pub fn remove_trash_entry(entry: &TrashEntry) -> Result<(), InstallError> {
    match fs::remove_dir_all(&entry.path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn write_meta(entry: &TrashEntry) -> Result<(), InstallError> {
    let text = format!(
        "folder={}\naddons_dir={}\n",
        entry.folder,
        entry.addons_dir.display()
    );
    fs::write(entry.path.join("meta"), text)?;
    Ok(())
}

fn read_meta(dir: &Path) -> Option<TrashEntry> {
    let text = fs::read_to_string(dir.join("meta")).ok()?;
    let mut folder = None;
    let mut addons_dir = None;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "folder" => folder = Some(value.trim().to_string()),
                "addons_dir" => addons_dir = Some(PathBuf::from(value.trim())),
                _ => {}
            }
        }
    }
    let folder = folder?;
    let addons_dir = addons_dir?;
    Some(TrashEntry {
        original: addons_dir.join(&folder),
        path: dir.to_path_buf(),
        folder,
        addons_dir,
    })
}

/// Verschiebt einen Ordner; scheitert `rename` (EXDEV: Papierkorb auf einem
/// anderen Dateisystem als das Spiel), wird kopiert und danach geloescht.
fn move_dir(source: &Path, target: &Path) -> Result<(), InstallError> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_dir(source, target)?;
            fs::remove_dir_all(source).map_err(|_| InstallError::Io(rename_error))
        }
    }
}

fn copy_dir(source: &Path, target: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)?.flatten() {
        let path = entry.path();
        let dest = target.join(entry.file_name());
        let md = fs::symlink_metadata(&path)?;
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn addons_with_folder(tag: &str) -> (PathBuf, Paths, PathBuf) {
        let base = temp_dir(tag);
        let addons = base.join("Wow/_retail_/Interface/AddOns");
        let folder = addons.join("Tidy");
        fs::create_dir_all(folder.join("lib")).unwrap();
        fs::write(folder.join("Tidy.toc"), "## Version: 1.0\n").unwrap();
        fs::write(folder.join("lib/a.lua"), "inhalt").unwrap();
        (addons, Paths::at(base.join("data")), folder)
    }

    #[test]
    fn papierkorb_und_rueckgaengig() {
        let (addons, paths, folder) = addons_with_folder("trash-rundlauf");

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();

        assert!(!folder.exists());
        assert!(entry.path.starts_with(&paths.trash_dir));
        let name = entry
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(name.ends_with("-Tidy"), "{name}");
        assert!(entry.path.join("tree/Tidy/lib/a.lua").is_file());
        assert_eq!(entry.original, folder);

        let back = restore_from_trash(&entry).unwrap();

        assert_eq!(back, folder);
        assert_eq!(
            fs::read_to_string(folder.join("lib/a.lua")).unwrap(),
            "inhalt"
        );
        assert!(
            !entry.path.exists(),
            "leerer Eintrag bleibt liegen: {}",
            entry.path.display()
        );
    }

    #[test]
    fn papierkorb_liste_kennt_den_ziel_client() {
        let (addons, paths, _folder) = addons_with_folder("trash-liste");

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        let list = list_trash(&paths).unwrap();

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].folder, "Tidy");
        assert_eq!(list[0].addons_dir, addons);
        assert_eq!(list[0].path, entry.path);
    }

    #[test]
    fn rueckgaengig_ueberschreibt_nichts() {
        let (addons, paths, folder) = addons_with_folder("trash-belegt");
        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();

        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Tidy.toc"), "## Version: 9\n").unwrap();

        assert!(matches!(
            restore_from_trash(&entry),
            Err(InstallError::AlreadyInstalled(_))
        ));
        assert_eq!(
            fs::read_to_string(folder.join("Tidy.toc")).unwrap(),
            "## Version: 9\n"
        );
    }

    #[test]
    fn entgueltig_loeschen() {
        let (addons, paths, _folder) = addons_with_folder("trash-loeschen");

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        remove_trash_entry(&entry).unwrap();

        assert!(!entry.path.exists());
        assert!(list_trash(&paths).unwrap().is_empty());
    }

    #[test]
    fn papierkorb_eines_fehlenden_ordners_scheitert() {
        let (addons, paths, _folder) = addons_with_folder("trash-fehlt");
        assert!(matches!(
            trash_addon(&addons, "GibtsNicht", &paths),
            Err(InstallError::NotFound(_))
        ));
    }

    #[test]
    fn eintraege_ohne_meta_werden_ignoriert() {
        let (addons, paths, _folder) = addons_with_folder("trash-fremd");
        fs::create_dir_all(paths.trash_dir.join("von-hand")).unwrap();
        assert!(list_trash(&paths).unwrap().is_empty());

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        assert_eq!(list_trash(&paths).unwrap().len(), 1);
        assert!(entry.path.join("meta").is_file());
    }
}
