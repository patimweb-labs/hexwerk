//! Bildschirm „Quellen": Zustand jeder Quelle, Verbindungstest, Schlüssel.
//!
//! Zwei Ebenen in einem Stack: eine Übersicht mit einer Karte je Quelle und
//! eine Detailseite. Die Detailseite ist der Ort, an dem ein Schlüssel gesetzt
//! oder entfernt wird; gespeichert wird über `hexwerk_sources::keys`, damit
//! Oberfläche und Kommandozeile dieselbe Datei benutzen.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use adw::prelude::*;
use hexwerk_sources::keys::{KeyOrigin, KeyProvider};
use hexwerk_sources::Source;

/// Was der Bildschirm über eine Quelle weiß.
#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    /// Einträge dieser Quelle im lokalen Index.
    pub entries: usize,
    /// Ergebnis des letzten Tests, falls einer lief.
    pub test: Option<(bool, String)>,
}

type TestCallback = Box<dyn Fn(Source)>;
type KeyCallback = Box<dyn Fn(Source)>;

pub struct SourcesView {
    root: gtk::Stack,
    cards: gtk::Grid,
    /// Detailseite
    detail_title: gtk::Label,
    detail_summary: gtk::Label,
    detail_rows: gtk::Grid,
    detail_state: gtk::Label,
    detail_key: gtk::Button,
    detail_test: gtk::Button,
    current: RefCell<Option<Source>>,
    on_test: RefCell<Option<TestCallback>>,
    on_key: RefCell<Option<KeyCallback>>,
}

const ORDER: [Source; 5] = [
    Source::GitHub,
    Source::Tukui,
    Source::WowInterface,
    Source::Wago,
    Source::CurseForge,
];

impl SourcesView {
    pub fn new() -> Rc<Self> {
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(120);

        // ---- Übersicht ----
        let title = gtk::Label::new(Some("Quellen"));
        title.add_css_class("hw-h1");
        title.set_xalign(0.0);

        let sub = gtk::Label::new(Some(
            "Woher der Katalog kommt. Ohne Schlüssel arbeiten GitHub, Tukui und \
             WoWInterface; Wago und CurseForge brauchen einen.",
        ));
        sub.add_css_class("hw-sub");
        sub.set_xalign(0.0);
        sub.set_wrap(true);

        let cards = gtk::Grid::new();
        cards.set_row_spacing(12);
        cards.set_column_spacing(12);
        cards.set_column_homogeneous(true);
        cards.set_margin_top(14);

        let overview_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        overview_body.append(&title);
        overview_body.append(&sub);
        overview_body.append(&cards);

        let overview = scrolled(&overview_body);
        stack.add_named(&overview, Some("overview"));

        // ---- Detail ----
        let detail_title = gtk::Label::new(Some("Quelle"));
        detail_title.add_css_class("hw-h1");
        detail_title.set_xalign(0.0);

        let detail_summary = gtk::Label::new(Some(""));
        detail_summary.add_css_class("hw-sub");
        detail_summary.set_xalign(0.0);
        detail_summary.set_wrap(true);

        let detail_rows = gtk::Grid::new();
        detail_rows.set_row_spacing(6);
        detail_rows.set_column_spacing(18);
        detail_rows.set_margin_top(16);

        let detail_state = gtk::Label::new(Some(""));
        detail_state.add_css_class("hw-card");
        detail_state.set_xalign(0.0);
        detail_state.set_wrap(true);
        detail_state.set_margin_top(14);

        let detail_test = gtk::Button::with_label("Verbindung testen");
        detail_test.add_css_class("hw-btn");
        crate::theme::pointer(&detail_test);

        let detail_key = gtk::Button::with_label("Schlüssel setzen…");
        detail_key.add_css_class("hw-btn");
        detail_key.add_css_class("ghost");
        crate::theme::pointer(&detail_key);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        actions.set_margin_top(16);
        actions.append(&detail_test);
        actions.append(&detail_key);

        let detail_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail_body.append(&detail_title);
        detail_body.append(&detail_summary);
        detail_body.append(&detail_rows);
        detail_body.append(&detail_state);
        detail_body.append(&actions);

        let detail = scrolled(&detail_body);
        stack.add_named(&detail, Some("detail"));

        let view = Rc::new(Self {
            root: stack,
            cards,
            detail_title,
            detail_summary,
            detail_rows,
            detail_state,
            detail_key,
            detail_test,
            current: RefCell::new(None),
            on_test: RefCell::new(None),
            on_key: RefCell::new(None),
        });
        view.wire();
        view
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Zeigt wieder die Übersicht (beim Wechsel auf den Bildschirm).
    pub fn show_overview(&self) {
        self.root.set_visible_child_name("overview");
        self.root.set_visible_child_name("overview");
    }

    pub fn set_on_test<F: Fn(Source) + 'static>(&self, f: F) {
        *self.on_test.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_key<F: Fn(Source) + 'static>(&self, f: F) {
        *self.on_key.borrow_mut() = Some(Box::new(f));
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.detail_test.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let Some(source) = *view.current.borrow() else {
                return;
            };
            view.detail_state
                .set_text(&format!("{}: Test läuft …", source.label()));
            let cb = view.on_test.borrow_mut().take();
            if let Some(cb) = cb {
                cb(source);
                *view.on_test.borrow_mut() = Some(cb);
            }
        });

        let weak = Rc::downgrade(self);
        self.detail_key.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else { return };
            let Some(source) = *view.current.borrow() else {
                return;
            };
            let cb = view.on_key.borrow_mut().take();
            if let Some(cb) = cb {
                cb(source);
                *view.on_key.borrow_mut() = Some(cb);
            }
        });
    }

    /// Baut Übersicht und Detail neu. `counts` kommt aus dem Index, `keys` aus
    /// der Schlüsselverwaltung, `infos` trägt die Testergebnisse dieser Sitzung.
    pub fn refresh(
        self: &Rc<Self>,
        counts: &BTreeMap<&'static str, usize>,
        keys: &KeyProvider,
        infos: &BTreeMap<Source, SourceInfo>,
    ) {
        while let Some(child) = self.cards.first_child() {
            self.cards.remove(&child);
        }

        let mut row = 0;
        let mut col = 0;
        for source in ORDER {
            let info = infos.get(&source).cloned().unwrap_or_default();
            self.cards
                .attach(&self.card(source, &info, counts, keys), col, row, 1, 1);
            col += 1;
            if col == 2 {
                col = 0;
                row += 1;
            }
        }

        if let Some(source) = *self.current.borrow() {
            self.fill_detail(source, keys, infos);
        }
    }

    fn card(
        self: &Rc<Self>,
        source: Source,
        info: &SourceInfo,
        counts: &BTreeMap<&'static str, usize>,
        keys: &KeyProvider,
    ) -> gtk::Widget {
        let enabled = !source.needs_key() || keys.enabled(source);
        let entries = counts
            .get(hexwerk_index::source_key(source))
            .copied()
            .unwrap_or(0);

        let name = gtk::Label::new(Some(source.label()));
        name.add_css_class("hw-card-title");
        name.set_xalign(0.0);

        let state = gtk::Label::new(Some(if enabled { "aktiv" } else { "abgeschaltet" }));
        state.add_css_class("hw-chip");
        state.add_css_class(if enabled { "sig" } else { "off" });

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&name);
        line.append(&state);

        let detail = gtk::Label::new(Some(&format!(
            "{entries} Einträge im Index{}",
            match &info.test {
                Some((true, text)) => format!(" · Test: {text}"),
                Some((false, text)) => format!(" · Test fehlgeschlagen: {text}"),
                None => String::new(),
            }
        )));
        detail.add_css_class("hw-path");
        detail.set_xalign(0.0);
        detail.set_wrap(true);

        let key_row = gtk::Label::new(Some(&key_text(source, keys)));
        key_row.add_css_class("hw-dim2");
        key_row.set_xalign(0.0);

        let open = gtk::Button::with_label("Details");
        open.add_css_class("hw-btn");
        open.add_css_class("ghost");
        crate::theme::pointer(&open);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.add_css_class("hw-card");
        body.append(&line);
        body.append(&detail);
        body.append(&key_row);
        body.append(&open);

        let weak = Rc::downgrade(self);
        open.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                *view.current.borrow_mut() = Some(source);
                view.root.set_visible_child_name("detail");
            }
        });

        body.upcast()
    }

    fn fill_detail(
        &self,
        source: Source,
        keys: &KeyProvider,
        infos: &BTreeMap<Source, SourceInfo>,
    ) {
        self.detail_title.set_text(source.label());
        self.detail_summary.set_text(&description(source));

        while let Some(child) = self.detail_rows.first_child() {
            self.detail_rows.remove(&child);
        }

        let info = infos.get(&source).cloned().unwrap_or_default();
        for (row, (label, value)) in [
            ("Endpunkt", endpoint(source).to_string()),
            ("Schlüssel", key_text(source, keys)),
            ("Einträge im Index", info.entries.to_string()),
        ]
        .into_iter()
        .enumerate()
        {
            let key = gtk::Label::new(Some(label));
            key.add_css_class("hw-badge");
            key.set_xalign(0.0);
            let val = gtk::Label::new(Some(&value));
            val.add_css_class("hw-card-title");
            val.set_xalign(0.0);
            val.set_selectable(true);
            self.detail_rows.attach(&key, 0, row as i32, 1, 1);
            self.detail_rows.attach(&val, 1, row as i32, 1, 1);
        }

        let state = match &info.test {
            Some((true, text)) => format!("Verbindung steht: {text}"),
            Some((false, text)) => format!("Verbindung fehlgeschlagen: {text}"),
            None => "Noch nicht getestet in dieser Sitzung.".to_string(),
        };
        self.detail_state.set_text(&state);

        let has_key = keys.key(source).is_some();
        self.detail_key.set_label(if has_key {
            "Schlüssel entfernen"
        } else {
            "Schlüssel setzen…"
        });
        self.detail_key.set_visible(source.needs_key());

        // Diese Sicht liest nur den Index, sie fragt nicht selbst ins Netz.
        self.detail_test.set_sensitive(true);
    }

    /// Ergebnis eines Tests eintragen, ohne die ganze Ansicht neu zu bauen.
    pub fn set_test(&self, source: Source, ok: bool, text: &str) {
        if *self.current.borrow() == Some(source) {
            self.detail_state.set_text(&format!(
                "{}: {text}",
                if ok {
                    "Verbindung steht"
                } else {
                    "Fehlgeschlagen"
                }
            ));
        }
    }
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    // Rand hier statt im Kind: so bleibt er beim Scrollen stehen und die
    // Karten kleben nicht am Fensterrand.
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.append(child);
    wrap.set_margin_top(18);
    wrap.set_margin_bottom(18);
    wrap.set_margin_start(24);
    wrap.set_margin_end(24);

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&wrap)
        .build()
}

fn endpoint(source: Source) -> &'static str {
    match source {
        Source::GitHub => "https://api.github.com",
        Source::Tukui => "https://api.tukui.org/v1/addons",
        Source::WowInterface => "https://api.mmoui.com/v4/game/WOW",
        Source::Wago => "https://addons.wago.io/api",
        Source::CurseForge => "https://api.curseforge.com/v1",
        Source::Local => "kein Netz",
    }
}

fn description(source: Source) -> String {
    match source {
        Source::GitHub => {
            "Projekte und ihre Releases. Ohne Token 60 Anfragen je Stunde, deshalb nur wenige \
             Seiten je Lauf."
                .to_string()
        }
        Source::Tukui => {
            "ElvUI und die Tukui-Familie, vollständige Liste ohne Schlüssel.".to_string()
        }
        Source::WowInterface => {
            "Über 7.700 Addons, vollständige Liste ohne Schlüssel, meistgeladen zuerst.".to_string()
        }
        Source::Wago => {
            "Braucht ein Token. Ohne Token bleibt die Quelle abgeschaltet, ohne dass etwas \
             fehlschlägt."
                .to_string()
        }
        Source::CurseForge => {
            "Braucht einen API-Key. CurseForge vergibt Keys an einzelne Entwickler, deshalb muss \
             ein eigener hinterlegt werden."
                .to_string()
        }
        Source::Local => {
            "Addons, die nur im Client liegen und in keiner Quelle stehen.".to_string()
        }
    }
}

fn key_text(source: Source, keys: &KeyProvider) -> String {
    if !source.needs_key() {
        return "nicht nötig".to_string();
    }
    match keys.key_with_origin(source) {
        Some((secret, origin)) => format!(
            "{} ({})",
            secret.masked(),
            match origin {
                KeyOrigin::Environment => "Umgebungsvariable",
                KeyOrigin::File => "keys.json",
                KeyOrigin::Keyring => "Schlüsselbund",
                KeyOrigin::Missing => "nicht gesetzt",
                KeyOrigin::NotNeeded => "nicht nötig",
            }
        ),
        None => "nicht gesetzt".to_string(),
    }
}
