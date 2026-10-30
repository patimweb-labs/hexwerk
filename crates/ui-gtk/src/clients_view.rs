//! Bildschirm „Clients": je gefundenem Präfix eine Karte mit den
//! Client-Ordnern (`_retail_`, `_classic_era_` …), Interface-Nummern und der
//! Zahl der Addons, die dort liegen.
//!
//! Die Zahlen sind gezaehlt, nicht geschaetzt: `FlavorData` enthaelt die
//! Addon-Liste, die beim Scan sowieso gelesen wurde.

use adw::prelude::*;
use std::rc::Rc;

use crate::format::{human_size, initial, interface_list, shorten_home};
use crate::state::{ClientData, FlavorData, ScanData};

type OpenFlavor = Box<dyn Fn(usize, usize)>;

pub struct ClientsView {
    root: gtk::ScrolledWindow,
    /// Kopfzeile; beim Sprachwechsel nachzuziehen.
    title: gtk::Label,
    sub: gtk::Label,
    data: std::cell::RefCell<Option<std::rc::Rc<crate::state::ScanData>>>,
    /// Zusammenfassung des letzten Scans: Herkuenfte, Zahlen, Knopf.
    scan_summary: gtk::Label,
    rescan: gtk::Button,
    on_rescan: std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn()>>>>,
    grid: gtk::Grid,
    empty: gtk::Box,
    /// Wird mit (Client-Index, Flavor-Index) gerufen, wenn eine Zeile
    /// angeklickt wird. In einem Rc, damit der Aufbau der Karten ihn teilen
    /// kann, ohne die Ansicht selbst zu kennen.
    on_open_flavor: std::rc::Rc<std::cell::RefCell<Option<OpenFlavor>>>,
}

impl ClientsView {
    pub fn new() -> Rc<Self> {
        let title = gtk::Label::new(Some(crate::lang::tn("cl.title")));
        title.add_css_class("hw-h1");
        title.set_xalign(0.0);

        let sub = gtk::Label::new(Some(crate::lang::tn("cl.sub")));
        sub.add_css_class("hw-sub");
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_margin_top(2);
        sub.set_margin_bottom(16);

        // Zwei Karten je Zeile, wie im Entwurf. Ein Raster statt eines FlowBox:
        // so bekommt jede Karte genau die halbe Breite, auch wenn nur eine da
        // ist - eine einzelne Karte ueber die volle Fensterbreite sieht aus wie
        // ein Versehen.
        // Zusammenfassung des Scans als **eine Zeile** unter der Liste.
        // Bewusst kein eingebetteter Bildschirm: der brachte eigene Ueberschrift,
        // eigenen Scroller und hat die Karten verschoben.
        let scan_summary = gtk::Label::new(Some(crate::lang::tn("cl.noScan")));
        scan_summary.add_css_class("hw-dim2");
        scan_summary.set_xalign(0.0);
        scan_summary.set_hexpand(true);
        scan_summary.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let rescan = gtk::Button::with_label(crate::lang::tn("cl.rescan"));
        rescan.add_css_class("hw-btn");
        rescan.set_tooltip_text(Some(crate::lang::tn("cl.rescan.hint")));
        crate::theme::pointer(&rescan);

        let scan_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        scan_row.set_margin_top(16);
        scan_row.append(&scan_summary);
        scan_row.append(&rescan);

        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(14);
        grid.set_row_spacing(14);
        grid.set_valign(gtk::Align::Start);

        let empty = empty_state();

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_margin_top(20);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(&title);
        body.append(&sub);
        body.append(&empty);
        body.append(&grid);
        body.append(&scan_row);

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&body)
            .build();
        root.add_css_class("hw-pane");

        Rc::new(Self {
            root,
            title: title.clone(),
            sub: sub.clone(),
            data: std::cell::RefCell::new(None),
            scan_summary,
            rescan,
            on_rescan: std::rc::Rc::new(std::cell::RefCell::new(None)),
            grid,
            empty,
            on_open_flavor: std::rc::Rc::new(std::cell::RefCell::new(None)),
        })
    }

    /// Setzt die Zusammenfassung des letzten Scans und schaltet den Knopf.
    pub fn set_scan_summary(&self, text: &str, running: bool) {
        self.scan_summary.set_text(text);
        self.scan_summary.set_tooltip_text(Some(text));
        self.rescan.set_sensitive(!running);
        self.rescan.set_label(if running {
            crate::lang::tn("cl.running")
        } else {
            crate::lang::tn("cl.rescan")
        });
    }

    /// Rueckruf des Knopfs „Erneut scannen".
    /// Zieht die festen Texte auf eine Sprache nach und baut die Karten neu,
    /// weil deren Texte beim Zeichnen entstehen.
    pub fn retranslate(self: &Rc<Self>) {
        self.title.set_text(crate::lang::tn("cl.title"));
        self.sub.set_text(crate::lang::tn("cl.sub"));
        self.rescan.set_label(crate::lang::tn("cl.rescan"));
        self.rescan
            .set_tooltip_text(Some(crate::lang::tn("cl.rescan.hint")));
        // Erst die Ausleihe beenden, dann neu bauen: `set_data` schreibt in
        // dasselbe Feld, und zwei Ausleihen gleichzeitig lassen das Programm
        // abbrechen (gemessen: „RefCell already borrowed" beim Start).
        let data = self.data.borrow().clone();
        if let Some(data) = data {
            self.set_data(&data);
        }
    }

    pub fn set_on_rescan<F: Fn() + 'static>(self: &Rc<Self>, f: F) {
        *self.on_rescan.borrow_mut() = Some(Box::new(f));
        let weak = Rc::downgrade(self);
        self.rescan.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            let callback = view.on_rescan.borrow_mut().take();
            if let Some(callback) = callback {
                callback();
                *view.on_rescan.borrow_mut() = Some(callback);
            }
        });
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub fn set_on_open_flavor<F: Fn(usize, usize) + 'static>(&self, f: F) {
        *self.on_open_flavor.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_data(&self, data: &std::rc::Rc<ScanData>) {
        // Fuer den Sprachwechsel: Karten werden aus denselben Daten neu gebaut.
        *self.data.borrow_mut() = Some(std::rc::Rc::clone(data));
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        self.empty.set_visible(data.clients.is_empty());
        for (index, client) in data.clients.iter().enumerate() {
            let column = (index % 2) as i32;
            let row = (index / 2) as i32;
            let card = card(client, index, &self.on_open_flavor);
            card.set_hexpand(true);
            self.grid.attach(&card, column, row, 1, 1);
        }
        // Der Hinweis auf weitere Herkuenfte gehoert ans Ende, nicht in die
        // Kartenreihe: er ist keine Karte, sondern eine Fussnote - und deshalb
        // ueber die volle Breite, nicht in eine der beiden Spalten gezwungen.
        // Eigene Zeile, ueber beide Spalten: eine halbbreite Fussnote neben der
        // letzten Karte sieht aus wie eine Karte, die niemand gefuellt hat.
        let hint_row = data.clients.len().div_ceil(2) as i32;
        self.grid.attach(&hint_card(), 0, hint_row, 2, 1);
    }
}

fn empty_state() -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 6);
    box_.add_css_class("hw-card");
    box_.set_margin_bottom(14);
    let title = gtk::Label::new(Some(crate::lang::tn("cl.none.title")));
    title.add_css_class("hw-note-title");
    title.set_xalign(0.0);
    let text = gtk::Label::new(Some(crate::lang::tn("cl.none.text")));
    text.add_css_class("hw-dim");
    text.set_xalign(0.0);
    text.set_wrap(true);
    box_.append(&title);
    box_.append(&text);
    box_.set_visible(false);
    box_
}

fn card(
    client: &ClientData,
    client_index: usize,
    on_open: &std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn(usize, usize)>>>>,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("hw-card");
    card.set_valign(gtk::Align::Start);

    let glyph = gtk::Label::new(Some(&initial(client.origin.label())));
    glyph.add_css_class("hw-glyph");
    glyph.set_valign(gtk::Align::Center);
    glyph.set_hexpand(false);

    let name = gtk::Label::new(Some(&client.label));
    name.add_css_class("hw-card-title");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let badge = gtk::Label::new(Some(&client.origin.label().to_uppercase()));
    badge.add_css_class("hw-badge");
    badge.set_valign(gtk::Align::Center);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.append(&glyph);
    head.append(&name);
    head.append(&badge);

    let path_text = shorten_home(&client.game_dir);
    let path = gtk::Label::new(Some(&path_text));
    path.add_css_class("hw-path");
    path.set_xalign(0.0);
    path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    path.set_tooltip_text(Some(&path_text));
    path.set_selectable(true);

    card.append(&head);
    card.append(&path);

    let flavors = gtk::Box::new(gtk::Orientation::Vertical, 6);
    flavors.set_margin_top(2);
    for (flavor_index, flavor) in client.flavors.iter().enumerate() {
        flavors.append(&flavor_row(flavor, client_index, flavor_index, on_open));
    }
    if client.flavors.is_empty() {
        let none = gtk::Label::new(Some(crate::lang::tn("cl.no.flavor")));
        none.add_css_class("hw-path");
        none.set_xalign(0.0);
        none.set_wrap(true);
        flavors.append(&none);
    }
    card.append(&flavors);
    card
}

fn flavor_row(
    flavor: &FlavorData,
    client_index: usize,
    flavor_index: usize,
    on_open: &std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn(usize, usize)>>>>,
) -> gtk::Widget {
    // Ein Knopf statt einer Box: klickbar, mit der Tastatur erreichbar und mit
    // Zeigefinger. Der Klick springt in die Ansicht "Installiert" und waehlt
    // genau diesen Client-Ordner aus.
    let button = gtk::Button::new();
    button.add_css_class("hw-flavor");
    button.add_css_class("hw-flavor-button");
    crate::theme::pointer(&button);
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} mit {} Addons öffnen",
        flavor.dir_name,
        flavor.addon_count()
    ))]);
    {
        let on_open = on_open.clone();
        button.connect_clicked(move |_| {
            if let Some(cb) = on_open.borrow().as_ref() {
                cb(client_index, flavor_index);
            }
        });
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_halign(gtk::Align::Fill);
    row.set_tooltip_text(Some(&shorten_home(&flavor.addons_dir)));

    let left = gtk::Box::new(gtk::Orientation::Vertical, 2);
    left.set_hexpand(true);
    let dir = gtk::Label::new(Some(&flavor.dir_name));
    dir.add_css_class("hw-flavor-name");
    dir.set_xalign(0.0);
    let detail = gtk::Label::new(Some(&interface_list(&flavor.interface_numbers)));
    detail.add_css_class("hw-path");
    detail.set_xalign(0.0);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
    left.append(&dir);
    left.append(&detail);

    let count = flavor.addon_count();
    let count_label = gtk::Label::new(Some(&if count == 1 {
        crate::lang::tn("cl.addon.one").to_string()
    } else {
        crate::lang::tn("cl.addons").replacen("{}", &count.to_string(), 1)
    }));
    count_label.add_css_class("hw-chip");
    count_label.add_css_class(if count > 0 { "sig" } else { "off" });
    count_label.set_valign(gtk::Align::Center);

    let size = gtk::Label::new(Some(&if count > 0 {
        human_size(flavor.size_bytes)
    } else {
        "—".to_string()
    }));
    size.add_css_class("hw-chip");
    size.set_valign(gtk::Align::Center);

    row.append(&left);
    row.append(&count_label);
    row.append(&size);
    button.set_child(Some(&row));
    button.upcast()
}

/// Fussnote zu den Herkuenften, die keine eigene Karte bekommen haben, weil
/// sie nichts gefunden haben: der Nutzer soll sehen, dass sie mitgesucht wurden.
fn hint_card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("hw-card-dashed");
    card.set_valign(gtk::Align::Start);

    let title = gtk::Label::new(Some(crate::lang::tn("cl.more.title")));
    title.add_css_class("hw-note-title");
    title.set_xalign(0.0);

    let text = gtk::Label::new(Some(crate::lang::tn("cl.more.text")));
    text.add_css_class("hw-path");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_max_width_chars(96);

    card.append(&title);
    card.append(&text);
    card
}
