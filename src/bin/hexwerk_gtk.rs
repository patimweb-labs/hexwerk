//! Startet die GTK4-Oberflaeche (Feature `ui`).

fn main() -> std::process::ExitCode {
    // `ExitCode::value` stammt aus glib. Dieses Paket haengt bewusst nicht an
    // glib (das GTK-Binary soll ohne `ui`-Feature bauen koennen), deshalb wird
    // der Code hier ueber std weitergereicht.
    std::process::ExitCode::from(hexwerk_ui::run().value() as u8)
}
