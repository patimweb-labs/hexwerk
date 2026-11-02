# ADR 0002: Oberfläche als Web-App (Tauri 2 + Vue 3)

Status: entschieden · Datum: 2026-11-02

## Zusammenhang

Hexwerk hat eine GTK4/libadwaita-Oberfläche (`crates/ui-gtk`, rund 400 KB Rust).
Die Logik liegt bereits getrennt in `crates/core`, `crates/sources`,
`crates/index` und `crates/install`; die Oberfläche ruft sie über `backend.rs` auf.

## Entscheidung

Die Oberfläche wird mit **Tauri 2 + Vue 3** neu gebaut, so wie bei WLED Ambilight
und Crosspoint. Die GTK-Oberfläche bleibt vorerst im Baum, wird aber nicht mehr
weiterentwickelt und nach dem Umzug entfernt.

Der Schnitt bleibt, wie er ist:

```
core ─┬─ sources ─┐
      ├─ index ───┤
      └─ install ─┴─ desktop (Tauri) ── ui/ (Vue)
```

`crates/desktop` ist die neue Bin: Sie übersetzt Tauri-Befehle in Aufrufe der
vier Logik-Crates. Sie enthält **keine** Domänenlogik — was rechnet, liegt in den
Crates und ist dort getestet.

## Warum

- **Ein Komponentensatz für alle drei Apps.** `weppelmann-ui` liefert Atome,
  Moleküle und Organismen; die Oberfläche muss dieselbe Sprache sprechen.
- **Drei Betriebssysteme.** GTK4 gibt es unter Windows und macOS nicht. Tauri
  nutzt dort WebView2 bzw. WKWebView.
- **Testbarkeit.** Reine TS-Funktionen und Vue-Komponenten lassen sich ohne
  laufendes Fenster prüfen; GTK-Widgets nur mit Aufwand.

## Folgen

- Das Frontend bekommt dieselben Regeln wie die anderen: keine `cfg(target_os)`,
  Plattform als Daten, drei Themes, Tokens statt Hex-Werten (AGENTS.md §2).
- Icons, Desktop-Datei und Metainfo bleiben unverändert — der App-Name und die
  Kennung `de.weppelmann.Hexwerk` sind der Vertrag nach außen.
- Die Flatpak- und CI-Abläufe müssen GTK-Abhängigkeiten verlieren und
  WebKit-Abhängigkeiten bekommen.

## Verworfen

- **GTK behalten und polieren.** Erreicht Windows und macOS nicht.
- **egui.** Wurde bei WLED Ambilight geprüft und für die Oberflächenarbeit
  verworfen (ADR 0001 im UI-Repo).
- **Slint.** Jünger, kleineres Ökosystem, kein geteilter Komponentensatz.
