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
//! und die Wahl ueberlebt den Neustart in `~/.config/hexwerk/ui.json`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
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
}

impl Theme {
    pub fn new(window: &gtk::Window) -> Rc<Self> {
        let theme = Rc::new(Self {
            window: window.clone(),
            palette: Cell::new(load_palette()),
            menu_label: RefCell::new(None),
            popover: RefCell::new(None),
            choices: RefCell::new(Vec::new()),
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

    /// Auswahl aus dem Menue: merken, anwenden, Datei schreiben.
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

/// Merkdatei `~/.config/hexwerk/ui.json`.
fn config_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("hexwerk").join("ui.json"))
}

pub fn load_palette() -> Palette {
    // Vorgabe ist Dunkel: die Marke ist dunkel, und ein heller Start waere
    // beim ersten Oeffnen ein Bruch mit dem Entwurf. "System" bleibt waehlbar.
    let Some(path) = config_path() else {
        return Palette::Dark;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Palette::Dark;
    };
    parse_theme_value(&text).unwrap_or(Palette::Dark)
}

pub fn save_palette(palette: Palette) -> std::io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Handgeschrieben statt serde: die Datei hat genau einen Schluessel, und
    // ihre Form soll lesbar bleiben, weil Nutzer sie von Hand aendern koennen.
    let text = format!(
        "{{\n  \"theme\": \"{}\",\n  \"hinweis\": \"system | dark | light\"\n}}\n",
        palette.key()
    );
    std::fs::write(path, text)
}

/// Liest den Wert von `"theme"` aus dem JSON. Bewusst kein vollstaendiger
/// JSON-Parser: ein kaputter Wert soll auf „System" zurueckfallen, nicht die
/// Anwendung aufhalten.
fn parse_theme_value(text: &str) -> Option<Palette> {
    let after_key = text.split("\"theme\"").nth(1)?;
    let after_colon = after_key.split(':').nth(1)?;
    let value = after_colon.trim_start().strip_prefix('"')?;
    let end = value.find('"')?;
    Palette::parse(&value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_wert_wird_erkannt() {
        assert_eq!(
            parse_theme_value("{\"theme\": \"dark\"}"),
            Some(Palette::Dark)
        );
        assert_eq!(
            parse_theme_value("{\n \"theme\" : \"light\"\n}"),
            Some(Palette::Light)
        );
        assert_eq!(parse_theme_value("{\"theme\": \"pink\"}"), None);
        assert_eq!(parse_theme_value("{}"), None);
    }
}
