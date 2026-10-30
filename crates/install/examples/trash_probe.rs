//! Prueft den Papierkorb am echten Standardort: XDG_DATA_HOME wird gesetzt,
//! danach muss der Eintrag unter `$XDG_DATA_HOME/Trash/files` liegen und eine
//! `.trashinfo` daneben, so wie es der Dateimanager erwartet.

use std::fs;
use std::path::PathBuf;

fn main() {
    let base = PathBuf::from(
        std::env::var("HEXWERK_PROBE_DIR").unwrap_or_else(|_| "/tmp/hexwerk-probe".to_string()),
    );
    let _ = fs::remove_dir_all(&base);
    let data = base.join("data");
    fs::create_dir_all(&data).unwrap();
    std::env::set_var("XDG_DATA_HOME", &data);

    let addons = base.join("Wow/_classic_era_/Interface/AddOns");
    let folder = addons.join("Tidy");
    fs::create_dir_all(folder.join("lib")).unwrap();
    fs::write(folder.join("Tidy.toc"), "## Version: 1.0\n").unwrap();
    fs::write(folder.join("lib/a.lua"), "inhalt").unwrap();

    let paths = hexwerk_install::Paths::system();
    println!("Papierkorb: {}", paths.trash_root.display());

    let entry = hexwerk_install::trash_addon(&addons, "Tidy", &paths).unwrap();
    println!("Eintrag: {}", entry.path().display());
    println!("Herkunft: {}", entry.info_path().display());
    println!("Inhalt der Herkunft:");
    for line in fs::read_to_string(entry.info_path()).unwrap().lines() {
        println!("  {line}");
    }
    println!(
        "Datei im Papierkorb: {}",
        entry.path().join("lib/a.lua").is_file()
    );
    println!("Addon-Ordner weg: {}", !folder.exists());

    let liste = hexwerk_install::list_trash(&paths).unwrap();
    println!("Eintraege in der Liste: {}", liste.len());

    let back = hexwerk_install::restore_from_trash(&entry).unwrap();
    println!("Zurueck nach: {}", back.display());
    println!("Datei wieder da: {}", back.join("lib/a.lua").is_file());
    println!("Herkunft weg: {}", !entry.info_path().exists());
}
