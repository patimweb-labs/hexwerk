//! Zeigt, was der Index ueber installierte Addons weiss.

fn main() {
    let idx = match hexwerk_index::Index::open(&hexwerk_index::Index::default_path()) {
        Ok(i) => i,
        Err(e) => { eprintln!("Index: {e}"); std::process::exit(1); }
    };
    let stats = idx.stats().expect("stats");
    println!("Katalog {} · installiert {} · Updates {}", stats.catalogue, stats.installed, stats.updates);
    for treffer in idx.search(&hexwerk_index::Filters::new(""), 400, 0).unwrap_or_default() {
        if treffer.installed.is_empty() { continue; }
        for i in &treffer.installed {
            println!("  {} [{} / {}] {} {}", treffer.name, i.client_label, i.flavor, i.folder, i.version.clone().unwrap_or_default());
        }
    }
}
