//! Anwendung und Hauptfenster.

use adw::prelude::*;

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("de.weppelmann.Hexwerk")
        .build();
    app.connect_activate(|app| {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Hexwerk")
            .default_width(1180)
            .default_height(760)
            .build();
        let header = adw::HeaderBar::new();
        let label = gtk::Label::new(Some("Hexwerk"));
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&label));
        window.set_content(Some(&view));
        window.present();
    });
    app.run()
}
