//! Bildschirm „Addon-Detail“: Banner, Kennzahlen, Tabs und rechte
//! Seitenleiste mit Ziel-Client, Versionen und Quellen.
//!
//! Zwei Wege fuehren hierher, und beide zeigen dieselbe Seite:
//! 1. aus dem Katalog ueber [`DetailView::show_item`] — dann kommt der Inhalt
//!    vollstaendig aus `Index::addon(key)` (Versionen je Quelle, Kennungen,
//!    installierte Fassungen);
//! 2. aus „Installiert“ ueber [`DetailView::show_local`] — dann steht hier, was
//!    die `.toc` und der Ordner hergeben, und es gibt keinen Installationsknopf,
//!    weil ohne Katalogeintrag keine Download-Adresse existiert.
//!
//! Was keine Quelle liefert (Screenshots, Changelog), steht als benannter
//! Platzhalter. Eine erfundene Beschreibung waere schlimmer als eine Luecke.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use hexwerk::InstalledAddon;
use hexwerk_index::{CatalogueItem, ReleaseRow, SourceRelease};
use hexwerk_sources::Release;

use crate::format::{human_size, interface_list};
use crate::state::{locale_label, note_texts, source_rows};

/// Was die Detailseite anzeigt, wenn sie aus der `.toc` kommt (installiert,
/// aber ohne Katalogeintrag).
pub struct LocalDetail {
    pub folder: String,
    pub title: String,
    pub author: String,
    pub version: String,
    pub interface: String,
    pub category: String,
    pub size: String,
    pub client_label: String,
    pub flavor: String,
    pub addons_path: String,
    pub sources: Vec<(String, String)>,
    pub notes: Vec<(String, String)>,
}

impl LocalDetail {
    pub fn from_addon(
        addon: &InstalledAddon,
        client_label: &str,
        flavor: &str,
        addons_path: &str,
    ) -> Self {
        let toc = addon.toc.as_ref();
        Self {
            folder: addon.folder.clone(),
            title: toc
                .and_then(|t| t.title.clone())
                .unwrap_or_else(|| addon.folder.clone()),
            author: toc
                .and_then(|t| t.author.clone())
                .unwrap_or_else(|| "keine Angabe".to_string()),
            version: addon
                .version
                .clone()
                .unwrap_or_else(|| "keine Version in der .toc".to_string()),
            interface: interface_list(&addon.interface_numbers),
            category: toc
                .and_then(|t| t.category.clone())
                .unwrap_or_else(|| "keine Angabe".to_string()),
            size: human_size(addon.size_bytes),
            client_label: client_label.to_string(),
            flavor: flavor.to_string(),
            addons_path: addons_path.to_string(),
            sources: source_rows(addon),
            notes: note_texts(addon),
        }
    }
}

/// Ein moegliches Installationsziel: Index in `ScanData.clients` und dessen
/// `flavors`. Die Indizes statt fertiger Pfade, damit der Aufrufer genau den
/// Client findet, den er fuer `InstallRequest` braucht.
#[derive(Debug, Clone)]
pub struct DetailTarget {
    pub client: usize,
    pub flavor: usize,
    pub label: String,
}

/// Was der Installationsknopf abschickt.
#[derive(Debug, Clone)]
pub struct InstallChoice {
    pub client: usize,
    pub flavor: usize,
    pub release: Release,
    pub name: String,
}

type InstallCallback = Box<dyn Fn(InstallChoice)>;

pub struct DetailView {
    root: gtk::Stack,
    banner_title: gtk::Label,
    banner_byline: gtk::Label,
    stats: Vec<gtk::Label>,
    pages: Vec<(&'static str, gtk::Box)>,
    tab_buttons: Vec<(&'static str, gtk::Button)>,
    tab_stack: gtk::Stack,
    target_dd: gtk::DropDown,
    release_dd: gtk::DropDown,
    install: gtk::Button,
    install_hint: gtk::Label,
    side_local: gtk::Label,
    side_latest: gtk::Label,
    side_sources: gtk::Box,
    side_key: gtk::Label,
    side_interface: gtk::Label,
    side_folder: gtk::Label,
    targets: RefCell<Vec<DetailTarget>>,
    releases: RefCell<Vec<Release>>,
    current: RefCell<Option<CatalogueItem>>,
    on_install: RefCell<Option<InstallCallback>>,
}

const TABS: [&str; 5] = [
    "Beschreibung",
    "Screenshots",
    "Changelog",
    "Dateien",
    "Quellen",
];

impl DetailView {
    pub fn new() -> Rc<Self> {
        // ---- Banner ----
        let banner_title = gtk::Label::new(None);
        banner_title.add_css_class("hw-banner-title");
        banner_title.set_xalign(0.0);
        banner_title.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let banner_byline = gtk::Label::new(None);
        banner_byline.add_css_class("hw-banner-byline");
        banner_byline.set_xalign(0.0);
        banner_byline.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let banner_text = gtk::Box::new(gtk::Orientation::Vertical, 3);
        banner_text.set_valign(gtk::Align::End);
        banner_text.append(&banner_title);
        banner_text.append(&banner_byline);

        let banner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        banner.add_css_class("hw-banner");
        banner.set_valign(gtk::Align::End);
        banner.append(&banner_text);
        let filler = gtk::Box::new(gtk::Orientation::Vertical, 0);
        filler.set_vexpand(true);
        banner.prepend(&filler);

        // ---- Kennzahlen ----
        let mut stats = Vec::new();
        let stat_grid = gtk::Grid::new();
        stat_grid.add_css_class("hw-stats");
        stat_grid.set_column_homogeneous(true);
        for (index, label) in ["VERSION", "INTERFACE", "GRÖSSE", "ADDON-ORDNER"]
            .iter()
            .enumerate()
        {
            let value = gtk::Label::new(Some("—"));
            value.add_css_class("hw-stat-value");
            value.set_xalign(0.0);
            // Umbruch statt Auslassung: „5.22.1 · Gith…“ war keine Auskunft.
            value.set_wrap(true);
            value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            value.set_lines(2);

            let caption = gtk::Label::new(Some(&label.to_uppercase()));
            caption.add_css_class("hw-stat-label");
            caption.set_xalign(0.0);

            let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
            cell.add_css_class("hw-stat");
            cell.append(&value);
            cell.append(&caption);
            stat_grid.attach(&cell, index as i32, 0, 1, 1);
            stats.push(value);
        }

        // ---- Galerie ----
        let gallery = gtk::Grid::new();
        gallery.set_column_homogeneous(true);
        gallery.set_column_spacing(8);
        gallery.set_row_spacing(8);
        gallery.set_margin_top(14);
        gallery.set_margin_bottom(14);
        for index in 1..=4 {
            let cell = gtk::Label::new(Some(&format!("VORSCHAU {index:02}")));
            cell.add_css_class("hw-gal-cell");
            cell.set_hexpand(true);
            cell.set_tooltip_text(Some(
                "Platzhalter: keine der angebundenen Quellen liefert Screenshots über ihre API.",
            ));
            gallery.attach(&cell, index - 1, 0, 1, 1);
        }

        // ---- Tabs ----
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabs.add_css_class("hw-tabbar");
        tabs.set_margin_top(4);

        let tab_stack = gtk::Stack::new();
        tab_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        tab_stack.set_margin_top(12);

        let mut tab_buttons = Vec::new();
        let mut pages = Vec::new();
        for name in TABS {
            let button = gtk::Button::with_label(name);
            button.add_css_class("hw-tab");
            if name == TABS[0] {
                button.add_css_class("on");
            }
            tabs.append(&button);
            tab_buttons.push((name, button));

            let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
            tab_stack.add_named(&page, Some(name));
            pages.push((name, page));
        }
        tab_stack.set_visible_child_name(TABS[0]);

        for (name, button) in &tab_buttons {
            crate::theme::pointer(button);
            let stack = tab_stack.clone();
            let tabs_box = tabs.clone();
            let name = *name;
            button.connect_clicked(move |_| {
                stack.set_visible_child_name(name);
                let mut child = tabs_box.first_child();
                while let Some(widget) = child {
                    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                        button.remove_css_class("on");
                        if button.label().as_deref() == Some(name) {
                            button.add_css_class("on");
                        }
                    }
                    child = widget.next_sibling();
                }
            });
        }

        // ---- rechte Seitenleiste (Grundgeruest, Werte kommen beim Zeigen) ----
        let side = gtk::Box::new(gtk::Orientation::Vertical, 0);
        side.add_css_class("hw-side");
        side.set_valign(gtk::Align::Start);
        side.set_size_request(302, -1);
        side.set_hexpand(false);

        side.append(&section_title("INSTALLATIONSZIEL"));
        let target_dd = gtk::DropDown::from_strings(&["kein Client gefunden"]);
        target_dd.add_css_class("hw-drop");
        target_dd.set_sensitive(false);
        target_dd.set_margin_top(4);
        target_dd.set_tooltip_text(Some("Addon-Ordner, in den installiert wird"));
        crate::theme::pointer(&target_dd);
        side.append(&target_dd);

        let release_dd = gtk::DropDown::from_strings(&["keine Quelle"]);
        release_dd.add_css_class("hw-drop");
        release_dd.set_sensitive(false);
        release_dd.set_margin_top(6);
        release_dd.set_tooltip_text(Some("Release, das geladen wird"));
        crate::theme::pointer(&release_dd);
        side.append(&release_dd);

        let install = gtk::Button::with_label("Installieren");
        install.add_css_class("hw-btn");
        install.set_sensitive(false);
        install.set_margin_top(10);
        crate::theme::pointer(&install);
        side.append(&install);

        let install_hint = gtk::Label::new(Some(""));
        install_hint.add_css_class("hw-path");
        install_hint.set_xalign(0.0);
        install_hint.set_wrap(true);
        install_hint.set_margin_top(6);
        side.append(&install_hint);

        side.append(&section_title_spaced("VERSION"));
        let side_local = value_row(&side, "lokal installiert");
        let side_latest = value_row(&side, "aktuellste Quelle");

        side.append(&section_title_spaced("QUELLEN"));
        let side_sources = gtk::Box::new(gtk::Orientation::Vertical, 0);
        side.append(&side_sources);

        side.append(&section_title_spaced("KENNUNG"));
        let side_key = value_row(&side, "Schlüssel");
        let side_interface = value_row(&side, "Interface");
        let side_folder = value_row(&side, "Ordner");

        let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
        left.set_hexpand(true);
        left.append(&banner);
        left.append(&stat_grid);
        left.append(&gallery);
        left.append(&tabs);
        left.append(&tab_stack);

        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        columns.set_margin_top(18);
        columns.set_margin_bottom(24);
        columns.set_margin_start(24);
        columns.set_margin_end(24);
        columns.set_valign(gtk::Align::Start);
        columns.append(&left);
        columns.append(&side);

        let detail_page = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&columns)
            .build();
        detail_page.add_css_class("hw-pane");

        // ---- leerer Zustand ----
        let empty_headline = gtk::Label::new(Some("Kein Eintrag gewählt"));
        empty_headline.add_css_class("hw-h1");
        empty_headline.set_halign(gtk::Align::Center);

        let empty_text = gtk::Label::new(Some(
            "Öffne eine Karte im Katalog oder ein Addon unter „Installiert“ — dann steht hier, was \
             der Index und die `.toc` hergeben.",
        ));
        empty_text.add_css_class("hw-lead");
        empty_text.set_wrap(true);
        empty_text.set_justify(gtk::Justification::Center);
        empty_text.set_max_width_chars(76);

        let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        empty_box.set_valign(gtk::Align::Center);
        empty_box.set_halign(gtk::Align::Center);
        empty_box.set_margin_top(80);
        empty_box.append(&empty_headline);
        empty_box.append(&empty_text);

        let empty_page = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&empty_box)
            .build();
        empty_page.add_css_class("hw-pane");

        let root = gtk::Stack::new();
        root.add_named(&empty_page, Some("empty"));
        root.add_named(&detail_page, Some("detail"));
        root.set_visible_child_name("empty");

        let view = Rc::new(Self {
            root,
            banner_title,
            banner_byline,
            stats,
            pages,
            tab_buttons,
            tab_stack,
            target_dd,
            release_dd,
            install,
            install_hint,
            side_local,
            side_latest,
            side_sources,
            side_key,
            side_interface,
            side_folder,
            targets: RefCell::new(Vec::new()),
            releases: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            on_install: RefCell::new(None),
        });
        view.wire();
        view
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub fn show_empty(&self) {
        self.root.set_visible_child_name("empty");
    }

    /// Rueckruf des Installationsknopfs. Der Aufrufer besorgt Download und
    /// Entpacken - das Widget kennt weder Netz noch ZIP.
    pub fn set_on_install<F: Fn(InstallChoice) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_install.borrow_mut() = Some(Box::new(f));
    }

    fn wire(self: &Rc<Self>) {
        // Beide Auswahlen entscheiden darueber, ob der Knopf aktiv ist.
        let weak = Rc::downgrade(self);
        self.target_dd.connect_selected_notify(move |_| {
            if let Some(view) = weak.upgrade() {
                view.recompute_install();
            }
        });
        let weak = Rc::downgrade(self);
        self.release_dd.connect_selected_notify(move |_| {
            if let Some(view) = weak.upgrade() {
                view.recompute_install();
            }
        });

        let weak = Rc::downgrade(self);
        self.install.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            let Some(release) = view.selected_release() else {
                return;
            };
            let index = view.target_dd.selected();
            let Some(target) = view.targets.borrow().get(index as usize).cloned() else {
                return;
            };
            let name = view
                .current
                .borrow()
                .as_ref()
                .map(|item| item.name.clone())
                .unwrap_or_else(|| release.external_id.clone());
            // Der Borrow endet in diesem Block: ein `Ref` auf das Feld darf
            // nicht laenger leben als das `Rc`, aus dem es stammt.
            let choice = InstallChoice {
                client: target.client,
                flavor: target.flavor,
                release,
                name,
            };
            let callback = view.on_install.borrow();
            if let Some(callback) = callback.as_ref() {
                callback(choice);
            }
        });
    }

    fn selected_release(&self) -> Option<Release> {
        let index = self.release_dd.selected();
        self.releases.borrow().get(index as usize).cloned()
    }

    /// Knopf nur aktiv, wenn ein Ziel-Ordner und ein Release mit echter
    /// Download-Adresse gewaehlt sind. Der Hinweis darunter sagt, was fehlt -
    /// ein grauer Knopf ohne Begruendung ist ein Raetsel.
    fn recompute_install(&self) {
        let release = self.selected_release();
        let has_target = self
            .targets
            .borrow()
            .get(self.target_dd.selected() as usize)
            .is_some();
        let has_url = release
            .as_ref()
            .is_some_and(|release| !release.download_url.trim().is_empty());
        self.install.set_sensitive(has_target && has_url);
        let hint = if self.targets.borrow().is_empty() {
            "Kein Ziel-Client gefunden — „Scan“ sucht die Präfixe, dann lässt sich hier wählen."
                .to_string()
        } else if !has_target {
            "Wähle einen Ziel-Client-Ordner.".to_string()
        } else if release.is_none() {
            "Wähle ein Release.".to_string()
        } else if !has_url {
            format!(
                "{} nennt für diese Version keine Download-Adresse — Installation nicht möglich.",
                release
                    .as_ref()
                    .map(|r| r.source.label())
                    .unwrap_or("Diese Quelle")
            )
        } else {
            String::new()
        };
        self.install_hint.set_text(&hint);
    }

    /// Katalogeintrag zeigen: alle Angaben stammen aus `Index::addon`.
    pub fn show_item(self: &Rc<Self>, item: &CatalogueItem, targets: &[DetailTarget]) {
        let latest = item.latest.as_ref();
        self.banner_title.set_text(&item.name);
        let category = item
            .category
            .clone()
            .unwrap_or_else(|| "keine Kategorie".to_string());
        self.banner_byline.set_text(&format!(
            "{} · {} · {}",
            display_or(&item.author, "keine Angabe"),
            category,
            item.key
        ));

        self.stats[0].set_text(&version_of(latest));
        self.stats[1].set_text(&interface_of(latest));
        self.stats[2].set_text(&size_of(latest.map(|r| r.size_bytes).unwrap_or(None)));
        self.stats[3].set_text(
            &item
                .installed
                .first()
                .map(|row| format!("Interface/AddOns/{}", row.folder))
                .unwrap_or_else(|| "nicht installiert".to_string()),
        );

        self.fill_description_item(item);
        self.fill_shots();
        self.fill_changelog();
        self.fill_files_item(item);
        self.fill_sources_item(item);

        self.fill_side_item(item, targets);
        *self.current.borrow_mut() = Some(item.clone());

        self.mark_tab(TABS[0]);
        self.root.set_visible_child_name("detail");
    }

    /// Ein installiertes Addon ohne Katalogeintrag: alles aus `.toc` und Ordner.
    pub fn show_local(&self, detail: &LocalDetail) {
        self.banner_title.set_text(&detail.title);
        self.banner_byline.set_text(&format!(
            "{} · {} · Interface/AddOns/{}",
            detail.author, detail.category, detail.folder
        ));

        self.stats[0].set_text(&detail.version);
        self.stats[1].set_text(&detail.interface);
        self.stats[2].set_text(&detail.size);
        self.stats[3].set_text(&format!("Interface/AddOns/{}", detail.folder));

        self.fill_description_local(detail);
        self.fill_shots();
        self.fill_changelog();
        self.fill_files_local(detail);
        self.fill_sources_local(detail);

        // Ohne Katalogeintrag gibt es keine Download-Adresse - deshalb bleibt
        // der Knopf aus und sagt warum, statt ins Nichts zu zeigen.
        let no_targets = gtk::StringList::new(&["kein Katalogeintrag"]);
        self.target_dd.set_model(Some(&no_targets));
        self.target_dd.set_sensitive(false);
        let no_release = gtk::StringList::new(&["keine Quelle"]);
        self.release_dd.set_model(Some(&no_release));
        self.release_dd.set_sensitive(false);
        self.targets.borrow_mut().clear();
        self.releases.borrow_mut().clear();
        self.install.set_sensitive(false);
        self.install_hint.set_text(
            "Dieses Addon steht im Ordner, aber nicht im Katalog: ohne Quelle gibt es keine \
             Download-Adresse und damit nichts zu installieren.",
        );
        *self.current.borrow_mut() = None;

        self.side_local.set_text(&detail.version);
        self.side_latest.set_text("—");
        clear(&self.side_sources);
        if detail.sources.is_empty() {
            self.side_sources.append(&src_row("keine Quell-ID", "—"));
        } else {
            for (name, id) in &detail.sources {
                self.side_sources.append(&src_row(name, id));
            }
        }
        self.side_key
            .set_text(&format!("{} · {}", detail.client_label, detail.flavor));
        self.side_interface.set_text(&detail.interface);
        self.side_folder.set_text(&detail.folder);

        self.mark_tab(TABS[0]);
        self.root.set_visible_child_name("detail");
    }

    fn mark_tab(&self, active: &str) {
        for (name, button) in &self.tab_buttons {
            button.remove_css_class("on");
            if *name == active {
                button.add_css_class("on");
            }
        }
        self.tab_stack.set_visible_child_name(active);
    }

    fn page(&self, name: &str) -> Option<&gtk::Box> {
        self.pages
            .iter()
            .find(|(page_name, _)| *page_name == name)
            .map(|(_, page)| page)
    }

    fn fill_description_item(&self, item: &CatalogueItem) {
        let Some(page) = self.page("Beschreibung") else {
            return;
        };
        clear(page);

        let summary = item.summary.trim();
        let para = gtk::Label::new(Some(if summary.is_empty() {
            "Die Quelle liefert keinen Kurztext zu diesem Addon. Hexwerk erfindet keinen."
        } else {
            summary
        }));
        para.add_css_class(if summary.is_empty() {
            "hw-path"
        } else {
            "hw-prose"
        });
        para.set_xalign(0.0);
        para.set_wrap(true);
        para.set_selectable(true);
        para.set_max_width_chars(88);
        page.append(&para);

        let facts = gtk::Box::new(gtk::Orientation::Vertical, 0);
        facts.set_margin_top(10);
        facts.append(&src_row("Autor", &display_or(&item.author, "keine Angabe")));
        facts.append(&src_row(
            "Kategorie",
            &item
                .category
                .clone()
                .unwrap_or_else(|| "keine Angabe".to_string()),
        ));
        facts.append(&src_row(
            "Homepage",
            &item
                .homepage
                .clone()
                .unwrap_or_else(|| "keine Angabe".to_string()),
        ));
        facts.append(&src_row("Schlüssel", &item.key));
        if let Some(first) = item.installed.first() {
            facts.append(&src_row(
                "installiert",
                &format!(
                    "{} · {}",
                    first.client_display(),
                    first
                        .version
                        .clone()
                        .unwrap_or_else(|| "ohne Version".into())
                ),
            ));
        } else {
            facts.append(&src_row("installiert", "in keinem Client-Ordner"));
        }
        page.append(&facts);
    }

    fn fill_description_local(&self, detail: &LocalDetail) {
        let Some(page) = self.page("Beschreibung") else {
            return;
        };
        clear(page);

        match detail.notes.first() {
            Some((lang, text)) => {
                let para = gtk::Label::new(Some(text));
                para.add_css_class("hw-prose");
                para.set_xalign(0.0);
                para.set_wrap(true);
                para.set_selectable(true);
                para.set_max_width_chars(88);
                page.append(&para);

                let hint = gtk::Label::new(Some(&format!(
                    "Text aus `## Notes-{}` der .toc — nicht übersetzt, nicht umgeschrieben.",
                    locale_label(lang)
                )));
                hint.add_css_class("hw-path");
                hint.set_xalign(0.0);
                hint.set_wrap(true);
                page.append(&hint);
            }
            None => {
                let para = gtk::Label::new(Some(
                    "Die .toc dieses Addons enthält keine Beschreibung (`## Notes` fehlt). \
                     Hexwerk erfindet keinen Text.",
                ));
                para.add_css_class("hw-prose");
                para.set_xalign(0.0);
                para.set_wrap(true);
                page.append(&para);
            }
        }

        let facts = gtk::Box::new(gtk::Orientation::Vertical, 0);
        facts.set_margin_top(10);
        facts.append(&src_row("Autor", &detail.author));
        facts.append(&src_row("Kategorie", &detail.category));
        facts.append(&src_row("Ordner", &detail.folder));
        facts.append(&src_row(
            "Client",
            &format!("{} · {}", detail.client_label, detail.flavor),
        ));
        page.append(&facts);

        if detail.notes.len() > 1 {
            let more = gtk::Label::new(Some(&format!(
                "Weitere Sprachen in der .toc: {}",
                detail
                    .notes
                    .iter()
                    .skip(1)
                    .map(|(lang, _)| locale_label(lang))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
            more.add_css_class("hw-path");
            more.set_xalign(0.0);
            more.set_wrap(true);
            page.append(&more);
        }
    }

    fn fill_shots(&self) {
        let Some(page) = self.page("Screenshots") else {
            return;
        };
        clear(page);
        page.append(&placeholder_note(
            "Die angebundenen Quellen (GitHub, Tukui, WoWInterface) liefern über ihre API keine \
             Screenshots. Hexwerk zeigt deshalb keine fremden Bilder und keine Platzhalter, die \
             wie Bilder aussehen.",
        ));
    }

    fn fill_changelog(&self) {
        let Some(page) = self.page("Changelog") else {
            return;
        };
        clear(page);
        page.append(&placeholder_note(
            "Keine Angabe: keine der abgefragten Quellen liefert ein Changelog in ihrer Schnittstelle. \
             Die Versionshistorie steht im Tab „Dateien“ — das ist alles, was der Index belegen kann.",
        ));
    }

    fn fill_files_item(&self, item: &CatalogueItem) {
        let Some(page) = self.page("Dateien") else {
            return;
        };
        clear(page);
        if item.releases.is_empty() {
            page.append(&placeholder_note(
                "Der Index kennt für dieses Addon keine Veröffentlichung mit Download-Adresse.",
            ));
            return;
        }
        for release in &item.releases {
            page.append(&release_row(release));
        }
    }

    fn fill_files_local(&self, detail: &LocalDetail) {
        let Some(page) = self.page("Dateien") else {
            return;
        };
        clear(page);
        page.append(&src_row("Addon-Ordner", &detail.folder));
        page.append(&src_row("Pfad", &detail.addons_path));
        page.append(&src_row("Größe auf der Platte", &detail.size));
        page.append(&src_row("Interface", &detail.interface));
        page.append(&placeholder_note(
            "Das ist der lokale Ordner. Releases und Download-Adressen stehen hier, sobald das \
             Addon einem Katalogeintrag zugeordnet ist.",
        ));
    }

    fn fill_sources_item(&self, item: &CatalogueItem) {
        let Some(page) = self.page("Quellen") else {
            return;
        };
        clear(page);
        if item.sources.is_empty() {
            page.append(&placeholder_note(
                "Keine Quelle führt dieses Addon. Es kam über einen anderen Weg in den Index.",
            ));
            return;
        }
        for release in &item.sources {
            page.append(&source_block(release));
        }
        let note = gtk::Label::new(Some(
            "Je Quelle die höchste bekannte Version. Der Katalog zeigt eine Karte pro Addon — \
             auch wenn mehrere Märkte dasselbe Projekt führen.",
        ));
        note.add_css_class("hw-path");
        note.set_xalign(0.0);
        note.set_wrap(true);
        note.set_margin_top(6);
        page.append(&note);
    }

    fn fill_sources_local(&self, detail: &LocalDetail) {
        let Some(page) = self.page("Quellen") else {
            return;
        };
        clear(page);
        if detail.sources.is_empty() {
            page.append(&placeholder_note(
                "Die .toc nennt keine Quell-ID (`X-WoWI-ID`, `X-Tukui-Project-ID`, \
                 `X-Curse-Project-ID`, `X-Wago-ID`). Ohne diese Kennung kann Hexwerk das Addon \
                 keinem Markt zuordnen — es bleibt beim lokalen Ordner.",
            ));
            return;
        }
        for (name, id) in &detail.sources {
            page.append(&src_row(name, id));
        }
        page.append(&placeholder_note(
            "Kennungen aus der .toc. Versionen je Quelle stehen hier, sobald der Katalog dieses \
             Addon führt.",
        ));
    }

    fn fill_side_item(&self, item: &CatalogueItem, targets: &[DetailTarget]) {
        *self.targets.borrow_mut() = targets.to_vec();
        let labels: Vec<&str> = targets.iter().map(|t| t.label.as_str()).collect();
        let strings = if labels.is_empty() {
            gtk::StringList::new(&["kein Client gefunden"])
        } else {
            gtk::StringList::new(&labels)
        };
        self.target_dd.set_model(Some(&strings));
        self.target_dd.set_sensitive(!targets.is_empty());
        if !targets.is_empty() {
            self.target_dd.set_selected(0);
        }

        // Releases mit Version; die Reihenfolge ist die der Quellen, damit die
        // Auswahl nicht bei jedem Aufruf anders aussieht.
        let releases: Vec<Release> = item.sources.iter().map(release_of).collect();
        let release_labels: Vec<String> = releases
            .iter()
            .map(|release| {
                let version = if release.version.trim().is_empty() {
                    "keine Version".to_string()
                } else {
                    release.version.clone()
                };
                let suffix = if release.download_url.trim().is_empty() {
                    " · keine Download-Adresse"
                } else {
                    ""
                };
                format!("{} {version}{suffix}", release.source.label())
            })
            .collect();
        let release_refs: Vec<&str> = release_labels.iter().map(String::as_str).collect();
        let release_list = gtk::StringList::new(&release_refs);
        self.release_dd.set_model(Some(&release_list));
        self.release_dd.set_sensitive(!releases.is_empty());
        // Vorausgewaehlt ist die hoechste Version (PLAN §5: Anzeige-Version ist
        // die hoechste, mit Quellenangabe).
        let preferred = item
            .latest
            .as_ref()
            .and_then(|latest| {
                releases
                    .iter()
                    .position(|release| release.source == latest.source)
            })
            .unwrap_or(0);
        if !releases.is_empty() {
            self.release_dd.set_selected(preferred as u32);
        }
        *self.releases.borrow_mut() = releases;

        self.side_local.set_text(
            &item
                .installed
                .first()
                .map(|row| {
                    row.version
                        .clone()
                        .unwrap_or_else(|| "ohne Version".to_string())
                })
                .unwrap_or_else(|| "nicht installiert".to_string()),
        );
        self.side_latest.set_text(&version_of(item.latest.as_ref()));

        clear(&self.side_sources);
        if item.sources.is_empty() {
            self.side_sources.append(&src_row("keine Quelle", "—"));
        } else {
            for release in &item.sources {
                self.side_sources.append(&src_row(
                    release.source.label(),
                    &display_or(&release.version, "keine Version"),
                ));
            }
        }
        self.side_key.set_text(&item.key);
        self.side_interface
            .set_text(&interface_of(item.latest.as_ref()));
        self.side_folder.set_text(
            &item
                .installed
                .first()
                .map(|row| row.folder.clone())
                .unwrap_or_else(|| "nicht installiert".to_string()),
        );

        self.recompute_install();
    }
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn section_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(&text.to_uppercase()));
    label.add_css_class("hw-side-title");
    label.set_xalign(0.0);
    label
}

fn section_title_spaced(text: &str) -> gtk::Label {
    let label = section_title(text);
    label.set_margin_top(18);
    label.set_margin_bottom(4);
    label
}

/// Eine Zeile „Name … Wert“ wie in der Seitenleiste des Entwurfs.
fn src_row(name: &str, value: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("hw-srcrow");

    let key = gtk::Label::new(Some(name));
    key.add_css_class("hw-srcrow-name");
    key.set_xalign(0.0);
    key.set_hexpand(true);
    key.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let val = gtk::Label::new(Some(value));
    val.add_css_class("hw-srcrow-value");
    val.set_xalign(1.0);
    val.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    val.set_tooltip_text(Some(value));
    val.set_selectable(true);

    row.append(&key);
    row.append(&val);
    row
}

/// Zeile „Name … Wert“ in der Seitenleiste, deren Wert sich spaeter aendert.
/// Gibt das Wert-Label zurueck, damit der Aufrufer es behalten kann.
fn value_row(container: &gtk::Box, name: &str) -> gtk::Label {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("hw-srcrow");

    let key = gtk::Label::new(Some(name));
    key.add_css_class("hw-srcrow-name");
    key.set_xalign(0.0);
    key.set_hexpand(true);

    let value = gtk::Label::new(Some("—"));
    value.add_css_class("hw-srcrow-value");
    value.set_xalign(1.0);
    value.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

    row.append(&key);
    row.append(&value);
    container.append(&row);
    value
}

fn placeholder_note(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("hw-note");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_max_width_chars(88);
    label
}

/// Ein Quellenblock im Tab „Quellen“: Markt, Version, Kennung, Datum, Größe.
fn source_block(release: &SourceRelease) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 3);
    block.set_margin_bottom(10);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(release.source.label()));
    name.add_css_class("hw-flavor-name");
    name.set_xalign(0.0);
    name.set_hexpand(true);

    let version = gtk::Label::new(Some(&display_or(&release.version, "keine Version")));
    version.add_css_class("hw-chip");
    if release.download_url.trim().is_empty() {
        version.add_css_class("off");
    } else {
        version.add_css_class("sig");
    }

    head.append(&name);
    head.append(&version);
    block.append(&head);

    let mut parts = Vec::new();
    parts.push(if release.external_id.trim().is_empty() {
        "keine Projekt-ID".to_string()
    } else {
        format!("ID {}", release.external_id)
    });
    parts.push(match &release.released_at {
        Some(date) if !date.trim().is_empty() => format!("veröffentlicht {date}"),
        _ => "kein Datum".to_string(),
    });
    parts.push(match release.size_bytes {
        Some(size) => human_size(size),
        None => "keine Größe".to_string(),
    });
    let meta = gtk::Label::new(Some(&parts.join(" · ")));
    meta.add_css_class("hw-path");
    meta.set_xalign(0.0);
    meta.set_wrap(true);
    block.append(&meta);

    let url = gtk::Label::new(Some(&display_or(
        &release.download_url,
        "keine Download-Adresse",
    )));
    url.add_css_class("hw-path");
    url.set_xalign(0.0);
    url.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    url.set_selectable(true);
    url.set_tooltip_text(Some(&release.download_url));
    block.append(&url);

    block
}

/// Eine Zeile im Tab „Dateien“: Quelle, Version, Datum, Größe, Adresse.
fn release_row(release: &ReleaseRow) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("hw-srcrow");

    let source = gtk::Label::new(Some(release.source.label()));
    source.add_css_class("hw-srcrow-name");
    source.set_xalign(0.0);
    source.set_size_request(110, -1);

    let version = gtk::Label::new(Some(&display_or(&release.version, "keine Version")));
    version.add_css_class("hw-srcrow-value");
    version.set_xalign(0.0);
    version.set_size_request(110, -1);

    let meta = gtk::Label::new(Some(&format!(
        "{} · {}",
        release
            .fetched_at
            .clone()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "kein Datum".to_string()),
        release
            .size_bytes
            .map(human_size)
            .unwrap_or_else(|| "keine Größe".to_string())
    )));
    meta.add_css_class("hw-path");
    meta.set_xalign(1.0);
    meta.set_hexpand(true);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    meta.set_tooltip_text(Some(&release.url));

    row.append(&source);
    row.append(&version);
    row.append(&meta);
    row
}

/// `SourceRelease` → `hexwerk_sources::Release`, wie `Registry::download` ihn
/// erwartet.
fn release_of(source: &SourceRelease) -> Release {
    Release {
        source: source.source,
        external_id: source.external_id.clone(),
        version: source.version.clone(),
        released_at: source.released_at.clone(),
        download_url: source.download_url.clone(),
        size_bytes: source.size_bytes,
        interface_numbers: source.interfaces.clone(),
    }
}

fn version_of(latest: Option<&SourceRelease>) -> String {
    match latest {
        Some(release) => format!(
            "{} · {}",
            display_or(&release.version, "keine Version"),
            release.source.label()
        ),
        None => "keine Angabe".to_string(),
    }
}

fn interface_of(latest: Option<&SourceRelease>) -> String {
    match latest {
        Some(release) if !release.interfaces.is_empty() => interface_list(&release.interfaces),
        _ => "keine Angabe".to_string(),
    }
}

fn size_of(bytes: Option<u64>) -> String {
    match bytes {
        Some(size) => human_size(size),
        None => "keine Angabe".to_string(),
    }
}

fn display_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwerk_index::SourceRelease;
    use hexwerk_sources::Source;

    fn release(version: &str, interfaces: Vec<u32>) -> SourceRelease {
        SourceRelease {
            source: Source::GitHub,
            external_id: "4711".to_string(),
            version: version.to_string(),
            released_at: Some("2026-10-30T12:00:00Z".to_string()),
            download_url: "https://example.invalid/x.zip".to_string(),
            size_bytes: Some(2_400_000),
            interfaces,
        }
    }

    #[test]
    fn version_ohne_quelle_ist_keine_angabe() {
        assert_eq!(version_of(None), "keine Angabe");
        assert_eq!(
            version_of(Some(&release("5.19.2", vec![11509]))),
            "5.19.2 · GitHub"
        );
        assert_eq!(
            version_of(Some(&release("", vec![]))),
            "keine Version · GitHub"
        );
    }

    #[test]
    fn interface_ohne_nummern_ist_keine_angabe() {
        assert_eq!(interface_of(None), "keine Angabe");
        assert_eq!(interface_of(Some(&release("1.0", vec![]))), "keine Angabe");
        assert_eq!(
            interface_of(Some(&release("1.0", vec![11509, 20506]))),
            "11509 · 20506"
        );
    }

    #[test]
    fn leere_felder_bekommen_einen_text() {
        assert_eq!(display_or("  ", "keine Angabe"), "keine Angabe");
        assert_eq!(display_or(" ElvUI ", "keine Angabe"), "ElvUI");
    }

    #[test]
    fn release_uebernimmt_download_adresse() {
        let source = release("1.2.3", vec![11509]);
        let out = release_of(&source);
        assert_eq!(out.version, "1.2.3");
        assert_eq!(out.download_url, source.download_url);
        assert_eq!(out.interface_numbers, vec![11509]);
        assert_eq!(out.source, Source::GitHub);
    }
}
