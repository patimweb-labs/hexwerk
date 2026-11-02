//! Zahlen aus dem Index auf stdout — zum Pruefen nach einem Sync.

fn main() {
    match hexwerk_desktop::commands::catalog_stats() {
        Ok(s) => println!("Katalog {} · installiert {} · Updates {}", s.catalogue, s.installed, s.updates),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
