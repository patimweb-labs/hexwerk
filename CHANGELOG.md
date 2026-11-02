# Änderungen

## 0.4.0 — 2026-11-02

**Neu**

- Windows und macOS: World of Warcraft wird in `%ProgramFiles%`/`%ProgramFiles(x86)%`
  und in CrossOver- bzw. Whisky-Flaschen gefunden, dazu die native
  macOS-Installation in `/Applications`.
- Einstellungen: „Jetzt synchronisieren" und „Katalog leeren" (mit Rückfrage;
  der gepackte Snapshot im Cache geht mit).
- Der Katalog wird beim Start im Hintergrund ergänzt — vorher fehlte der Aufruf,
  nachdem der Knopf aus der Katalogansicht entfernt worden war.
- Die Nyan-Cat aus der GTK-Fassung ist zurück: sie läuft in der Fußzeile,
  solange Scan, Katalog oder Installation arbeiten.

**Besser**

- Pakete für Windows (Portable-ZIP und Inno-Setup-Installer) und macOS
  (Universal-DMG, arm64 + x86_64).

## 0.3.0 — 2026-11-02

Die Oberfläche ist neu gebaut: statt GTK jetzt Tauri 2 mit Vue 3, dieselbe
Fachlogik darunter. Sie teilt sich die Bausteine mit wledambilight und
crosspoint (`weppelmann-ui`).

**Neu**

- Detailseite: Bildergalerie mit Blättern, Reiter für Beschreibung, Dateien,
  Screenshots und Quellen, Autor, Kategorie, Homepage, Interface.
- Installationsstand je Spielordner: die Seite fragt im gewählten Ziel nach,
  statt dem Index zu glauben; liegt die neueste Fassung dort, ist der Knopf
  gesperrt.
- Deinstallieren in den Papierkorb, mit Rückfrage und Rückgängig.
- Vier Sprachen (de, en, es, fr) und drei Farbschemata (dunkel, hell, mono).
- Katalogfilter als Kästchen: mehrere Quellen und Interface-Nummern gleichzeitig.
- Lokal installierte Addons ohne Katalogeintrag haben eine eigene Detailansicht.
- Doppelklick in der Liste öffnet die Details.

**Besser**

- Bilder werden erst beim Öffnen eines Eintrags geholt, nicht beim Kataloglauf.
- Der Scan schreibt die gefundenen Clients in den Index — vorher wusste die
  Detailseite nichts von Installationen.
- Fenster schließt die Aufnahme, statt im Hintergrund weiterzulaufen.

