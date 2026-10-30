# Entwicklung

## Bauumgebung

Der Rechner, auf dem Hexwerk entsteht, ist ein Bazzite 44 (Fedora 44 als
Grundlage). Dort sind GTK4 und libadwaita zur Laufzeit vorhanden, aber nicht die
Entwicklungspakete. Der Kern baut deshalb direkt, die Oberfläche im Container.

```bash
podman build -t hexwerk-buildenv -f packaging/Containerfile .
podman run -d --name hexwerk-build \
  -v "$PWD":/work:z \
  -v "$HOME/.cargo/registry":/root/.cargo/registry:z \
  -w /work hexwerk-buildenv sleep infinity
podman exec -w /work hexwerk-build sh -lc 'cd crates/ui-gtk && cargo build'
```

Im Container liegen GTK 4.22.5 und libadwaita 1.9.4 sowie `Xvfb`, `xdotool`,
`ImageMagick` und `python3-pyatspi` für die Oberflächenprüfung.

## Prüfen

```bash
cargo test --workspace                                   # 165 Tests, ohne GTK
cargo clippy --workspace --all-targets
./target/release/hexwerk scan                            # gegen die echte Installation
podman exec -w /work hexwerk-build sh -lc 'cd crates/ui-gtk && cargo build'
podman exec -w /work hexwerk-build bash tools/ux_check.sh /tmp/ux
```

Der letzte Aufruf startet eine virtuelle Anzeige, die Anwendung, fährt alle
Ansichten durch und prüft dabei die Bedingungen aus `tools/gui_test.py`. Siehe
[`../tools/README.md`](../tools/README.md).

## Testdaten

Zwei Dinge am echten Bestand haben den Entwurf geprägt und stehen deshalb als
Testfälle in `crates/core/tests/`:

* `## Interface` ist kein Einzelwert. Gemessen: `11509, 20506, 16001` in einer
  Datei. Der Vergleich mit einem Client muss Menge gegen Menge prüfen.
* Ein UMU-Präfix ist nicht immer `pfx/drive_c/...`. Faugus legt `drive_c`
  zusätzlich direkt in den Präfix-Ordner.

Für die Entwicklung gegen echte Daten wird ein vorhandenes Präfix
schreibgeschützt in den Container gehängt:

```bash
podman run -d --name hexwerk-build --security-opt label=disable \
  -v "$PWD":/work \
  -v "$HOME/Faugus":/root/Faugus:ro \
  -w /work hexwerk-buildenv sleep infinity
podman exec -w /work -e HOME=/root hexwerk-build bash tools/ux_check.sh /tmp/ux
```

`--security-opt label=disable` statt `:z`, weil ein Relabel das Spielverzeichnis
auf dem Host verändern würde.

## Datenpfad im Flatpak

Im Sandkasten zeigt `XDG_DATA_HOME` nach `~/.var/app/de.weppelmann.Hexwerk/data`. Läge der
Papierkorb dort, wäre er beim Deinstallieren der App weg und die Kommandozeile auf dem Host
fände ihn nicht. Der Papierkorb liegt deshalb gar nicht in der Datenwurzel, sondern nach der
Freedesktop-Spezifikation unter `~/.local/share/Trash`: dort sehen ihn der Dateimanager und
"Papierkorb leeren" des Desktops, und `gio trash --empty` räumt ihn auf. `hexwerk::data_root()` benutzt deshalb im Flatpak bewusst das echte
`~/.local/share/hexwerk`, denselben Ort wie die Kommandozeile. Reihenfolge:
`HEXWERK_DATA_DIR`, dann im Flatpak das echte Home, dann `XDG_DATA_HOME`.

## Verzeichnisse zur Laufzeit

| Zweck | Pfad |
|---|---|
| Index | `~/.local/share/hexwerk/index.db` |
| Sicherungen | `~/.local/share/hexwerk/backup` |
| Papierkorb | `~/.local/share/Trash` (Freedesktop, der des Desktops) |
| Schlüssel | `~/.config/hexwerk/keys.json` |
| Fensterzustand | `~/.config/hexwerk/ui.json` |
| HTTP-Zwischenspeicher | `~/.cache/hexwerk` |
