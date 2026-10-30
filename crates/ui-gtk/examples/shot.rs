//! Vorschau: baut das echte Fenster, laesst den Scan laufen und rendert einen
//! Bildschirm nach PNG. Nur fuer die Entwicklung - kein Teil der Anwendung.
//!
//! Aufruf: shot <bildschirm> <datei.png>

use adw::prelude::*;
use std::time::Duration;

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("de.weppelmann.HexwerkShot")
        .build();
    app.connect_startup(|_| hexwerk_ui::theme::install_css());
    app.connect_activate(|app| {
        let window = hexwerk_ui::build_window(app);
        window.present();

        let page = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "scan".to_string());
        let path = std::env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/hexwerk-shot.png".to_string());

        // Groesse pruefen: ein Fenster mit Mindesthoehe jenseits 32767 laesst
        // den X-Server mit BadAlloc abbrechen - dann ist das Layout schuld.
        let widget = window.clone().upcast::<gtk::Widget>();
        let (min_w, nat_w, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
        let (min_h, nat_h, _, _) = widget.measure(gtk::Orientation::Vertical, -1);
        println!("mass: min {min_w}x{min_h} nat {nat_w}x{nat_h}");

        let window_weak = window.downgrade();
        let app = app.clone();
        // Erst laufen lassen: der Scan braucht einen Moment, und ein Fenster
        // ohne Layout ergaebe ein leeres Bild.
        let toc_dir = std::env::args().nth(3);
        // Wartezeit bis zum Bild: kurz fuer den laufenden Scan, sonst lang
        // genug, dass der Hintergrund-Thread fertig ist.
        let delay: u64 = std::env::var("HEXWERK_SHOT_DELAY_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2500);
        glib::timeout_add_local_once(Duration::from_millis(delay), move || {
            match toc_dir.as_deref() {
                Some(dir) if page == "detail" => match detail_from(dir) {
                    Some(detail) => hexwerk_ui::app::show_local_detail(&detail),
                    None => hexwerk_ui::app::show_screen(&page),
                },
                _ => hexwerk_ui::app::show_screen(&page),
            }
            let window_weak = window_weak.clone();
            let path = path.clone();
            let app = app.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                if let Some(window) = window_weak.upgrade() {
                    match snapshot(&window, &path) {
                        Ok(()) => println!("geschrieben: {path}"),
                        Err(err) => println!("fehlgeschlagen: {err}"),
                    }
                }
                app.quit();
            });
        });
    });

    // Ohne eigene Argumentliste denkt GApplication, das Programm solle Dateien
    // oeffnen. Die Argumente (Bildschirm, Datei) sind schon eingelesen.
    let no_args: [&str; 0] = [];
    app.run_with_args(&no_args)
}

/// Baut einen Detail-Datensatz aus einem echten Addon-Ordner im Container.
fn detail_from(folder: &str) -> Option<hexwerk_ui::detail_view::LocalDetail> {
    let dir = std::path::PathBuf::from(folder);
    let toc_path = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("toc"))?;
    let toc = hexwerk::parse_file(&toc_path).ok()?;
    let addon = hexwerk::InstalledAddon {
        folder: dir.file_name()?.to_string_lossy().to_string(),
        path: dir.clone(),
        version: toc.version.clone(),
        interface_numbers: toc.interface_numbers.clone(),
        size_bytes: 2_400_000,
        toc: Some(toc),
    };
    Some(hexwerk_ui::detail_view::LocalDetail::from_addon(
        &addon,
        "battlenet",
        "_retail_",
        "~/Faugus/battlenet/drive_c/Program Files (x86)/World of Warcraft/_retail_/Interface/AddOns",
    ))
}

fn snapshot(window: &adw::ApplicationWindow, path: &str) -> Result<(), glib::BoolError> {
    let widget = window.upcast_ref::<gtk::Widget>();
    let width = widget.width();
    let height = widget.height();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let Some(node) = snapshot.to_node() else {
        return Ok(());
    };
    let Some(native) = widget.native() else {
        return Ok(());
    };
    let Some(renderer) = native.renderer() else {
        return Ok(());
    };
    let rect = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&rect));
    texture.save_to_png(path)
}
