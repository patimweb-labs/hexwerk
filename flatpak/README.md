# Flatpak für Hexwerk

`de.weppelmann.Hexwerk.yml` baut Hexwerk gegen `org.gnome.Platform//51` und
`org.gnome.Sdk//51`. Im Sandkasten wird **kein** Rust gebaut: die Binaries
`hexwerk` und `hexwerk-gtk` entstehen in der CI und werden als Datei-Quellen aus
`prebuilt/` eingesetzt. Die Cargo-Abhängigkeiten aus `Cargo.lock` liegen trotzdem
eingefroren in `new-sources.json`, als Prüfsummenkontrolle, damit eine veraltete
Liste auffällt und ein fehlendes Paket den Bau abbricht.

## new-sources.json erzeugen

Das Werkzeug kommt aus `flatpak-builder-tools`. Es braucht Python mit `aiohttp`,
`toml` und (in der aktuellen Fassung) `tomlkit`; ein eigenes venv hält das aus dem
System-Python heraus:

```
python3 -m venv /tmp/venv-fpcg
/tmp/venv-fpcg/bin/pip install aiohttp toml tomlkit
curl -sSfL -o /tmp/flatpak-cargo-generator.py \
    https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
```

Dann im Wurzelverzeichnis des Repos, direkt neben `Cargo.lock`:

```
/tmp/venv-fpcg/bin/python /tmp/flatpak-cargo-generator.py \
    Cargo.lock -o flatpak/new-sources.json
```

Ergebnis prüfen: je eingefrorenem Paket ein Archiv und eine Prüfsummen-Datei:

```
grep -c '"type": "archive"' flatpak/new-sources.json
```

Stand 2026-10-30: 313 Einträge, davon 156 Archive (`cargo/vendor/<name>-<version>`).
Das Skript legt keine Hilfsdateien im Repo ab; es muss nicht mitversioniert werden.

## Wann neu erzeugen

Immer wenn sich `Cargo.lock` ändert, also nach jedem `cargo update`, jedem neuen Paket
und jedem Versionssprung in einem der Crates. Der Zählschritt im Modul
`cargo-quellen` vergleicht die Zahl der eingefrorenen Archive mit den entpackten
Verzeichnissen und bricht bei Abweichung ab.

`crates/ui-gtk/Cargo.lock` wird **nicht** eingefroren: die Oberfläche kommt als
fertiges Binary, nicht als Bau im Sandkasten.

## Bauen

```
mkdir -p prebuilt
cp target/release/hexwerk target/release/hexwerk-gtk prebuilt/
flatpak-builder --force-clean --user --install-deps-from=flathub \
    build-dir flatpak/de.weppelmann.Hexwerk.yml
```

Ohne `flatpak-builder` im System geht derselbe Bau über die Builder-App:

```
flatpak run org.flatpak.Builder --force-clean --user --disable-rofiles-fuse \
    build-dir flatpak/de.weppelmann.Hexwerk.yml
```

## Ohne new-sources.json

Fehlt die Datei, läuft der Bau **nicht** stillschweigend durch. `flatpak-builder`
selbst meldet eine fehlende Quellenliste nicht als Fehler. Deshalb prüft das
Modul `cargo-quellen` die Datei ausdrücklich und bricht mit Hinweis ab:

```
FEHLER: flatpak/new-sources.json fehlt.
Erzeugen mit flatpak-cargo-generator.py aus Cargo.lock - siehe flatpak/README.md.
Error: module cargo-quellen: Der Kindprozess wurde mit Status 1 beendet
```

In der CI ist der Flatpak-Job deshalb an die Existenz der Datei gebunden: fehlt
sie, erscheint er als „skipped" und nicht als Fehler (Job
`flatpak-voraussetzungen`). Im Release fehlt dann das Flatpak-Bundle, der Tarball
wird trotzdem veröffentlicht.

## Stolperstein: Kommentare vor dem Wurzelelement

`appstreamcli compose` (läuft in `flatpak-builder` beim Finish-Schritt) liest das
skalierbare Icon und scheitert an einem XML-Kommentar **zwischen** XML-Prolog und
`<svg>`:

```
de.weppelmann.Hexwerk
  E: icon-file-read-error
general
  E: filters-but-no-output
Error: ERROR: appstreamcli compose failed
```

Der Kommentar gehört deshalb hinter das öffnende `<svg>`-Tag. Ein SVG ohne
führenden Kommentar, aber sonst identisch, wird anstandslos verarbeitet.
