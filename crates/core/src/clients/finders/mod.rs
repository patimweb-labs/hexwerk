//! Ein Sucher je Herkunft. Jeder liest nur, schreibt nie.

use std::path::PathBuf;

use super::{describe, Client, Note, NoteStatus, Origin};

pub(crate) fn run(origin: Origin) -> (Vec<Client>, Note) {
    let found = match origin {
        Origin::Steam => steam::find(),
        Origin::Faugus => generic::find_faugus(),
        Origin::Heroic => generic::find_heroic(),
        Origin::Lutris => generic::find_lutris(),
        Origin::Bottles => generic::find_bottles(),
        Origin::PortProton => generic::find_portproton(),
        Origin::Manual => Vec::new(),
    };
    let status = if found.is_empty() {
        NoteStatus::Skip
    } else {
        NoteStatus::Ok
    };
    let detail = if found.is_empty() {
        format!("{}: nichts gefunden", origin.label())
    } else {
        format!("{}: {} Installation(en)", origin.label(), found.len())
    };
    (
        found,
        Note {
            step: origin.label().to_string(),
            status,
            detail,
        },
    )
}

pub(crate) fn manual(paths: &[PathBuf]) -> (Vec<Client>, Note) {
    let mut out = Vec::new();
    for p in paths {
        let flavors = describe(p);
        if flavors.is_empty() {
            continue;
        }
        out.push(Client {
            id: format!("manual:{}", p.display()),
            origin: Origin::Manual,
            label: p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Ordner")
                .to_string(),
            prefix: p.clone(),
            game_dir: p.clone(),
            flavors,
        });
    }
    let detail = if out.is_empty() {
        "Manuell: nichts".into()
    } else {
        format!("Manuell: {}", out.len())
    };
    (
        out,
        Note {
            step: "Manuell".into(),
            status: NoteStatus::Ok,
            detail,
        },
    )
}

pub(crate) mod generic;
pub(crate) mod steam;
