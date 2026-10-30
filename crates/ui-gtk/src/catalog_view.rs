//! Bildschirm „Katalog“: Suche und Quellenfilter über den lokalen Index.
//!
//! Alles, was hier steht, steht im Index (`~/.local/share/hexwerk/index.db`);
//! die Oberflaeche fragt nie das Netz. Deshalb gibt es vier ehrliche Zustaende:
//! „wird aufgebaut“ (mit Spinner, solange der Hintergrundlauf schreibt),
//! „Treffer“, „Index ist leer“ und „Index nicht lesbar“. Ein Quellenausfall
//! steht als Text der Quelle daneben — ein leerer Bildschirm ohne Grund waere
//! die unbrauchbarste aller Antworten (PLAN §5, Ausfallverhalten).

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use hexwerk_index::{CatalogueItem, Filters, Stats};

use crate::backend::{SourceRun, CATALOGUE_SOURCES};
use crate::format::initial;

/// Karten je Abfrage. Der Index kann mehr; eine Liste mit tausend Karten ist
/// kein Katalog, sondern eine Wand. Wer mehr braucht, verfeinert die Suche.
pub const PAGE: u32 = 60;

const SEARCH_HINT: &str = "Suche: Name, Autor, Beschreibung …";

/// Was gesucht wird. Eigener Rueckruf statt direkter Indexzugriff: das Widget
/// kennt keine Datenbank, der Aufrufer keine Karten.
type SearchCallback = Box<dyn Fn(Filters)>;
type OpenCallback = Box<dyn Fn(i64)>;

pub struct CatalogView {
    root: gtk::ScrolledWindow,
    search: gtk::SearchEntry,
    chip_box: gtk::Box,
    summary: gtk::Label,
    stack: gtk::Stack,
    building_detail: gtk::Label,
    empty_headline: gtk::Label,
    empty_text: gtk::Label,
    error_text: gtk::Label,
    grid: gtk::Grid,
    notes: gtk::Box,
    filters: RefCell<Filters>,
    on_search: RefCell<Option<SearchCallback>>,
    on_open: RefCell<Option<OpenCallback>>,
    debounce: RefCell<Option<glib::SourceId>>,
}

impl CatalogView {
    pub fn new() -> Rc<Self> {
        let title = gtk::Label::new(Some("Katalog"));
        title.add_css_class("hw-h1");
        title.set_xalign(0.0);

        let sub = gtk::Label::new(Some(
            "Dasselbe Addon aus mehreren Märkten ist ein Eintrag. Suche und Filter laufen über den \
             lokalen Index, nie über das Netz.",
        ));
        sub.add_css_class("hw-sub");
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_margin_top(2);
        sub.set_margin_bottom(16);

        let search = gtk::SearchEntry::new();
        search.add_css_class("hw-search");
        search.set_placeholder_text(Some(SEARCH_HINT));
        search.set_hexpand(true);

        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        search_row.set_margin_bottom(12);
        search_row.append(&search);

        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chips.set_margin_bottom(14);

        let summary = gtk::Label::new(Some("Katalog wird aufgebaut …"));
        summary.add_css_class("hw-dim2");
        summary.set_margin_bottom(14);
        summary.set_xalign(0.0);

        // ---- Zustand „wird aufgebaut“ ----
        let spinner = adw::Spinner::new();
        spinner.set_size_request(34, 34);
        spinner.set_halign(gtk::Align::Start);

        let building_headline = gtk::Label::new(Some("Katalog wird aufgebaut …"));
        building_headline.add_css_class("hw-note-title");
        building_headline.set_xalign(0.0);

        let building_detail = gtk::Label::new(Some(
            "Clients sind im Index; die Quellen werden befragt. Das dauert je nach Leitung ein \
             paar Sekunden bis Minuten.",
        ));
        building_detail.add_css_class("hw-dim");
        building_detail.set_xalign(0.0);
        building_detail.set_wrap(true);
        building_detail.set_max_width_chars(96);

        let building = gtk::Box::new(gtk::Orientation::Vertical, 10);
        building.add_css_class("hw-card");
        building.append(&spinner);
        building.append(&building_headline);
        building.append(&building_detail);

        // ---- Zustand „nichts gefunden / leer“ ----
        let empty_headline = gtk::Label::new(Some("Der Index ist leer"));
        empty_headline.add_css_class("hw-note-title");
        empty_headline.set_xalign(0.0);

        let empty_text = gtk::Label::new(Some(""));
        empty_text.add_css_class("hw-dim");
        empty_text.set_xalign(0.0);
        empty_text.set_wrap(true);
        empty_text.set_max_width_chars(96);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 6);
        empty.add_css_class("hw-card");
        empty.append(&empty_headline);
        empty.append(&empty_text);

        // ---- Zustand „Index nicht lesbar“ ----
        let error_headline = gtk::Label::new(Some("Der Index lässt sich nicht lesen"));
        error_headline.add_css_class("hw-note-title");
        error_headline.set_xalign(0.0);

        let error_text = gtk::Label::new(Some(""));
        error_text.add_css_class("hw-dim");
        error_text.set_xalign(0.0);
        error_text.set_wrap(true);
        error_text.set_max_width_chars(96);
        error_text.set_selectable(true);

        let error = gtk::Box::new(gtk::Orientation::Vertical, 6);
        error.add_css_class("hw-card");
        error.append(&error_headline);
        error.append(&error_text);

        // ---- Treffer ----
        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(14);
        grid.set_row_spacing(14);
        grid.set_valign(gtk::Align::Start);

        let results = gtk::Box::new(gtk::Orientation::Vertical, 0);
        results.append(&grid);

        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(120);
        stack.add_named(&building, Some("building"));
        stack.add_named(&results, Some("results"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&error, Some("error"));
        stack.set_visible_child_name("building");

        // Quellenzustand: steht unter den Karten und im leeren Zustand, weil er
        // dort die Erklaerung ist.
        let notes = gtk::Box::new(gtk::Orientation::Vertical, 4);
        notes.set_margin_top(16);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_margin_top(20);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(&title);
        body.append(&sub);
        body.append(&search_row);
        body.append(&chips);
        body.append(&summary);
        body.append(&stack);
        body.append(&notes);

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&body)
            .build();
        root.add_css_class("hw-pane");

        let view = Rc::new(Self {
            root,
            search,
            chip_box: chips,
            summary,
            stack,
            building_detail,
            empty_headline,
            empty_text,
            error_text,
            grid,
            notes,
            filters: RefCell::new(Filters::default()),
            on_search: RefCell::new(None),
            on_open: RefCell::new(None),
            debounce: RefCell::new(None),
        });
        view.build_chips();
        view.wire();
        view
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Rueckruf fuer jede Suchanfrage. Der Aufrufer beantwortet sie aus dem
    /// Index und ruft danach [`CatalogView::show_results`].
    pub fn set_on_search<F: Fn(Filters) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    /// Rueckruf beim Klick auf eine Karte. Uebergeben wird die Index-ID, nicht
    /// der Eintrag: die Detailseite soll die volle Fassung aus `Index::addon`
    /// lesen, nicht die abgespeckte Katalogzeile.
    pub fn set_on_open<F: Fn(i64) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    /// Der aktuelle Filter (Suchtext + gewaehlte Quellen).
    pub fn filters(&self) -> Filters {
        self.filters.borrow().clone()
    }

    fn build_chips(self: &Rc<Self>) {
        for source in CATALOGUE_SOURCES {
            let button = gtk::ToggleButton::with_label(source.label());
            button.add_css_class("hw-chip");
            button.set_tooltip_text(Some(&format!(
                "Nur Einträge, die {} als Quelle haben",
                source.label()
            )));
            let weak = Rc::downgrade(self);
            button.connect_toggled(move |button| {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                {
                    let mut filters = view.filters.borrow_mut();
                    if button.is_active() {
                        button.add_css_class("sig");
                        if !filters.sources.contains(&source) {
                            filters.sources.push(source);
                        }
                    } else {
                        button.remove_css_class("sig");
                        filters.sources.retain(|s| *s != source);
                    }
                }
                view.request_search(false);
            });
            crate::theme::pointer(&button);
            self.chip_box.append(&button);
        }
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.search.connect_search_changed(move |entry| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            view.filters.borrow_mut().query = entry.text().to_string();
            // Tippen ist kein Klick: 220 ms Ruhe, damit nicht jeder Buchstabe
            // eine Abfrage ausloest.
            view.request_search(true);
        });
    }

    fn request_search(self: &Rc<Self>, debounce: bool) {
        if let Some(id) = self.debounce.borrow_mut().take() {
            id.remove();
        }
        if debounce {
            let weak = Rc::downgrade(self);
            let id = glib::timeout_add_local_once(Duration::from_millis(220), move || {
                if let Some(view) = weak.upgrade() {
                    view.dispatch();
                }
            });
            *self.debounce.borrow_mut() = Some(id);
        } else {
            self.dispatch();
        }
    }

    fn dispatch(&self) {
        if let Some(callback) = self.on_search.borrow().as_ref() {
            callback(self.filters.borrow().clone());
        }
    }

    /// Zustand „laeuft“: Spinner, Zwischenstand, keine Karten.
    pub fn set_building(&self) {
        self.stack.set_visible_child_name("building");
        self.summary.set_text("Katalog wird aufgebaut …");
        self.clear_cards();
    }

    /// Zwischenstand im Aufbau („Index: 2 Client-Ordner, 14 Addons“).
    pub fn set_building_detail(&self, text: &str) {
        self.building_detail.set_text(text);
    }

    /// Kein Client gefunden: dann wurde der Kataloglauf uebersprungen.
    pub fn set_no_clients(&self) {
        self.show_empty(
            "Kein Client-Ordner gefunden",
            "Deshalb wurden die Quellen nicht befragt: ein Katalog ohne Ziel-Client wäre eine \
             Liste, aus der man nichts installieren kann. Der Scan-Bildschirm zeigt je Herkunft, \
             wo gesucht wurde.",
        );
    }

    /// Quellenzustand als Zeilen unter den Karten.
    pub fn set_sources(&self, runs: &[SourceRun]) {
        while let Some(child) = self.notes.first_child() {
            self.notes.remove(&child);
        }
        for run in runs {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("hw-info-row");

            let mark = gtk::Label::new(Some(if run.ok { "✓" } else { "!" }));
            mark.set_width_request(12);
            mark.add_css_class("hw-chip");
            mark.add_css_class(if run.ok { "sig" } else { "bad" });

            let name = gtk::Label::new(Some(run.source.label()));
            name.add_css_class("hw-dim");
            name.set_xalign(0.0);
            name.set_hexpand(true);

            let detail = gtk::Label::new(Some(&run.detail));
            detail.add_css_class("hw-path");
            detail.set_xalign(1.0);
            detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            detail.set_selectable(true);
            detail.set_tooltip_text(Some(&run.detail));

            row.append(&mark);
            row.append(&name);
            row.append(&detail);
            self.notes.append(&row);
        }
    }

    /// Treffer anzeigen. `stats` liefert die Gesamtzahl im Index — ohne sie
    /// koennte die Leiste nicht sagen, ob 3 Treffer von 3 oder von 1204 kommen.
    pub fn show_results(self: &Rc<Self>, items: &[CatalogueItem], stats: &Stats) {
        self.clear_cards();
        if stats.catalogue == 0 {
            self.show_empty(
                "Der Index enthält keine Katalogeinträge",
                "Das passiert, wenn keine Quelle geantwortet hat oder noch kein Client gefunden \
                 wurde. Die Zeilen unten nennen den Zustand je Quelle. „Erneut scannen“ auf dem \
                 Scan-Bildschirm baut Clients und Katalog noch einmal.",
            );
            return;
        }
        if items.is_empty() {
            let filters = self.filters.borrow();
            let text = if filters.query.trim().is_empty() {
                format!(
                    "Kein Eintrag passt zu den gewählten Quellen. Es sind {} Einträge im Index.",
                    stats.catalogue
                )
            } else {
                format!(
                    "Kein Treffer für „{}“. Gesucht wird über Name, Autor und Kurztext: {} \
                     Einträge sind im Index.",
                    filters.query.trim(),
                    stats.catalogue
                )
            };
            drop(filters);
            self.show_empty("Keine Treffer", &text);
            return;
        }

        for (index, item) in items.iter().enumerate() {
            let card = card(item, Rc::downgrade(self));
            card.set_hexpand(true);
            self.grid
                .attach(&card, (index % 2) as i32, (index / 2) as i32, 1, 1);
        }

        self.summary
            .set_text(&self.summary_text(items.len(), stats));
        self.stack.set_visible_child_name("results");
    }

    /// Index nicht lesbar: den echten Text zeigen, nicht „0 Einträge“.
    pub fn show_error(&self, text: &str) {
        self.clear_cards();
        self.error_text.set_text(text);
        self.summary.set_text("Index nicht lesbar");
        self.stack.set_visible_child_name("error");
    }

    fn show_empty(&self, headline: &str, text: &str) {
        self.empty_headline.set_text(headline);
        self.empty_text.set_text(text);
        self.summary.set_text("0 Einträge");
        self.stack.set_visible_child_name("empty");
    }

    fn summary_text(&self, shown: usize, stats: &Stats) -> String {
        let filters = self.filters.borrow();
        let mut parts = vec![format!("{shown} von {} Einträgen", stats.catalogue)];
        if !filters.query.trim().is_empty() {
            parts.push(format!("Suche „{}“", filters.query.trim()));
        }
        if !filters.sources.is_empty() {
            let names: Vec<&str> = filters.sources.iter().map(|s| s.label()).collect();
            parts.push(format!("Quellen: {}", names.join(", ")));
        }
        if shown as u32 >= PAGE {
            parts.push("erste Seite, Suche verfeinern".to_string());
        }
        parts.join(" · ")
    }

    fn clear_cards(&self) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
    }
}

/// Eine Katalokkarte nach dem Entwurf: Kennzeichen links, Name und Herkunft,
/// Kurztext, darunter je Quelle eine Version und die installierte Fassung.
///
/// Die Karte ist ein `GtkButton` und kein `Box` mit Klickgeste: nur so kommt
/// sie in die Tastaturreihenfolge, bekommt einen Fokusrahmen und wird von
/// Screenreadern als „Knopf“ vorgelesen.
fn card(item: &CatalogueItem, weak: Weak<CatalogView>) -> gtk::Button {
    let card = gtk::Button::new();
    card.add_css_class("hw-card");
    card.add_css_class("hw-card-clickable");
    card.set_valign(gtk::Align::Start);
    card.update_property(&[gtk::accessible::Property::Label(item.name.as_str())]);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_halign(gtk::Align::Fill);
    content.set_valign(gtk::Align::Fill);

    let glyph = gtk::Label::new(Some(&initial(&item.name)));
    glyph.add_css_class("hw-glyph");
    glyph.set_valign(gtk::Align::Start);

    let name = gtk::Label::new(Some(&item.name));
    name.add_css_class("hw-card-title");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let byline = gtk::Label::new(Some(&byline(item)));
    byline.add_css_class("hw-path");
    byline.set_xalign(0.0);
    byline.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_hexpand(true);
    texts.append(&name);
    texts.append(&byline);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.append(&glyph);
    head.append(&texts);
    content.append(&head);

    // Was die Quelle nicht liefert, wird nicht ersetzt: „keine Angabe“ ist
    // eine Aussage, ein erfundener Kurztext waere eine Luege.
    let summary = if item.summary.trim().is_empty() {
        "keine Angabe".to_string()
    } else {
        item.summary.trim().to_string()
    };
    let text = gtk::Label::new(Some(&summary));
    text.add_css_class(if item.summary.trim().is_empty() {
        "hw-path"
    } else {
        "hw-dim"
    });
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_lines(3);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    content.append(&text);

    let chips = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    chips.set_margin_top(2);
    for release in &item.sources {
        let version = if release.version.trim().is_empty() {
            "keine Version".to_string()
        } else {
            release.version.clone()
        };
        // Kurzform statt Ellipse: „GitHub 1.3.…“ war auf der Karte nicht zu
        // lesen. Die Quelle bekommt ein Kuerzel, die Version hoechstens zwoelf
        // Zeichen; der vollstaendige Text steht im Tooltip.
        let text = format!("{} {}", short_source(release.source), short_version(&version));
        let chip = gtk::Label::new(Some(&text));
        chip.add_css_class("hw-chip");
        chip.set_tooltip_text(Some(&if release.download_url.trim().is_empty() {
            format!("{version}: diese Quelle nennt keine Download-Adresse.")
        } else {
            format!("{} {version}", release.source.label())
        }));
        if release.download_url.trim().is_empty() {
            chip.add_css_class("off");
        }
        chips.append(&chip);
    }
    if item.sources.is_empty() {
        let chip = gtk::Label::new(Some("ohne Quelle"));
        chip.add_css_class("hw-chip");
        chip.add_css_class("off");
        chips.append(&chip);
    }
    // Installierte Fassung: aus dem Index, eine je Client. Bei mehreren
    // Clients steht die erste mit Anzahl, damit die Karte nicht luegt.
    if let Some(first) = item.installed.first() {
        let version = first
            .version
            .clone()
            .unwrap_or_else(|| "ohne Version".to_string());
        let label = if item.installed.len() > 1 {
            format!("installiert {version} · {} Clients", item.installed.len())
        } else {
            format!("installiert {version}")
        };
        let chip = gtk::Label::new(Some(&label));
        chip.add_css_class("hw-chip");
        chip.add_css_class("warn");
        chip.set_tooltip_text(Some(&format!(
            "{} · {}",
            first.client_display(),
            first.folder
        )));
        chips.append(&chip);
    }
    content.append(&chips);

    card.set_child(Some(&content));
    crate::theme::pointer(&card);
    let id = item.id;
    card.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            let callback = view.on_open.borrow();
            if let Some(callback) = callback.as_ref() {
                callback(id);
            }
        }
    });
    card.set_tooltip_text(Some("Details öffnen"));
    card
}

/// Zweite Zeile: Herkunft und Grad der Zusammenfuehrung.
fn byline(item: &CatalogueItem) -> String {
    let author = item.author.trim();
    let sources = item.sources.len();
    let origin = match sources {
        0 => "ohne Quelle".to_string(),
        1 => format!("{} · 1 Quelle", item.sources[0].source.label()),
        n => format!("{} · {n} Quellen gemergt", item.sources[0].source.label()),
    };
    if author.is_empty() {
        origin
    } else {
        format!("{author} · {origin}")
    }
}

/// Kuerzel der Quelle fuer enge Stellen wie die Karte im Katalog.
fn short_source(source: hexwerk_sources::Source) -> &'static str {
    match source {
        hexwerk_sources::Source::GitHub => "GitHub",
        hexwerk_sources::Source::Tukui => "Tukui",
        hexwerk_sources::Source::WowInterface => "WoWI",
        hexwerk_sources::Source::Wago => "Wago",
        hexwerk_sources::Source::CurseForge => "CurseForge",
        hexwerk_sources::Source::Local => "lokal",
    }
}

/// Version auf Kartenbreite bringen, ohne die Aussage zu verfaelschen: gekuerzt
/// wird sichtbar mit Auslassungszeichen, der volle Text steht im Tooltip.
fn short_version(version: &str) -> String {
    const MAX: usize = 12;
    if version.chars().count() <= MAX {
        return version.to_string();
    }
    let mut out: String = version.chars().take(MAX - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwerk_index::SourceRelease;
    use hexwerk_sources::Source;

    fn release(source: Source, version: &str) -> SourceRelease {
        SourceRelease {
            source,
            external_id: "1".to_string(),
            version: version.to_string(),
            released_at: None,
            download_url: "https://example.invalid/x.zip".to_string(),
            size_bytes: None,
            interfaces: Vec::new(),
        }
    }

    fn item(author: &str, sources: Vec<SourceRelease>) -> CatalogueItem {
        CatalogueItem {
            id: 1,
            key: "test".to_string(),
            name: "Test".to_string(),
            author: author.to_string(),
            summary: String::new(),
            homepage: None,
            icon_path: None,
            category: None,
            sources,
            latest: None,
            releases: Vec::new(),
            installed: Vec::new(),
        }
    }

    #[test]
    fn herkunft_zaehlt_quellen() {
        assert_eq!(byline(&item("Autor", vec![])), "Autor · ohne Quelle");
        assert_eq!(
            byline(&item("Autor", vec![release(Source::Tukui, "13.88")])),
            "Autor · Tukui · 1 Quelle"
        );
        assert_eq!(
            byline(&item(
                "Autor",
                vec![
                    release(Source::GitHub, "5.19.2"),
                    release(Source::Tukui, "5.19.2")
                ]
            )),
            "Autor · GitHub · 2 Quellen gemergt"
        );
    }

    #[test]
    fn ohne_autor_kein_leitzeichen() {
        assert_eq!(
            byline(&item("  ", vec![release(Source::GitHub, "1.0")])),
            "GitHub · 1 Quelle"
        );
    }
}
