//! Loeschen ueber den Papierkorb des Desktops.
//!
//! Ablage nach der Freedesktop-Spezifikation, also dort, wo auch Dateien und
//! Nautilus ihren Muell ablegen:
//!
//! ```text
//! ~/.local/share/Trash/files/<name>           der verschobene Ordner
//! ~/.local/share/Trash/info/<name>.trashinfo  Herkunft und Zeitpunkt
//! ```
//!
//! Der Grund ist die Leerung: einen eigenen Papierkorb unter
//! `~/.local/share/hexwerk/trash` leert niemand, er waechst still weiter und
//! ist in keinem Dateimanager zu sehen. Im echten Papierkorb gilt
//! "Papierkorb leeren" des Desktops, und `gio trash --empty` raeumt ihn auf.
//!
//! Das `Path=` in der `.trashinfo` ist die Herkunft; ohne sie kann
//! "rueckgaengig" nach einem Neustart nicht wissen, in welchen Client-Ordner
//! zurueckzulegen ist.
//!
//! Liegt das Spiel auf einem anderen Dateisystem als das Benutzerverzeichnis,
//! wird kopiert statt verschoben. Der Papierkorb bleibt trotzdem der eine des
//! Benutzers, damit "rueckgaengig" und die Liste eine einzige Quelle haben.

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
    /// Name in `files/` und, mit `.trashinfo`, in `info/`.
    pub name: String,
    /// Wurzel des Papierkorbs, enthaelt `files/` und `info/`.
    pub root: PathBuf,
    /// Vollstaendiger Pfad, an dem der Ordner vorher lag.
    pub original: PathBuf,
    /// Addon-Verzeichnis des Clients, aus dem entfernt wurde.
    pub addons_dir: PathBuf,
}

impl TrashEntry {
    /// Pfad des verschobenen Ordners im Papierkorb.
    pub fn path(&self) -> PathBuf {
        self.root.join("files").join(&self.name)
    }

    /// Pfad der Herkunftsdatei.
    pub fn info_path(&self) -> PathBuf {
        self.root
            .join("info")
            .join(format!("{}.trashinfo", self.name))
    }
}

/// Verschiebt `<addons_dir>/<folder>` in den Papierkorb des Benutzers.
///
/// Der Ordner verschwindet damit aus dem Spiel, bleibt aber vollstaendig
/// erhalten und ist im Dateimanager zu sehen.
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

    let root = paths.trash_root.clone();
    fs::create_dir_all(root.join("files"))?;
    fs::create_dir_all(root.join("info"))?;

    // Namen nicht ueberschreiben: liegt schon ein "Tidy" im Papierkorb, wird
    // der neue Eintrag "Tidy.2". Sonst waere der aeltere Eintrag weg, obwohl
    // der Nutzer zwei Loeschvorgaenge gemacht hat.
    let target = unique_path(&root.join("files"), &folder, None);
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| folder.clone());

    let entry = TrashEntry {
        folder,
        name,
        root,
        original: source.clone(),
        addons_dir: addons_dir.to_path_buf(),
    };

    // Erst die Herkunft schreiben, dann verschieben. Scheitert das Schreiben,
    // liegt nichts im Papierkorb, das niemand mehr zuordnen kann.
    if let Err(e) = write_info(&entry).and_then(|()| move_dir(&source, &target)) {
        let _ = fs::remove_file(entry.info_path());
        let _ = fs::remove_dir_all(&target);
        return Err(e);
    }
    Ok(entry)
}

/// Alle Eintraege im Papierkorb des Benutzers, neueste zuerst.
///
/// Eintraege ohne lesbare Herkunft werden uebersprungen: sie koennen mit
/// fremder Hand entstanden sein, und ohne Pfad waere nicht klar, wohin
/// zurueckzulegen ist.
pub fn list_trash(paths: &Paths) -> Result<Vec<TrashEntry>, InstallError> {
    let mut out = Vec::new();
    let info_dir = paths.trash_root.join("info");
    let entries = match fs::read_dir(&info_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("trashinfo") {
            continue;
        }
        if let Some(trash) = read_info(&path, &paths.trash_root) {
            out.push(trash);
        }
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Holt einen Eintrag zurueck an seinen alten Platz.
pub fn restore_from_trash(entry: &TrashEntry) -> Result<PathBuf, InstallError> {
    let source = entry.path();
    if !source.is_dir() {
        return Err(InstallError::NotFound(source.display().to_string()));
    }
    let target = entry.original.clone();
    if target.exists() {
        // Nicht ueberschreiben: der Nutzer hat inzwischen etwas anderes dort.
        return Err(InstallError::AlreadyInstalled(target.display().to_string()));
    }

    fs::create_dir_all(&entry.addons_dir)?;
    move_dir(&source, &target)?;
    // Eintrag und Herkunft gehoeren zusammen weg, sonst bleibt eine
    // Herkunftsdatei ohne Ordner im Papierkorb liegen.
    let _ = fs::remove_file(entry.info_path());
    Ok(target)
}

/// Loescht einen Papierkorb-Eintrag endgueltig.
pub fn remove_trash_entry(entry: &TrashEntry) -> Result<(), InstallError> {
    let dir = entry.path();
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let _ = fs::remove_file(entry.info_path());
    Ok(())
}

fn write_info(entry: &TrashEntry) -> Result<(), InstallError> {
    let text = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(&entry.original.display().to_string()),
        crate::iso_timestamp_now()
    );
    fs::write(entry.info_path(), text)?;
    Ok(())
}

fn read_info(info_path: &Path, root: &Path) -> Option<TrashEntry> {
    let text = fs::read_to_string(info_path).ok()?;
    let mut original = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Path=") {
            original = Some(PathBuf::from(percent_decode(value.trim())?));
        }
    }
    let original = original?;
    let name = info_path.file_stem()?.to_string_lossy().to_string();
    let folder = original.file_name()?.to_string_lossy().to_string();
    let addons_dir = original.parent()?.to_path_buf();
    Some(TrashEntry {
        folder,
        name,
        root: root.to_path_buf(),
        original,
        addons_dir,
    })
}

/// Kodiert einen Pfad fuer die `.trashinfo`.
///
/// Der Schraegstrich bleibt stehen, alles andere ausser den unbedingt
/// erlaubten Zeichen wird prozentkodiert, so wie es GLib auch macht.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/');
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Verschiebt einen Ordner; scheitert `rename` (EXDEV: Spiel auf einem
/// anderen Dateisystem als der Papierkorb), wird kopiert und danach geloescht.
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
        assert_eq!(entry.name, "Tidy");
        assert_eq!(entry.path(), paths.trash_root.join("files/Tidy"));
        assert!(entry.path().join("lib/a.lua").is_file());
        assert_eq!(entry.original, folder);
        assert!(
            entry.info_path().is_file(),
            "Herkunft fehlt: {}",
            entry.info_path().display()
        );

        let back = restore_from_trash(&entry).unwrap();

        assert_eq!(back, folder);
        assert_eq!(
            fs::read_to_string(folder.join("lib/a.lua")).unwrap(),
            "inhalt"
        );
        assert!(!entry.path().exists(), "Ordner blieb im Papierkorb liegen");
        assert!(
            !entry.info_path().exists(),
            "Herkunft blieb im Papierkorb liegen"
        );
    }

    #[test]
    fn herkunft_steht_im_klartext_und_prozentkodiert() {
        let (addons, paths, folder) = addons_with_folder("trash-herkunft");

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        let text = fs::read_to_string(entry.info_path()).unwrap();

        assert!(text.starts_with("[Trash Info]\n"), "{text}");
        let erwartet = format!("Path={}", folder.display());
        assert!(
            text.contains(&erwartet),
            "Pfad fehlt oder falsch kodiert: {text}"
        );
        assert!(text.contains("\nDeletionDate="), "{text}");
        assert_eq!(percent_encode("/a b/ä"), "/a%20b/%C3%A4");
        assert_eq!(percent_decode("/a%20b/%C3%A4").unwrap(), "/a b/ä");
    }

    #[test]
    fn papierkorb_liste_kennt_den_ziel_client() {
        let (addons, paths, _folder) = addons_with_folder("trash-liste");

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        let list = list_trash(&paths).unwrap();

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].folder, "Tidy");
        assert_eq!(list[0].addons_dir, addons);
        assert_eq!(list[0].path(), entry.path());
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

        assert!(!entry.path().exists());
        assert!(!entry.info_path().exists());
        assert!(list_trash(&paths).unwrap().is_empty());
    }

    #[test]
    fn zweiter_eintrag_ueberschreibt_den_ersten_nicht() {
        let (addons, paths, _folder) = addons_with_folder("trash-doppelt");

        let first = trash_addon(&addons, "Tidy", &paths).unwrap();
        fs::create_dir_all(addons.join("Tidy")).unwrap();
        fs::write(addons.join("Tidy/Tidy.toc"), "## Version: 2\n").unwrap();
        let second = trash_addon(&addons, "Tidy", &paths).unwrap();

        assert_ne!(first.name, second.name);
        assert_eq!(list_trash(&paths).unwrap().len(), 2);
    }

    #[test]
    fn fremde_papierkorb_eintraege_werden_nicht_angefasst() {
        let (addons, paths, _folder) = addons_with_folder("trash-fremd");
        let fremd = paths.trash_root.join("files/Urlaub.mp4");
        fs::create_dir_all(fremd.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.trash_root.join("info")).unwrap();
        fs::write(&fremd, "nicht von hexwerk").unwrap();
        fs::write(
            paths.trash_root.join("info/Urlaub.mp4.trashinfo"),
            "[Trash Info]\nPath=/home/jemand/Urlaub.mp4\nDeletionDate=2026-01-01T00:00:00\n",
        )
        .unwrap();

        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        let list = list_trash(&paths).unwrap();

        assert!(list.iter().any(|e| e.name == "Urlaub.mp4"));
        assert!(list.iter().any(|e| e.name == entry.name));
        remove_trash_entry(&entry).unwrap();
        assert!(!entry.path().exists());
        assert!(fremd.exists(), "fremder Eintrag wurde geloescht");
    }

    #[test]
    fn eintrag_ohne_herkunft_wird_nicht_gelistet() {
        let (addons, paths, _folder) = addons_with_folder("trash-ohne-herkunft");
        let entry = trash_addon(&addons, "Tidy", &paths).unwrap();
        fs::remove_file(entry.info_path()).unwrap();

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
}
