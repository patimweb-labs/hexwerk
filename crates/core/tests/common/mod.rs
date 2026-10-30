//! Kuenstliche Verzeichnisbaeume fuer die Erkennungstests.
//!
//! Alles entsteht unter `CARGO_TARGET_TMPDIR`. `$HOME` wird hier nie gelesen:
//! auf dem Entwicklungsrechner liegt dort ein echtes Faugus-Praefix mit WoW,
//! ein Test, der das mitnimmt, waere auf jeder anderen Maschine rot.

// Nicht jedes Testprogramm nutzt jeden Helfer.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub struct Tree {
    root: PathBuf,
}

impl Tree {
    /// Frischer Baum je Test. Der Name trennt die Tests, `remove_dir_all`
    /// macht einen abgebrochenen Lauf wieder sauber.
    pub fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("client-detection")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("Testbaum anlegen");
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, rel: impl AsRef<str>) -> PathBuf {
        self.root.join(rel.as_ref())
    }

    pub fn dir(&self, rel: impl AsRef<str>) -> PathBuf {
        let p = self.path(rel);
        std::fs::create_dir_all(&p).expect("Ordner anlegen");
        p
    }

    pub fn file(&self, rel: impl AsRef<str>, content: &str) -> PathBuf {
        let p = self.path(rel.as_ref());
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("Elternordner anlegen");
        }
        std::fs::write(&p, content).expect("Datei schreiben");
        p
    }

    /// Legt `<flavor>/Interface/AddOns/<folder>/<folder>.toc` an. `interface`
    /// ist der Rohwert aus `## Interface`, also auch eine Komma-Liste.
    pub fn addon(&self, flavor_rel: &str, folder: &str, interface: &str) {
        self.file(
            format!("{flavor_rel}/Interface/AddOns/{folder}/{folder}.toc"),
            &format!("## Interface: {interface}\n## Title: {folder}\n## Version: 1.0.0\n"),
        );
    }

    /// Ein Client-Ordner ohne Addons, aber mit Spiel-Binary - der Zustand
    /// direkt nach einer Installation.
    pub fn fresh_flavor(&self, flavor_rel: &str, exe: &str) {
        self.file(format!("{flavor_rel}/{exe}"), "MZ");
    }
}
