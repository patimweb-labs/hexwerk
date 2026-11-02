# Änderungen

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

