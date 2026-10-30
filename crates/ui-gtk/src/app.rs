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
use hexwerk_index::{source_key, CatalogueItem, Filters, Index, Stats};
use hexwerk_sources::Source;

use crate::backend::{self, IndexMsg, InstallJob, InstallMsg, SourceRun};
use crate::catalog_view::{self, CatalogView};
use crate::clients_view::ClientsView;
use crate::detail_view::{DetailTarget, DetailView, InstallChoice};
use crate::format::iso_date;
use crate::installed_view::{InstalledView, OpenRequest};
use crate::scan_view::ScanView;
use crate::sources_view::{SourceInfo, SourcesView};
use crate::state::{ScanData, ScanMsg};
use crate::theme::Theme;

pub const APP_ID: &str = "de.weppelmann.Hexwerk";

/// Reihenfolge der Bildschirme: Name im Stack, Titel, Untertitel fuer den
/// Kopfbereich. Eine Tabelle, damit Leiste und Kopf nicht auseinanderlaufen.
const PAGES: [(&str, &str, &str); 6] = [
    ("scan", "Scan", "Herkünfte prüfen"),
    ("clients", "Clients", "Gefundene WoW-Installationen"),
    ("installed", "Installiert", "Addons je Client-Ordner"),
    ("catalog", "Katalog", "Suche im lokalen Index"),
    ("sources", "Quellen", "Zustand, Verbindungstest, Schlüssel"),
    ("detail", "Addon-Detail", "Quellen, Versionen, Ziel"),
];

/// Zaehler in der Leiste und in der Fusszeile. Eigene Labels, weil ein Widget
/// nur an einer Stelle im Baum haengen kann.
struct Counts {
    nav_clients: gtk::Label,
    nav_installed: gtk::Label,
    nav_catalog: gtk::Label,
    info_addons: gtk::Label,
    footer: gtk::Label,
    footer_scan: gtk::Label,
}

/// Eine Ansicht in der Leiste. Ein Knopf statt einer Listenzeile: so ist die
/// Navigation mit Tastatur und Screenreader bedienbar (Rolle „push button“),
/// und der Klickpfad ist derselbe wie bei jedem anderen Knopf.
struct NavItem {
    name: &'static str,
    button: gtk::Button,
}

struct Ui {
    split: adw::NavigationSplitView,
    stack: gtk::Stack,
    nav: Vec<NavItem>,
    title: adw::WindowTitle,
    back: gtk::Button,
    toast: adw::ToastOverlay,
    scan: ScanView,
    clients_view: ClientsView,
    installed: Rc<InstalledView>,
    catalog: Rc<CatalogView>,
    detail: Rc<DetailView>,
    counts: Counts,
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
    /// Brotkrumen in der Kopfleiste: Weg zurueck in den Katalog.
    crumb: gtk::Box,
    crumb_name: gtk::Label,
    /// Bildschirm "Quellen": Zustand, Test, Schluessel.
    sources: Rc<SourcesView>,
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
    let window = adw::ApplicationWindow::builder()
        .application(app)
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
    let sources = SourcesView::new();

    stack.add_named(scan.widget(), Some("scan"));
    stack.add_named(clients_view.widget(), Some("clients"));
    stack.add_named(installed.widget(), Some("installed"));
    stack.add_named(catalog.widget(), Some("catalog"));
    stack.add_named(sources.widget(), Some("sources"));
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

    let switcher = theme.switcher();
    header.pack_end(&switcher);

    // ---- Fusszeile ----
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    footer.add_css_class("hw-footer");

    let brand = gtk::Label::new(Some("HEXWERK"));
    brand.add_css_class("hw-footer-item");

    let source_note = gtk::Label::new(Some(
        "Clients aus dem Präfix-Scan, Katalog aus dem lokalen Index",
    ));
    source_note.add_css_class("hw-footer-item");
    source_note.set_hexpand(true);
    source_note.set_xalign(0.0);

    let footer_stats = gtk::Label::new(Some("kein Scan gelaufen"));
    footer_stats.add_css_class("hw-footer-item");

    let footer_scan = gtk::Label::new(Some("Stand —"));
    footer_scan.add_css_class("hw-footer-item");

    footer.append(&brand);
    footer.append(&source_note);
    footer.append(&footer_stats);
    footer.append(&footer_scan);

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&header);
    content_view.add_bottom_bar(&footer);
    content_view.set_content(Some(&stack));

    // ---- Leiste links ----
    let (rail, nav, counts, rail_sources) = build_rail();

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
            ..counts
        },
        rail_sources,
        data: RefCell::new(None),
        index: RefCell::new(None),
        index_sources: RefCell::new(Vec::new()),
        pipeline: Cell::new(0),
        _theme: theme,
        crumb,
        crumb_name,
        sources,
        source_tests: RefCell::new(std::collections::BTreeMap::new()),
    });

    wire(&ui);
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
            start_scan(&ui, true);
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

    // Der Scan startet, sobald das Fenster steht - der Startbildschirm ist
    // sonst ein leerer Bildschirm mit einem Knopf.
    let weak = Rc::downgrade(ui);
    glib::idle_add_local_once(move || {
        if let Some(ui) = weak.upgrade() {
            show_page(&ui, "scan");
            start_scan(&ui, true);
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
fn show_page(ui: &Rc<Ui>, name: &str) {
    if ui.stack.visible_child_name().as_deref() != Some(name) {
        ui.stack.set_visible_child_name(name);
    }
    if let Some((_, title, subtitle)) = PAGES.iter().find(|(page, _, _)| *page == name) {
        ui.title.set_title(title);
        ui.title.set_subtitle(subtitle);
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
    if name == "sources" {
        ui.sources.show_overview();
        refresh_sources(ui);
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
    search_catalogue(ui, &filters)
}

fn update_counts(ui: &Rc<Ui>, stats: &Stats) {
    ui.counts.nav_catalog.set_text(&stats.catalogue.to_string());
    ui.counts.info_addons.set_text(&stats.installed.to_string());
    ui.counts.footer.set_text(&format!(
        "Katalog {} · installiert {} · Client-Ordner {}",
        stats.catalogue, stats.installed, stats.clients
    ));
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
            if let Ok(stats) = with_index(ui, |index| index.stats()) {
                update_counts(ui, &stats);
            }
        }
        IndexMsg::Catalogue(Ok(report)) => ui.catalog.set_building_detail(&format!(
            "Katalog: {} Einträge im Index. Karten werden gezeichnet …",
            report.catalogue
        )),
        IndexMsg::Catalogue(Err(err)) => {
            ui.catalog.show_error(&format!("Katalog schreiben: {err}"))
        }
        IndexMsg::NoClients => ui.catalog.set_no_clients(),
        IndexMsg::Error(err) => ui.catalog.show_error(&err),
        IndexMsg::Finished => {
            // Erst jetzt lesen: waehrend des Schreibens waere jede Abfrage ein
            // Blick auf eine halb geschriebene Transaktion.
            if let Err(err) = refresh_catalogue(ui) {
                ui.catalog.show_error(&err);
            }
        }
    }
}

/// Katalogeintrag in voller Tiefe oeffnen (`Index::addon`).
fn open_item(ui: &Rc<Ui>, id: i64) {
    match with_index(ui, |index| index.addon(id)) {
        Ok(Some(item)) => {
            let targets = detail_targets(ui, &item);
            ui.detail.show_item(&item, &targets);
            set_crumb(ui, &item.name);
            show_page(ui, "detail");
        }
        Ok(None) => toast(ui, "Der Eintrag steht nicht mehr im Index."),
        Err(err) => toast_error(ui, &format!("Index: {err}")),
    }
}

/// Ziele fuer die Installation: jeder Client-Ordner, dessen Interface zum
/// Release passt. Passt keiner, werden alle angeboten - lieber eine bewusste
/// Wahl als ein leerer Knopf.
fn detail_targets(ui: &Rc<Ui>, item: &CatalogueItem) -> Vec<DetailTarget> {
    let Some(data) = ui.data.borrow().clone() else {
        return Vec::new();
    };
    let interfaces: Vec<u32> = item
        .latest
        .as_ref()
        .map(|release| release.interfaces.clone())
        .unwrap_or_default();

    let mut matching = Vec::new();
    let mut others = Vec::new();
    for (client_index, client) in data.clients.iter().enumerate() {
        for (flavor_index, flavor) in client.flavors.iter().enumerate() {
            let target = DetailTarget {
                client: client_index,
                flavor: flavor_index,
                label: format!(
                    "{} · {} · {}",
                    client.origin.label(),
                    client.label,
                    flavor.dir_name
                ),
            };
            let fits = interfaces.is_empty()
                || flavor.interface_numbers.is_empty()
                || flavor
                    .interface_numbers
                    .iter()
                    .any(|number| interfaces.contains(number));
            if fits {
                matching.push(target);
            } else {
                others.push(target);
            }
        }
    }
    if matching.is_empty() {
        others
    } else {
        matching
    }
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
        release: choice.release,
        name: choice.name,
    };
    let (tx, rx) = async_channel::unbounded::<InstallMsg>();
    backend::spawn_install(job, tx);

    let weak = Rc::downgrade(ui);
    glib::spawn_future_local(async move {
        let Ok(msg) = rx.recv().await else {
            return;
        };
        let Some(ui) = weak.upgrade() else {
            return;
        };
        match msg {
            InstallMsg::Done {
                name,
                version,
                folder,
            } => {
                toast(
                    &ui,
                    &format!("{name} {version} installiert: Interface/AddOns/{folder}"),
                );
                // Dateisystem hat sich geaendert: Scan ohne Netzlauf, danach
                // steht die neue Fassung in „Installiert“ und im Katalog.
                start_scan(&ui, false);
                show_page(&ui, "installed");
            }
            InstallMsg::Failed(err) => {
                toast_error(&ui, &format!("Installation fehlgeschlagen: {err}"))
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

fn start_scan(ui: &Rc<Ui>, run_sources: bool) {
    ui.scan.set_running(true);
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
    ui.counts.footer.set_text(&format!(
        "{} Clients · {} Addons",
        shared.clients.len(),
        shared.addon_count()
    ));
    ui.counts
        .footer_scan
        .set_text(&format!("Scan {}", iso_date(shared.finished)));

    ui.scan.show(&shared);
    ui.scan.set_running(false);
    ui.clients_view.set_data(&shared);
    ui.installed.set_data(shared.clone());
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

fn toast(ui: &Rc<Ui>, text: &str) {
    let toast = adw::Toast::new(text);
    toast.set_timeout(5);
    ui.toast.add_toast(toast);
}

/// Fehler bleiben laenger stehen und stehen oben: sie sind der Grund, warum
/// etwas nicht passiert ist.
fn toast_error(ui: &Rc<Ui>, text: &str) {
    let toast = adw::Toast::new(text);
    toast.set_timeout(9);
    toast.set_priority(adw::ToastPriority::High);
    ui.toast.add_toast(toast);
}

// ---------------------------------------------------------------------------
// Leiste
// ---------------------------------------------------------------------------

/// Leiste: Ansichten oben, Bestand und Quellen darunter. Die Quellen zeigen
/// ihren Zustand, damit ein leerer Katalog nicht wie ein Fehler aussieht.
fn build_rail() -> (gtk::Box, Vec<NavItem>, Counts, Vec<(Source, gtk::Label)>) {
    let rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rail.set_margin_bottom(14);

    // Marke oben in der Leiste. Kein eigener HeaderBar mehr: zwei Kopfleisten
    // nebeneinander liessen die Trennlinie der SplitView mitten durch laufen.
    let brand_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    brand_row.add_css_class("hw-brand");
    brand_row.set_margin_top(14);
    brand_row.set_margin_bottom(12);
    brand_row.set_margin_start(18);
    brand_row.append(&brand_icon(34));
    let wordmark = gtk::Label::new(Some("HEXWERK"));
    wordmark.add_css_class("hw-wordmark");
    brand_row.append(&wordmark);
    rail.append(&brand_row);

    rail.append(&section("ANSICHT"));

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

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
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
        nav_items.push(NavItem { name, button });
    }
    rail.append(&nav);

    rail.append(&section("BESTAND"));
    let info_addons = gtk::Label::new(Some("—"));
    info_addons.add_css_class("hw-info-value");
    info_addons.set_xalign(1.0);
    rail.append(&info_line("Addons", Some(&info_addons)));
    rail.append(&info_line("Herkünfte", None));

    rail.append(&section("QUELLEN"));
    let mut rail_sources = Vec::new();
    for source in backend::CATALOGUE_SOURCES {
        let (row, value) = source_line(source.label());
        value.set_tooltip_text(Some(&format!("{}: noch nicht abgefragt", source.label())));
        rail.append(&row);
        rail_sources.push((source, value));
    }

    let counts = Counts {
        nav_clients,
        nav_installed,
        nav_catalog,
        info_addons,
        footer: gtk::Label::new(None),
        footer_scan: gtk::Label::new(None),
    };
    (rail, nav_items, counts, rail_sources)
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
fn brand_icon(px: i32) -> gtk::Image {
    const LOGO: &[u8] =
        include_bytes!("../../../data/icons/hicolor/64x64/apps/de.weppelmann.Hexwerk.png");
    let image = gtk::Image::new();
    let bytes = glib::Bytes::from_static(LOGO);
    match gtk::gdk::Texture::from_bytes(&bytes) {
        Ok(texture) => image.set_paintable(Some(&texture)),
        // Ohne Textur bleibt ein leerer Platz; das ist besser als ein Absturz
        // beim Start, und der Fehler steht im Protokoll.
        Err(err) => eprintln!("hexwerk: Logo nicht lesbar: {err}"),
    }
    image.set_pixel_size(px);
    image
}
