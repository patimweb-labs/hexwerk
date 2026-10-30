//! Bildschirm „Einstellungen": Quellen-Reihenfolge und Sprache.
//!
//! Zwei Dinge, die das Verhalten des ganzen Programms bestimmen und deshalb an
//! einer Stelle stehen:
//!
//! - **Quellen-Reihenfolge**: welche Quelle gewinnt, wenn mehrere dasselbe
//!   Addon fuehren (Version, Kurztext, Bilder). Die Liste ist sortierbar; sie
//!   blendet keine Quelle aus.
//! - **Sprache**: Deutsch, Englisch, Spanisch, Franzoesisch.
//!
//! Ablage: `~/.config/hexwerk/settings.json`. Geschrieben wird direkt beim
//! Klick — kein „Speichern"-Knopf, den man vergessen kann. Fehler beim Schreiben
//! stehen als Text in der Leiste, statt still zu verschwinden.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;

use hexwerk_sources::Source;

/// Was in der Einstellungsdatei steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Gewuenschte Rangfolge der Quellen, wichtigste zuerst.
    pub priority: Vec<Source>,
    /// Sprachkuerzel: `de`, `en`, `es`, `fr`.
    pub language: String,
    /// Leiste: `full` (mit Beschriftung und Zahlen) oder `compact` (nur Symbole).
    pub nav: String,
    /// Schluessel je Quelle (`curseforge`, `wago`) — in derselben Datei wie
    /// alles andere, damit man nicht zwei Orte pflegen muss.
    pub keys: Vec<(String, String)>,
    /// Basisadresse je Quelle; leer heisst: die Vorgabe aus dem Code gilt.
    pub urls: Vec<(String, String)>,
    /// Farbschema: `system`, `dark` oder `light`.
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            priority: vec![Source::CurseForge, Source::WowInterface, Source::Wago],
            language: "de".to_string(),
            nav: "full".to_string(),
            keys: Vec::new(),
            urls: Vec::new(),
            theme: "dark".to_string(),
        }
    }
}

impl Settings {
    /// Liest die Datei; fehlt sie oder ist sie kaputt, gilt die Vorgabe.
    pub fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut out = Self::default();
        if let Some(list) = list_after(&text, "source_priority") {
            let order: Vec<Source> = list.iter().filter_map(|key| source_of(key)).collect();
            if !order.is_empty() {
                out.priority = order;
            }
        }
        out.keys = map_after(&text, "keys");
        out.urls = map_after(&text, "urls");
        if let Some(theme) = string_after(&text, "theme") {
            if matches!(theme.as_str(), "system" | "dark" | "light") {
                out.theme = theme;
            }
        }
        if let Some(nav) = string_after(&text, "nav") {
            if matches!(nav.as_str(), "full" | "compact") {
                out.nav = nav;
            }
        }
        if let Some(lang) = string_after(&text, "language") {
            if matches!(lang.as_str(), "de" | "en" | "es" | "fr") {
                out.language = lang;
            }
        }
        // Nur noch die vorhandenen Quellen: GitHub und Tukui sind entfernt und
        // fliegen aus alten Einstellungsdateien raus, statt als Leiche
        // mitgeschleppt zu werden.
        out.priority.retain(|source| {
            matches!(
                source,
                Source::CurseForge | Source::WowInterface | Source::Wago
            )
        });
        // Fehlende Quellen hinten anhaengen: eine unvollstaendige Liste darf
        // keine Quelle verschlucken.
        for source in Self::default().priority {
            if !out.priority.contains(&source) {
                out.priority.push(source);
            }
        }
        out
    }

    /// Schreibt die Datei (Verzeichnis wird angelegt).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = path() else {
            return Err(std::io::Error::other("kein Konfigurationsverzeichnis"));
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let keys: Vec<String> = self
            .priority
            .iter()
            .map(|source| format!("\"{}\"", source_key(*source)))
            .collect();
        let secrets: Vec<String> = self
            .keys
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(name, value)| format!("\"{name}\": \"{value}\""))
            .collect();
        let urls: Vec<String> = self
            .urls
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(name, value)| format!("\"{name}\": \"{value}\""))
            .collect();
        let text = format!(
            "{{\n  \"source_priority\": [{}],\n  \"language\": \"{}\",\n  \"nav\": \"{}\",\n  \"theme\": \"{}\",\n  \"keys\": {{{}}},\n  \"urls\": {{{}}}\n}}\n",
            keys.join(", "),
            self.language,
            self.nav,
            self.theme,
            secrets.join(", "),
            urls.join(", ")
        );
        std::fs::write(&path, text)
    }
}

/// `~/.config/hexwerk/settings.json`.
pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("hexwerk").join("settings.json"))
}

/// Kennung einer Quelle, wie sie in der Datei steht.
pub fn source_key(source: Source) -> &'static str {
    match source {
        Source::CurseForge => "curseforge",
        Source::WowInterface => "wowi",
        Source::Tukui => "tukui",
        Source::GitHub => "github",
        Source::Wago => "wago",
        Source::Local => "local",
    }
}

/// Rueckweg; unbekannte Kennungen ergeben `None`.
pub fn source_of(key: &str) -> Option<Source> {
    match key.trim().to_ascii_lowercase().as_str() {
        "curseforge" => Some(Source::CurseForge),
        "wowinterface" | "wowi" => Some(Source::WowInterface),
        "tukui" => Some(Source::Tukui),
        "github" => Some(Source::GitHub),
        "wago" => Some(Source::Wago),
        "local" | "lokal" => Some(Source::Local),
        _ => None,
    }
}

/// Liste von Zeichenketten hinter einem Schluessel: `"key": ["a", "b"]`.
fn list_after(text: &str, key: &str) -> Option<Vec<String>> {
    let at = text.find(&format!("\"{key}\""))?;
    let rest = &text[at..];
    let open = rest.find('[')?;
    let close = rest[open..].find(']')? + open;
    let items: Vec<String> = rest[open + 1..close]
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

/// Abbildung hinter einem Schluessel: `"keys": { "a": "1", "b": "2" }`.
fn map_after(text: &str, key: &str) -> Vec<(String, String)> {
    let Some(at) = text.find(&format!("\"{key}\"")) else {
        return Vec::new();
    };
    let rest = &text[at..];
    let (Some(open), Some(close)) = (
        rest.find('{'),
        rest.find('}')
            .filter(|end| *end > rest.find('{').unwrap_or(0)),
    ) else {
        return Vec::new();
    };
    rest[open + 1..close]
        .split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, ':');
            let name = parts.next()?.trim().trim_matches('"').trim().to_string();
            let value = parts.next()?.trim().trim_matches('"').trim().to_string();
            if name.is_empty() {
                None
            } else {
                Some((name, value))
            }
        })
        .collect()
}

/// Zeichenkette hinter einem Schluessel: `"key": "wert"`.
fn string_after(text: &str, key: &str) -> Option<String> {
    let at = text.find(&format!("\"{key}\""))?;
    let rest = &text[at + key.len() + 2..];
    let open = rest.find('"')?;
    let tail = &rest[open + 1..];
    let close = tail.find('"')?;
    Some(tail[..close].to_string())
}

/// Setzt einen Eintrag in einer Paarliste: ersetzen, anhaengen oder entfernen.
/// Bewusst keine Abbildung, weil die Einstellungsdatei die Reihenfolge haelt.
fn update(list: &mut Vec<(String, String)>, name: &str, value: String) {
    list.retain(|(key, _)| key != name);
    if !value.is_empty() {
        list.push((name.to_string(), value));
    }
}

type ChangeCallback = Box<dyn Fn(Settings)>;

pub struct SettingsView {
    root: gtk::ScrolledWindow,
    /// Zuletzt gemeldete Quellen; Grundlage fuer jeden Neuaufbau der Tabelle.
    last_rows: RefCell<Vec<(Source, String, String, String)>>,
    language_box: gtk::Box,
    /// Liste der Quellen: je Quelle eine Zeile mit Zustand, Menge und Schluessel.
    sources_list: gtk::Box,
    /// Platz fuer den Farbschema-Umschalter (kommt vom Theme).
    appearance_slot: gtk::Box,
    /// Schalter „kompakte Leiste" (nur Symbole).
    nav_switch: gtk::Switch,
    status: gtk::Label,
    path_label: gtk::Label,
    settings: RefCell<Settings>,
    on_change: RefCell<Option<ChangeCallback>>,
    /// Wird nach jedem gespeicherten Schritt mit einer Meldung gerufen
    /// (Rueckmeldung als Toast, wie beim Papierkorb).
    on_saved: RefCell<Option<Box<dyn Fn(&str)>>>,
    /// Texte mit Schluessel, damit ein Sprachwechsel sie nachziehen kann.
    /// Ohne diese Liste bleibt die Seite in der Sprache stehen, in der sie
    /// gebaut wurde.
    labels: RefCell<Vec<(gtk::Label, &'static str)>>,
}

impl SettingsView {
    pub fn new() -> Rc<Self> {
        let settings = Settings::load();
        let lang = settings.language.clone();

        let title = gtk::Label::new(Some(crate::lang::t(&lang, "set.title")));
        title.add_css_class("hw-h1");
        title.set_xalign(0.0);

        let sub = gtk::Label::new(Some(crate::lang::t(&lang, "set.hint.priority")));
        sub.add_css_class("hw-sub");
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_max_width_chars(90);
        sub.set_margin_bottom(16);

        let language_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        language_box.set_margin_bottom(16);

        // Die Quellen gehoeren hierher, nicht in einen eigenen Reiter: es sind
        // Einstellungen (Schluessel, Verbindung), keine Ansicht der Sammlung.
        // Als Liste, nicht als Kartenraster: im Einstellungsreiter soll man
        // Zustand und Schluessel je Quelle in einer Zeile lesen koennen.
        let sources_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sources_list.add_css_class("hw-card");
        sources_list.set_margin_bottom(16);

        // Der Umschalter fuer das Farbschema gehoert hierher (und bleibt
        // zusaetzlich in der Kopfzeile, wo man ihn schnell erreicht).
        let appearance_slot = gtk::Box::new(gtk::Orientation::Vertical, 8);
        appearance_slot.set_margin_bottom(16);

        // Katalogdienst: Adresse und Schluessel. Die Quellen selbst sind aus
        // der Oberflaeche verschwunden, weil der Katalog fertig vom Dienst
        // kommt. Was hier steht, ist die Verbindung zu diesem Dienst.
        let catalog_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        catalog_box.add_css_class("hw-card");
        catalog_box.set_margin_bottom(16);

        let url_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let url_label = gtk::Label::new(Some(crate::lang::t(&lang, "set.catalog.url")));
        url_label.add_css_class("hw-cell");
        url_label.set_xalign(0.0);
        url_label.set_width_chars(10);
        let url_entry = gtk::Entry::new();
        url_entry.set_hexpand(true);
        url_entry.set_placeholder_text(Some("https://hexwerk-api.weppelmann.eu"));
        url_entry.set_text(
            &settings
                .urls
                .iter()
                .find(|(name, _)| name == "catalog")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "https://hexwerk-api.weppelmann.eu".to_string()),
        );
        url_row.append(&url_label);
        url_row.append(&url_entry);

        let key_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let key_label = gtk::Label::new(Some(crate::lang::t(&lang, "set.catalog.key")));
        key_label.add_css_class("hw-cell");
        key_label.set_xalign(0.0);
        key_label.set_width_chars(10);
        let key_entry = gtk::PasswordEntry::new();
        key_entry.set_hexpand(true);
        key_entry.set_show_peek_icon(true);
        key_entry.set_text(
            &settings
                .keys
                .iter()
                .find(|(name, _)| name == "catalog")
                .map(|(_, value)| value.clone())
                .unwrap_or_default(),
        );
        key_row.append(&key_label);
        key_row.append(&key_entry);

        let catalog_hint = gtk::Label::new(Some(crate::lang::t(&lang, "set.catalog.hint")));
        catalog_hint.add_css_class("hw-dim2");
        catalog_hint.set_xalign(0.0);
        catalog_hint.set_wrap(true);
        catalog_hint.set_max_width_chars(90);

        catalog_box.append(&url_row);
        catalog_box.append(&key_row);
        catalog_box.append(&catalog_hint);

        // Kompakte Leiste: nur Symbole, Statistik weg. Der Schalter steht bei
        // „Aussehen", weil er die Form der Oberflaeche aendert, nicht ihren Inhalt.
        let nav_switch = gtk::Switch::new();
        nav_switch.set_active(settings.nav == "compact");
        nav_switch.set_valign(gtk::Align::Center);
        nav_switch.set_tooltip_text(Some(
            "Schmale Leiste mit reinen Symbolen; Zahlen und Abschnitte verschwinden",
        ));
        crate::theme::pointer(&nav_switch);

        let nav_label = gtk::Label::new(Some(crate::lang::t(&lang, "set.nav.compact")));
        nav_label.add_css_class("hw-cell");

        let nav_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        nav_row.append(&nav_switch);
        nav_row.append(&nav_label);

        let status = gtk::Label::new(Some(""));
        status.add_css_class("hw-dim2");
        status.set_xalign(0.0);
        status.set_margin_bottom(8);

        let path_label = gtk::Label::new(Some(""));
        path_label.add_css_class("hw-path");
        path_label.set_xalign(0.0);
        path_label.set_selectable(true);
        path_label.set_wrap(true);

        let mut tracked: Vec<(gtk::Label, &'static str)> = Vec::new();
        tracked.push((title.clone(), "set.title"));
        tracked.push((sub.clone(), "set.hint.priority"));
        tracked.push((nav_label.clone(), "set.nav.compact"));
        tracked.push((url_label.clone(), "set.catalog.url"));
        tracked.push((key_label.clone(), "set.catalog.key"));
        tracked.push((catalog_hint.clone(), "set.catalog.hint"));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_margin_top(20);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(&title);
        body.append(&sub);
        // Die Quellen sind aus der Oberflaeche verschwunden: der Katalog kommt
        // fertig vom Dienst, die Herkunft ist fuer die Bedienung ohne Belang.
        for key in ["set.language", "set.appearance", "set.catalog", "set.file"] {
            let label = section(crate::lang::t(&lang, key));
            tracked.push((label.clone(), key));
            body.append(&label);
            match key {
                "set.language" => body.append(&language_box),
                "set.appearance" => {
                    body.append(&appearance_slot);
                    body.append(&nav_row);
                }
                "set.catalog" => body.append(&catalog_box),
                _ => body.append(&path_label),
            }
        }

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_width(false)
            .vexpand(true)
            .child(&body)
            .build();
        root.add_css_class("hw-pane");

        let view = Rc::new(Self {
            root,
            last_rows: RefCell::new(Vec::new()),
            language_box,
            sources_list,
            appearance_slot,
            nav_switch,
            status,
            path_label,
            settings: RefCell::new(settings),
            on_change: RefCell::new(None),
            on_saved: RefCell::new(None),
            labels: RefCell::new(tracked),
        });
        view.rebuild();
        {
            let weak = Rc::downgrade(&view);
            let url_entry_save = url_entry.clone();
            let key_entry_save = key_entry.clone();
            let store = move || {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                let mut settings = view.settings.borrow_mut();
                let url = url_entry_save.text().trim().to_string();
                let key = key_entry_save.text().trim().to_string();
                update(&mut settings.urls, "catalog", url);
                update(&mut settings.keys, "catalog", key);
                drop(settings);
                view.persist();
            };
            let store_a = store.clone();
            url_entry.connect_activate(move |_| store_a());
            let store_b = store.clone();
            key_entry.connect_activate(move |_| store_b());
            let store_c = store.clone();
            let focus_url = gtk::EventControllerFocus::new();
            focus_url.connect_leave(move |_| store_c());
            url_entry.add_controller(focus_url);

            let store_d = store.clone();
            let focus_key = gtk::EventControllerFocus::new();
            focus_key.connect_leave(move |_| store_d());
            key_entry.add_controller(focus_key);
        }
        {
            let weak = Rc::downgrade(&view);
            view.nav_switch.connect_active_notify(move |switch| {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                let compact = switch.is_active();
                // Nur reagieren, wenn sich wirklich etwas aendert: `rebuild`
                // setzt den Schalter selbst und wuerde sonst erneut speichern.
                if (view.settings.borrow().nav == "compact") == compact {
                    return;
                }
                view.settings.borrow_mut().nav = if compact {
                    "compact".into()
                } else {
                    "full".into()
                };
                view.persist();
            });
        }
        view
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Haengt den Farbschema-Umschalter ein.
    pub fn attach_appearance(&self, widget: &impl IsA<gtk::Widget>) {
        if self.appearance_slot.first_child().is_some() {
            return;
        }
        self.appearance_slot.append(widget);
    }

    /// Setzt die Quellenliste als **Tabelle mit Akkordeon**.
    ///
    /// Vier feste Spalten (Name, Zustand, Menge, Schlüssel), damit die Zeilen
    /// untereinander stehen wie in einer Tabelle. Aufklappen zeigt die
    /// Einstellungen der Quelle: API-Schlüssel und Basis-Adresse.
    pub fn set_sources(self: &Rc<Self>, rows: &[(Source, String, String, String)]) {
        *self.last_rows.borrow_mut() = rows.to_vec();
        self.render_sources();
    }

    /// Zeichnet die Tabelle in der Reihenfolge der Rangfolge.
    ///
    /// Reihenfolge und Quelle stehen damit in **einer** Liste: Zeile 1 oben ist
    /// die wichtigste Quelle. Der Griff links verschiebt sie.
    fn render_sources(self: &Rc<Self>) {
        while let Some(child) = self.sources_list.first_child() {
            self.sources_list.remove(&child);
        }
        let rows: Vec<(Source, String, String, String)> = {
            let stored = self.last_rows.borrow().clone();
            let priority = self.settings.borrow().priority.clone();
            let mut ordered = Vec::with_capacity(stored.len());
            for source in priority {
                if let Some(row) = stored.iter().find(|(known, _, _, _)| *known == source) {
                    ordered.push(row.clone());
                }
            }
            for row in stored {
                if !ordered.iter().any(|(known, _, _, _)| *known == row.0) {
                    ordered.push(row);
                }
            }
            ordered
        };
        if rows.is_empty() {
            let lang = self.settings.borrow().language.clone();
            let hint = gtk::Label::new(Some(crate::lang::t(&lang, "set.empty")));
            hint.add_css_class("hw-dim2");
            hint.set_xalign(0.0);
            hint.set_margin_top(6);
            hint.set_margin_bottom(6);
            hint.set_margin_start(10);
            self.sources_list.append(&hint);
            return;
        }

        // Gemeinsame Spaltenbreiten: an jeder Zeile dieselben WNumbers, sonst
        // „fliegen" die Werte und die Liste ist keine Tabelle.
        const NAME_W: i32 = 190;
        const STATE_W: i32 = 110;
        const COUNT_W: i32 = 140;
        const KEY_W: i32 = 130;

        for (index, (source, count, key, state)) in rows.iter().enumerate() {
            let rank = gtk::Label::new(Some(&format!("{}", index + 1)));
            rank.add_css_class("hw-nav-count");
            rank.set_xalign(0.0);
            rank.set_size_request(18, -1);

            let grip = grip_widget();
            grip.add_css_class("hw-grip");
            grip.set_tooltip_text(Some("Ziehen ändert die Rangfolge dieser Quelle"));
            let drag = gtk::DragSource::new();
            drag.set_actions(gtk::gdk::DragAction::MOVE);
            let payload = source_key(*source).to_string();
            drag.connect_prepare(move |_, _, _| {
                Some(gtk::gdk::ContentProvider::for_value(&payload.to_value()))
            });
            grip.add_controller(drag);

            let name = gtk::Label::new(Some(source.label()));
            name.add_css_class("hw-cell");
            name.set_xalign(0.0);
            name.set_size_request(NAME_W, -1);

            let state_label = gtk::Label::new(Some(state));
            state_label.add_css_class("hw-chip");
            state_label.add_css_class(if state == "aktiv" { "sig" } else { "off" });
            state_label.set_size_request(STATE_W, -1);
            state_label.set_halign(gtk::Align::Start);

            let count_label = gtk::Label::new(Some(count));
            count_label.add_css_class("hw-cell-dim");
            count_label.set_xalign(1.0);
            count_label.set_size_request(COUNT_W, -1);

            // Der Text kommt bereits uebersetzt aus `refresh_settings_sources`;
            // eine zweite Zuordnung hier wuerde ihn wieder eindeutschen.
            let key_label = gtk::Label::new(Some(key));
            key_label.add_css_class("hw-dim2");
            key_label.set_xalign(1.0);
            key_label.set_size_request(KEY_W, -1);

            let arrow = gtk::Image::from_icon_name("pan-end-symbolic");
            arrow.add_css_class("hw-dim2");
            arrow.set_pixel_size(14);

            let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            head.add_css_class("hw-setting-row");
            head.set_hexpand(true);
            head.append(&rank);
            head.append(&grip);
            head.append(&name);
            head.append(&state_label);
            head.append(&count_label);
            head.append(&key_label);
            // Abstandhalter: der Pfeil gehoert an den rechten Rand, nicht
            // direkt hinter den letzten Wert.
            let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            gap.set_hexpand(true);
            head.append(&gap);
            arrow.set_halign(gtk::Align::End);
            head.append(&arrow);

            // Aufklappbarer Teil: Schluessel und Adresse dieser Quelle.
            let detail = gtk::Box::new(gtk::Orientation::Vertical, 8);
            detail.add_css_class("hw-source-detail");
            detail.set_margin_start(10);
            detail.set_margin_top(8);
            detail.set_margin_bottom(10);
            detail.set_visible(false);

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("API-Schlüssel …"));
            entry.set_visibility(false);
            entry.set_input_purpose(gtk::InputPurpose::Password);
            entry.set_hexpand(true);

            let save = gtk::Button::with_label("Schlüssel speichern");
            save.add_css_class("hw-btn");
            crate::theme::pointer(&save);

            let entry_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let entry_label = gtk::Label::new(Some("Schlüssel"));
            entry_label.add_css_class("hw-srcrow-name");
            entry_label.set_xalign(0.0);
            entry_label.set_size_request(KEY_W, -1);
            entry_row.append(&entry_label);
            entry_row.append(&entry);

            let hint = gtk::Label::new(Some(
                "Wird in derselben Einstellungsdatei abgelegt wie alles andere. Leer lassen und speichern schaltet die Quelle ab.",
            ));
            hint.add_css_class("hw-dim2");
            hint.set_xalign(0.0);
            hint.set_wrap(true);
            hint.set_max_width_chars(80);

            let status = gtk::Label::new(Some(""));
            status.add_css_class("hw-dim2");
            status.set_xalign(0.0);
            status.set_wrap(true);

            let source_value = *source;
            {
                let status = status.clone();
                let entry = entry.clone();
                let weak_save = Rc::downgrade(self);
                save.connect_clicked(move |_| {
                    let value = entry.text().to_string();
                    let name = source_key(source_value).to_string();
                    let Some(view) = weak_save.upgrade() else {
                        return;
                    };
                    let result = {
                        let mut settings = view.settings.borrow_mut();
                        settings.keys.retain(|(known, _)| *known != name);
                        if !value.trim().is_empty() {
                            settings.keys.push((name.clone(), value.trim().to_string()));
                        }
                        settings.save().map(|_| {
                            if value.trim().is_empty() {
                                "Schlüssel entfernt, die Quelle ist abgeschaltet".to_string()
                            } else {
                                "Schlüssel gespeichert, gilt ab dem nächsten Lauf".to_string()
                            }
                        })
                    };
                    status.set_text(&match result {
                        Ok(text) => text,
                        Err(err) => format!("nicht gespeichert: {err}"),
                    });
                    entry.set_text("");
                });
            }

            detail.append(&entry_row);
            detail.append(&save);
            detail.append(&hint);
            detail.append(&status);

            let drop = gtk::DropTarget::new(glib::types::Type::STRING, gtk::gdk::DragAction::MOVE);
            let weak_drop = Rc::downgrade(self);
            let target_source = *source;
            drop.connect_drop(move |_, value, _, y| {
                let Ok(key) = value.get::<String>() else {
                    return false;
                };
                let Some(moved) = source_of(&key) else {
                    return false;
                };
                let Some(view) = weak_drop.upgrade() else {
                    return false;
                };
                let after = y >= 20.0;
                view.move_near(moved, target_source, after);
                true
            });
            head.add_controller(drop);

            // Tastaturweg: Zeile fokussieren, Strg+Pfeil verschiebt.
            head.set_focusable(true);
            let key = gtk::EventControllerKey::new();
            let weak_key = Rc::downgrade(self);
            let moved_source = *source;
            key.connect_key_pressed(move |_, keyval, _, modifiers| {
                let step: i32 = match keyval {
                    gtk::gdk::Key::Up
                        if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) =>
                    {
                        -1
                    }
                    gtk::gdk::Key::Down
                        if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) =>
                    {
                        1
                    }
                    _ => return glib::Propagation::Proceed,
                };
                let Some(view) = weak_key.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                view.move_source(moved_source, step);
                glib::Propagation::Stop
            });
            head.add_controller(key);

            let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
            group.append(&head);
            group.append(&detail);

            // Auf-/zuklappen ueber die ganze Kopfzeile.
            let gesture = gtk::GestureClick::new();
            let detail_toggle = detail.clone();
            let arrow_toggle = arrow.clone();
            gesture.connect_released(move |_, _, _, _| {
                let open = !detail_toggle.is_visible();
                detail_toggle.set_visible(open);
                arrow_toggle.set_icon_name(Some(if open {
                    "pan-down-symbolic"
                } else {
                    "pan-end-symbolic"
                }));
            });
            head.add_controller(gesture);
            head.set_cursor_from_name(Some("pointer"));
            head.set_tooltip_text(Some("Aufklappen: Schlüssel setzen"));

            self.sources_list.append(&group);
        }
    }

    /// Zieht alle Texte der Seite auf eine Sprache nach.
    ///
    /// Wird beim Sprachwechsel gerufen: sonst bleibt die Seite in der Sprache
    /// stehen, in der sie gebaut wurde, waehrend Kopfzeile und Leiste schon
    /// umgestellt sind.
    pub fn retranslate(&self, lang: &str) {
        for (label, key) in self.labels.borrow().iter() {
            label.set_text(crate::lang::t(lang, key));
        }
    }

    /// Rueckmeldung nach dem Speichern (Toast im Hauptfenster).
    pub fn set_on_saved<F: Fn(&str) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_saved.borrow_mut() = Some(Box::new(f));
    }

    /// Wird nach jeder Aenderung gerufen (Sprache, Rangfolge).
    pub fn set_on_change<F: Fn(Settings) + 'static>(self: &Rc<Self>, f: F) {
        *self.on_change.borrow_mut() = Some(Box::new(f));
    }

    /// Was gerade eingestellt ist.
    pub fn settings(&self) -> Settings {
        self.settings.borrow().clone()
    }

    /// Liest die Datei neu ein — noetig, wenn sie von aussen geaendert wurde
    /// (Editor, Skript) und der Reiter wieder geoeffnet wird.
    pub fn reload(self: &Rc<Self>) {
        *self.settings.borrow_mut() = Settings::load();
        self.rebuild();
    }

    fn rebuild(self: &Rc<Self>) {
        // ---- Sprache ----
        while let Some(child) = self.language_box.first_child() {
            self.language_box.remove(&child);
        }
        let current = self.settings.borrow().language.clone();
        for (code, label) in [
            ("de", "Deutsch"),
            ("en", "English"),
            ("es", "Español"),
            ("fr", "Français"),
        ] {
            let button = gtk::ToggleButton::with_label(label);
            button.add_css_class("hw-chip");
            button.set_active(current == code);
            if current == code {
                button.add_css_class("sig");
            }
            button.set_tooltip_text(Some(&format!("Sprache der Oberfläche: {label}")));
            crate::theme::pointer(&button);
            let weak = Rc::downgrade(self);
            let chosen = code.to_string();
            button.connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                let Some(view) = weak.upgrade() else {
                    return;
                };
                view.set_language(&chosen);
            });
            self.language_box.append(&button);
        }

        let compact = self.settings.borrow().nav == "compact";
        if self.nav_switch.is_active() != compact {
            self.nav_switch.set_active(compact);
        }
        self.path_label.set_text(
            &path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "kein Konfigurationsverzeichnis gefunden".to_string()),
        );
    }

    /// Setzt `moved` vor oder hinter `target` (Drag & Drop) und speichert.
    fn move_near(self: &Rc<Self>, moved: Source, target: Source, after: bool) {
        if moved == target {
            return;
        }
        {
            let mut settings = self.settings.borrow_mut();
            let Some(from) = settings.priority.iter().position(|known| *known == moved) else {
                return;
            };
            let removed = settings.priority.remove(from);
            let at = settings
                .priority
                .iter()
                .position(|known| *known == target)
                .unwrap_or(settings.priority.len());
            let to = if after { at + 1 } else { at };
            let limit = settings.priority.len();
            settings.priority.insert(to.min(limit), removed);
        }
        self.persist();
        self.render_sources();
    }

    /// Verschiebt eine Quelle in der Rangfolge und speichert (Tastaturweg).
    fn move_source(self: &Rc<Self>, source: Source, step: i32) {
        {
            let mut settings = self.settings.borrow_mut();
            let Some(at) = settings.priority.iter().position(|known| *known == source) else {
                return;
            };
            let target = at as i32 + step;
            if target < 0 || target as usize >= settings.priority.len() {
                return;
            }
            settings.priority.swap(at, target as usize);
        }
        self.persist();
        self.render_sources();
    }

    fn set_language(self: &Rc<Self>, code: &str) {
        self.settings.borrow_mut().language = code.to_string();
        self.persist();
        self.rebuild();
    }

    /// Schreibt die Datei und meldet das Ergebnis in der Leiste.
    fn persist(self: &Rc<Self>) {
        self.persist_with(None);
    }

    /// Speichert und meldet. `note` ist der Text fuer die Rueckmeldung; ohne
    /// Angabe steht dort „Einstellungen gespeichert".
    ///
    /// Die Rueckmeldung geht als Toast ins Hauptfenster, genau wie beim
    /// Papierkorb: eine Handlung, eine Meldung, an derselben Stelle.
    fn persist_with(self: &Rc<Self>, note: Option<&str>) {
        let settings = self.settings.borrow().clone();
        let result = settings.save();
        let lang = self.settings.borrow().language.clone();
        let message = match &result {
            Ok(()) => note
                .map(str::to_string)
                .unwrap_or_else(|| crate::lang::t(&lang, "set.saved").to_string()),
            Err(err) => format!("{}: {err}", crate::lang::t(&lang, "set.notsaved")),
        };
        self.status.set_text(&message);
        let saved = self.on_saved.borrow().as_ref().map(|f| f(&message));
        let _ = saved;
        let callback = self.on_change.borrow_mut().take();
        if let Some(callback) = callback {
            callback(settings);
            *self.on_change.borrow_mut() = Some(callback);
        }
    }
}

/// Griff zum Ziehen: drei waagerechte Striche.
///
/// WARUM selbst gezeichnet: `open-menu-symbolic` ist in Adwaita ein
/// senkrechtes Punktetrio (Burger-Menue) und liest sich als „Menue", nicht als
/// „anfassen und ziehen". Drei waagerechte Linien sind die verbreitete
/// Zieh-Marke und haengen an keinem Icon-Satz.
fn grip_widget() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(20);
    area.set_content_height(20);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(|_, cr, width, height| {
        let color = gtk::gdk::RGBA::new(0.62, 0.66, 0.72, 0.95);
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.95,
        );
        cr.set_line_width(1.8);
        let left = 3.0;
        let right = (width as f64 - 3.0).max(left + 1.0);
        for row in 0..3 {
            let y = height as f64 / 2.0 - 5.0 + row as f64 * 5.0;
            cr.move_to(left, y);
            cr.line_to(right, y);
        }
        let _ = cr.stroke();
    });
    area
}

/// Einheitliche Schreibweise der Schluesselspalte.
///
/// Vorher stand dort mal ein Dateiname (`keys.json`), mal `nicht nötig`, mal
/// `kein Schlüssel` — drei Vokabulare in einer Spalte. Jetzt drei feste Werte:
/// `gesetzt`, `nicht gesetzt`, `—` (Quelle braucht keinen Schluessel).

fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("hw-section");
    label.set_xalign(0.0);
    label.set_margin_bottom(6);
    label
}
