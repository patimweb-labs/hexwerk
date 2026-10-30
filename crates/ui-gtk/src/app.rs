//! Anwendung, Fenster und Navigation.
//!
//! Aufbau: `AdwNavigationSplitView` mit der Leiste links (Ansichten, Bestand,
//! Quellen) und einem `GtkStack` rechts, in dem die fuenf Bildschirme liegen.
//! Die Leiste ist bewusst keine Registerkarte: was gerade laeuft, sieht man an
//! der Markierung links, und der Kopfbereich bleibt fuer den
//! Farbschema-Umschalter und die Fensterknoepfe frei.
//!
//! Dieses Modul ist die einzige Stelle, die Index, Quellen und Installation
//! zusammenschaltet: der Scan laeuft im Thread (state.rs), der Indexaufbau und
//! die Installation ebenfalls (backend.rs), und hier werden die Ergebnisse auf
//! die Bildschirme verteilt. Die Oberflaeche liest den Index selbst - aber nur
//! ueber die Methoden aus `hexwerk-index`, nie mit eigenem SQL.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use hexwerk_index::{source_key, Filters, Index, Stats};
use hexwerk_sources::{KeyProvider, Source};

use crate::backend::{self, IndexMsg, InstallJob, InstallMsg, SourceRun};
use crate::catalog_view::{self, CatalogView};
use crate::clients_view::ClientsView;
use crate::detail_view::{DetailTarget, DetailView, InstallChoice};
use crate::format::iso_date;
use crate::installed_view::{InstalledView, OpenRequest};
use crate::scan_view::ScanView;
use crate::settings_view::SettingsView;
use crate::sources_view::{SourceInfo, SourcesView};
use crate::state::{ScanData, ScanMsg};
use crate::theme::Theme;

pub const APP_ID: &str = "de.weppelmann.Hexwerk";

/// Reihenfolge der Bildschirme: Name im Stack, Titel, Untertitel fuer den
/// Kopfbereich. Eine Tabelle, damit Leiste und Kopf nicht auseinanderlaufen.
const PAGES: [(&str, &str, &str); 5] = [
    ("clients", "Clients", "Gefundene WoW-Installationen"),
    ("installed", "Installiert", "Addons je Client-Ordner"),
    ("catalog", "Katalog", "Suche im lokalen Index"),
    ("detail", "Addon-Detail", "Quellen, Versionen, Ziel"),
    (
        "settings",
        "Einstellungen",
        "Sprache, Aussehen und Ablage",
    ),
];

/// Zaehler in der Leiste und in der Fusszeile. Eigene Labels, weil ein Widget
/// nur an einer Stelle im Baum haengen kann.
struct Counts {
    nav_clients: gtk::Label,
    nav_installed: gtk::Label,
    nav_catalog: gtk::Label,
    info_addons: gtk::Label,
    footer: gtk::Label,
    /// Erklaerender Text links in der Fusszeile; muss beim Sprachwechsel
    /// mitziehen wie alle anderen Texte dort.
    footer_scan: gtk::Label,
    /// Was gerade laeuft („Scan läuft …", „Katalog wird befragt …") - der
    /// einzige Ort, an dem man ohne Klick sieht, dass die App arbeitet.
    footer_state: gtk::Label,
    footer_busy: adw::Spinner,
    /// Die Markenzeile oben in der Leiste (Logo plus Wortmarke).
    brand_row: gtk::Box,
}

/// Eine Ansicht in der Leiste. Ein Knopf statt einer Listenzeile: so ist die
/// Navigation mit Tastatur und Screenreader bedienbar (Rolle „push button“),
/// und der Klickpfad ist derselbe wie bei jedem anderen Knopf.
struct NavItem {
    name: &'static str,
    button: gtk::Button,
    /// Die sichtbare Beschriftung: sie haengt an der Sprache und wird beim
    /// Wechsel neu gesetzt, ohne die Leiste neu zu bauen.
    label: gtk::Label,
    /// Symbol fuer die kompakte Leiste (nur Symbole, keine Beschriftung).
    icon: gtk::Image,
}

struct Ui {
    split: adw::NavigationSplitView,
    stack: gtk::Stack,
    nav: Vec<NavItem>,
    title: adw::WindowTitle,
    back: gtk::Button,
    toast: adw::ToastOverlay,
    scan: ScanView,
    clients_view: Rc<ClientsView>,
    installed: Rc<InstalledView>,
    catalog: Rc<CatalogView>,
    detail: Rc<DetailView>,
    counts: Counts,
    /// Laufband in der Fusszeile: laeuft, solange Katalog oder Scan arbeiten.
    nyan: crate::nyan::NyanCat,
    /// Quellenzustand links; die Texte kommen aus dem Index und dem letzten
    /// Lauf, nicht aus einer festen Liste.
    rail_sources: Vec<(Source, gtk::Label)>,
    data: RefCell<Option<Rc<ScanData>>>,
    /// Leseverbindung des Hauptthreads. Der Hintergrundlauf oeffnet seine
    /// eigene - WAL laesst Lesen und Schreiben parallel zu (schema.rs).
    index: RefCell<Option<Index>>,
    /// Ergebnis des letzten Quellenlaufs, fuer Tooltips und Fusszeile.
    index_sources: RefCell<Vec<SourceRun>>,
    /// Nummer des laufenden Indexaufbaus. Meldungen eines ueberholten Laufs
    /// werden verworfen, sonst ueberschreibt ein alter Lauf den neuen Zustand.
    pipeline: Cell<u64>,
    /// Muss am Leben bleiben: der Umschalter haelt nur schwache Verweise auf
    /// das Theme. Faellt der starke Verweis weg, reagiert das Menue nicht mehr.
    /// Der Unterstrich sagt dem Compiler, dass hier absichtlich nichts gelesen
    /// wird: der Wert existiert nur, um zu leben.
    _theme: Rc<Theme>,
    /// Fortschrittsleiste ueber der Fusszeile: nur waehrend einer Installation sichtbar.
    progress_box: gtk::Box,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    /// Ueberschriften und Zahlen der Leiste: in der kompakten Leiste weg.
    rail_extra: Vec<gtk::Widget>,
    /// Brotkrumen in der Kopfleiste: Weg zurueck in den Katalog.
    crumb: gtk::Box,
    crumb_name: gtk::Label,
    /// Bildschirm "Quellen": Zustand, Test, Schluessel.
    sources: Rc<SourcesView>,
    /// Bildschirm "Einstellungen": Rangfolge der Quellen, Sprache.
    settings_view: Rc<SettingsView>,
    /// Testergebnisse dieser Sitzung, je Quelle.
    source_tests: RefCell<std::collections::BTreeMap<hexwerk_sources::Source, SourceInfo>>,
}

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    // CSS erst, wenn eine Anzeige da ist - vorher gibt es keinen StyleContext,
    // an dem der Provider haengen koennte.
    app.connect_startup(|_| crate::theme::install_css());
    app.connect_activate(|app| {
        let window = build_window(app);
        window.present();
    });
    app.run()
}

/// Baut das Hauptfenster. Oeffentlich, damit eine Vorschau oder ein Test es
/// ohne `run()` und ohne eigene Hauptschleife aufbauen kann.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    // Allererste Handlung: die Sprache steht, bevor irgendein Widget einen
    // Text bekommt. Sonst entstehen Titel und Hinweise auf Deutsch, waehrend
    // die Leiste schon in der gewaehlten Sprache steht.
    crate::lang::set_current(&crate::settings_view::Settings::load().language);
    let window: adw::ApplicationWindow = adw::ApplicationWindow::builder()
        .application(app)
        .icon_name(APP_ID)
        .title("Hexwerk")
        .default_width(1280)
        .default_height(820)
        .build();
    window.add_css_class("hw-window");

    let theme = Theme::new(window.upcast_ref::<gtk::Window>());

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(120);

    let scan = ScanView::new();
    let clients_view = ClientsView::new();
    let installed = InstalledView::new();
    let catalog = CatalogView::new();
    let detail = DetailView::new();
    let settings_view = SettingsView::new();

    let sources = SourcesView::new();

    stack.add_named(clients_view.widget(), Some("clients"));
    stack.add_named(installed.widget(), Some("installed"));
    stack.add_named(catalog.widget(), Some("catalog"));

    stack.add_named(settings_view.widget(), Some("settings"));
    stack.add_named(detail.widget(), Some("detail"));

    // ---- Kopfbereich der Inhaltsspalte ----
    let header = adw::HeaderBar::new();
    header.add_css_class("hw-header");
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(true);

    let title = adw::WindowTitle::new("Scan", "Herkünfte prüfen");
    header.set_title_widget(Some(&title));

    // Im schmalen Fenster klappt die Leiste weg; dann braucht es einen Weg
    // zurueck, sonst ist die Auswahl nicht mehr erreichbar.
    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.add_css_class("hw-choice");
    back.set_tooltip_text(Some("Leiste zeigen"));
    back.set_visible(false);
    crate::theme::pointer(&back);
    header.pack_start(&back);

    // Brotkrumen: auf der Detailseite steht hier der Weg zurueck in den
    // Katalog. Die Detailseite ist keine eigene Ansicht in der Leiste mehr,
    // deshalb braucht sie diesen Weg.
    let crumb = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    crumb.add_css_class("hw-crumb");
    crumb.set_visible(false);
    let crumb_root = gtk::Button::with_label("Katalog");
    crumb_root.add_css_class("hw-crumb-link");
    crate::theme::pointer(&crumb_root);
    let crumb_sep = gtk::Label::new(Some("›"));
    crumb_sep.add_css_class("hw-dim2");
    let crumb_name = gtk::Label::new(Some(""));
    crumb_name.add_css_class("hw-crumb-here");
    crumb.append(&crumb_root);
    crumb.append(&crumb_sep);
    crumb.append(&crumb_name);
    // Der Rueckweg haengt am Thread-Platz, weil `ui` erst nach dem Aufbau des
    // Fensters existiert; geklickt wird ohnehin spaeter.
    crumb_root.connect_clicked(|_| {
        if let Some(ui) = UI.with(|slot| slot.borrow().clone()) {
            show_page(&ui, "catalog");
        }
    });
    header.pack_start(&crumb);

    // Sonne/Mond statt Menue: ein Klick schaltet, das Icon zeigt den Zustand.
    let theme_toggle = theme.dark_toggle();
    header.pack_end(&theme_toggle);

    // ---- Fusszeile ----
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    footer.add_css_class("hw-footer");

    let brand = gtk::Label::new(Some("HEXWERK"));
    brand.add_css_class("hw-footer-item");

    // Erklaertexte stehen nicht mehr in der Fusszeile: sie war zur Haelfte
    // Prosa. Was bleibt, sind Schlagworte und Zahlen.

    let lang_now = crate::settings_view::Settings::load().language;
    let footer_stats = gtk::Label::new(Some(crate::lang::t(&lang_now, "st.footer.noScan")));
    footer_stats.add_css_class("hw-footer-item");

    let footer_scan = gtk::Label::new(Some(crate::lang::t(&lang_now, "st.footer.stand")));
    footer_scan.add_css_class("hw-footer-item");

    // Was gerade laeuft. Ohne diese Zeile sieht ein Katalogaufbau aus wie eine
    // haengende App: der Scan laeuft im Hintergrund, das Fenster bleibt
    // bedienbar, und nichts sagt, dass Arbeit im Gange ist.
    let nyan = crate::nyan::NyanCat::new();

    let footer_busy = adw::Spinner::new();
    footer_busy.set_size_request(14, 14);
    footer_busy.set_valign(gtk::Align::Center);
    footer_busy.set_visible(false);

    let footer_state = gtk::Label::new(Some(crate::lang::t(&lang_now, "st.footer.idle")));
    footer_state.add_css_class("hw-footer-item");
    footer_state.set_ellipsize(gtk::pango::EllipsizeMode::End);
    footer_state.set_max_width_chars(70);
    footer_state.set_hexpand(true);
    footer_state.set_xalign(0.0);

    footer.append(&brand);
    // Die Katze fliegt rechts neben der Wortmarke, nicht davor: links steht
    // der Name, danach die Anzeige, danach der Zustand.
    footer.append(nyan.widget());
    footer.append(&footer_busy);
    footer.append(&footer_state);
    footer.append(&footer_stats);
    footer.append(&footer_scan);

    // ---- Fortschritt einer Installation ----
    // Eigene Leiste ueber der Fusszeile: sie erscheint nur, solange etwas
    // geladen oder entpackt wird, und nennt Schritt, Groesse und Tempo.
    let progress_label = gtk::Label::new(Some(""));
    progress_label.add_css_class("hw-footer-item");
    progress_label.set_xalign(0.0);
    progress_label.set_hexpand(true);
    progress_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let progress_bar = gtk::ProgressBar::new();
    progress_bar.set_show_text(false);
    progress_bar.set_hexpand(true);

    let progress_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    progress_box.add_css_class("hw-progress");
    progress_box.set_margin_start(18);
    progress_box.set_margin_end(18);
    progress_box.set_margin_top(8);
    progress_box.set_margin_bottom(8);
    progress_box.append(&progress_label);
    progress_box.append(&progress_bar);
    progress_box.set_visible(false);

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&header);
    content_view.add_bottom_bar(&progress_box);
    content_view.add_bottom_bar(&footer);
    content_view.set_content(Some(&stack));

    // ---- Leiste links ----
    let (rail, nav, counts, rail_sources, extras) = build_rail();

    let rail_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&rail)
        .build();
    rail_scroll.add_css_class("hw-rail");

    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.set_content(Some(&rail_scroll));

    let split = adw::NavigationSplitView::new();
    split.set_min_sidebar_width(212.0);
    split.set_max_sidebar_width(268.0);
    split.set_sidebar(Some(&adw::NavigationPage::new(&sidebar_view, "Hexwerk")));
    split.set_content(Some(&adw::NavigationPage::new(&content_view, "Hexwerk")));

    // Eine Rueckmeldung ueber Erfolg oder Fehler braucht einen Ort, der ueber
    // allen Bildschirmen liegt - deshalb umschliesst das Overlay die Ansicht
    // und nicht nur einen Bildschirm.
    let toast = adw::ToastOverlay::new();
    toast.set_child(Some(&split));
    window.set_content(Some(&toast));

    let ui = Rc::new(Ui {
        split,
        stack,
        nav,
        back,
        title,
        toast,
        scan,
        clients_view,
        installed,
        catalog,
        detail,
        counts: Counts {
            footer: footer_stats,
            footer_scan,
            footer_state,
            footer_busy,
            ..counts
        },
        nyan,
        progress_box,
        progress_bar,
        progress_label,
        rail_sources,
        data: RefCell::new(None),
        index: RefCell::new(None),
        index_sources: RefCell::new(Vec::new()),
        pipeline: Cell::new(0),
        _theme: theme,
        rail_extra: extras,
        crumb,
        crumb_name,
        sources,
        settings_view,
        source_tests: RefCell::new(std::collections::BTreeMap::new()),
    });

    wire(&ui);
    // Drag and drop: ein ZIP oder eine Adresse aufs Fenster ziehen.
    attach_drop_targets(&ui, &window);
    // Leiste und Kopfzeile einmal in der gespeicherten Sprache setzen: die
    // Beschriftungen entstehen auf Deutsch, und ohne diesen Aufruf stand beim
    // Start „Katalog" neben „Catalogue" in der Fusszeile.
    apply_language(&ui, &crate::lang::current());
    // Die Rueckrufe haengen als schwache Verweise an diesen Strukturen. Ohne
    // einen starken Verweis hier waeren sie nach dem Aufbau sofort weg - das
    // Fenster haette dann Knoepfe, die nichts tun. Ein Fenster je Prozess,
    // deshalb reicht ein Platz im Thread.
    UI.with(|slot| *slot.borrow_mut() = Some(ui));

    window
}

thread_local! {
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

fn wire(ui: &Rc<Ui>) {
    // Jeder Knopf in der Leiste schaltet seinen Bildschirm.
    for item in &ui.nav {
        let weak = Rc::downgrade(ui);
        let name = item.name;
        item.button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                show_page(&ui, name);
            }
        });
    }

    // Scan-Knopf: voller Lauf, inklusive Quellen.
    let weak = Rc::downgrade(ui);
    {
        // Klick auf einen Client-Ordner: nach "Installiert" springen und genau
        // diesen Ordner auswaehlen.
        let weak = Rc::downgrade(ui);
        ui.clients_view.set_on_open_flavor(move |client, flavor| {
            if let Some(ui) = weak.upgrade() {
                ui.installed.select_target(client, flavor);
                show_page(&ui, "installed");
            }
        });
    }

    {
        let weak = Rc::downgrade(ui);
        ui.sources.set_on_test(move |source| {
            if let Some(ui) = weak.upgrade() {
                run_source_test(&ui, source);
            }
        });
    }
    {
        let weak = Rc::downgrade(ui);
        ui.sources.set_on_key(move |source| {
            if let Some(ui) = weak.upgrade() {
                ask_key(&ui, source);
            }
        });
    }

    // Deinstallieren und Leeren: bestaetigen, in den Papierkorb, Bericht als
    // Toast mit Rueckgaengig.
    {
        let weak = Rc::downgrade(ui);
        ui.installed.set_on_remove(move |folders| {
            if let Some(ui) = weak.upgrade() {
                confirm_remove(&ui, folders);
            }
        });
    }
    {
        let weak = Rc::downgrade(ui);
        ui.installed.set_on_clear(move || {
            if let Some(ui) = weak.upgrade() {
                confirm_clear(&ui);
            }
        });
    }

    ui.scan.connect_rescan(move || {
        if let Some(ui) = weak.upgrade() {
            start_scan(&ui, false);
        }
    });

    // Doppelklick in der Tabelle oeffnet die Detailseite.
    let weak = Rc::downgrade(ui);
    ui.installed.set_on_open(move |request| {
        if let Some(ui) = weak.upgrade() {
            open_installed(&ui, request);
        }
    });

    // Suche und Quellenfilter im Katalog beantwortet der Index.
    let weak = Rc::downgrade(ui);
    ui.catalog.set_on_search(move |filters| {
        if let Some(ui) = weak.upgrade() {
            if let Err(err) = search_catalogue(&ui, &filters) {
                ui.catalog.show_error(&err);
            }
        }
    });

    // Klick auf eine Karte: die Detailseite liest den vollen Eintrag.
    let weak = Rc::downgrade(ui);
    ui.catalog.set_on_open(move |id| {
        if let Some(ui) = weak.upgrade() {
            open_item(&ui, id);
        }
    });

    // Installieren: Download und Entpacken im Hintergrund.
    let weak = Rc::downgrade(ui);
    ui.detail.set_on_install(move |choice| {
        if let Some(ui) = weak.upgrade() {
            start_install(&ui, choice);
        }
    });

    // Zurueck zur Leiste, wenn sie weggeklappt ist.
    let weak = Rc::downgrade(ui);
    ui.back.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.split.set_show_content(false);
        }
    });

    let weak = Rc::downgrade(ui);
    let back = ui.back.clone();
    ui.split.connect_collapsed_notify(move |split| {
        back.set_visible(split.is_collapsed());
        // Im schmalen Fenster bleibt die Leiste stehen, bis man etwas waehlt.
        if let Some(_ui) = weak.upgrade() {
            if !split.is_collapsed() {
                split.set_show_content(true);
            }
        }
    });

    // Einstellungen: Rueckmeldung als Toast und sofortige Wirkung.
    let weak = Rc::downgrade(ui);
    ui.settings_view.set_on_saved(move |message| {
        if let Some(ui) = weak.upgrade() {
            toast(&ui, message);
        }
    });

    let weak = Rc::downgrade(ui);
    ui.settings_view.set_on_change(move |settings| {
        if let Some(ui) = weak.upgrade() {
            apply_language(&ui, &settings.language);
            apply_nav_mode(&ui, settings.nav == "compact");
            refresh_settings_sources(&ui);
            start_scan(&ui, false);
        }
    });

    // Installieren von Hand: Datei waehlen oder Adresse eingeben. Beides
    // landet im selben Weg wie ein abgelegtes ZIP.
    let weak = Rc::downgrade(ui);
    ui.installed.set_on_install_file(move || {
        let Some(ui) = weak.upgrade() else { return };
        // `FileChooserDialog` statt `FileDialog`: gebunden ist GTK 4.6,
        // `FileDialog` gibt es erst ab 4.10.
        // `FileChooserNative`: in GTK4 gibt es den Dateidialog nur noch so,
        // `FileChooserDialog` kennt die Auswahl-Schnittstelle nicht mehr.
        let dialog = gtk::FileChooserNative::new(
            Some(crate::lang::tn("iv.install")),
            None::<&gtk::Window>,
            gtk::FileChooserAction::Open,
            Some(crate::lang::tn("iv.install.ok")),
            Some(crate::lang::tn("iv.install.cancel")),
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Addon-ZIP"));
        filter.add_pattern("*.zip");
        filter.add_pattern("*.ZIP");
        dialog.add_filter(&filter);

        let weak_inner = Rc::downgrade(&ui);
        dialog.connect_response(move |dialog, response| {
            let path = dialog.file().and_then(|file| file.path());
            dialog.hide();
            if response != gtk::ResponseType::Accept {
                return;
            }
            let Some(ui) = weak_inner.upgrade() else { return };
            let Some(path) = path else { return };
            match std::fs::read(&path) {
                Ok(data) => {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default();
                    install_dropped(&ui, data, name);
                }
                Err(err) => toast_error(&ui, &format!("{}: {err}", path.display())),
            }
        });
        dialog.show();
    });

    let weak = Rc::downgrade(ui);
    ui.installed.set_on_install_url(move |url| {
        let Some(ui) = weak.upgrade() else { return };
        let lang = ui_lang(&ui);
        set_activity(&ui, &crate::lang::t(&lang, "st.kw.download"), true);
        let (tx, rx) = async_channel::bounded::<Result<Vec<u8>, String>>(1);
        let target = url.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<u8>, String> {
                let agent = ureq::AgentBuilder::new()
                    .timeout(std::time::Duration::from_secs(180))
                    .build();
                let response = agent
                    .get(&target)
                    .call()
                    .map_err(|err| format!("{target}: {err}"))?;
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
                    .map_err(|err| format!("{target}: {err}"))?;
                Ok(bytes)
            })();
            let _ = tx.send_blocking(result);
        });
        let weak_inner = Rc::downgrade(&ui);
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(ui) = weak_inner.upgrade() else { return };
            set_activity(&ui, crate::lang::tn("st.ready"), false);
            match result {
                Ok(data) => {
                    let name = url.rsplit('/').next().unwrap_or("addon.zip").to_string();
                    install_dropped(&ui, data, name);
                }
                Err(err) => toast_error(&ui, &err),
            }
        });
    });

    let weak = Rc::downgrade(ui);
    ui.installed.set_on_update(move || {
        if let Some(ui) = weak.upgrade() {
            start_update_all(&ui);
        }
    });

    let weak = Rc::downgrade(ui);
    ui.clients_view.set_on_rescan(move || {
        if let Some(ui) = weak.upgrade() {
            start_scan(&ui, false);
        }
    });

    let compact = ui.settings_view.settings().nav == "compact";
    apply_nav_mode(ui, compact);

    // Der Scan startet, sobald das Fenster steht - der Startbildschirm ist
    // sonst ein leerer Bildschirm mit einem Knopf.
    let weak = Rc::downgrade(ui);
    glib::idle_add_local_once(move || {
        if let Some(ui) = weak.upgrade() {
            show_page(&ui, "scan");
            // Nur Ordner lesen: den Katalog liefert der Dienst. Ein eigener
            // Quellenlauf wuerde ihn mit denselben Addons aus zweiter Hand
            // auffuellen und die Zahl verfaelschen.
            start_scan(&ui, false);
            // Der Katalog kommt fertig vom Dienst: der Abruf laeuft neben dem
            // Scan, damit die Liste gefuellt ist, bevor man sie braucht.
            start_catalog_sync(&ui);
        }
    });
}

/// Nur fuer Vorschau und Tests: einen Bildschirm anzeigen, ohne Klick. Die
/// Vorschau (siehe `examples/shot.rs`) rendert damit jeden Bildschirm in eine
/// PNG-Datei, ohne die Oberflaeche umbauen zu muessen.
pub fn show_screen(name: &str) {
    if let Some(ui) = UI.with(|slot| slot.borrow().clone()) {
        show_page(&ui, name);
    }
}

/// Nur fuer Vorschau und Tests: die Detailseite mit einem Datensatz aus der
/// `.toc` fuellen (der Weg ohne Katalogeintrag).
pub fn show_local_detail(detail: &crate::detail_view::LocalDetail) {
    if let Some(ui) = UI.with(|slot| slot.borrow().clone()) {
        ui.detail.show_local(detail);
        set_crumb(
            &ui,
            if detail.title.is_empty() {
                &detail.folder
            } else {
                &detail.title
            },
        );
        show_page(&ui, "detail");
    }
}

// ---------------------------------------------------------------------------
// Quellen
// ---------------------------------------------------------------------------

/// Baut die Uebersicht neu: Zahlen aus dem Index, Schluessel aus der Ablage,
/// Testergebnisse aus dieser Sitzung.
fn refresh_sources(ui: &Rc<Ui>) {
    let keys = hexwerk_sources::keys::KeyProvider::default_provider();
    let counts: std::collections::BTreeMap<&'static str, usize> =
        with_index(ui, |index| index.source_counts()).unwrap_or_default();
    let mut infos = ui.source_tests.borrow().clone();
    for (source, entries) in &counts {
        if let Some(source) = hexwerk_index::source_from_key(source) {
            let info = infos.entry(source).or_default();
            info.entries = *entries;
        }
    }
    ui.sources.refresh(&counts, &keys, &infos);
}

/// Fuehrt einen Verbindungstest im Hintergrund aus. Netz im Hauptthread waere
/// ein eingefrorenes Fenster bei jeder langsamen Quelle.
fn run_source_test(ui: &Rc<Ui>, source: hexwerk_sources::Source) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("hexwerk-quellentest".to_string())
        .spawn(move || {
            let keys = hexwerk_sources::keys::KeyProvider::default_provider();
            let http: std::sync::Arc<dyn hexwerk_sources::http::Http> =
                std::sync::Arc::new(hexwerk_sources::http::UreqHttp::new());
            let registry = hexwerk_sources::Registry::build(
                &keys,
                http,
                hexwerk_sources::cache::Cache::open_default(),
            );
            let health = match registry.source(source) {
                Some(s) => hexwerk_sources::AddonSource::probe(s),
                None => hexwerk_sources::SourceHealth {
                    source,
                    ok: Some(false),
                    detail: "nicht eingetragen, kein Schlüssel".to_string(),
                },
            };
            let _ = tx.send_blocking(health);
        })
        .ok();

    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        let Ok(health) = rx.recv().await else { return };
        let Some(ui) = weak.upgrade() else { return };
        let ok = health.ok.unwrap_or(false);
        {
            let mut tests = ui.source_tests.borrow_mut();
            let info = tests.entry(health.source).or_default();
            info.test = Some((ok, health.detail.clone()));
        }
        ui.sources.set_test(health.source, ok, &health.detail);
        refresh_sources(&ui);
        if ok {
            toast(
                &ui,
                &format!("{}: {}", health.source.label(), health.detail),
            );
        } else {
            toast_error(
                &ui,
                &format!("{}: {}", health.source.label(), health.detail),
            );
        }
    });
}

/// Schluessel setzen oder entfernen. Der Wert wird nie angezeigt, nur die
/// letzten vier Zeichen.
fn ask_key(ui: &Rc<Ui>, source: hexwerk_sources::Source) {
    use hexwerk_sources::keys::KeyProvider;
    let keys = KeyProvider::default_provider();
    let Some(window) = main_window(ui) else {
        return;
    };

    if keys.key(source).is_some() {
        let dialog = adw::AlertDialog::new(
            Some(&format!("{}: Schlüssel entfernen?", source.label())),
            Some("Ohne Schlüssel bleibt die Quelle abgeschaltet. Der Katalog behält, was schon im Index steht."),
        );
        dialog.add_response("cancel", "Abbrechen");
        dialog.add_response("remove", "Entfernen");
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(ui);
        let src = source;
        dialog.connect_response(None, move |_dialog, response| {
            if response != "remove" {
                return;
            }
            let Some(ui) = weak.upgrade() else { return };
            match keys.remove(src) {
                Ok(_) => {
                    toast(&ui, &format!("{}: Schlüssel entfernt", src.label()));
                    refresh_sources(&ui);
                }
                Err(err) => toast_error(&ui, &format!("Entfernen: {err}")),
            }
        });
        dialog.present(Some(&window));
        return;
    }

    let dialog = adw::AlertDialog::new(
        Some(&format!("{}: Schlüssel setzen", source.label())),
        Some("Der Schlüssel wird in ~/.config/hexwerk/keys.json abgelegt und nie vollständig angezeigt."),
    );
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("Schlüssel oder Token"));
    entry.set_visibility(false);
    entry.add_css_class("hw-search");
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", "Abbrechen");
    dialog.add_response("save", "Speichern");
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    dialog.set_close_response("cancel");

    let weak = Rc::downgrade(ui);
    let src = source;
    dialog.connect_response(None, move |_dialog, response| {
        if response != "save" {
            return;
        }
        let Some(ui) = weak.upgrade() else { return };
        let value = entry.text().to_string();
        if value.trim().is_empty() {
            toast_error(&ui, "Leerer Schlüssel wird nicht gespeichert.");
            return;
        }
        match keys.set(src, value.trim()) {
            Ok(()) => {
                toast(&ui, &format!("{}: Schlüssel gespeichert", src.label()));
                refresh_sources(&ui);
            }
            Err(err) => toast_error(&ui, &format!("Speichern: {err}")),
        }
    });
    dialog.present(Some(&window));
}

/// Brotkrumen setzen: der Weg zurueck in den Katalog nennt das offene Addon.
fn set_crumb(ui: &Ui, name: &str) {
    ui.crumb_name.set_text(name);
    ui.crumb.set_visible(true);
}

/// Bildschirm zeigen und Leiste nachziehen. Idempotent, damit Aufrufe aus
/// Rueckrufen und aus der Leiste sich nicht im Kreis drehen.
/// Beschriftung einer Seite in der gewaehlten Sprache (Rueckfall: Deutsch).
fn nav_text(lang: &str, page: &str, fallback: &str) -> String {
    let key = format!("nav.{page}");
    let text = crate::lang::t(lang, &key);
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.to_string()
    }
}

/// Untertitel einer Seite in der gewaehlten Sprache.
fn page_text(lang: &str, page: &str, fallback: &str) -> String {
    let key = format!("page.{page}.sub");
    let text = crate::lang::t(lang, &key);
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.to_string()
    }
}

/// Symbol je Seite fuer die kompakte Leiste.
/// Waagerechter Ausgleich je Symbol, in Pixeln. Nur wo das Symbol nicht
/// mittig in seinem Feld steht; alles andere bleibt bei 0.
fn nav_icon_nudge(page: &str) -> Option<i32> {
    match page {
        "catalog" => Some(4),
        _ => None,
    }
}

fn nav_icon(page: &str) -> &'static str {
    match page {
        "clients" => "computer-symbolic",
        "installed" => "folder-symbolic",
        "catalog" => "system-search-symbolic",
        "settings" => "emblem-system-symbolic",
        _ => "view-list-symbolic",
    }
}

/// Stellt die Leiste um: voll (Symbol, Beschriftung, Zahlen) oder kompakt
/// (nur Symbole, Statistik weg, schmal).
fn apply_nav_mode(ui: &Rc<Ui>, compact: bool) {
    for item in &ui.nav {
        item.label.set_visible(!compact);
        item.icon.set_visible(true);
        // Ohne Beschriftung muss der Tooltip sagen, wohin der Knopf fuehrt —
        // ein Ordner-Icon allein erklaert „Installiert" nicht.
        let title = PAGES
            .iter()
            .find(|(page, _, _)| *page == item.name)
            .map(|(_, title, subtitle)| format!("{title}: {subtitle}"))
            .unwrap_or_else(|| item.name.to_string());
        item.button.set_tooltip_text(Some(&title));

        // In der schmalen Leiste: quadratische Flaeche, Inhalt mittig. Sonst
        // kleben die Symbole links am Rand, waehrend das Logo oben mittig steht.
        if compact {
            item.button.set_size_request(48, 44);
            item.button.set_halign(gtk::Align::Fill);
            if let Some(child) = item.button.child() {
                child.set_halign(gtk::Align::Center);
            }
        } else {
            item.button.set_size_request(-1, -1);
            if let Some(child) = item.button.child() {
                child.set_halign(gtk::Align::Fill);
            }
        }
    }
    for extra in &ui.rail_extra {
        extra.set_visible(!compact);
    }
    ui.counts.nav_clients.set_visible(!compact);
    ui.counts.nav_installed.set_visible(!compact);
    ui.counts.nav_catalog.set_visible(!compact);
    // Klasse fuer das symmetrische Innenmass der schmalen Leiste.
    if let Some(nav_box) = ui.nav.first().and_then(|item| item.button.parent()) {
        if compact {
            nav_box.add_css_class("hw-nav-compact");
        } else {
            nav_box.remove_css_class("hw-nav-compact");
        }
    }

    // Markenzeile: schmal mittig, breit wieder linksbuendig.
    ui.counts.brand_row.set_halign(if compact {
        gtk::Align::Center
    } else {
        gtk::Align::Fill
    });
    ui.counts
        .brand_row
        .set_margin_start(if compact { 0 } else { 18 });

    // Schmal: 64 px reichen fuer ein Symbol mit Rand.
    ui.split
        .set_min_sidebar_width(if compact { 64.0 } else { 212.0 });
    ui.split
        .set_max_sidebar_width(if compact { 92.0 } else { 268.0 });
}

/// Setzt Navigation und Kopfzeile auf die gewaehlte Sprache um.
///
/// Nur das Geruest: die Fliesstexte der einzelnen Bildschirme kommen als
/// naechster Block. Ein Wechsel wirkt sofort, ohne Neustart.
fn apply_language(ui: &Rc<Ui>, lang: &str) {
    // Zuerst die globale Sprache: alles, was danach Texte baut, hat sie schon.
    crate::lang::set_current(lang);
    for item in &ui.nav {
        let title = PAGES
            .iter()
            .find(|(page, _, _)| *page == item.name)
            .map(|(page, title, _)| nav_text(lang, page, title))
            .unwrap_or_else(|| item.name.to_string());
        item.label.set_text(&title);
        item.button
            .update_property(&[gtk::accessible::Property::Label(&title)]);
    }
    // Seite und Einstellungen ziehen ihre Texte nach; sonst bleibt der offene
    // Bildschirm in der alten Sprache stehen.
    ui.settings_view.retranslate(lang);
    ui.installed.retranslate(lang);
    ui.catalog.retranslate();
    ui.clients_view.retranslate();
    relabel_footer(ui, lang);
    let current = ui
        .stack
        .visible_child_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "scan".to_string());
    show_page(ui, &current);
}

fn show_page(ui: &Rc<Ui>, name: &str) {
    if ui.stack.visible_child_name().as_deref() != Some(name) {
        ui.stack.set_visible_child_name(name);
    }
    let language = ui.settings_view.settings().language;
    if let Some((page, title, subtitle)) = PAGES.iter().find(|(page, _, _)| *page == name) {
        ui.title.set_title(&nav_text(&language, page, title));
        ui.title.set_subtitle(&page_text(&language, page, subtitle));
    }
    for item in &ui.nav {
        if item.name == name {
            item.button.add_css_class("on");
            item.button
                .update_state(&[gtk::accessible::State::Selected(Some(true))]);
        } else {
            item.button.remove_css_class("on");
            item.button
                .update_state(&[gtk::accessible::State::Selected(Some(false))]);
        }
    }
    if name == "installed" {
        // Beim Betreten der Seite wird gegen den Index geprueft, ob es neuere
        // Releases gibt. Der Knopf unten heisst dann „Updates installieren".
        refresh_updates(ui);
    }
    if name == "settings" {
        // Die Datei kann von aussen geaendert worden sein; der Reiter zeigt
        // immer den Stand auf der Platte.
        ui.settings_view.reload();
        // Farbschema-Knoepfe angleichen: sonst zeigt der Schalter hier einen
        // aelteren Zustand als das Icon in der Kopfzeile.
        ui._theme.sync_widgets();
        refresh_settings_sources(ui);
    }
    // Brotkrumen nur dort, wo es einen Rueckweg gibt.
    ui.crumb.set_visible(name == "detail");
    ui.split.set_show_content(true);
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

/// Fuehrt einen Lesezugriff auf den Index aus und oeffnet ihn beim ersten Mal.
///
/// Fehler werden als Text zurueckgegeben, nicht geschluckt: „Index nicht
/// lesbar“ mit Grund ist eine brauchbare Auskunft, ein leerer Katalog nicht.
fn with_index<R>(ui: &Rc<Ui>, f: impl FnOnce(&Index) -> Result<R, String>) -> Result<R, String> {
    let mut slot = ui.index.borrow_mut();
    if slot.is_none() {
        *slot = Some(backend::open_index()?);
    }
    let index = slot.as_ref().expect("gerade geoeffnet");
    f(index)
}

/// Eine Suchanfrage beantworten und anzeigen.
fn search_catalogue(ui: &Rc<Ui>, filters: &Filters) -> Result<Stats, String> {
    let (items, stats) = with_index(ui, |index| {
        let items = index.search(filters, catalog_view::PAGE, 0)?;
        let stats = index.stats()?;
        Ok((items, stats))
    })?;
    ui.catalog.show_results(&items, &stats);
    update_counts(ui, &stats);
    Ok(stats)
}

/// Aktuellen Filter des Katalogs neu beantworten.
fn refresh_catalogue(ui: &Rc<Ui>) -> Result<Stats, String> {
    let filters = ui.catalog.filters();
    let stats = search_catalogue(ui, &filters)?;
    refresh_updates(ui);
    Ok(stats)
}

/// Faellige Updates in die Tabelle „Installiert“ geben.
///
/// Quelle ist `Index::updates` — der Vergleich installierte Fassung gegen die
/// Releases der Quellen. Ohne Katalog gibt es nichts zu vergleichen; dann wird
/// die Markierung geleert, statt eine alte stehen zu lassen.
fn refresh_updates(ui: &Rc<Ui>) {
    let updates = with_index(ui, |index| index.updates()).unwrap_or_default();
    ui.installed.set_updates(updates);
}

fn update_counts(ui: &Rc<Ui>, stats: &Stats) {
    ui.counts.nav_catalog.set_text(&stats.catalogue.to_string());
    ui.counts.info_addons.set_text(&stats.installed.to_string());
    ui.counts.footer.set_text(
        &crate::lang::t(&ui_lang(ui), "st.kw.counts")
            .replacen("{}", &stats.catalogue.to_string(), 1)
            .replacen("{}", &stats.installed.to_string(), 1)
            .replacen("{}", &stats.clients.to_string(), 1),
    );
    update_rail_sources(ui, stats);
}

/// Quellenzustand links: Zahl der Eintraege je Quelle, sonst der Grund.
fn update_rail_sources(ui: &Rc<Ui>, stats: &Stats) {
    let runs = ui.index_sources.borrow();
    for (source, label) in &ui.rail_sources {
        let count = stats
            .per_source
            .get(source_key(*source))
            .copied()
            .unwrap_or(0);
        label.set_text(&if count > 0 {
            count.to_string()
        } else {
            "—".to_string()
        });
        let run = runs.iter().find(|run| run.source == *source);
        let tooltip = match run {
            Some(run) if run.ok => format!("{}: {}", source.label(), run.detail),
            Some(run) => format!("{}: {}", source.label(), run.detail),
            None => format!("{}: noch nicht abgefragt", source.label()),
        };
        label.set_tooltip_text(Some(&tooltip));
    }
}

/// Index im Hintergrund aufbauen: erst Clients, dann der Katalog.
///
/// `run_sources == false` schreibt nur das Dateisystem neu - das ist der Weg
/// nach einer Installation, bei der der Katalog schon steht.
fn start_index(ui: &Rc<Ui>, data: &ScanData, run_sources: bool) {
    // Was der Index schon hat, ist sofort sichtbar: ein zweiter Start soll
    // nicht wie ein kalter Start aussehen.
    match refresh_catalogue(ui) {
        Ok(stats) if stats.catalogue == 0 => ui.catalog.set_building(),
        Ok(_) => {}
        Err(err) => ui.catalog.show_error(&err),
    }
    ui.catalog.set_building_detail(&format!(
        "{} Client-Ordner mit {} Addons werden in den Index geschrieben …",
        data.clients.len(),
        data.addon_count()
    ));

    let seq = ui.pipeline.get() + 1;
    ui.pipeline.set(seq);

    let (tx, rx) = async_channel::unbounded::<IndexMsg>();
    backend::spawn_pipeline(backend::discovery_from(data), run_sources, tx);

    let weak = Rc::downgrade(ui);
    // `spawn_future_local`: das Future bleibt im Hauptthread und darf deshalb
    // direkt Widgets anfassen - keine `Send`-Anforderung, kein Nachfragen.
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            let Some(ui) = weak.upgrade() else {
                break;
            };
            // Ein neuerer Lauf hat uebernommen: alte Meldungen verwerfen.
            if ui.pipeline.get() != seq {
                continue;
            }
            handle_index_msg(&ui, msg);
        }
    });
}

fn handle_index_msg(ui: &Rc<Ui>, msg: IndexMsg) {
    match msg {
        IndexMsg::Clients(Ok(report)) => ui.catalog.set_building_detail(&format!(
            "Index: {} Client-Ordner, {} installierte Addons. Die Quellen werden befragt …",
            report.clients, report.installed
        )),
        IndexMsg::Clients(Err(err)) => ui.catalog.show_error(&format!("Index schreiben: {err}")),
        IndexMsg::Sources(runs) => {
            ui.catalog.set_sources(&runs);
            *ui.index_sources.borrow_mut() = runs;
            set_activity(ui, crate::lang::t(&ui_lang(ui), "st.kw.index"), true);
            if let Ok(stats) = with_index(ui, |index| index.stats()) {
                update_counts(ui, &stats);
            }
        }
        IndexMsg::Catalogue(Ok(report)) => {
            ui.catalog.set_building_detail(&format!(
                "Katalog: {} Einträge im Index. Karten werden gezeichnet …",
                report.catalogue
            ));
            set_activity(ui, crate::lang::t(&ui_lang(ui), "st.kw.ready"), false);
        }
        IndexMsg::Catalogue(Err(err)) => {
            ui.catalog.show_error(&format!("Katalog schreiben: {err}"))
        }
        IndexMsg::NoClients => {
            ui.catalog.set_no_clients();
            set_activity(ui, crate::lang::t(&ui_lang(ui), "st.kw.ready"), false);
        }
        IndexMsg::Error(err) => {
            ui.catalog.show_error(&err);
            set_activity(ui, &format!("Index nicht lesbar: {err}"), false);
        }
        IndexMsg::Finished => {
            // Erst jetzt lesen: waehrend des Schreibens waere jede Abfrage ein
            // Blick auf eine halb geschriebene Transaktion.
            if let Err(err) = refresh_catalogue(ui) {
                ui.catalog.show_error(&err);
            }
            // Der Lauf ist zu Ende, gleich mit welchem Ausgang: der Spinner
            // darf nicht weiterlaufen, der Text bleibt als Stand stehen.
            ui.counts.footer_busy.set_visible(false);
        }
    }
}

/// Katalogeintrag in voller Tiefe oeffnen (`Index::addon`).
fn open_item(ui: &Rc<Ui>, id: i64) {
    match with_index(ui, |index| index.addon(id)) {
        Ok(Some(item)) => {
            let targets = detail_targets(ui);
            // Bildabruf vor dem Zeigen starten: die Antwort darf frueher
            // eintreffen als das Zeichnen, sie wird ueber die Nummer zugeordnet.
            let generation = ui.detail.begin_images();
            ui.detail.show_item(&item, &targets);
            set_crumb(ui, &item.name);
            show_page(ui, "detail");
            load_images(ui, generation, item.screenshots.clone());
        }
        Ok(None) => toast(ui, "Der Eintrag steht nicht mehr im Index."),
        Err(err) => toast_error(ui, &format!("Index: {err}")),
    }
}

/// Bilder eines Eintrags im Hintergrund holen und in die Galerie setzen.
fn load_images(ui: &Rc<Ui>, generation: u64, urls: Vec<String>) {
    if urls.is_empty() {
        ui.detail.set_images(generation, &[]);
        return;
    }
    let (tx, rx) = async_channel::unbounded::<backend::ImageMsg>();
    backend::spawn_image_fetch(urls, tx);
    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match msg {
                backend::ImageMsg::Done(images) => {
                    let pairs: Vec<(String, Vec<u8>)> = images
                        .into_iter()
                        .map(|image| (image.url, image.bytes))
                        .collect();
                    ui.detail.set_images(generation, &pairs);
                }
            }
        }
    });
}

/// Quellenliste des Einstellungsreiters fuellen: Zustand, Menge, Schluessel.
///
/// Die Zahlen kommen aus dem Index (`Stats::per_source`), die Schluesselherkunft
/// aus `KeyProvider` — dieselbe Quelle wie im Quellenbildschirm, nur als Zeile
/// statt als Karte.
fn refresh_settings_sources(ui: &Rc<Ui>) {
    let lang = ui.settings_view.settings().language;
    let (stats, keys) = (
        with_index(ui, |index| index.stats()).unwrap_or_default(),
        KeyProvider::default_provider(),
    );
    // Reihenfolge wie die Rangfolge darueber: zwei Listen mit verschiedener
    // Sortierung auf einer Seite liest niemand gern, und die Prioritaet ist
    // ohnehin das wichtigere Kriterium.
    let order = ui.settings_view.settings().priority;
    let ordered: Vec<&hexwerk_sources::Source> = order
        .iter()
        .filter(|source| backend::CATALOGUE_SOURCES.contains(source))
        .collect();
    let rows: Vec<(hexwerk_sources::Source, String, String, String)> = ordered
        .iter()
        .map(|source| {
            let count = stats
                .per_source
                .get(source_key(**source))
                .copied()
                .unwrap_or(0);
            let origin = keys.origin(**source);
            let off = origin == hexwerk_sources::KeyOrigin::Missing;
            let key_text = if off {
                crate::lang::t(&lang, "set.key.missing")
            } else if !source.needs_key() {
                crate::lang::t(&lang, "set.key.none")
            } else {
                crate::lang::t(&lang, "set.key.set")
            };
            (
                **source,
                format!("{count} {}", crate::lang::t(&lang, "set.entries.word")),
                key_text.to_string(),
                if off {
                    crate::lang::t(&lang, "set.state.off")
                } else {
                    crate::lang::t(&lang, "set.state.on")
                }
                .to_string(),
            )
        })
        .collect();
    ui.settings_view.set_sources(&rows);
}

/// Alle Client-Ordner als moegliche Installationsziele.
///
/// Welche davon zu einem Release passen, entscheidet die Detailseite - sie
/// kennt das gewaehlte Release und filtert danach. Hier wird absichtlich
/// nichts vorsortiert und nichts „zur Not" angeboten: eine Liste, die bei
/// fehlender Eignung trotzdem alle Ziele zeigt, ist genau der Weg, auf dem ein
/// Classic-Era-Addon in einem Forever-Client landet.
fn detail_targets(ui: &Rc<Ui>) -> Vec<DetailTarget> {
    let Some(data) = ui.data.borrow().clone() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (client_index, client) in data.clients.iter().enumerate() {
        for (flavor_index, flavor) in client.flavors.iter().enumerate() {
            out.push(DetailTarget {
                client: client_index,
                flavor: flavor_index,
                // Kurz halten: ein `GtkDropDown` fordert die Breite seines
                // laengsten Eintrags als Mindestbreite an. „Faugus · battlenet ·
                // Classic Era 1.15.9" hat die rechte Spalte aus dem Fenster
                // geschoben, deshalb Fassung plus Clientname und die Herkunft
                // nur im Tooltip.
                label: format!("{} · {}", flavor.version_label(), client.label),
                interfaces: flavor.interface_numbers.clone(),
            });
        }
    }
    out
}

/// Doppelklick in „Installiert“: wenn der Index den Ordner einem Katalog
/// zuordnet, kommt die Seite aus `Index::addon`; sonst aus der `.toc`.
fn open_installed(ui: &Rc<Ui>, request: OpenRequest) {
    let addon_id = with_index(ui, |index| index.installed(Some(&request.client_key)))
        .ok()
        .and_then(|rows| {
            rows.into_iter()
                .find(|row| row.folder == request.folder)
                .and_then(|row| row.addon_id)
        });
    match addon_id {
        Some(id) => open_item(ui, id),
        None => {
            ui.detail.show_local(&request.local);
            set_crumb(
                ui,
                if request.local.title.is_empty() {
                    &request.local.folder
                } else {
                    &request.local.title
                },
            );
            show_page(ui, "detail");
        }
    }
}

// ---------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------

/// Download und Entpacken laufen im Thread (backend.rs). Danach wird neu
/// gescannt - ohne Netzlauf, der Katalog steht ja schon -, damit Liste und
/// Index die neue Fassung zeigen.
fn start_install(ui: &Rc<Ui>, choice: InstallChoice) {
    let Some(data) = ui.data.borrow().clone() else {
        toast_error(ui, "Kein Scan gelaufen, der Ziel-Client ist unbekannt.");
        return;
    };
    let Some(client) = data.clients.get(choice.client) else {
        toast_error(
            ui,
            "Der gewählte Client ist nicht mehr da. Bitte neu scannen.",
        );
        return;
    };
    let Some(flavor) = client.flavors.get(choice.flavor) else {
        toast_error(
            ui,
            "Der gewählte Client-Ordner ist nicht mehr da. Bitte neu scannen.",
        );
        return;
    };

    let job = InstallJob {
        client: backend::core_client(client),
        flavor: flavor.flavor.clone(),
        flavor_interfaces: flavor.interface_numbers.clone(),
        release: choice.release,
        name: choice.name,
    };
    let (tx, rx) = async_channel::unbounded::<InstallMsg>();
    backend::spawn_install(job, tx);
    install_loop(ui, rx);
}

/// Alle faelligen Updates des gewaehlten Client-Ordners installieren.
///
/// Nur Eintraege, deren neue Fassung zur Schnittstelle des Client-Ordners
/// passt: der Filter sitzt schon in `Index::updates` (dort wird gegen die
/// Interface-Nummern des Releases verglichen) und noch einmal im Backend.
fn start_update_all(ui: &Rc<Ui>) {
    let Some((client_index, flavor_index)) = ui.installed.current_target() else {
        toast_error(ui, "Kein Client-Ordner gewählt.");
        return;
    };
    let Some(data) = ui.data.borrow().clone() else {
        toast_error(ui, "Kein Scan gelaufen, der Ziel-Client ist unbekannt.");
        return;
    };
    let Some(client) = data.clients.get(client_index) else {
        toast_error(
            ui,
            "Der gewählte Client ist nicht mehr da. Bitte neu scannen.",
        );
        return;
    };
    let Some(flavor) = client.flavors.get(flavor_index) else {
        toast_error(ui, "Der gewählte Client-Ordner ist nicht mehr da.");
        return;
    };

    let jobs: Vec<InstallJob> = ui
        .installed
        .current_updates()
        .into_iter()
        .map(|update| InstallJob {
            client: backend::core_client(client),
            flavor: flavor.flavor.clone(),
            flavor_interfaces: flavor.interface_numbers.clone(),
            release: hexwerk_sources::Release {
                source: update.source,
                external_id: update.external_id,
                version: update.available_version,
                released_at: update.released_at,
                download_url: update.download_url,
                size_bytes: update.size_bytes,
                interface_numbers: update.interface_numbers,
            },
            name: update.addon_name,
        })
        .collect();
    if jobs.is_empty() {
        toast(ui, "Keine Updates in diesem Client-Ordner.");
        return;
    }
    let (tx, rx) = async_channel::unbounded::<InstallMsg>();
    backend::spawn_update_batch(jobs, tx);
    install_loop(ui, rx);
}

/// Rueckmeldungen eines Installationslaufs bis zum Ende lesen und anzeigen.
///
/// Gilt fuer einen einzelnen Download wie fuer eine Sammlung: der Fortschritt
/// kommt als Folge von Zwischenstaenden, das Ergebnis zuletzt.
fn install_loop(ui: &Rc<Ui>, rx: async_channel::Receiver<InstallMsg>) {
    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match msg {
                InstallMsg::Progress { phase, done, total } => {
                    show_install_progress(&ui, phase, done, total)
                }
                InstallMsg::Done {
                    name,
                    version,
                    folder,
                } => {
                    ui.progress_box.set_visible(false);
                    toast(
                        &ui,
                        &format!("{name} {version} installiert: Interface/AddOns/{folder}"),
                    );
                    // Dateisystem hat sich geaendert: Scan ohne Netzlauf, danach
                    // steht die neue Fassung in „Installiert“ und im Katalog.
                    start_scan(&ui, false);
                    show_page(&ui, "installed");
                }
                InstallMsg::BatchDone { ok, failed } => {
                    ui.progress_box.set_visible(false);
                    if failed.is_empty() {
                        toast(&ui, &format!("{ok} Update(s) installiert"));
                    } else {
                        // Die Namen nennen, nicht nur die Zahl: sonst sucht der
                        // Nutzer selbst, welches Addon liegen blieb.
                        toast_error(
                            &ui,
                            &format!(
                                "{ok} aktualisiert, {} fehlgeschlagen: {}",
                                failed.len(),
                                failed.join(" · ")
                            ),
                        );
                    }
                    start_scan(&ui, false);
                    show_page(&ui, "installed");
                }
                InstallMsg::Failed(err) => {
                    ui.progress_box.set_visible(false);
                    toast_error(&ui, &format!("Installation fehlgeschlagen: {err}"));
                }
            }
        }
    });
}

/// Fortschrittsleiste fuellen: Schritt, Groesse und Balken.
///
/// Prozent nur, wenn die Quelle eine Gesamtgroesse nennt; sonst laeuft der
/// Balken unbestimmt weiter und der Text zeigt die geladenen Bytes. Ein
/// geratener Prozentsatz waere schlimmer als keiner.
fn show_install_progress(ui: &Rc<Ui>, phase: &str, done: u64, total: Option<u64>) {
    ui.progress_box.set_visible(true);
    let text = match (total, phase) {
        (Some(total), _) if total > 0 => format!(
            "{phase} · {} von {} ({:.0} %)",
            crate::format::human_size(done),
            crate::format::human_size(total),
            (done as f64 / total as f64) * 100.0
        ),
        (_, "Entpacken") => "Entpacken · ZIP wird in den Addon-Ordner geschrieben".to_string(),
        (_, _) => format!("{phase} · {}", crate::format::human_size(done)),
    };
    ui.progress_label.set_text(&text);
    match total {
        Some(total) if total > 0 => {
            ui.progress_bar
                .set_fraction((done as f64 / total as f64).min(1.0));
            ui.progress_bar.set_show_text(false);
        }
        _ => ui.progress_bar.pulse(),
    }
}

/// Zustand in der Fusszeile: was gerade laeuft, ohne Klick sichtbar.
fn set_activity(ui: &Ui, text: &str, busy: bool) {
    ui.counts.footer_state.set_text(text);
    ui.counts.footer_state.set_tooltip_text(Some(text));
    // Der Spinner ist durch das Laufband ersetzt: eine Bewegung sagt mehr als
    // zwei, und die Katze laeuft genau so lange wie die Arbeit.
    ui.counts.footer_busy.set_visible(false);
    ui.nyan.set_running(busy);
    ui.nyan.widget().set_visible(busy);
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

/// Was der Katalog als Versionsfilter anbietet: je Interface-Nummer eine
/// Beschriftung aus Flavor und Spielversion.
///
/// Zwei Client-Ordner derselben Fassung (etwa Steam und Faugus) ergeben nur
/// einen Knopf - die Schnittstelle ist dieselbe. Sortiert wird aufsteigend,
/// damit die aelteste Fassung links steht und die Anzeige nicht springt.
fn catalogue_versions(data: &ScanData) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    for client in &data.clients {
        for flavor in &client.flavors {
            let Some(number) = flavor.interface_number() else {
                continue;
            };
            // Nach Paar entdoppeln, nicht nach Nummer: Retail und eine
            // Classic-Fassung koennen dieselbe Interface-Nummer tragen, sind
            // aber zwei getrennte Installationen.
            let label = flavor.version_label();
            if out
                .iter()
                .any(|(known, known_label)| *known == number && *known_label == label)
            {
                continue;
            }
            out.push((number, label));
        }
    }
    out.sort_by_key(|(number, _)| *number);
    out
}

/// Setzt alle Texte der Fusszeile neu.
///
/// Die Fusszeile wird an mehreren Stellen geschrieben (Scan, Katalog,
/// Installieren). Ohne diesen Durchlauf bleiben einzelne Stuecke in der alten
/// Sprache stehen, weil sie ihren Text beim Entstehen bekommen haben.
fn relabel_footer(ui: &Rc<Ui>, lang: &str) {
    match ui.data.borrow().clone() {
        Some(data) => {
            ui.counts.footer_scan.set_text(&iso_date(data.finished));
        }
        None => {
            ui.counts.footer_scan.set_text("");
        }
    }
    if !ui.counts.footer_busy.is_visible() {
        ui.counts
            .footer_state
            .set_text(crate::lang::t(lang, "st.footer.idle"));
    }
    if let Ok(stats) = with_index(ui, |index| index.stats()) {
        update_counts(ui, &stats);
    }
}

/// Sprache der Oberflaeche, wie sie in den Einstellungen steht.
fn ui_lang(ui: &Rc<Ui>) -> String {
    ui.settings_view.settings().language.clone()
}

/// Adresse und Schluessel des Katalogdienstes.
///
/// Bewusst im Programm und nicht in den Einstellungen: der Nutzer soll nichts
/// einrichten muessen. Der Schluessel oeffnet nur den Katalogabruf; wer ihn
/// missbraucht, wird beim Dienst gesperrt, ohne dass eine neue Fassung noetig
/// waere. In der Einstellungsdatei laesst sich beides weiterhin ueberschreiben
/// (`urls`/`keys` mit dem Namen `catalog`), was fuer die Entwicklung reicht.
const CATALOG_URL: &str = "https://hexwerk-api.weppelmann.eu";
const CATALOG_KEY: &str = "3rp4y4t5zyexg8rcaej84tbpp44sx2bc";

/// Holt den Katalog vom Dienst und uebernimmt ihn in den Index.
///
/// Laeuft im Hintergrund: der Abruf kann einige Megabyte sein.
fn start_catalog_sync(ui: &Rc<Ui>) {
    let settings = crate::settings_view::Settings::load();
    let base = settings
        .urls
        .iter()
        .find(|(name, _)| name == "catalog")
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| CATALOG_URL.to_string());
    let key = settings
        .keys
        .iter()
        .find(|(name, _)| name == "catalog")
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| CATALOG_KEY.to_string());
    if key.trim().is_empty() {
        return;
    }

    let lang = ui_lang(ui);
    set_activity(ui, &crate::lang::t(&lang, "st.kw.catalog"), true);

    let (tx, rx) = async_channel::bounded::<Result<(usize, bool), String>>(1);
    std::thread::spawn(move || {
        let result = sync_catalog_blocking(&base, &key);
        let _ = tx.send_blocking(result);
    });

    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        let Ok(result) = rx.recv().await else {
            return;
        };
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let lang = ui_lang(&ui);
        match result {
            Ok((entries, changed)) => {
                if let Err(err) = refresh_catalogue(&ui) {
                    eprintln!("Katalog nach dem Abruf nicht lesbar: {err}");
                }
                set_activity(
                    &ui,
                    &crate::lang::t(&lang, "st.ready").replacen("{}", &entries.to_string(), 1),
                    false,
                );
                // Nur melden, wenn wirklich etwas passiert ist: bei jedem Start
                // eine Meldung „nichts Neues“ waere Laerm.
                if changed {
                    let message = crate::lang::t(&lang, "ct.sync.done")
                        .replacen("{}", &entries.to_string(), 1);
                    toast(&ui, &message);
                }
            }
            Err(err) => {
                set_activity(&ui, &crate::lang::t(&lang, "st.kw.ready"), false);
                let message =
                    crate::lang::t(&lang, "ct.sync.fail").replacen("{}", &err, 1);
                toast(&ui, &message);
            }
        }
    });
}

/// Der Abruf selbst, ohne Oberflaeche: eigener Index, eigener Thread.
fn sync_catalog_blocking(base: &str, key: &str) -> Result<(usize, bool), String> {
    let path = Index::default_path();
    let mut index = Index::open(&path)?;
    let cache = path
        .parent()
        .map(|dir| dir.join("catalog"))
        .unwrap_or_else(|| std::path::PathBuf::from("catalog"));
    let remote = hexwerk_index::RemoteCatalog::new(base, key);
    match remote.sync(&mut index, &cache, false)? {
        hexwerk_index::SyncOutcome::UpToDate { .. } => {
            let entries = index.stats().map(|stats| stats.catalogue).unwrap_or(0);
            Ok((entries, false))
        }
        hexwerk_index::SyncOutcome::Imported { entries, .. } => {
            Ok((entries.max(0) as usize, true))
        }
    }
}

fn start_scan(ui: &Rc<Ui>, run_sources: bool) {
    let lang = ui_lang(ui);
    ui.clients_view
        .set_scan_summary(crate::lang::t(&lang, "st.scan.folders"), true);
    set_activity(
        ui,
        if run_sources {
            crate::lang::t(&lang, "st.kw.scan")
        } else {
            crate::lang::t(&lang, "st.kw.scan")
        },
        true,
    );
    let (tx, rx) = async_channel::unbounded::<ScanMsg>();
    crate::state::spawn_scan(tx);

    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                ScanMsg::Done(data) => {
                    if let Some(ui) = weak.upgrade() {
                        apply_scan(&ui, data, run_sources);
                    }
                }
            }
        }
    });
}

fn apply_scan(ui: &Rc<Ui>, data: ScanData, run_sources: bool) {
    let shared = Rc::new(data);
    ui.counts
        .nav_clients
        .set_text(&shared.clients.len().to_string());
    ui.counts
        .nav_installed
        .set_text(&shared.addon_count().to_string());
    ui.counts
        .info_addons
        .set_text(&shared.addon_count().to_string());
    let lang = ui_lang(ui);
    ui.counts.footer.set_text(
        &crate::lang::t(&lang, "st.kw.counts")
            .replacen("{}", &ui.counts.nav_catalog.text().to_string(), 1)
            .replacen("{}", &shared.addon_count().to_string(), 1)
            .replacen("{}", &shared.clients.len().to_string(), 1),
    );
    ui.counts.footer_scan.set_text(&iso_date(shared.finished));

    ui.clients_view.set_scan_summary(
        &crate::lang::t(&lang, "st.intros")
            .replacen("{}", &shared.notes.len().to_string(), 1)
            .replacen("{}", &shared.clients.len().to_string(), 1)
            .replacen("{}", &shared.addon_count().to_string(), 1),
        false,
    );
    ui.clients_view.set_data(&shared);
    ui.installed.set_data(shared.clone());
    // Versionsfilter im Katalog: was auf diesem Rechner installiert ist, wird
    // zur Auswahl. Das gehoert an den Scan und nicht an den Index, weil die
    // Spielversion aus `.build.info` kommt und nicht aus der Datenbank.
    ui.catalog.set_versions(&catalogue_versions(&shared));
    set_activity(ui, crate::lang::t(&lang, "st.kw.index"), true);
    *ui.data.borrow_mut() = Some(shared.clone());

    // Der Index haengt am Scan: ohne Client-Ordner gaebe es weder Zuordnung
    // noch Installationsziel.
    start_index(ui, &shared, run_sources);
}

// ---------------------------------------------------------------------------
// Rueckmeldungen
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Deinstallieren und Leeren
// ---------------------------------------------------------------------------

/// Ziel-Client der Tabelle: Addon-Ordner und Beschriftung.
fn current_addons_dir(ui: &Ui) -> Option<(std::path::PathBuf, String)> {
    let (ci, fi) = ui.installed.current_target()?;
    let data = ui.data.borrow().clone()?;
    let client = data.clients.get(ci)?;
    let flavor = client.flavors.get(fi)?;
    Some((
        flavor.addons_dir.clone(),
        format!("{} · {}", client.label, flavor.dir_name),
    ))
}

fn main_window(ui: &Ui) -> Option<gtk::Window> {
    ui.installed
        .widget()
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok())
}

/// Fragt nach und verschiebt die markierten Ordner in den Papierkorb.
///
/// `adw::AlertDialog` statt `MessageDialog`: letzteres ist seit libadwaita 1.6
/// abgekuendigt, und wir verlangen 1.6 (AdwSpinner).
fn confirm_remove(ui: &Rc<Ui>, folders: Vec<String>) {
    let Some((addons_dir, client_label)) = current_addons_dir(ui) else {
        toast_error(ui, "Kein Ziel-Client gewählt.");
        return;
    };
    let Some(window) = main_window(ui) else {
        return;
    };

    let mut list = folders.clone();
    list.sort();
    let shown = list.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
    let more = if list.len() > 6 {
        format!(" und {} weitere", list.len() - 6)
    } else {
        String::new()
    };

    let dialog = adw::AlertDialog::new(
        Some(&format!("{} Addon(s) entfernen?", folders.len())),
        Some(&format!(
            "{shown}{more}\n\nZiel: {client_label}\nDie Ordner wandern in den Papierkorb, gelöscht wird nichts."
        )),
    );
    dialog.add_response("cancel", "Abbrechen");
    dialog.add_response("remove", "In den Papierkorb");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let weak = Rc::downgrade(ui);
    dialog.connect_response(None, move |_dialog, response| {
        if response != "remove" {
            return;
        }
        let Some(ui) = weak.upgrade() else { return };
        let paths = hexwerk_install::Paths::system();
        let mut entries = Vec::new();
        for folder in &folders {
            match hexwerk_install::trash_addon(&addons_dir, folder, &paths) {
                Ok(entry) => entries.push(entry),
                Err(err) => {
                    toast_error(&ui, &format!("{folder}: {err}"));
                    return;
                }
            }
        }
        let count = entries.len();
        show_undo_toast(&ui, &format!("{count} Addon(s) im Papierkorb"), entries);
        start_scan(&ui, false);
    });
    dialog.present(Some(&window));
}

/// Fragt nach und leert den ganzen Addon-Ordner des Ziel-Clients.
///
/// Die Bestätigung verlangt den Ordnernamen im Klartext. Bei mehreren Gigabyte
/// Addons ist ein Klick zu billig für ein Versehen.
fn confirm_clear(ui: &Rc<Ui>) {
    let Some((addons_dir, client_label)) = current_addons_dir(ui) else {
        toast_error(ui, "Kein Ziel-Client gewählt.");
        return;
    };
    let Some(window) = main_window(ui) else {
        return;
    };

    // Wie viele Addons betroffen sind, gehoert in die Rueckfrage: eine Zahl
    // sagt mehr als das Wort "alle".
    let count = hexwerk::scan_addons(&addons_dir).len();

    let dialog = adw::AlertDialog::new(
        Some("Addon-Ordner leeren?"),
        Some(&format!(
            "{count} Addons in {client_label} wandern in den Papierkorb und lassen sich danach zurückholen. Der Ordner WTF bleibt unangetastet."
        )),
    );

    dialog.add_response("cancel", "Abbrechen");
    dialog.add_response("clear", "Leeren");
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let weak = Rc::downgrade(ui);
    dialog.connect_response(None, move |_dialog, response| {
        if response != "clear" {
            return;
        }
        let Some(ui) = weak.upgrade() else { return };
        let paths = hexwerk_install::Paths::system();
        match hexwerk_install::clear_addons(&addons_dir, &paths, &[]) {
            Ok(outcome) => {
                let text = format!(
                    "{} Addons verschoben ({})",
                    outcome.folders,
                    crate::format::human_size(outcome.bytes)
                );
                show_undo_toast(&ui, &text, outcome.trashed);
                start_scan(&ui, false);
            }
            Err(err) => toast_error(&ui, &format!("Leeren: {err}")),
        }
    });
    dialog.present(Some(&window));
}

/// Toast mit Rueckgaengig-Aktion. Die Eintraege werden mitgereicht, damit die
/// Aktion auch dann funktioniert, wenn der Papierkorb inzwischen weitere
/// Eintraege hat.
fn show_undo_toast(ui: &Rc<Ui>, text: &str, entries: Vec<hexwerk_install::TrashEntry>) {
    if entries.is_empty() {
        toast(ui, text);
        return;
    }
    let meldung = adw::Toast::new(text);
    meldung.set_button_label(Some("Rückgängig"));
    meldung.set_timeout(8);
    dismiss_later(&meldung, 8);
    let weak = Rc::downgrade(ui);
    meldung.connect_button_clicked(move |_| {
        let Some(ui) = weak.upgrade() else { return };
        match hexwerk_install::clear::restore_all(&entries) {
            Ok(_) => {
                toast(&ui, "Zurückgeholt");
                start_scan(&ui, false);
            }
            Err(err) => toast_error(&ui, &format!("Rückgängig: {err}")),
        }
    });
    ui.toast.add_toast(meldung);
}

// ---------------------------------------------------------------------------
// Drag and drop: ZIP oder Adresse
// ---------------------------------------------------------------------------

/// Nimmt Dateien und Adressen an, die aufs Fenster gezogen werden.
///
/// Zwei Ziele, weil GTK sie getrennt liefert: eine Datei kommt als
/// `FileList`, eine aus dem Browser gezogene Adresse als Text. Beides endet in
/// derselben Pruefung - ohne `.toc` im Archiv wird nichts installiert.
fn attach_drop_targets(ui: &Rc<Ui>, window: &adw::ApplicationWindow) {
    let weak = Rc::downgrade(ui);
    let files = gtk::DropTarget::new(
        gtk::gdk::FileList::static_type(),
        gtk::gdk::DragAction::COPY,
    );
    files.connect_drop(move |_, value, _, _| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        let Ok(list) = value.get::<gtk::gdk::FileList>() else {
            return false;
        };
        let mut taken = false;
        for file in list.files() {
            let Some(path) = file.path() else { continue };
            let is_zip = path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("zip"))
                .unwrap_or(false);
            if !is_zip {
                toast_error(&ui, crate::lang::tn("drop.only.zip"));
                continue;
            }
            match std::fs::read(&path) {
                Ok(data) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    install_dropped(&ui, data, name);
                    taken = true;
                }
                Err(err) => toast_error(&ui, &format!("{}: {err}", path.display())),
            }
        }
        taken
    });
    window.add_controller(files);

    let weak = Rc::downgrade(ui);
    let text = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::COPY);
    text.connect_drop(move |_, value, _, _| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        let Ok(text) = value.get::<String>() else {
            return false;
        };
        let url = text.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return false;
        }
        download_and_install(&ui, url);
        true
    });
    window.add_controller(text);
}

/// Interface-Nummern aus dem `.toc` im ZIP.
///
/// Ohne `.toc` ist es kein Addon: dann bleibt die Liste leer und die
/// Installation wird abgelehnt, statt fremde Archive ins Spiel zu schreiben.
fn toc_interfaces(data: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    let Ok(mut archive) = ::zip::ZipArchive::new(std::io::Cursor::new(data)) else {
        return out;
    };
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        let Ok(name) = entry.name().map(|value| value.to_string()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".toc") {
            continue;
        }
        // Nur `.toc` in der ersten oder zweiten Ebene: ein `.toc` tief im
        // Archiv gehoert zu einer Bibliothek, nicht zum Addon selbst.
        if name.matches('/').count() > 1 {
            continue;
        }
        let mut text = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut text).is_err() {
            continue;
        }
        let toc = hexwerk::toc::parse_str(&text);
        for number in toc.interface_numbers {
            if !out.contains(&number) {
                out.push(number);
            }
        }
    }
    out
}

/// Sucht den Client-Ordner, dessen Fassung zu den Interface-Nummern passt.
fn pick_flavor(data: &crate::state::ScanData, interfaces: &[u32]) -> Option<(usize, usize)> {
    for (client_index, client) in data.clients.iter().enumerate() {
        for (flavor_index, flavor) in client.flavors.iter().enumerate() {
            if flavor
                .interface_numbers
                .iter()
                .any(|number| interfaces.contains(number))
            {
                return Some((client_index, flavor_index));
            }
        }
    }
    None
}

/// Prueft ein ZIP und entpackt es in den passenden Client.
fn install_dropped(ui: &Rc<Ui>, data: Vec<u8>, label: String) {
    let interfaces = toc_interfaces(&data);
    if interfaces.is_empty() {
        toast_error(ui, crate::lang::tn("drop.no.toc"));
        return;
    }
    let Some(data_scan) = ui.data.borrow().clone() else {
        toast_error(ui, crate::lang::tn("drop.no.scan"));
        return;
    };
    let Some((client_index, flavor_index)) = pick_flavor(&data_scan, &interfaces) else {
        toast_error(ui, crate::lang::tn("drop.no.target"));
        return;
    };
    let Some(client_data) = data_scan.clients.get(client_index) else {
        toast_error(ui, crate::lang::tn("drop.no.target"));
        return;
    };
    let client = backend::core_client(client_data);
    let Some(flavor) = data_scan
        .clients
        .get(client_index)
        .and_then(|c| c.flavors.get(flavor_index))
    else {
        toast_error(ui, crate::lang::tn("drop.no.target"));
        return;
    };
    let target_flavor = flavor.flavor.clone();
    let release = hexwerk_sources::Release {
        source: hexwerk_sources::Source::Local,
        external_id: label.clone(),
        version: String::new(),
        released_at: None,
        download_url: String::new(),
        size_bytes: Some(data.len() as u64),
        interface_numbers: interfaces.clone(),
    };
    let request = hexwerk_install::InstallRequest {
        client,
        flavor: target_flavor,
        release,
        folder: None,
    };
    match hexwerk_install::apply(&request, &data) {
        Ok(installed) => {
            let name = installed
                .folder
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(label);
            toast(ui, &format!("{name} {}", crate::lang::tn("drop.done")));
            start_scan(ui, false);
        }
        Err(err) => toast_error(ui, &format!("{err}")),
    }
}

/// Laedt eine Adresse im Hintergrund und installiert sie wie eine Datei.
fn download_and_install(ui: &Rc<Ui>, url: String) {
    use hexwerk_sources::http::Http;
    toast(ui, crate::lang::tn("drop.downloading"));
    let (tx, rx) = async_channel::bounded::<Result<Vec<u8>, String>>(1);
    let url_for_thread = url.clone();
    std::thread::spawn(move || {
        let http = hexwerk_sources::http::UreqHttp::new();
        let mut bytes = Vec::new();
        let result = match http.get(&url_for_thread) {
            Ok(response) => {
                bytes.extend_from_slice(&response.body);
                Ok(bytes)
            }
            Err(err) => Err(err.to_string()),
        };
        let _ = tx.send_blocking(result);
    });
    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(bytes) => install_dropped(&ui, bytes, url),
                Err(err) => toast_error(&ui, &format!("{err}")),
            }
        }
    });
}

fn toast(ui: &Rc<Ui>, text: &str) {
    show_toast(ui, text, 4);
}

thread_local! {
    /// Zaehlt die Hinweise: nur der jeweils letzte darf sich selbst schliessen,
    /// sonst raeumt ein alter Zeitgeber einen neueren Hinweis weg.
    static TOAST_SEQ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Zeigt einen Hinweis und raeumt ihn selbst wieder weg.
///
/// Der Zeitgeber von `AdwToast` allein reicht nicht: haengt die Oberflaeche
/// kurz (Kataloglauf, Netzzugriff), steht die Meldung weiter im Bild. Der
/// eigene Zeitgeber haelt eine starke Referenz und schliesst sie selbst; das
/// Kreuz bleibt als schnellerer Weg.
fn show_toast(ui: &Rc<Ui>, text: &str, seconds: u32) {
    let toast = adw::Toast::new(text);
    toast.set_timeout(seconds);
    ui.toast.add_toast(toast.clone());
    dismiss_later(&toast, seconds);
}

/// Schliesst einen Hinweis nach Ablauf, auch wenn ein Knopf daran haengt.
fn dismiss_later(toast: &adw::Toast, seconds: u32) {
    let handle = toast.clone();
    let mine = TOAST_SEQ.with(|seq| {
        seq.set(seq.get() + 1);
        seq.get()
    });
    glib::timeout_add_seconds_local(seconds, move || {
        if TOAST_SEQ.with(|seq| seq.get() == mine) {
            handle.dismiss();
        }
        glib::ControlFlow::Break
    });
}

/// Fehler bleiben laenger stehen und stehen oben: sie sind der Grund, warum
/// etwas nicht passiert ist.
fn toast_error(ui: &Rc<Ui>, text: &str) {
    let toast = adw::Toast::new(text);
    toast.set_timeout(9);
    toast.set_priority(adw::ToastPriority::High);
    ui.toast.add_toast(toast.clone());
    dismiss_later(&toast, 9);
}

// ---------------------------------------------------------------------------
// Leiste
// ---------------------------------------------------------------------------

/// Leiste: Ansichten oben, Bestand und Quellen darunter. Die Quellen zeigen
/// ihren Zustand, damit ein leerer Katalog nicht wie ein Fehler aussieht.
fn build_rail() -> (
    gtk::Box,
    Vec<NavItem>,
    Counts,
    Vec<(Source, gtk::Label)>,
    Vec<gtk::Widget>,
) {
    let rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rail.set_margin_bottom(14);

    // Marke oben in der Leiste. Kein eigener HeaderBar mehr: zwei Kopfleisten
    // nebeneinander liessen die Trennlinie der SplitView mitten durch laufen.
    let mut extras: Vec<gtk::Widget> = Vec::new();
    let brand_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    brand_row.add_css_class("hw-brand");
    brand_row.set_margin_top(14);
    brand_row.set_margin_bottom(12);
    brand_row.set_margin_start(18);
    // Groesser als die Beschriftung darunter: das Logo ist der Anker der
    // Leiste, kein Beiwerk.
    brand_row.append(&brand_icon(58));
    let wordmark = gtk::Label::new(Some("HEXWERK"));
    wordmark.add_css_class("hw-wordmark");
    brand_row.append(&wordmark);
    rail.append(&brand_row);
    extras.push(wordmark.clone().upcast());

    let view_title = section("ANSICHT");
    rail.append(&view_title);
    extras.push(view_title.clone().upcast());

    let nav = gtk::Box::new(gtk::Orientation::Vertical, 0);
    nav.add_css_class("hw-nav");
    let mut nav_items = Vec::new();

    let nav_clients = gtk::Label::new(Some("—"));
    nav_clients.add_css_class("hw-nav-count");
    nav_clients.set_xalign(1.0);

    let nav_installed = gtk::Label::new(Some("—"));
    nav_installed.add_css_class("hw-nav-count");
    nav_installed.set_xalign(1.0);

    let nav_catalog = gtk::Label::new(Some("—"));
    nav_catalog.add_css_class("hw-nav-count");
    nav_catalog.set_xalign(1.0);

    for (name, title, _) in PAGES {
        // Die Detailseite haengt an einer Auswahl, nicht an der Leiste: sie
        // wird aus Katalog oder Installiert geoeffnet und ueber die Brotkrumen
        // wieder verlassen.
        if name == "detail" {
            continue;
        }
        let label = gtk::Label::new(Some(title));
        label.add_css_class("hw-nav-label");
        label.set_xalign(0.0);
        label.set_hexpand(true);

        // Symbol je Seite: in der kompakten Leiste ist es das Einzige, was
        // bleibt — deshalb bekommt es eine feste Breite und steht vorn.
        let icon = gtk::Image::from_icon_name(nav_icon(name));
        icon.add_css_class("hw-nav-item-icon");
        icon.set_pixel_size(18);
        // Adwaita zeichnet die Lupe nicht mittig in ihr Feld: gemessen am
        // Bildschirmabzug sitzt sie 2 px zu weit links, waehrend Monitor,
        // Ordner und Zahnrad auf derselben Achse stehen. Der Versatz wird
        // ausgeglichen, damit alle vier Symbole eine Flucht haben.
        if let Some(nudge) = nav_icon_nudge(name) {
            icon.set_margin_start(nudge);
        }

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&icon);
        line.append(&label);

        // Zaehler nur dort, wo es etwas zu zaehlen gibt.
        match name {
            "clients" => line.append(&nav_clients),
            "installed" => line.append(&nav_installed),
            "catalog" => line.append(&nav_catalog),
            _ => {}
        }

        let button = gtk::Button::new();
        button.add_css_class("hw-nav-item");
        button.set_child(Some(&line));
        // Ohne gesetzten Namen liest der Screenreader nur „Knopf“ vor: der
        // sichtbare Text steht in Kindern, nicht im Knopf selbst.
        button.update_property(&[gtk::accessible::Property::Label(title)]);
        crate::theme::pointer(&button);
        nav.append(&button);
        nav_items.push(NavItem {
            name,
            button,
            label,
            icon,
        });
    }
    rail.append(&nav);

    let bestand_title = section("BESTAND");
    rail.append(&bestand_title);
    extras.push(bestand_title.clone().upcast());

    let info_addons = gtk::Label::new(Some("—"));
    info_addons.add_css_class("hw-info-value");
    info_addons.set_xalign(1.0);
    let addons_line = info_line("Addons", Some(&info_addons));
    rail.append(&addons_line);
    extras.push(addons_line.upcast());
    let origins_line = info_line("Herkünfte", None);
    rail.append(&origins_line);
    extras.push(origins_line.upcast());

    let sources_title = section("QUELLEN");
    rail.append(&sources_title);
    extras.push(sources_title.clone().upcast());
    let mut rail_sources = Vec::new();
    for source in backend::CATALOGUE_SOURCES {
        let (row, value) = source_line(source.label());
        value.set_tooltip_text(Some(&format!("{}: noch nicht abgefragt", source.label())));
        rail.append(&row);
        extras.push(row.clone().upcast());
        rail_sources.push((source, value));
    }

    let counts = Counts {
        nav_clients,
        nav_installed,
        nav_catalog,
        info_addons,
        // Die echten Fusszeilen-Widgets entstehen weiter oben im Fensterbau und
        // werden dort eingesetzt; hier stehen nur Platzhalter.
        footer: gtk::Label::new(None),
        footer_scan: gtk::Label::new(None),
        footer_state: gtk::Label::new(None),
        footer_busy: adw::Spinner::new(),
        brand_row: brand_row.clone(),
    };
    // `extras`: Ueberschriften und Zahlen, die in der kompakten Leiste
    // verschwinden. Die Navigation selbst bleibt immer sichtbar.
    (rail, nav_items, counts, rail_sources, extras)
}

fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(&text.to_uppercase()));
    label.add_css_class("hw-section");
    label.set_xalign(0.0);
    label
}

fn info_line(name: &str, value: Option<&gtk::Label>) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("hw-info-row");
    let key = gtk::Label::new(Some(name));
    key.add_css_class("hw-dim");
    key.set_xalign(0.0);
    key.set_hexpand(true);
    line.append(&key);
    match value {
        Some(label) => line.append(label),
        None => {
            let text = gtk::Label::new(Some(&hexwerk::Origin::all().len().to_string()));
            text.add_css_class("hw-info-value");
            line.append(&text);
        }
    }
    line
}

/// Quellenzustand links. Der Zustand ist keine Dekoration: er sagt, warum der
/// Katalog leer ist. Gibt das Wert-Label zurueck, damit es sich aktualisieren
/// laesst, sobald der Index Zahlen kennt.
fn source_line(name: &str) -> (gtk::Box, gtk::Label) {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("hw-info-row");
    let key = gtk::Label::new(Some(name));
    key.set_xalign(0.0);
    key.set_hexpand(true);
    let value = gtk::Label::new(Some("—"));
    value.add_css_class("hw-info-value");
    value.set_xalign(1.0);
    line.append(&key);
    line.append(&value);
    (line, value)
}

/// Das App-Logo als Bild. Die PNG-Fassung ist eingebettet; SVG kann GTK nicht
/// direkt als Textur laden, und eine Datei zur Laufzeit wuerde im Flatpak einen
/// weiteren Pfad voraussetzen, der stimmen muss.
/// Das Zeichen der Leiste.
///
/// Bewusst das **symbolische** Bild statt der farbigen PNG-Datei: ein
/// symbolisches Symbol faerbt GTK ueber die CSS-Eigenschaft `color`, also
/// uebernimmt es `--hw-signal` der aktiven Palette. Damit steht das Zeichen
/// im dunklen Thema in Signalgruen (#39DD93, 10,76:1 auf #0A111C) und im
/// hellen in dunklem Gruen (#0B7A52, 4,99:1 auf #F5F7FB). Eine farbige Datei
/// kann das nicht: sie brachte beide Themen auf denselben Ton und kam im
/// hellen Modus auf 1,64:1, war also praktisch unsichtbar.
fn brand_icon(px: i32) -> gtk::Image {
    let image = gtk::Image::from_icon_name("de.weppelmann.Hexwerk-symbolic");
    image.add_css_class("hw-logo");
    image.set_pixel_size(px);
    image
}


#[cfg(test)]
mod drop_tests {
    use super::*;
    use std::io::Write;

    /// Baut ein ZIP mit einer `.toc` im Wurzelordner.
    fn zip_with_toc(interface: &str) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = ::zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = ::zip::write::SimpleFileOptions::default();
            writer
                .start_file("MeinAddon/MeinAddon.toc", options)
                .unwrap();
            writer
                .write_all(format!("## Interface: {interface}\n## Title: MeinAddon\n").as_bytes())
                .unwrap();
            writer.finish().unwrap();
        }
        buffer
    }

    #[test]
    fn toc_im_zip_liefert_die_interface_nummer() {
        let data = zip_with_toc("11509");
        assert_eq!(toc_interfaces(&data), vec![11509]);
    }

    #[test]
    fn zip_ohne_toc_ergibt_keine_nummer() {
        let mut buffer = Vec::new();
        {
            let mut writer = ::zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = ::zip::write::SimpleFileOptions::default();
            writer.start_file("MeinAddon/readme.txt", options).unwrap();
            writer.write_all(b"kein toc").unwrap();
            writer.finish().unwrap();
        }
        assert!(toc_interfaces(&buffer).is_empty());
    }
}
