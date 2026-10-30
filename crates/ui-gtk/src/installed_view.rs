//! Bildschirm „Installiert": die Addons eines Client-Ordners als Tabelle mit
//! Mehrfachauswahl.
//!
//! Die Zeilen sind genau das, was in den `.toc`-Dateien steht — keine eigene
//! Datenbank, keine geschaetzte Version. Eine Spalte „Status" gibt es trotzdem:
//! sie sagt ehrlich „unbekannt", solange keine Quelle den Ordner zuordnen kann.
//! Sobald der Index steht, vergleicht dieselbe Spalte gegen die Quellen.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;

use crate::detail_view::LocalDetail;
use crate::format::{interface_list, shorten_home};
use crate::state::{source_rows, ScanData};

/// Was die Tabelle je Zeile braucht; wird beim Filtern neu gebaut.
struct CellView {
    folder: String,
    version: String,
    interface: String,
    source: String,
    source_id: String,
    status: String,
    tone: String,
}

mod imp {
    use glib::prelude::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;

    /// Eine Zeile. Warum ein GObject? `ColumnView` haengt an einem `ListModel`,
    /// und das nimmt nur GObjects - Zeilen als reine Strings wuerden die
    /// Spalten an eine feste Reihenfolge binden.
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::AddonRow)]
    pub struct AddonRow {
        #[property(get, set)]
        folder: RefCell<String>,
        #[property(get, set)]
        version: RefCell<String>,
        #[property(get, set)]
        interface: RefCell<String>,
        #[property(get, set)]
        source: RefCell<String>,
        #[property(get, set)]
        source_id: RefCell<String>,
        #[property(get, set)]
        status: RefCell<String>,
        #[property(get, set)]
        tone: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AddonRow {
        const NAME: &'static str = "HexwerkAddonRow";
        type Type = super::AddonRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for AddonRow {}
}

glib::wrapper! {
    pub struct AddonRow(ObjectSubclass<imp::AddonRow>);
}

impl AddonRow {
    fn new(cell: CellView) -> Self {
        glib::Object::builder()
            .property("folder", cell.folder)
            .property("version", cell.version)
            .property("interface", cell.interface)
            .property("source", cell.source)
            .property("source-id", cell.source_id)
            .property("status", cell.status)
            .property("tone", cell.tone)
            .build()
    }

    fn prop(&self, name: &str) -> String {
        self.property::<String>(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    WithSource,
    NoSource,
    Updates,
}

impl Filter {
    const ALL: [Filter; 4] = [
        Filter::All,
        Filter::WithSource,
        Filter::NoSource,
        Filter::Updates,
    ];

    fn label(self, lang: &str) -> &'static str {
        match self {
            Filter::All => crate::lang::t(lang, "iv.filter.all"),
            Filter::WithSource => crate::lang::t(lang, "iv.filter.with"),
            Filter::NoSource => crate::lang::t(lang, "iv.filter.without"),
            Filter::Updates => crate::lang::t(lang, "iv.filter.updates"),
        }
    }

    fn keeps(self, addon: &hexwerk::InstalledAddon, has_update: bool) -> bool {
        let has_source = addon
            .toc
            .as_ref()
            .map(|toc| {
                let ids = &toc.source_ids;
                ids.github.is_some()
                    || ids.tukui.is_some()
                    || ids.wowinterface.is_some()
                    || ids.wago.is_some()
                    || ids.curseforge.is_some()
                    || ids.website.is_some()
            })
            .unwrap_or(false);
        match self {
            Filter::All => true,
            Filter::WithSource => has_source,
            Filter::NoSource => !has_source,
            Filter::Updates => has_update,
        }
    }
}

/// Was beim Doppelklick auf eine Zeile passiert. Eigener Typ, damit das Feld
/// nicht als unlesbare Verschachtelung dasteht.
///
/// Mitgeliefert wird der Index-Schluessel des Client-Ordners (`client::variante`)
/// und der Addon-Ordner: nur damit findet der Aufrufer im Index die Zuordnung
/// zu einem Katalogeintrag. Der `.toc`-Inhalt reist als Rueckfallebene mit,
/// damit ein Addon ohne Katalogeintrag trotzdem eine Detailseite bekommt.
pub struct OpenRequest {
    pub client_key: String,
    pub folder: String,
    pub local: LocalDetail,
}

type OpenCallback = Box<dyn Fn(OpenRequest)>;
/// „Alle faelligen Updates dieses Client-Ordners installieren."
type UpdateCallback = Box<dyn Fn()>;
/// Wird mit den markierten Ordnernamen gerufen.
type RemoveCallback = Box<dyn Fn(Vec<String>)>;
/// Wird ohne Argumente gerufen: der Bildschirm kennt das Ziel, die Aktion
/// nicht. So bleibt die Bestätigung dort, wo die Fenster sind.
type ClearCallback = Box<dyn Fn()>;

/// Ein waehlbarer Ziel-Client: Index in `ScanData.clients` und `flavors`.
#[derive(Clone, Copy)]
struct Target {
    client: usize,
    flavor: usize,
}

pub struct InstalledView {
    root: gtk::Box,
    /// Kopfzeile und Beschriftungen, damit ein Sprachwechsel sie nachziehen kann.
    title: gtk::Label,
    sub: gtk::Label,
    target_label: gtk::Label,
    lang: RefCell<String>,
    target: gtk::DropDown,
    columns: gtk::ColumnView,
    store: gtk::gio::ListStore,
    selection: gtk::MultiSelection,
    chips: gtk::Box,
    count: gtk::Label,
    empty: gtk::Box,
    data: RefCell<Option<Rc<ScanData>>>,
    targets: RefCell<Vec<Target>>,
    /// Faellige Updates aus `Index::updates`. Die Tabelle zeigt nur das
    /// Ergebnis, sie rechnet nicht selbst; der Schluessel ist (Client, Ordner),
    /// weil derselbe Ordnername in mehreren Client-Ordnern liegen kann.
    updates: RefCell<Vec<hexwerk_index::Update>>,
    filter: Cell<Filter>,
    on_open: RefCell<Option<OpenCallback>>,
    remove: gtk::Button,
    clear: gtk::Button,
    /// Installieren aus Datei und von einer Adresse: Handarbeit sichtbar machen.
    install_file: gtk::Button,
    install_url: gtk::Button,
    update: gtk::Button,
    on_remove: RefCell<Option<RemoveCallback>>,
    on_clear: RefCell<Option<ClearCallback>>,
    on_install_file: RefCell<Option<Box<dyn Fn()>>>,
    on_install_url: RefCell<Option<Box<dyn Fn(String)>>>,
    on_update: RefCell<Option<UpdateCallback>>,
}

impl InstalledView {
    pub fn new() -> Rc<Self> {
        let lang = crate::settings_view::Settings::load().language;
        let title = gtk::Label::new(Some(crate::lang::t(&lang, "iv.title")));
        title.add_css_class("hw-h1");
        title.set_xalign(0.0);

        let sub = gtk::Label::new(Some(crate::lang::t(&lang, "iv.sub")));
        sub.add_css_class("hw-sub");
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_margin_top(2);
        sub.set_margin_bottom(14);

        let target_label = gtk::Label::new(Some(crate::lang::t(&lang, "iv.target")));
        target_label.add_css_class("hw-badge");

        let target = gtk::DropDown::from_strings(&["—"]);
        target.set_sensitive(false);
        target.add_css_class("hw-drop");
        target.set_tooltip_text(Some("Client-Ordner, dessen Addons die Tabelle zeigt"));

        let target_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        target_row.set_margin_bottom(12);
        target_row.append(&target_label);
        target_row.append(&target);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        target_row.append(&spacer);

        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let count = gtk::Label::new(Some("—"));
        count.add_css_class("hw-dim2");

        let remove = gtk::Button::with_label(crate::lang::t(&lang, "iv.remove"));
        crate::theme::pointer(&remove);
        remove.add_css_class("hw-btn");
        remove.add_css_class("ghost");
        remove.set_sensitive(false);
        remove.set_tooltip_text(Some(
            "Markierte Addons in den Papierkorb verschieben. Rückgängig geht danach direkt.",
        ));
        let clear = gtk::Button::with_label(crate::lang::t(&lang, "iv.clear"));
        crate::theme::pointer(&clear);
        clear.add_css_class("hw-btn");
        clear.add_css_class("danger");
        clear.set_sensitive(false);
        clear.set_tooltip_text(Some(
            "Alle Addons dieses Client-Ordners in den Papierkorb. WTF bleibt unangetastet.",
        ));

        let update = gtk::Button::with_label("Updates installieren…");
        crate::theme::pointer(&update);
        update.add_css_class("hw-btn");
        update.set_sensitive(false);
        update.set_tooltip_text(Some(
            "Alle als „Update“ markierten Addons dieses Client-Ordners neu laden und entpacken.",
        ));

        // Handarbeit gehoert in die Oberflaeche, nicht nur ins Ablegen:
        // nicht jeder zieht eine Datei, und ein selbst geladenes ZIP ist
        // der Normalfall.
        let install_file = gtk::Button::with_label(crate::lang::t(&lang, "iv.install"));
        crate::theme::pointer(&install_file);
        install_file.add_css_class("hw-btn");
        install_file.set_tooltip_text(Some(
            "Addon aus einer ZIP-Datei in den gewaehlten Client-Ordner installieren.",
        ));

        let install_url = gtk::Button::with_label(crate::lang::t(&lang, "iv.install.url"));
        crate::theme::pointer(&install_url);
        install_url.add_css_class("hw-btn");
        install_url.add_css_class("ghost");
        install_url.set_tooltip_text(Some(
            "Addon von einer Adresse laden und in den gewaehlten Client-Ordner installieren.",
        ));

        let filter_bar = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        filter_bar.set_margin_bottom(12);
        filter_bar.append(&chips);
        let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        gap.set_hexpand(true);
        filter_bar.append(&gap);
        filter_bar.append(&install_file);
        filter_bar.append(&install_url);
        filter_bar.append(&count);
        filter_bar.append(&remove);
        filter_bar.append(&clear);
        filter_bar.append(&update);

        let store = gtk::gio::ListStore::new::<AddonRow>();
        // `MultiSelection` erlaubt Strg- und Umschalt-Auswahl ohne weiteres
        // Zutun; ein eigener Auswahlmodus waere GTK3-Denke.
        let selection = gtk::MultiSelection::new(Some(store.clone()));

        let columns = gtk::ColumnView::new(Some(selection.clone()));
        columns.add_css_class("hw-table");
        columns.set_vexpand(true);
        columns.set_tooltip_text(Some(
            "Mehrfachauswahl mit Strg- oder Umschalt-Klick; Doppelklick öffnet das Addon.",
        ));
        columns.append_column(&text_column(
            crate::lang::t(&lang, "iv.col.addon"),
            "folder",
            220,
            true,
            "hw-cell",
        ));
        columns.append_column(&text_column(
            crate::lang::t(&lang, "iv.col.version"),
            "version",
            90,
            false,
            "hw-cell-dim",
        ));
        columns.append_column(&text_column(
            crate::lang::t(&lang, "iv.col.interface"),
            "interface",
            170,
            false,
            "hw-cell-dim",
        ));
        columns.append_column(&chip_column(
            crate::lang::t(&lang, "iv.col.source"),
            "source",
        ));
        columns.append_column(&chip_column(
            crate::lang::t(&lang, "iv.col.status"),
            "status",
        ));
        // Die Zeilen oeffnen die Detailseite; der Zeiger gehoert auf die Tabelle,
        // weil jede Zeile ein Ziel ist (Kopfzeile ausgenommen - der Unterschied
        // ist im Raster klein genug, um ihn zu verschmerzen).
        crate::theme::pointer(&columns);

        let empty = empty_state();

        // Die Tabelle bekommt einen eigenen Scrollbereich mit fester Hoehe.
        // Ohne ihn waechst das Widget mit der Zeilenzahl, und bei sehr vielen
        // Addons in einem Ordner verlangt das Fenster eine Hoehe, die der
        // X-Server nicht mehr annimmt (BadAlloc).
        let table_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(160)
            .child(&columns)
            .build();
        table_scroll.set_vexpand(true);

        let table_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
        table_area.set_vexpand(true);
        table_area.append(&empty);
        table_area.append(&table_scroll);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("hw-pane");
        body.set_margin_top(20);
        body.set_margin_bottom(20);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.set_vexpand(true);
        body.append(&title);
        body.append(&sub);
        body.append(&target_row);
        body.append(&filter_bar);
        body.append(&table_area);

        let view = Rc::new(Self {
            root: body,
            target,
            columns,
            store,
            selection,
            chips,
            title: title.clone(),
            sub: sub.clone(),
            target_label: target_label.clone(),
            lang: RefCell::new(lang.clone()),
            count,
            empty,
            data: RefCell::new(None),
            targets: RefCell::new(Vec::new()),
            updates: RefCell::new(Vec::new()),
            filter: Cell::new(Filter::All),
            on_open: RefCell::new(None),
            remove,
            clear,
            update: update.clone(),
            on_remove: RefCell::new(None),
            on_clear: RefCell::new(None),
            install_file,
            install_url,
            on_install_file: RefCell::new(None),
            on_install_url: RefCell::new(None),
            on_update: RefCell::new(None),
        });
        view.wire();
        view
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Zieht die festen Texte auf eine Sprache nach: Titel, Untertitel,
    /// Ziel-Beschriftung, Knoepfe und Filterleiste.
    pub fn retranslate(self: &Rc<Self>, lang: &str) {
        *self.lang.borrow_mut() = lang.to_string();
        self.title.set_text(crate::lang::t(lang, "iv.title"));
        self.sub.set_text(crate::lang::t(lang, "iv.sub"));
        self.target_label
            .set_text(crate::lang::t(lang, "iv.target"));
        self.remove.set_label(crate::lang::t(lang, "iv.remove"));
        self.clear.set_label(crate::lang::t(lang, "iv.clear"));
        let mut index = 0;
        let mut child = self.chips.first_child();
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if let Some(filter) = Filter::ALL.get(index) {
                    button.set_label(filter.label(lang));
                }
                index += 1;
            }
            child = widget.next_sibling();
        }
        // Spaltenkoepfe: sie tragen den Text aus dem Aufbau.
        let keys = [
            "iv.col.addon",
            "iv.col.version",
            "iv.col.interface",
            "iv.col.source",
            "iv.col.status",
        ];
        let model = self.columns.columns();
        for (index, key) in keys.iter().enumerate() {
            if let Some(column) = model
                .item(index as u32)
                .and_then(|item| item.downcast::<gtk::ColumnViewColumn>().ok())
            {
                column.set_title(Some(crate::lang::t(lang, key)));
            }
        }
        self.rebuild();
    }

    pub fn set_on_open<F: Fn(OpenRequest) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    /// Rueckrufe erst nach dem `Rc`: sie brauchen einen schwachen Verweis auf
    /// genau dieses Objekt, sonst entstuende ein Kreis (Widget -> Closure ->
    /// View -> Widget).
    fn wire(self: &Rc<Self>) {
        for filter in Filter::ALL {
            let button = gtk::Button::with_label(filter.label(&self.lang.borrow()));
            button.add_css_class("hw-chip");
            button.set_tooltip_text(Some("Filtert die Zeilen nach Quell-Kennung in der .toc"));
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.filter.set(filter);
                    view.rebuild();
                }
            });
            crate::theme::pointer(&button);
            self.chips.append(&button);
        }

        let weak = Rc::downgrade(self);
        self.target.connect_selected_notify(move |_| {
            if let Some(view) = weak.upgrade() {
                view.rebuild();
            }
        });

        let weak = Rc::downgrade(self);
        self.columns.connect_activate(move |_, position| {
            if let Some(view) = weak.upgrade() {
                view.open_at(position);
            }
        });

        let weak = Rc::downgrade(self);
        self.selection.connect_selection_changed(move |_, _, _| {
            if let Some(view) = weak.upgrade() {
                view.update_count();
            }
        });

        // Deinstallieren: die Auswahl wird eingesammelt und an die Anwendung
        // gereicht. Bestätigt wird dort, wo ein Fenster für den Dialog da ist.
        let weak = Rc::downgrade(self);
        self.remove.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let folders = view.selected_folders();
            if folders.is_empty() {
                return;
            }
            // Den Rueckruf aus dem RefCell holen, bevor er laeuft: sonst haelt
            // der Borrow ueber den Aufruf hinaus und der naechste Zugriff
            // panic't.
            let cb = view.on_remove.borrow_mut().take();
            if let Some(cb) = cb {
                cb(folders);
                *view.on_remove.borrow_mut() = Some(cb);
            }
        });

        let weak = Rc::downgrade(self);
        self.clear.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let cb = view.on_clear.borrow_mut().take();
            if let Some(cb) = cb {
                cb();
                *view.on_clear.borrow_mut() = Some(cb);
            }
        });

        let weak = Rc::downgrade(self);
        self.update.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let cb = view.on_update.borrow_mut().take();
            if let Some(cb) = cb {
                cb();
                *view.on_update.borrow_mut() = Some(cb);
            }
        });
    }

    /// Waehlt den Ziel-Client aus, ohne die Ansicht zu wechseln. Der Aufrufer
    /// entscheidet, ob er danach auf "Installiert" springt.
    pub fn select_target(&self, client: usize, flavor: usize) {
        let position = self
            .targets
            .borrow()
            .iter()
            .position(|t| t.client == client && t.flavor == flavor);
        if let Some(position) = position {
            self.target.set_selected(position as u32);
            self.rebuild();
            self.update_count();
        }
    }

    pub fn set_data(&self, data: Rc<ScanData>) {
        // Welcher Ordner war gewaehlt? Gemerkt wird der Addon-Pfad, nicht der
        // Index: nach jeder Installation/Deinstallation kommt ein neuer Scan,
        // und Indizes koennen sich verschieben. Ohne das springt die Auswahl
        // bei „Deinstallieren" auf den ersten Eintrag zurueck.
        let previous: Option<std::path::PathBuf> = {
            let targets = self.targets.borrow();
            let data = self.data.borrow();
            targets
                .get(self.target.selected() as usize)
                .and_then(|target| {
                    let client = data.as_ref()?.clients.get(target.client)?;
                    let flavor = client.flavors.get(target.flavor)?;
                    Some(flavor.addons_dir.clone())
                })
        };

        let mut targets = Vec::new();
        let mut labels = Vec::new();
        for (ci, client) in data.clients.iter().enumerate() {
            for (fi, flavor) in client.flavors.iter().enumerate() {
                targets.push(Target {
                    client: ci,
                    flavor: fi,
                });
                labels.push(format!("{} · {}", client.label, flavor.dir_name));
            }
        }
        *self.targets.borrow_mut() = targets;
        *self.data.borrow_mut() = Some(data);

        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let strings = gtk::StringList::new(&refs);
        self.target.set_model(Some(&strings));
        self.target.set_sensitive(!labels.is_empty());

        let has_targets = !labels.is_empty();
        self.empty.set_visible(!has_targets);
        self.columns.set_visible(has_targets);
        if has_targets {
            // Vorherige Wahl wiederherstellen; sonst der erste Eintrag.
            let restored = previous.and_then(|dir| {
                let data = self.data.borrow();
                self.targets.borrow().iter().position(|target| {
                    data.as_ref()
                        .and_then(|data| data.clients.get(target.client))
                        .and_then(|client| client.flavors.get(target.flavor))
                        .is_some_and(|flavor| flavor.addons_dir == dir)
                })
            });
            self.target.set_selected(restored.unwrap_or(0) as u32);
        }
        self.rebuild();
    }

    /// Neue Update-Kenntnis uebernehmen (aus `Index::updates`) und die Tabelle
    /// neu bauen. Eine leere Liste heisst: nichts ist veraltet.
    pub fn set_updates(&self, updates: Vec<hexwerk_index::Update>) {
        *self.updates.borrow_mut() = updates;
        self.rebuild();
    }

    /// Faelliges Update fuer genau diesen Client-Ordner und dieses Addon.
    fn update_for(&self, client_id: &str, folder: &str) -> Option<(String, String)> {
        self.updates
            .borrow()
            .iter()
            .find(|update| update.client_id == client_id && update.folder == folder)
            .map(|update| {
                (
                    update.available_version.clone(),
                    update.source.label().to_string(),
                )
            })
    }

    fn rebuild(&self) {
        self.store.remove_all();
        self.update_chip_classes();

        let Some(data) = self.data.borrow().clone() else {
            self.update_count();
            return;
        };
        let target = self
            .targets
            .borrow()
            .get(self.target.selected() as usize)
            .copied();
        let Some(target) = target else {
            self.update_count();
            return;
        };
        let Some(client) = data.clients.get(target.client) else {
            self.update_count();
            return;
        };
        let Some(flavor) = client.flavors.get(target.flavor) else {
            self.update_count();
            return;
        };

        let filter = self.filter.get();
        let client_id = client.id.clone();
        for addon in flavor
            .addons
            .iter()
            .filter(|a| filter.keeps(a, self.update_for(&client_id, &a.folder).is_some()))
        {
            let sources = source_rows(addon);
            let (source, source_id) = match sources.first() {
                Some((name, id)) => (name.clone(), id.clone()),
                None => ("—".to_string(), String::new()),
            };
            let known = !sources.is_empty();
            // Ein Update schlaegt jede andere Statusaussage: es ist die
            // einzige Zeile, auf die der Nutzer reagieren soll.
            let update = self.update_for(&client_id, &addon.folder);
            let (status, tone) = match update {
                Some((version, _)) => (format!("Update {version}"), "sig".to_string()),
                None if known => ("nicht geprüft".to_string(), "warn".to_string()),
                None => ("unbekannt".to_string(), "off".to_string()),
            };
            self.store.append(&AddonRow::new(CellView {
                folder: addon.folder.clone(),
                version: addon.version.clone().unwrap_or_else(|| "—".to_string()),
                interface: interface_list(&addon.interface_numbers),
                source,
                source_id,
                status,
                tone,
            }));
        }
        self.update_count();
    }

    fn update_chip_classes(&self) {
        let active = self.filter.get();
        let mut child = self.chips.first_child();
        let mut index = 0usize;
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                button.remove_css_class("sig");
                if Filter::ALL.get(index).copied() == Some(active) {
                    button.add_css_class("sig");
                }
                index += 1;
            }
            child = widget.next_sibling();
        }
    }

    fn update_count(&self) {
        let shown = self.store.n_items();
        let selected = self.selection.selection().size();
        let fresh = self.updates.borrow().len();
        let text = if selected > 0 {
            crate::lang::t(&self.lang.borrow(), "iv.count")
                .replacen("{}", &shown.to_string(), 1)
                .replacen("{}", &selected.to_string(), 1)
                .replacen("{}", &fresh.to_string(), 1)
        } else {
            crate::lang::t(&self.lang.borrow(), "iv.count")
                .replacen("{}", &shown.to_string(), 1)
                .replacen("{}", "0", 1)
                .replacen("{}", &fresh.to_string(), 1)
        };
        self.count.set_text(&text);
        // Loeschen braucht eine Auswahl, Leeren braucht einen Ziel-Client.
        self.remove.set_sensitive(selected > 0);
        self.clear.set_sensitive(shown > 0);
        let here = self.current_updates().len();
        self.update.set_sensitive(here > 0);
        self.update.set_label(if here > 0 {
            crate::lang::t(&self.lang.borrow(), "iv.update.all")
        } else {
            crate::lang::t(&self.lang.borrow(), "iv.update.none")
        });
    }

    /// Ordnernamen der markierten Zeilen, in Tabellenreihenfolge.
    pub fn selected_folders(&self) -> Vec<String> {
        let selection = self.selection.selection();
        let mut out = Vec::new();
        for i in 0..selection.size() {
            let position = selection.nth(i as u32);
            if let Some(row) = self
                .store
                .item(position)
                .and_then(|o| o.downcast::<AddonRow>().ok())
            {
                out.push(row.prop("folder"));
            }
        }
        out
    }

    /// Ziel-Client der Tabelle als (Client-Index, Flavor-Index) in `ScanData`.
    pub fn current_target(&self) -> Option<(usize, usize)> {
        let selected = self.target.selected();
        if selected == gtk::INVALID_LIST_POSITION {
            return None;
        }
        self.targets
            .borrow()
            .get(selected as usize)
            .map(|t| (t.client, t.flavor))
    }

    pub fn set_on_remove<F: Fn(Vec<String>) + 'static>(&self, f: F) {
        *self.on_remove.borrow_mut() = Some(Box::new(f));
    }

    /// Rueckruf fuer „Installieren…" (Dateiauswahl).
    pub fn set_on_install_file<F: Fn() + 'static>(self: &Rc<Self>, f: F) {
        *self.on_install_file.borrow_mut() = Some(Box::new(f));
        let weak = Rc::downgrade(self);
        self.install_file.clone().connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let cb = view.on_install_file.borrow_mut().take();
            if let Some(cb) = cb {
                cb();
                *view.on_install_file.borrow_mut() = Some(cb);
            }
        });
    }

    /// Rueckruf fuer „Adresse…": bekommt die eingegebene Adresse.
    pub fn set_on_install_url<F: Fn(String) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_install_url.borrow_mut() = Some(Box::new(f));
        let weak = Rc::downgrade(self);
        self.install_url.clone().connect_clicked(move |button| {
            let Some(view) = weak.upgrade() else { return };

            // Ein Feld, zwei Knoepfe: eine Adresse ist eine Zeile Text. Kein
            // eigener Reiter, nichts Verstecktes.
            let dialog = gtk::Window::builder()
                .title(crate::lang::tn("iv.url.title"))
                .modal(true)
                .default_width(560)
                .build();
            if let Some(root) = button.root().and_downcast::<gtk::Window>() {
                dialog.set_transient_for(Some(&root));
            }
            dialog.add_css_class("hw-window");
            let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
            body.set_margin_top(16);
            body.set_margin_bottom(16);
            body.set_margin_start(16);
            body.set_margin_end(16);
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("https://…/addon.zip"));
            entry.set_hexpand(true);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_halign(gtk::Align::End);
            let ok = gtk::Button::with_label(crate::lang::tn("iv.install.ok"));
            ok.add_css_class("hw-btn");
            let cancel = gtk::Button::with_label(crate::lang::tn("iv.install.cancel"));
            cancel.add_css_class("hw-btn");
            cancel.add_css_class("ghost");
            crate::theme::pointer(&ok);
            crate::theme::pointer(&cancel);
            row.append(&cancel);
            row.append(&ok);
            body.append(&entry);
            body.append(&row);
            dialog.set_child(Some(&body));

            let dialog_cancel = dialog.clone();
            cancel.connect_clicked(move |_| dialog_cancel.close());

            let weak_ok = Rc::downgrade(&view);
            let entry_ok = entry.clone();
            let dialog_ok = dialog.clone();
            ok.connect_clicked(move |_| {
                let url = entry_ok.text().trim().to_string();
                if url.is_empty() {
                    return;
                }
                if let Some(view) = weak_ok.upgrade() {
                    if let Some(cb) = view.on_install_url.borrow().as_ref() {
                        cb(url);
                    }
                }
                dialog_ok.close();
            });
            dialog.present();
        });
    }

    pub fn set_on_clear<F: Fn() + 'static>(&self, f: F) {
        *self.on_clear.borrow_mut() = Some(Box::new(f));
    }

    /// Klick auf „Updates installieren…": der Aufrufer holt sich die Liste
    /// ueber [`InstalledView::current_updates`] und arbeitet sie ab.
    pub fn set_on_update<F: Fn() + 'static>(&self, f: F) {
        *self.on_update.borrow_mut() = Some(Box::new(f));
    }

    /// Faellige Updates fuer den gerade gewaehlten Client-Ordner.
    pub fn current_updates(&self) -> Vec<hexwerk_index::Update> {
        let Some((client_id, folders)) = self.current_scope() else {
            return Vec::new();
        };
        self.updates
            .borrow()
            .iter()
            .filter(|update| {
                update.client_id == client_id && folders.iter().any(|f| *f == update.folder)
            })
            .cloned()
            .collect()
    }

    /// Client-Kennung und Ordnernamen des gewaehlten Ziels.
    fn current_scope(&self) -> Option<(String, Vec<String>)> {
        let data = self.data.borrow().clone()?;
        let target = self
            .targets
            .borrow()
            .get(self.target.selected() as usize)
            .copied()?;
        let client = data.clients.get(target.client)?;
        let flavor = client.flavors.get(target.flavor)?;
        Some((
            client.id.clone(),
            flavor.addons.iter().map(|a| a.folder.clone()).collect(),
        ))
    }

    /// Doppelklick auf eine Zeile: Detailansicht fuer dieses Addon bauen. Alles,
    /// was dort steht, kommt aus der `.toc` und dem Dateisystem.
    fn open_at(&self, position: u32) {
        let Some(row) = self
            .store
            .item(position)
            .and_then(|o| o.downcast::<AddonRow>().ok())
        else {
            return;
        };
        let folder = row.prop("folder");

        let data = self.data.borrow().clone();
        let Some(data) = data else {
            return;
        };
        let target = self
            .targets
            .borrow()
            .get(self.target.selected() as usize)
            .copied();
        let Some(target) = target else {
            return;
        };
        let Some(client) = data.clients.get(target.client) else {
            return;
        };
        let Some(flavor) = client.flavors.get(target.flavor) else {
            return;
        };
        let Some(addon) = flavor.addons.iter().find(|a| a.folder == folder) else {
            return;
        };

        let detail = LocalDetail::from_addon(
            addon,
            &client.label,
            &flavor.dir_name,
            &shorten_home(&flavor.addons_dir),
        );
        if let Some(callback) = self.on_open.borrow().as_ref() {
            callback(OpenRequest {
                // Genau der Schluessel, den `Index::sync_clients` bildet:
                // Client-Kennung und Varianten-Ordner.
                client_key: format!("{}::{}", client.id, flavor.dir_name),
                folder: addon.folder.clone(),
                local: detail,
            });
        }
    }
}

fn text_column(
    title: &str,
    property: &'static str,
    width: i32,
    expand: bool,
    css: &'static str,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.add_css_class(css);
        item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(row), Some(label)) = (
            item.item().and_then(|o| o.downcast::<AddonRow>().ok()),
            item.child().and_then(|c| c.downcast::<gtk::Label>().ok()),
        ) else {
            return;
        };
        label.set_text(&row.prop(property));
    });

    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    column.set_fixed_width(width);
    column.set_expand(expand);
    column.set_resizable(true);
    column
}

/// Spalte mit Chip-Optik. Die Farbe haengt an der Zeile (`tone`) - so bleibt
/// „unbekannt" grau und „nicht geprüft" gelb, ohne dass die Spalte die
/// Bedeutung kennen muss.
fn chip_column(title: &str, property: &'static str) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_valign(gtk::Align::Center);
        item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(row), Some(label)) = (
            item.item().and_then(|o| o.downcast::<AddonRow>().ok()),
            item.child().and_then(|c| c.downcast::<gtk::Label>().ok()),
        ) else {
            return;
        };
        label.set_text(&row.prop(property));
        let tone = row.prop("tone");
        label.set_css_classes(&["hw-chip", &tone]);
        let id = row.prop("source_id");
        if id.is_empty() {
            label.set_tooltip_text(None);
        } else {
            label.set_tooltip_text(Some(&id));
        }
    });

    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    column.set_fixed_width(130);
    column
}

fn empty_state() -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 6);
    box_.add_css_class("hw-card");
    let title = gtk::Label::new(Some("Kein Client-Ordner mit Addons"));
    title.add_css_class("hw-note-title");
    title.set_xalign(0.0);
    let text = gtk::Label::new(Some(
        "Sobald ein Scan einen Ordner mit `Interface/AddOns` findet, stehen die Addons hier.",
    ));
    text.add_css_class("hw-dim");
    text.set_xalign(0.0);
    text.set_wrap(true);
    box_.append(&title);
    box_.append(&text);
    box_.set_visible(false);
    box_
}
