//! Bildschirm „Scan": was Hexwerk gerade durchsucht und was dabei herauskam.
//!
//! Der Kern liefert je Herkunft genau eine [`Note`]; mehr Zwischenstand gibt es
//! nicht, und einen Fortschritt zu erfinden waere eine Luege. Deshalb: Spinner
//! und ein unbestimmter Balken waehrend des Laufs, danach eine Zeile je
//! Herkunft mit dem, was der Kern tatsaechlich gemeldet hat.

use std::cell::RefCell;
use std::time::Duration;

use adw::prelude::*;

use crate::state::{note_mark, ScanData};

pub struct ScanView {
    root: gtk::ScrolledWindow,
    mark_stack: gtk::Stack,
    rows: gtk::Box,
    bar: gtk::ProgressBar,
    status: gtk::Label,
    rescan: gtk::Button,
    /// Der Puls-Timer laeuft nur, solange gescannt wird. Ohne dieses Feld
    /// wuerde er nach dem Scan weiter alle 100 ms aufwachen.
    pulse: RefCell<Option<glib::SourceId>>,
}

impl ScanView {
    pub fn new() -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_halign(gtk::Align::Center);
        content.set_valign(gtk::Align::Start);
        content.set_width_request(720);
        content.set_margin_top(48);
        content.set_margin_bottom(32);
        content.set_margin_start(24);
        content.set_margin_end(24);

        // Spinner und Haken liegen uebereinander; sichtbar ist immer nur eins.
        let spinner = adw::Spinner::new();
        spinner.set_size_request(56, 56);
        spinner.add_css_class("hw-spinner");

        let done = gtk::Label::new(Some("✓"));
        done.add_css_class("hw-glyph");
        done.set_size_request(40, 40);
        done.set_halign(gtk::Align::Center);
        done.set_valign(gtk::Align::Center);

        let mark_stack = gtk::Stack::new();
        mark_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        mark_stack.set_halign(gtk::Align::Center);
        mark_stack.add_named(&spinner, Some("spinner"));
        mark_stack.add_named(&done, Some("done"));
        mark_stack.set_visible_child_name("spinner");

        let headline = gtk::Label::new(Some("Suche nach Clients"));
        headline.add_css_class("hw-h1");
        headline.set_margin_top(18);
        headline.set_halign(gtk::Align::Center);

        let lead = gtk::Label::new(Some(
            "Hexwerk liest Wine-Präfixe von Steam, Faugus, Heroic, Lutris, Bottles und PortProton. \
             Nichts wird gestartet, nichts geschrieben.",
        ));
        lead.add_css_class("hw-lead");
        lead.set_wrap(true);
        lead.set_justify(gtk::Justification::Center);
        lead.set_max_width_chars(88);
        lead.set_margin_top(6);
        lead.set_margin_bottom(22);

        let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let bar = gtk::ProgressBar::new();
        bar.add_css_class("hw-bar");
        bar.set_show_text(false);
        bar.set_margin_top(18);
        bar.set_pulse_step(0.06);

        let status = gtk::Label::new(Some("bereit"));
        status.add_css_class("hw-dim2");
        status.set_halign(gtk::Align::Start);

        let rescan = gtk::Button::with_label("Erneut scannen");
        rescan.add_css_class("hw-btn");
        rescan.add_css_class("ghost");
        rescan.set_halign(gtk::Align::End);
        crate::theme::pointer(&rescan);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        footer.set_margin_top(12);
        footer.append(&status);
        footer.append(&rescan);
        status.set_hexpand(true);

        content.append(&mark_stack);
        content.append(&headline);
        content.append(&lead);
        content.append(&rows);
        content.append(&bar);
        content.append(&footer);

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&content)
            .build();
        root.add_css_class("hw-pane");

        Self {
            root,
            mark_stack,
            rows,
            bar,
            status,
            rescan,
            pulse: RefCell::new(None),
        }
    }

    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub fn connect_rescan<F: Fn() + 'static>(&self, f: F) {
        self.rescan.connect_clicked(move |_| f());
    }

    /// Zustand „laeuft": Zeilen weg, Spinner an, Balken pulsiert.
    pub fn set_running(&self, running: bool) {
        if running {
            self.clear_rows();
            self.mark_stack.set_visible_child_name("spinner");
            self.bar.set_visible(true);
            self.status.set_text("Herkünfte werden geprüft …");
            self.rescan.set_sensitive(false);
            self.start_pulse();
        } else {
            self.stop_pulse();
            self.bar.set_visible(false);
            self.rescan.set_sensitive(true);
            self.mark_stack.set_visible_child_name("done");
        }
    }

    /// Ergebnis eines Laufs. Die Zeilen kommen aus `Discovery.notes`, also
    /// eins zu eins aus dem Kern.
    pub fn show(&self, data: &ScanData) {
        self.clear_rows();
        for note in &data.notes {
            let (mark, tone) = note_mark(note.status);
            self.rows.append(&row(mark, tone, &note.step, &note.detail));
        }
        self.status.set_text(&format!(
            "{} Herkünfte befragt · {} Clients · {} Addons",
            data.notes.len(),
            data.clients.len(),
            data.addon_count()
        ));
    }

    fn clear_rows(&self) {
        while let Some(child) = self.rows.first_child() {
            self.rows.remove(&child);
        }
    }

    fn start_pulse(&self) {
        self.stop_pulse();
        let bar = self.bar.clone();
        let id = glib::timeout_add_local(Duration::from_millis(110), move || {
            bar.pulse();
            glib::ControlFlow::Continue
        });
        *self.pulse.borrow_mut() = Some(id);
    }

    fn stop_pulse(&self) {
        if let Some(id) = self.pulse.borrow_mut().take() {
            id.remove();
        }
    }
}

/// Eine Zeile: Marke, Schrittname, Detail rechts.
fn row(mark: &str, tone: &str, step: &str, detail: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.add_css_class("hw-note-row");

    let mark_label = gtk::Label::new(Some(mark));
    mark_label.set_width_request(16);
    mark_label.set_xalign(0.5);
    mark_label.add_css_class(tone);

    let name = gtk::Label::new(Some(step));
    name.set_hexpand(true);
    name.set_xalign(0.0);
    name.add_css_class("hw-ink");

    let note = gtk::Label::new(Some(detail));
    note.set_xalign(1.0);
    note.set_wrap(true);
    note.set_max_width_chars(46);
    note.add_css_class("hw-path");

    line.append(&mark_label);
    line.append(&name);
    line.append(&note);
    line
}

impl Default for ScanView {
    fn default() -> Self {
        Self::new()
    }
}
