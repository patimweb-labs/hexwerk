//! `hexwerk` - Kommandozeile: finden, anzeigen, indexieren, Schluessel verwalten.
//!
//! Die Oberflaeche macht dasselbe im Fenster; hier ist es fuer Skripte, fuer die
//! Fehlersuche und fuer alles, was ohne Display laufen muss.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use hexwerk::clients::{discover, NoteStatus};
use hexwerk_index::{Filters, Index};
use hexwerk_sources::cache::Cache;
use hexwerk_sources::http::{Http, UreqHttp};
use hexwerk_sources::keys::KeyProvider;
use hexwerk_sources::{merge, Registry, Source};

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = if args.is_empty() {
        "scan".to_string()
    } else {
        args.remove(0)
    };
    match cmd.as_str() {
        "scan" | "clients" => {
            scan(&paths(&args));
            ExitCode::SUCCESS
        }
        "addons" => {
            addons(&paths(&args));
            ExitCode::SUCCESS
        }
        "index" => index_cmd(&args),
        "search" => search_cmd(&args),
        "keys" => keys_cmd(&args),
        "help" | "--help" | "-h" => {
            usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unbekannt: {other}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn paths(args: &[String]) -> Vec<PathBuf> {
    args.iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect()
}

fn usage() {
    println!("hexwerk - Addon-Manager fuer World of Warcraft unter Linux\n");
    println!("  hexwerk scan [ORDNER…]              Installationen suchen");
    println!("  hexwerk addons [ORDNER…]            Installationen und ihre Addons listen");
    println!("  hexwerk index [--net BEGRIFF]       lokalen Index bauen (--net: Quellen befragen)");
    println!("  hexwerk search BEGRIFF              im lokalen Index suchen");
    println!("  hexwerk keys list                   Schluessel und ihre Herkunft");
    println!(
        "  hexwerk keys set QUELLE SCHLUESSEL  Schluessel in ~/.config/hexwerk/keys.json ablegen"
    );
    println!("  hexwerk keys remove QUELLE          Schluessel entfernen");
    println!("  hexwerk keys test                   jede Quelle mit einer Anfrage pruefen");
    println!("  hexwerk help                        diese Hilfe");
    println!(
        "\nQUELLE ist eine von: wago, curseforge. Ohne ORDNER werden Steam, Faugus, Heroic,\n\
         Lutris, Bottles und PortProton durchsucht."
    );
}

fn scan(extra: &[PathBuf]) {
    let d = discover(extra);
    for n in &d.notes {
        let mark = match n.status {
            NoteStatus::Ok => "✓",
            NoteStatus::Skip => "○",
            NoteStatus::Error => "×",
            NoteStatus::Running => "›",
        };
        println!("{mark} {:<16} {}", n.step, n.detail);
    }
    println!("\n{} Installation(en)", d.clients.len());
    for c in &d.clients {
        println!(
            "  [{}] {} · {}",
            c.origin.label(),
            c.label,
            c.game_dir.display()
        );
        for f in &c.flavors {
            let list = hexwerk::scan_addons(&f.addons_dir);
            let iface = if f.interface_numbers.is_empty() {
                "-".to_string()
            } else {
                f.interface_numbers
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!(
                "      {:<16} Interface {:<24} {:>3} Addons",
                f.flavor.label(),
                iface,
                list.len()
            );
        }
    }
}

fn addons(extra: &[PathBuf]) {
    let d = discover(extra);
    for c in &d.clients {
        for f in &c.flavors {
            let list = hexwerk::scan_addons(&f.addons_dir);
            if list.is_empty() {
                continue;
            }
            println!(
                "\n{} · {} · {} ({})",
                c.origin.label(),
                c.label,
                f.flavor.label(),
                f.addons_dir.display()
            );
            for a in &list {
                println!(
                    "  {:<38} {:<12} {}",
                    a.folder,
                    a.version.clone().unwrap_or_else(|| "-".into()),
                    fmt_bytes(a.size_bytes)
                );
            }
        }
    }
}

/// Baut den Index: erst das Dateisystem, dann - nur mit `--net` - die Quellen.
///
/// Netzzugriff ist bewusst nicht die Vorgabe: der Index soll auch auf einem
/// Rechner ohne Internet entstehen, und ein Lauf ohne Netz darf nie so
/// aussehen, als haette er den Katalog geleert (sync_catalogue ist additiv).
fn index_cmd(args: &[String]) -> ExitCode {
    let mut net_query: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--net" {
            net_query = Some(args.get(i + 1).cloned().unwrap_or_default());
            i += 2;
            continue;
        }
        i += 1;
    }

    let path = Index::default_path();
    let mut index = match Index::open(&path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Index {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let discovery = discover(&paths(args));
    println!(
        "Scan: {} Installation(en), {} Schritte",
        discovery.clients.len(),
        discovery.notes.len()
    );
    match index.sync_clients(&discovery) {
        Ok(r) => println!(
            "Index: {} Client-Varianten, {} installierte Addons, Schema v{}",
            r.clients,
            r.installed,
            index.schema_version()
        ),
        Err(e) => {
            eprintln!("Index schreiben: {e}");
            return ExitCode::FAILURE;
        }
    }

    if let Some(query) = net_query {
        let keys = KeyProvider::default_provider();
        let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
        let registry = Registry::build(&keys, http, Cache::open_default());
        println!(
            "\nQuellen: {} aktiv{}",
            registry.sources().len(),
            if registry.absent().is_empty() {
                String::new()
            } else {
                format!(
                    ", abgeschaltet: {}",
                    registry
                        .absent()
                        .iter()
                        .map(|s| s.label())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        );
        let mut entries = Vec::new();
        for source in registry.sources() {
            match source.search(&query, 0) {
                Ok(mut e) => {
                    println!("  {:<13} {} Treffer", source.id().label(), e.len());
                    entries.append(&mut e);
                }
                Err(err) => println!("  {:<13} Fehler: {err}", source.id().label()),
            }
        }
        let merged = merge(entries);
        match index.sync_catalogue(&merged) {
            Ok(r) => println!("Katalog: {} Eintraege im Index", r.catalogue),
            Err(e) => {
                eprintln!("Katalog schreiben: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

fn search_cmd(args: &[String]) -> ExitCode {
    let query = args.join(" ");
    if query.trim().is_empty() {
        eprintln!("Aufruf: hexwerk search BEGRIFF");
        return ExitCode::from(2);
    }
    let index = match Index::open(&Index::default_path()) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Index: {e}");
            return ExitCode::FAILURE;
        }
    };
    let filters = Filters::new(query);
    match index.search(&filters, 25, 0) {
        Ok(hits) if hits.is_empty() => {
            println!("keine Treffer - `hexwerk index --net BEGRIFF` baut den Katalog.");
            ExitCode::SUCCESS
        }
        Ok(hits) => {
            for hit in hits {
                let versions = hit
                    .sources
                    .iter()
                    .map(|s| format!("{} {}", s.source.label(), s.version))
                    .collect::<Vec<_>>()
                    .join(" · ");
                println!("{:<28} {:<24} {}", hit.name, versions, hit.author);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Suche: {e}");
            ExitCode::FAILURE
        }
    }
}

fn keys_cmd(args: &[String]) -> ExitCode {
    let provider = KeyProvider::default_provider();
    let sub = args.first().map(String::as_str).unwrap_or("list");
    match sub {
        "list" => {
            println!("Konfiguration: {}", provider.config_path().display());
            for source in [Source::Wago, Source::CurseForge] {
                match provider.key_with_origin(source) {
                    Some((secret, origin)) => println!(
                        "  {:<12} {} ({})",
                        source.label(),
                        secret.masked(),
                        origin.label()
                    ),
                    None => println!("  {:<12} nicht gesetzt", source.label()),
                }
            }
            for warning in provider.warnings() {
                println!("  Warnung: {warning}");
            }
            ExitCode::SUCCESS
        }
        "set" => {
            let (Some(name), Some(value)) = (args.get(1), args.get(2)) else {
                eprintln!("Aufruf: hexwerk keys set QUELLE SCHLUESSEL");
                return ExitCode::from(2);
            };
            let Some(source) = parse_source(name) else {
                eprintln!("unbekannte Quelle: {name} (wago, curseforge)");
                return ExitCode::from(2);
            };
            match provider.set(source, value) {
                Ok(()) => {
                    println!(
                        "{} gespeichert in {}",
                        source.label(),
                        provider.config_path().display()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Schreiben: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "remove" => {
            let Some(name) = args.get(1) else {
                eprintln!("Aufruf: hexwerk keys remove QUELLE");
                return ExitCode::from(2);
            };
            let Some(source) = parse_source(name) else {
                eprintln!("unbekannte Quelle: {name}");
                return ExitCode::from(2);
            };
            match provider.remove(source) {
                Ok(true) => {
                    println!("{} entfernt", source.label());
                    ExitCode::SUCCESS
                }
                Ok(false) => {
                    println!("{} war nicht gesetzt", source.label());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Entfernen: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "test" => {
            let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
            let registry = Registry::build(&provider, http, Cache::open_default());
            for health in registry.probe_all() {
                let mark = match health.ok {
                    Some(true) => "✓",
                    Some(false) => "×",
                    None => "○",
                };
                println!("{mark} {:<13} {}", health.source.label(), health.detail);
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unbekannter Unterbefehl: {other}");
            ExitCode::from(2)
        }
    }
}

fn parse_source(name: &str) -> Option<Source> {
    match name.to_ascii_lowercase().as_str() {
        "wago" => Some(Source::Wago),
        "curseforge" | "curse" => Some(Source::CurseForge),
        _ => None,
    }
}

fn fmt_bytes(n: u64) -> String {
    let mb = n as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}
