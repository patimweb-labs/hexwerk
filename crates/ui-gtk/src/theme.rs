//! Farben: CSS laden, Palette umschalten, Wahl merken.
//!
//! Zwei Dinge sind hier absichtlich getrennt:
//!
//! * `AdwStyleManager` entscheidet, ob libadwaita selbst hell oder dunkel
//!   zeichnet (Knopfleisten, Popover, Standard-Widgets). Das haengt an der
//!   Systemeinstellung und laesst sich je Anwendung ueberschreiben.
//! * Unsere Palette haengt als Klasse `hw-dark` / `hw-light` am Fenster und
//!   faerbt die eigenen Flaechen. Sie folgt dem StyleManager, solange der
//!   Nutzer „System" gewaehlt hat.
//!
//! Beides zusammen ergibt: Adwaita-Teile und eigene Teile kippen gemeinsam um,
//! und die Wahl ueberlebt den Neustart in `~/.config/hexwerk/settings.json`
//! (zusammen mit Rangfolge, Sprache, Leistenform, Schluesseln und Adressen).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;

/// Das Stylesheet wird einkompiliert. Warum nicht als Datei zur Laufzeit? Weil
/// das Fenster auch dann richtig aussieht, wenn das Paket ohne `share/`
/// installiert wurde - und weil ein fehlendes CSS sonst nur ein stummes,
/// haessliches Fenster ergibt.
pub const CSS: &str = include_str!("../../../data/theme.css");

const CSS_CLASS_DARK: &str = "hw-dark";
const CSS_CLASS_LIGHT: &str = "hw-light";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    System,
    Dark,
    Light,
}

impl Palette {
    pub fn label(self) -> &'static str {
        match self {
            Palette::System => "System",
            Palette::Dark => "Dunkel",
            Palette::Light => "Hell",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Palette::System => "system",
            Palette::Dark => "dark",
            Palette::Light => "light",
        }
    }

    /// Rueckweg von [`Palette::key`]; unbekannte Werte ergeben `None`.
    pub fn from_key(value: &str) -> Option<Self> {
        Self::parse(value.trim())
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Palette::System),
            "dark" => Some(Palette::Dark),
            "light" => Some(Palette::Light),
            _ => None,
        }
    }

    /// Reihenfolge im Umschalter.
    pub const ALL: [Palette; 3] = [Palette::System, Palette::Dark, Palette::Light];
}

/// Zeigefinger auf klickbaren Widgets.
///
/// WARUM im Code und nicht im Stylesheet: GTK4 hat das CSS-Attribut `cursor`
/// abgeschafft — der Theme-Parser meldet „No property named cursor“. Der Zeiger
/// wird deshalb je Widget gesetzt; diese Funktion ist die eine Stelle, damit
/// „klickbar“ ueberall gleich aussieht.
pub fn pointer(widget: &impl IsA<gtk::Widget>) {
    widget.set_cursor_from_name(Some("pointer"));
}

/// Sonne bei hellem, Mond bei dunklem Schema.
fn icon_for(dark: bool) -> &'static str {
    if dark {
        "weather-clear-night-symbolic"
    } else {
        "weather-clear-symbolic"
    }
}

/// Provider einmal pro Anzeige registrieren. Prioritaet APPLICATION: das
/// Adwaita-Theme liegt darunter, Nutzer-CSS (~/.config/gtk-4.0) darueber.
pub fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Haelt Fenster, Wahl und die Bedienelemente des Umschalters zusammen.
pub struct Theme {
    window: gtk::Window,
    palette: Cell<Palette>,
    /// Schwache Verweise: der Umschalter gehoert dem Fenster, nicht dem Theme.
    /// Ein starker Verweis waere ein Kreis (Fenster -> Knopf -> Theme -> Fenster).
    menu_label: RefCell<Option<glib::WeakRef<gtk::Label>>>,
    popover: RefCell<Option<glib::WeakRef<gtk::Popover>>>,
    choices: RefCell<Vec<(Palette, glib::WeakRef<gtk::Button>)>>,
    /// Alle Sonne/Mond-Knoepfe: sie muessen den Zustand mitziehen, egal wo
    /// umgeschaltet wurde (Kopfzeile oder Einstellungen).
    toggles: RefCell<Vec<glib::WeakRef<gtk::ToggleButton>>>,
    /// Die Schalter aus den Einstellungen: sie muessen ebenfalls mitziehen,
    /// wenn in der Kopfzeile umgeschaltet wird.
    switches: RefCell<Vec<glib::WeakRef<gtk::Switch>>>,
}

impl Theme {
    pub fn new(window: &gtk::Window) -> Rc<Self> {
        let theme = Rc::new(Self {
            window: window.clone(),
            palette: Cell::new(load_palette()),
            menu_label: RefCell::new(None),
            popover: RefCell::new(None),
            choices: RefCell::new(Vec::new()),
            toggles: RefCell::new(Vec::new()),
            switches: RefCell::new(Vec::new()),
        });

        // Wechselt das System (oder der Nutzer in den GNOME-Einstellungen), muss
        // die eigene Palette nachziehen - aber nur im Modus „System".
        let weak = Rc::downgrade(&theme);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(theme) = weak.upgrade() {
                if theme.palette.get() == Palette::System {
                    theme.apply();
                }
            }
        });

        theme.set_style_scheme(theme.palette.get());
        theme.apply();
        theme
    }

    pub fn current(&self) -> Palette {
        self.palette.get()
    }

    /// Sonne/Mond als ein Knopf: Klick wechselt zwischen hell und dunkel.
    ///
    /// Warum kein Menue mehr: drei Eintraege (System, dunkel, hell) fuer eine
    /// Entscheidung, die fast immer „das andere von jetzt" heisst. Das Icon
    /// zeigt den **aktuellen** Zustand (Sonne = hell, Mond = dunkel), der
    /// Tooltip sagt, was der Klick bewirkt.
    pub fn dark_toggle(self: &Rc<Self>) -> gtk::ToggleButton {
        let button = gtk::ToggleButton::new();
        button.add_css_class("hw-choice");
        button.set_active(self.is_dark());
        button.set_tooltip_text(Some(if self.is_dark() {
            "Dunkel — Klick schaltet auf hell"
        } else {
            "Hell — Klick schaltet auf dunkel"
        }));

        let icon = gtk::Image::from_icon_name(icon_for(self.is_dark()));
        button.set_child(Some(&icon));
        pointer(&button);

        let weak = Rc::downgrade(self);
        let icon_click = icon.clone();
        button.connect_toggled(move |button| {
            let Some(theme) = weak.upgrade() else {
                return;
            };
            let wanted = if button.is_active() {
                Palette::Dark
            } else {
                Palette::Light
            };
            icon_click.set_icon_name(Some(icon_for(button.is_active())));
            button.set_tooltip_text(Some(if button.is_active() {
                "Dunkel — Klick schaltet auf hell"
            } else {
                "Hell — Klick schaltet auf dunkel"
            }));
            if theme.current() != wanted {
                theme.choose(wanted);
            }
        });

        self.toggles.borrow_mut().push(button.downgrade());
        button
    }

    /// Ein Schalter statt Menue: „Dunkel" an oder aus.
    ///
    /// `System` bleibt als gespeicherter Wert bestehen (wer nichts waehlt,
    /// bekommt die Systemeinstellung); der Schalter zeigt und setzt nur
    /// dunkel/hell. Beim Umschalten wird daraus eine feste Wahl — ein Schalter,
    /// der nichts bewirkt, weil „System" gewinnt, waere die schlechtere Loesung.
    pub fn dark_switch(self: &Rc<Self>) -> gtk::Box {
        let switch = gtk::Switch::new();
        switch.set_active(self.current() != Palette::Light);
        switch.set_valign(gtk::Align::Center);
        switch.set_tooltip_text(Some("Dunkles Farbschema (aus = hell)"));
        pointer(&switch);

        let weak = Rc::downgrade(self);
        switch.connect_active_notify(move |switch| {
            let Some(theme) = weak.upgrade() else {
                return;
            };
            let wanted = if switch.is_active() {
                Palette::Dark
            } else {
                Palette::Light
            };
            if theme.current() != wanted {
                theme.choose(wanted);
            }
        });

        let lang = crate::settings_view::Settings::load().language;
        let label = gtk::Label::new(Some(crate::lang::t(&lang, "set.dark")));
        label.add_css_class("hw-cell");

        self.switches.borrow_mut().push(switch.downgrade());

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.append(&switch);
        row.append(&label);
        row
    }

    /// Auswahl aus dem Menue oder vom Schalter: merken, anwenden, schreiben.
    pub fn choose(&self, palette: Palette) {
        self.palette.set(palette);
        self.set_style_scheme(palette);
        self.apply();
        if let Err(err) = save_palette(palette) {
            // Kein Grund, den Nutzer mit einem Dialog zu stoeren: die Wahl gilt
            // fuer diese Sitzung, nur das Merken ist gescheitert.
            eprintln!("hexwerk: ui.json nicht schreibbar: {err}");
        }
        self.refresh_menu();
        self.sync_widgets();
    }

    /// Zieht alle Sonne/Mond-Knoepfe und Schalter auf den aktuellen Zustand.
    ///
    /// Wird nach jeder Wahl gerufen **und** beim Oeffnen der Einstellungen:
    /// so kann kein Widget einen alten Zustand anzeigen, auch wenn eine
    /// Aenderung an ihm vorbeigelaufen ist.
    pub fn sync_widgets(&self) {
        let dark = self.is_dark();
        self.toggles.borrow_mut().retain(|weak| {
            let Some(button) = weak.upgrade() else {
                return false;
            };
            if button.is_active() != dark {
                button.set_active(dark);
            }
            if let Some(child) = button.child() {
                if let Some(icon) = child.downcast_ref::<gtk::Image>() {
                    icon.set_icon_name(Some(icon_for(dark)));
                }
            }
            true
        });
        self.switches.borrow_mut().retain(|weak| {
            let Some(switch) = weak.upgrade() else {
                return false;
            };
            if switch.is_active() != dark {
                switch.set_active(dark);
            }
            true
        });
    }

    fn set_style_scheme(&self, palette: Palette) {
        let scheme = match palette {
            Palette::System => adw::ColorScheme::Default,
            Palette::Dark => adw::ColorScheme::ForceDark,
            Palette::Light => adw::ColorScheme::ForceLight,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
    }

    fn is_dark(&self) -> bool {
        match self.palette.get() {
            Palette::Dark => true,
            Palette::Light => false,
            Palette::System => adw::StyleManager::default().is_dark(),
        }
    }

    fn apply(&self) {
        let dark = self.is_dark();
        self.window.remove_css_class(CSS_CLASS_DARK);
        self.window.remove_css_class(CSS_CLASS_LIGHT);
        self.window.add_css_class(if dark {
            CSS_CLASS_DARK
        } else {
            CSS_CLASS_LIGHT
        });

        // Popover haengen in einer eigenen Oberflaeche und erben die Klasse des
        // Fensters nicht - sie brauchen sie selbst.
        if let Some(popover) = self.popover.borrow().as_ref().and_then(|w| w.upgrade()) {
            popover.remove_css_class(CSS_CLASS_DARK);
            popover.remove_css_class(CSS_CLASS_LIGHT);
            popover.add_css_class(if dark {
                CSS_CLASS_DARK
            } else {
                CSS_CLASS_LIGHT
            });
        }
        self.refresh_menu();
    }

    fn refresh_menu(&self) {
        let current = self.palette.get();
        if let Some(label) = self.menu_label.borrow().as_ref().and_then(|w| w.upgrade()) {
            label.set_text(current.label());
        }
        for (palette, weak) in self.choices.borrow().iter() {
            if let Some(button) = weak.upgrade() {
                button.remove_css_class("on");
                if *palette == current {
                    button.add_css_class("on");
                }
            }
        }
    }

    /// Der Umschalter fuer die Kopfleiste: aktueller Name, Popover mit drei
    /// Zeilen. Kein eigenes Widget-Typ - drei Knoepfe reichen.
    pub fn switcher(self: &Rc<Self>) -> gtk::MenuButton {
        let label = gtk::Label::new(Some(self.palette.get().label()));
        label.add_css_class("hw-dim");
        let icon = gtk::Image::from_icon_name("display-brightness-symbolic");
        icon.add_css_class("hw-dim");

        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.append(&icon);
        content.append(&label);

        let button = gtk::MenuButton::new();
        button.set_child(Some(&content));
        button.add_css_class("hw-choice");
        button.set_tooltip_text(Some("Farbschema: System, dunkel oder hell"));
        pointer(&button);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&list));
        popover.set_has_arrow(false);
        popover.add_css_class("hw-popover");
        button.set_popover(Some(&popover));

        for palette in Palette::ALL {
            let entry = gtk::Button::with_label(palette.label());
            entry.add_css_class("hw-choice");
            entry.set_halign(gtk::Align::Fill);
            pointer(&entry);
            if let Some(child) = entry.child() {
                if let Some(label) = child.downcast_ref::<gtk::Label>() {
                    label.set_xalign(0.0);
                }
            }
            let theme = Rc::downgrade(self);
            entry.connect_clicked(move |_| {
                if let Some(theme) = theme.upgrade() {
                    theme.choose(palette);
                    if let Some(popover) = theme.popover.borrow().as_ref().and_then(|w| w.upgrade())
                    {
                        popover.popdown();
                    }
                }
            });
            list.append(&entry);
            let weak = glib::WeakRef::new();
            weak.set(Some(&entry));
            self.choices.borrow_mut().push((palette, weak));
        }

        let label_weak = glib::WeakRef::new();
        label_weak.set(Some(&label));
        *self.menu_label.borrow_mut() = Some(label_weak);

        let popover_weak = glib::WeakRef::new();
        popover_weak.set(Some(&popover));
        *self.popover.borrow_mut() = Some(popover_weak);
        self.refresh_menu();
        button
    }
}

pub fn load_palette() -> Palette {
    // Alles in EINER Datei: `~/.config/hexwerk/settings.json`. Vorgabe ist
    // Dunkel (Marke), „System" bleibt waehlbar.
    let value = crate::settings_view::Settings::load().theme;
    Palette::from_key(&value).unwrap_or(Palette::Dark)
}

pub fn save_palette(palette: Palette) -> std::io::Result<()> {
    // Nicht die Datei ueberschreiben, sondern die Einstellungen laden, das
    // Theme ersetzen und alles zusammen schreiben — sonst waeren Rangfolge,
    // Sprache, Schluessel und Adressen nach einem Themewechsel weg.
    let mut settings = crate::settings_view::Settings::load();
    settings.theme = palette.key().to_string();
    settings.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farbschema_wird_erkannt() {
        // Der Wert steht jetzt in `settings.json`; gelesen wird ueber
        // `Palette::from_key`, damit „system", „dark" und „light" gelten.
        assert_eq!(Palette::from_key("dark"), Some(Palette::Dark));
        assert_eq!(Palette::from_key(" light "), Some(Palette::Light));
        assert_eq!(Palette::from_key("system"), Some(Palette::System));
        assert_eq!(Palette::from_key("pink"), None);
    }
}
