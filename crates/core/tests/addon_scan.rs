//! Addon-Erfassung, soweit die Client-Erkennung sie braucht: `## Interface`
//! sammeln und Groessen messen, ohne an einem Symlink haengen zu bleiben.

mod common;

use common::Tree;
use hexwerk_core::scan_addons;

#[test]
fn toc_mit_ordnernamen_hat_vorrang() {
    let t = Tree::new("addon_toc");
    // Manche Addons bringen eine zweite `.toc` fuer eine andere Client-Version
    // mit; massgeblich ist die, die so heisst wie der Ordner.
    t.file(
        "AddOns/ElvUI/ElvUI.toc",
        "## Version: 13.5\n## Interface: 11509\n",
    );
    t.file(
        "AddOns/ElvUI/ElvUI_Classic.toc",
        "## Version: 1.0\n## Interface: 11502\n",
    );
    // Ordner ohne `.toc` sind kein Addon.
    t.dir("AddOns/nur_ein_ordner");

    let found = scan_addons(&t.path("AddOns"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].folder, "ElvUI");
    assert_eq!(found[0].version.as_deref(), Some("13.5"));
    assert_eq!(found[0].interface_numbers, vec![11509]);
    assert!(found[0].size_bytes > 0);
}

#[test]
fn fehlender_ordner_ergibt_nichts() {
    let t = Tree::new("addon_fehlt");
    assert!(scan_addons(&t.path("gibt-es-nicht")).is_empty());
}

#[cfg(unix)]
#[test]
fn selbstverweis_haengt_die_groessenmessung_nicht_auf() {
    let t = Tree::new("addon_symlink");
    t.file(
        "AddOns/Loop/Loop.toc",
        "## Version: 1.0\n## Interface: 11509\n",
    );
    // Ein Verweis auf das Addon-Verzeichnis selbst: wer Symlinks verfolgt,
    // laeuft hier endlos.
    std::os::unix::fs::symlink(t.path("AddOns/Loop"), t.path("AddOns/Loop/zurueck"))
        .expect("Symlink anlegen");

    let found = scan_addons(&t.path("AddOns"));
    assert_eq!(found.len(), 1);
    // Nur die `.toc` zaehlt; der Verweis traegt nichts bei.
    assert!(found[0].size_bytes > 0);
}
