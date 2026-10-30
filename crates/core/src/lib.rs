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
pub use clients::{discover, Client, Discovery, Note, NoteStatus, Origin};
pub use flavor::{Flavor, FlavorDir};
pub use toc::{parse_file, parse_str, Toc, TocSourceIds};
