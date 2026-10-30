//! Bettet den CurseForge-Schluessel zur Bauzeit ein, wenn er vorhanden ist.
//!
//! Warum nicht im Quelltext: ein Schluessel im Repo ist ein Schluessel fuer
//! alle, die ihn finden. Der Bau holt ihn deshalb aus der Umgebung
//! (`HEXWERK_CURSEFORGE_KEY`), wie sie eine CI aus ihren Secrets setzt, oder
//! aus einer Datei ausserhalb des Repos (`.secrets/curseforge.key`, steht in
//! `.gitignore`). Fehlt beides, wird nichts eingebettet und das Programm
//! verhaelt sich wie ohne Schluessel.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=HEXWERK_CURSEFORGE_KEY");
    println!("cargo:rerun-if-changed=../../.secrets/curseforge.key");

    if let Ok(value) = std::env::var("HEXWERK_CURSEFORGE_KEY") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            println!("cargo:rustc-env=HEXWERK_CURSEFORGE_KEY={value}");
            return;
        }
    }

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.secrets/curseforge.key");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let value = text.trim().to_string();
        if !value.is_empty() {
            println!("cargo:rustc-env=HEXWERK_CURSEFORGE_KEY={value}");
        }
    }
}
