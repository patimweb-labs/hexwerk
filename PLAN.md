# Hexwerk: Umsetzungsplan

Stand 2026-10-30. Grundlage: Fensterentwurf `design/entwurf-2026-10-30.html`, gemessene
Umgebung in `README.md`, Stack-Ablesung aus Crosspoint (`crates/core` + `crates/ui-gtk`,
UI hinter Feature, Flatpak aus vorgebauten Binaries).

## 1. Ziel

Ein Addon-Manager für World of Warcraft unter Linux, der **alle** Präfix-Herkünfte kennt
(Steam/Proton, Faugus/UMU, Heroic, Lutris, Bottles, PortProton, beliebiger Ordner), **alle
Quellen** ohne Zugangsdaten nutzt (GitHub, Tukui, WoWInterface) und die key-pflichtigen
(Wago, CurseForge) vorbereitet, aber abschaltbar hält. Bedienung: GTK4-Fenster, Suche über
einen **lokalen Index**, Katalog aus gemergten Quellen, Installation und Update je Client.

Nicht-Ziele: Windows/macOS, Retail-Spielstart, Addon-Entwicklung, Lua-Ausführung, DRM irgendwas.

## 2. Stack

| Baustein | Wahl | Begründung |
|---|---|---|
| Sprache | Rust 2021, Workspace wie Crosspoint | gleicher Stack, gleicher Zuschnitt |
| Oberfläche | `gtk4 0.9` + `libadwaita 0.7` (`v4_6`) | `gtk4 0.9` = Crosspoint; libadwaita liefert `AdwSpinner`, `AdwToast`, `AdwStyleManager` für Dunkel und Hell, also die gewünschten Animationen ohne Eigenbau |
| HTTP | `ureq` (blockierend, rustls) in eigenem Thread | kein async-Ökosystem nötig; in `std::thread` + Kanal zur UI |
| ZIP | `zip` | Entpacken mit Ordnerkorrektur |
| Index | `rusqlite` (bundled, FTS5) | einprozentiger Suchindex, kein Server |
| Hash | `sha2` | Fingerprint der Addon-Ordner |
| Konfiguration | `serde` + `serde_json` | wie Crosspoint (`macros`) |

Abweichungen von Crosspoint, die der Zweck erzwingt: HTTP-Client, ZIP, SQLite; Crosspoint
braucht nichts davon. Alles andere (Kern als eigenes Paket, UI als eigenes Paket mit eigener
`[workspace]`-Tabelle, UI hinter Feature `ui`, Flatpak aus vorgebauten Binaries) bleibt gleich.

## 3. Crates

```
Cargo.toml                 App-Paket "hexwerk": CLI-Binary + GTK-Binary (Feature ui)
crates/core/               hexwerk-core
  src/clients/             steam, faugus, heroic, lutris, bottles, portproton, generic
  src/toc.rs               .toc lesen (## Interface als Liste!, ## Version, X-* IDs, Notizen je Sprache)
  src/addon.rs             installierte Addons je Client erfassen
  src/flavor.rs            _retail_, _classic_, _classic_era_, _anniversary_, _forever_, PTRs
crates/sources/            hexwerk-sources
  src/github.rs tukui.rs wowinterface.rs wago.rs curseforge.rs
  src/registry.rs          Merge: ein Addon, mehrere Quellen
  src/keys.rs              KeyProvider (siehe §7)
  src/fingerprint.rs       Ordner-Hash + Hub-Abgleich
crates/index/              hexwerk-index
  src/schema.rs queries.rs sync.rs   SQLite/FTS5
crates/install/            hexwerk-install
  src/plan.rs apply.rs zip.rs backup.rs trash.rs
crates/ui-gtk/             hexwerk-ui (eigene [workspace]-Wurzel, GTK4 + libadwaita)
  src/app.rs window.rs scan_view.rs clients_view.rs installed_view.rs catalog_view.rs detail_view.rs theme.rs
data/                      CSS, Icons (hicolor 16…512), Desktop, AppStream-Metainfo
flatpak/                   de.weppelmann.Hexwerk.yml + new-sources.json
```

## 4. Client-Erkennung

Je Herkunft ein eigener Sucher, alle liefern `Client { origin, prefix, game_dir, flavors[] }`.

| Herkunft | Suchorte |
|---|---|
| Steam | `libraryfolders.vdf` → `steamapps/compatdata/*/pfx/drive_c/**/World of Warcraft`, `appmanifest_*.acf` für den Namen |
| Faugus/UMU | `~/Faugus/*`, `~/.config/faugus-launcher/prefixes`, `~/.var/app/io.github.Faugus.faugus-launcher/data`. **Der Präfix hat `drive_c/` und `pfx/drive_c/` auf derselben Ebene** |
| Heroic | `~/Games/Heroic/Prefixes/*`, `~/.var/app/com.heroicgameslauncher.hgl/config/heroic/GamesConfig/*.json` |
| Lutris | `~/Games/*/drive_c`, `~/.var/app/net.lutris.Lutris/data/lutris/games/*.yml` |
| Bottles | `~/.var/app/com.usebottles.bottles/data/bottles/bottles/*/drive_c`, `~/.local/share/bottles/bottles/*` |
| PortProton | `~/.local/share/PortWine/*/drive_c` |
| beliebig | vom Nutzer gewählter Ordner, der `_retail_` o. ä. enthält |

Regeln: nur lesen, nie schreiben beim Suchen; Treffer deduplizieren über `canonicalize` des
`game_dir`. `## Interface` ist eine **Menge** (gemessen: `11509, 20506, 16001`); ein Client
gilt als passend, wenn seine Interface-Nummer in der Menge steht.

## 5. Quellen und Katalog

| Quelle | Zugang | Rolle |
|---|---|---|
| GitHub Releases | ohne Schlüssel | Hauptquelle; Release-Assets + Quell-ZIP, `.toc` wird beim Bau angepasst |
| Tukui | ohne Schlüssel (`api.tukui.org/v1/addons`) | ElvUI-Familie, Gesamtliste 3,7 KB |
| WoWInterface | ohne Schlüssel (`api.mmoui.com/v4`) | 1.204 Addons, MD5 je Datei |
| Wago | Token nötig | vorbereitet, standardmäßig aus |
| CurseForge | API-Key, Vergabe an Einzelentwickler gebunden | vorbereitet, standardmäßig aus |
| WowUp-Hub | offen, kein SLA | nur Fingerprint-Abgleich als Beschleuniger, nicht als Pflicht |

Quellen mit Gesamtbestand (Tukui, WoWInterface) werden **seitenweise** gelesen, bis eine
Seite kürzer als die Seitengröße zurückkommt. Nur Seite 0 zu lesen war der Grund, warum der
Katalog anfangs 57 statt 7.858 Einträge hatte. GitHub bleibt bei wenigen Seiten, weil ohne
Token 60 Anfragen je Stunde erlaubt sind.

**Merge:** Schlüssel eines Katalogeintrags ist zuerst die externe Projekt-ID (aus `.toc`:
`X-WoWI-ID`, `X-Tukui-Project-ID`, `X-Curse-Project-ID`, `X-Wago-ID`), sonst der normalisierte
Name + Autor. Treffer werden zu **einem** Eintrag mit `sources[]`; je Quelle Version, Datum,
Download-URL. Anzeige-Version = höchste, mit Quellenangabe.

Ausfallverhalten: fällt eine Quelle aus, bleibt der Rest nutzbar; der Katalog zeigt den
Zustand je Quelle („Tukui: nicht erreichbar"), nicht einen leeren Bildschirm.

## 6. Indexer

SQLite-Datei `~/.local/share/hexwerk/index.db`, WAL, FTS5 über `addons_fts(name, author, description)`.

```
clients(id, origin, prefix, game_dir, flavor, interface_no, addons_dir, seen_at)
addons(id, key, name, author, summary, homepage, icon_path, updated_at)
addon_sources(addon_id, source, external_id, version, released_at, download_url, size, checked_at)
releases(addon_id, source, version, interface_json, url, sha256, fetched_at)
installed(addon_id, client_id, folder, version, interface_json, fingerprint, foreign_keys_json)
meta(k, v)   -- Schema-Version, letzter Vollsync
```

Ablauf: `scan` (Dateisystem, Sekunden) → `sync` (Quellen, Minuten, im Hintergrund) →
`index` (FTS neu bauen). Die UI fragt nur den Index, nie das Netz. Suche: FTS5 mit
Präfix-Query, Sortierung nach Relevanz, Filter als zusätzliche WHERE-Bedingungen.

## 7. Schlüssel

`KeyProvider` mit fester Reihenfolge: **Umgebungsvariable** → **Konfigdatei**
(`~/.config/hexwerk/keys.json`, Modus 0600) → **Schlüsselbund** (`secret-tool`, optional) →
nichts (Quelle bleibt aus). Kein Schlüssel wird je geloggt, in den Index geschrieben oder in
der UI vollständig angezeigt (nur `…letzte 4`).

Wichtig und ehrlich: **ein mitgelieferter CurseForge-Schlüssel ist laut deren Bedingungen
unzulässig** (Keys sind je Entwickler, nicht übertragbar; genau daran scheiterte WowUp). Wir
bereiten den Schlitz vor, liefern aber keinen fremden Schlüssel. Was automatisiert werden kann:
`hexwerk keys set curseforge <key>` schreibt in die Konfigdatei, `hexwerk keys test` prüft
jede Quelle mit einer Anfrage und meldet den HTTP-Code. Für einen eigenen Schlüssel müsste ein
Antrag bei Overwolf gestellt werden; Wago vergibt Tokens je Anwendung.

## 8. Oberfläche

Fünf Bildschirme wie im Entwurf, plus Einstellungen: **Scan** (AdwSpinner + Fortschritt je
Quelle), **Clients** (Karten je Herkunft, Client-Ordner mit Interface und Addon-Zahl),
**Installiert** (Tabelle je Client, Mehrfachauswahl, Aktionen), **Katalog** (Suche, Filter je
Quelle, gemergte Karten), **Addon-Detail** (Banner, Kennzahlen, Galerie, Tabs Beschreibung /
Screenshots / Changelog / Dateien / Quellen, Seitenleiste mit Ziel-Client, Version, Quellen,
lokal installierter Version).

Theming: eigenes CSS aus den Tokens des Entwurfs (`--bg #060A12`, `--signal #39DD93`,
Monospace, Blueprint-Raster). Zwei Paletten als CSS-Klassen, Umschaltung über
`AdwStyleManager`; Hell = `#F5F7FB` Grund, `#0B7A52` Signal, gleiche Rasterlinien. Schalter in
der Kopfleiste (System / Dunkel / Hell), Auswahl in `~/.config/hexwerk/ui.json`.

## 9. Marke

- **Logo/App-Icon:** „H" als Runen-Glyph im WoW-Icon-Stil: Sechseck-Rahmen, innen das H aus
  gemeißelten Balken mit Fase, Verlauf dunkel → Signalgrün, als SVG mit PNG-Renderings
  16/32/48/64/128/256/512.
- **Tray-Icon:** eigene, vereinfachte 22×22-Fassung (nur Hexagon + H, einfarbig, lesbar bei
  16 px), `de.weppelmann.Hexwerk-symbolic.svg`.
- Desktop-Eintrag, AppStream-Metainfo, `StartupWMClass`.

## 10. Flatpak

`flatpak/de.weppelmann.Hexwerk.yml`, Runtime `org.gnome.Platform//51` (installiert), SDK
`org.gnome.Sdk//51`. Kein Rust im Bau: die Binaries entstehen im CI, das Manifest setzt sie
ein (`prebuilt/`), die Cargo-Quellen stehen als `new-sources.json` zur Prüfsummenkontrolle.
Berechtigungen: `--filesystem=home` (Präfixe liegen im Home), `--share=network` (Quellen),
`--socket=wayland`, `--socket=fallback-x11`, `--device=dri`, `--talk-name=org.freedesktop.secrets`
(Schlüsselbund), `--filesystem=xdg-config/hexwerk:create`.

## 11. CI und Release

`ci.yml`: fmt → clippy (`-D clippy::correctness`) → `cargo test --locked --workspace` →
Release-Build → GTK-Build mit `libgtk-4-dev libadwaita-1-dev` → Metadaten-Validierung
(`desktop-file-validate`, `appstreamcli`) → Flatpak-Bau. `release.yml` + `release-please`
wie Crosspoint. Lizenz GPL-3.0 (wie Crosspoint).

## 12. Arbeitspakete

| # | Paket | Dateien (Eigentum) |
|---|---|---|
| A | Kern: Client-Erkennung, TOC, Addon-Erfassung, Tests | `crates/core/**` |
| B | Quellen: HTTP, GitHub/Tukui/WoWInterface/Wago/CurseForge, Merge, Keys | `crates/sources/**` |
| C | Index: Schema, Sync, Suche, Tests | `crates/index/**` |
| D | Installation: ZIP, Plan, Apply, Backup, Papierkorb | `crates/install/**` |
| E | Oberfläche: 5 Bildschirme, Theming, Spinner | `crates/ui-gtk/**`, `src/bin/hexwerk_gtk.rs`, `data/*.css` |
| F | Marke + Pakete: Icons, Desktop, Metainfo, Flatpak, CI | `data/icons/**`, `packaging/**`, `flatpak/**`, `.github/**` |

Reihenfolge: A liefert die Typen, auf denen B–E aufsetzen; B, C, D laufen parallel; E und F
parallel dazu. Der Rahmen (`Cargo.toml` je Crate, `lib.rs` mit den öffentlichen Typen) wird
**vor** dem Verteilen festgelegt, damit niemand gegen eine wandernde Schnittstelle baut.

## 13. Risiken

1. **GTK4-Header fehlen auf dem Host** (Bazzite, kein `gtk4-devel`). Gebaut und geprüft wird in
   einem Fedora-Container mit `gtk4-devel libadwaita-devel`; die CI macht dasselbe.
2. **CurseForge** ohne Schlüssel nicht erreichbar, deshalb abschaltbar und nie Voraussetzung.
3. **WowUp-Hub** ohne Zusage; Fingerprint nur als Beschleuniger, Fallback ist die `.toc`-ID.
4. **Addon-ZIPs ohne Ordnernamen** (manche Release-Assets entpacken flach): Ordnername kommt
   aus dem `.toc`-Namen, nicht aus dem ZIP.
5. **Multi-Client**: dasselbe Addon mit verschiedenen Builds je Client; Installation immer
   client-scharf, nie „für alle".

## 14. Geplant: Addons leeren (Clear)

Wunsch: mit einem Griff bei null anfangen, ohne im Dateimanager zu wühlen. Das ist die
gefährlichste Aktion im Programm, deshalb mit Rückweg.

Verhalten:

1. Ziel ist immer genau ein Client-Ordner (`_retail_`, `_classic_era_` und so weiter), nie
   das ganze Spielverzeichnis und nie alle Clients auf einmal.
2. Die Ordner wandern in den Papierkorb (`~/.local/share/hexwerk/trash`), nicht in die Tonne,
   und lassen sich mit einem Klick zurückholen. Eine zusätzliche ZIP-Sicherung entfällt
   bewusst: bei einem vollen Classic-Ordner sind das mehrere Gigabyte, eine Sicherung würde
   den Platz verdoppeln. Verschieben auf demselben Datenträger kostet dagegen nichts.
3. Bestätigung im Fenster verlangt das Eintippen des Client-Ordners (etwa `_classic_era_`).
   Ein Klick auf „Ja" ist zu billig für eine Aktion, die 2,6 GB Addons verschiebt.
4. Ergebnis als Bericht: Anzahl Ordner, Größe, Pfad der Sicherung, Pfad des Papierkorbs.
5. Index: Einträge des Clients werden neu erfasst, die Katalogdaten bleiben. `SaveVariables`
   (WTF-Ordner) wird nicht angefasst, weil dort die Einstellungen der Addons liegen.
6. Kommandozeile bekommt dasselbe als `hexwerk clear --client <id> --yes`, damit man es in
   einem Skript benutzen kann, mit derselben Sicherung und demselben Papierkorb.
7. Rückweg: `hexwerk restore <papierkorb-eintrag>` gibt es bereits; ein Eintrag „Rückgängig"
   direkt nach dem Leeren im Fenster ist Teil desselben Arbeitspakets.

Abgrenzung: „Addons leeren" heißt nicht „Spiel reparieren" und nicht „WTF löschen". Wer
wirklich alles wegwerfen will, löscht die Sicherung selbst.
