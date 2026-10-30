//! Kern von Hexwerk: WoW-Installationen finden, `.toc` lesen, Addons erfassen.
//!
//! Der Kern kennt kein Netz und keine Oberflaeche. Er liest Dateisystem und
//! liefert Typen, auf denen Quellen (`hexwerk-sources`), Index
//! (`hexwerk-index`), Installation (`hexwerk-install`) und Oberflaeche
//! (`hexwerk-ui`) aufsetzen.

pub mod addon;
pub mod clients;
pub mod flavor;
pub mod toc;

pub use addon::{scan_addons, InstalledAddon};
pub use clients::{
    discover, discover_at, Client, Discovery, Note, NoteStatus, Origin, SearchRoots,
};
pub use flavor::{Flavor, FlavorDir};
pub use toc::{parse_file, parse_str, Toc, TocSourceIds};

/// Wurzel für Index, Sicherungen und Papierkorb.
///
/// Regel: `HEXWERK_DATA_DIR` gewinnt, sonst `XDG_DATA_HOME`, sonst
/// `~/.local/share`, jeweils mit `hexwerk` dahinter.
///
/// Sonderfall Flatpak: dort zeigt `XDG_DATA_HOME` in den Sandkasten
/// (`~/.var/app/<id>/data`). Der Papierkorb läge dann in einem Verzeichnis, das
/// beim Deinstallieren der App verschwindet, und die Kommandozeile auf dem Host
/// fände ihn nicht. Im Flatpak wird deshalb bewusst das echte
/// `~/.local/share/hexwerk` benutzt; der Zugriff auf das Home steht im Manifest.
pub fn data_root() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("HEXWERK_DATA_DIR") {
        if !dir.is_empty() {
            return std::path::PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if std::env::var_os("FLATPAK_ID").is_some() {
        if let Some(home) = home {
            return home.join(".local").join("share").join("hexwerk");
        }
    }
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        if !dir.is_empty() {
            return std::path::PathBuf::from(dir).join("hexwerk");
        }
    }
    home.unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("hexwerk")
}
