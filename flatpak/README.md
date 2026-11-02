# Flatpak für Hexwerk

`de.weppelmann.Hexwerk.yml` baut Hexwerk gegen `org.gnome.Platform//51` und
`org.gnome.Sdk//51`. Gebaut wird **im Sandkasten**, so wie im Nachbarprojekt
wledambilight: das Rust-Werkzeug kommt aus der freedesktop-Erweiterung, die
Cargo-Quellen liegen eingefrorenen in `new-sources.json`.

Im Sandkasten gemessen (`flatpak run --build-only` mit einem Sondierungsmodul):

| Werkzeug | Version | Herkunft |
|---|---|---|
| cargo | 1.98.0 | `org.freedesktop.Sdk.Extension.rust-stable`, eingebunden über `sdk-extensions` |
| GTK | 4.24.1 | `org.gnome.Sdk//51` |
| libadwaita | 1.10.0 | `org.gnome.Sdk//51` |

Weitere Module braucht das Manifest deshalb nicht, auch kein
`org.gnome.Sdk.Extension.rust-stable` (das gibt es auf Flathub nicht, geprüft:
`flatpak install flathub org.gnome.Sdk.Extension.rust-stable//51` endet mit
„Kein Treffer").

## new-sources.json erzeugen

```bash
bash flatpak/update-sources.sh
```

Das Skript legt sich sein venv an (`/tmp/venv-fpcg`, überschreibbar über
`HEXWERK_VENV`), lädt `flatpak-cargo-generator.py` aus `flatpak-builder-tools`
und schreibt `flatpak/new-sources.json` aus `Cargo.lock`. Die Gegenprobe steht
am Ende der Ausgabe:

```
new-sources.json: 156 Archive, Cargo.lock: 162 Pakete (Differenz sind die eigenen Crates)
```

## Wann neu erzeugen

Immer wenn sich `Cargo.lock` ändert, also nach jedem `cargo update`, jedem neuen
Paket und jedem Versionssprung. Fehlt ein Paket im eingefrorenen Satz, bricht
`cargo --offline` im Sandkasten mit einer klaren Meldung ab; der Bau läuft nicht
mit einer halben Abhängigkeitsliste durch.

`crates/desktop/Cargo.lock` wird nicht eigens eingefroren. Die Oberfläche ist ein
eigenes Paket mit eigener `[workspace]`-Tabelle, ihre Abhängigkeiten sind aber
dieselben wie die der Wurzel und liegen damit im selben Quellensatz. Deshalb
baut der zweite `cargo`-Aufruf im Manifest ohne `--locked`.

## Bauen

Ohne `flatpak-builder` im System geht der Bau über die Builder-App:

```bash
flatpak run org.flatpak.Builder --user --install --force-clean --disable-rofiles-fuse \
    build-dir flatpak/de.weppelmann.Hexwerk.yml
```

Mit `flatpak-builder` im System:

```bash
flatpak-builder --user --install --force-clean \
    build-dir flatpak/de.weppelmann.Hexwerk.yml
```

Der Bau dauert beim ersten Mal einige Minuten, weil GTK-Bindings und SQLite im
Sandkasten kompiliert werden. Danach liegt die App als
`de.weppelmann.Hexwerk` bereit.

## Ergebnis prüfen

```bash
flatpak run --command=hexwerk de.weppelmann.Hexwerk scan
flatpak run --command=sh de.weppelmann.Hexwerk -c 'ldd /app/bin/hexwerk-desktop | grep -c "not found"'
```

Erwartet: der Scan findet die echten Wine-Präfixe (dafür steht
`--filesystem=home` im Manifest), und `ldd` meldet `0` fehlende Bibliotheken.

## Ohne new-sources.json

Fehlt die Datei, bricht `flatpak-builder` beim Auflösen der Quellenliste ab
(„No such file or directory"). In der CI ist der Flatpak-Job zusätzlich an die
Existenz der Datei gebunden: fehlt sie, erscheint er als „skipped" und nicht als
Fehler (Job `flatpak-voraussetzungen`). Im Release fehlt dann das Bundle, der
Tarball wird trotzdem veröffentlicht.

## Stolpersteine, die schon Zeit gekostet haben

**Cargo findet die eingefrorenen Quellen nicht von selbst.** Das Manifest
schreibt deshalb im Bauverzeichnis eine `.cargo/config.toml` mit
`replace-with = "vendored-sources"` und `directory = "cargo/vendor"`. Die Datei
darf **nicht** ins Repo wandern: auf dem Host gibt es kein `cargo/vendor`, der
normale Bau würde brechen.

**Der Flatpak-Sandkasten sieht `/tmp` nicht.** Ein Manifest, das unter `/tmp`
liegt, wird mit „Datei nicht gefunden" abgelehnt, obwohl es existiert. Bauen
deshalb immer aus dem Repo unter `$HOME`.

**`appstreamcli compose` scheitert an einem Kommentar vor dem Wurzelelement.**
Ein XML-Kommentar zwischen XML-Prolog und `<svg>` führt beim Finish-Schritt zu
`E: icon-file-read-error` und `E: filters-but-no-output`. Der Kommentar gehört
hinter das öffnende `<svg>`-Tag.

**Mehrzeilige Einzelwerte in `build-commands` ohne `|` falten den Zeilenumbruch**
zu einem Leerzeichen und lassen den Backslash stehen; `install` legt dann ein
Verzeichnis namens „ " an, und der Eintrag fehlt im App, ohne dass der Bau
scheitert. Mehrzeilige Blöcke deshalb immer als `- |` schreiben.
