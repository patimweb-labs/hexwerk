//! `hexwerk` - Kommandozeile: finden, anzeigen, indexieren.

use std::path::PathBuf;
use std::process::ExitCode;

use hexwerk::clients::{discover, NoteStatus};
use hexwerk::{scan_addons, Flavor};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "scan".to_string());
    let mut extra: Vec<PathBuf> = Vec::new();
    for a in args {
        extra.push(PathBuf::from(a));
    }
    match cmd.as_str() {
        "scan" | "clients" => {
            scan(&extra);
            ExitCode::SUCCESS
        }
        "addons" => {
            addons(&extra);
            ExitCode::SUCCESS
        }
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

fn usage() {
    println!("hexwerk - Addon-Manager fuer World of Warcraft unter Linux\n");
    println!("  hexwerk scan [ORDNER…]    Installationen suchen");
    println!("  hexwerk addons [ORDNER…]  Installationen und ihre Addons listen");
    println!("  hexwerk help              diese Hilfe");
    println!("\nOhne Angabe von ORDNERN werden Steam, Faugus, Heroic, Lutris, Bottles und PortProton durchsucht.");
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
            let n = scan_addons(&f.addons_dir).len();
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
                n
            );
        }
    }
}

fn addons(extra: &[PathBuf]) {
    let d = discover(extra);
    for c in &d.clients {
        for f in &c.flavors {
            let list = scan_addons(&f.addons_dir);
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

fn fmt_bytes(n: u64) -> String {
    let mb = n as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}

#[allow(dead_code)]
fn unused(_: Flavor) {}
