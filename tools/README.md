# Werkzeuge für die Oberflächenprüfung

Die Frage war: gibt es für GTK so etwas wie Playwright, damit die Oberfläche
automatisch geprüft wird, statt sie von Hand durchzuklicken. Hier steht, was es
gibt, was davon gemessen wurde und was dieses Projekt benutzt.

## Was es gibt

| Weg | Was er kann | Stand hier |
|---|---|---|
| AT-SPI über `pyatspi` (Selenium-Analogie) | Widget-Baum lesen, Elemente über Rolle und Namen finden, echte Klicks und Tastatureingaben auslösen, Zustände wie `enabled` und `visible` prüfen | benutzt, siehe `ux_check.py` und `gui_test.py` |
| `dogtail` | dasselbe auf einer bequemeren API, ist die bekannte GTK-Testbibliothek | nicht in den Fedora-Paketquellen (`dnf list dogtail`: keine Treffer); `pyatspi` genügt |
| Broadway-Backend + Playwright | GTK zeichnet in einen HTML-Canvas, der Browser zeigt das Fenster; Playwright kann dann klicken und fotografieren | Backend vorhanden (`gtk4-broadwayd`, `HTTP 200` auf Port 8085 gemessen). Nur Pixel und Koordinaten, keine DOM-Auswahl, deshalb hier nicht der Hauptweg |
| GTK-eigene Zusicherungen (`gtk::test_accessible_*`) | Rollen und Zustände eigener Widgets in Rust-Tests prüfen | im Crate `gtk4 0.9.7` vorhanden (`src/accessible.rs`), sinnvoll für Widget-Tests, nicht für Abläufe |
| Bildvergleich | Screenshot gegen Referenz, Abweichung melden | offen, sinnvoll sobald die Oberfläche steht |

Kurz: Playwright versteht GTK nicht, weil GTK keine DOM-Knoten hat. Die
Schnittstelle, die dasselbe leistet, ist die Barrierefreiheit, und die spricht
jedes GTK-Programm von sich aus.

## Benutzung

Alles im Baucontainer, weil auf dem Host die GTK-Header fehlen:

```bash
podman exec -w /work hexwerk-build bash tools/ux_check.sh /tmp/ux
```

Das Skript startet eine virtuelle Anzeige (`Xvfb`, 1440x900), startet die
Anwendung in einer eigenen dbus-Sitzung, fährt den Rundgang (`ux_check.py`)
und danach die Prüfungen (`gui_test.py`). Rückgabewert ungleich 0 heißt: eine
Prüfung ist gescheitert.

Ergebnisse:

| Datei | Inhalt |
|---|---|
| `/tmp/ux/bestand.txt` | jedes bedienbare Element mit Rolle, Name und Bildschirmkoordinaten |
| `/tmp/ux/*.png` | Bild je Schritt (Ansichten, Suche, Filter, Detail, Themenmenü) |
| `/tmp/ux/app.log` | Protokoll der Anwendung, wird auf `panic` und `CRITICAL` geprüft |

## Was `gui_test.py` prüft

1. Fenster vorhanden und mindestens 1000x700 groß.
2. Jede Ansicht (Clients, Installiert, Katalog) lässt sich öffnen und zeigt danach andere Texte.
3. Das Suchfeld nimmt Text an und die Trefferliste reagiert.
4. Quellenfilter sind bedienbar.
5. Kein Bedienelement ist niedriger als 24 px (Trefferfläche).
6. Im Stylesheet steht keine Schriftgröße unter 12 px und `cursor: pointer` ist gesetzt.
7. Das Anwendungsprotokoll enthält keinen Absturz.

Was es nicht prüft und auch nicht prüfen kann: ob die Oberfläche gut aussieht.
Dafür bleiben die Bilder, und die schaut ein Mensch an.
