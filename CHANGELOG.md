# Änderungen

## [0.5.0](https://github.com/patimweb-labs/hexwerk/compare/v0.4.2...v0.5.0) (2026-11-03)


### Features

* **clients:** eigenen Spielordner angeben und wieder entfernen ([#7](https://github.com/patimweb-labs/hexwerk/issues/7)) ([e4771b4](https://github.com/patimweb-labs/hexwerk/commit/e4771b4c0ef3f6d82ce1110b460933f0c6004c72))

## [0.4.2](https://github.com/patimweb-labs/hexwerk/compare/v0.4.1...v0.4.2) (2026-11-02)


### Bug Fixes

* **flatpak:** Binaries wieder aus dem Tarball-Job nehmen ([#3](https://github.com/patimweb-labs/hexwerk/issues/3)) ([4a1f54c](https://github.com/patimweb-labs/hexwerk/commit/4a1f54cfbbf05a0ca83002c6ceac06c314ba961b))
* **icons:** Zeichen aus dem Raster ueber markenwerk erzeugen ([#5](https://github.com/patimweb-labs/hexwerk/issues/5)) ([0ad8005](https://github.com/patimweb-labs/hexwerk/commit/0ad8005fca54a5bc43b24f3df2b566adca1a2dfc))

## 0.4.1 — 2026-11-02

**Behoben**

- Flatpak: Das Manifest baute die eingefrorene GTK-Oberfläche (`crates/ui-gtk`,
  letzter Stand 0.2.2) statt der Tauri-App — deshalb zeigten alle Pakete, auch
  das Release 0.4.0, die alte Oberfläche. Es baut jetzt `crates/desktop` mit
  `--features custom-protocol` und dem Vue-Frontend aus `ui/dist`.
- Die alte GTK-Oberfläche ist entfernt: `crates/ui-gtk`, das Feature `ui`, das
  Binary `hexwerk-gtk` und der zugehörige CI-Job.
- Flatpak-Quellen aus beiden Lockfiles (Kommandozeile und Desktop) erzeugt.

**Dokumentation**

- `AGENTS.md` in allen Weppelmann-Apps, `docs/architektur.md` und
  `docs/datenfluss.md` vereinheitlicht; README mit Installation, Screenshots
  und Abschnitt zu Daten und Datenschutz.

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
