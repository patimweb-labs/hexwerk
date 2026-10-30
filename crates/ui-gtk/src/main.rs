fn main() -> glib::ExitCode {
    // Der Name des Prozesses bestimmt unter X11 die WM_CLASS. Sie muss zur
    // .desktop-Datei passen (StartupWMClass), sonst gruppiert die Taskleiste
    // das Fenster nicht und zeigt statt unserem Zeichen ein Ersatzsymbol.
    glib::set_prgname(Some("de.weppelmann.Hexwerk"));
    hexwerk_ui::run()
}
