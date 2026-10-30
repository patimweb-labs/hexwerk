# Hexwerk

Addon-Manager für **World of Warcraft unter Linux**, gebaut für Installationen in
**Steam-Proton-Präfixen**. Alle Client-Versionen: `_retail_`, `_classic_`,
`_classic_era_`, `_anniversary_` und die PTR-Varianten.

Ziel: Addons finden, installieren, aktualisieren und aufräumen, ohne dass der
Nutzer je eine Addon-ID einträgt und ohne dass irgendetwas im Wine-Präfix
mitlaufen muss. Hexwerk arbeitet von außen auf dem Dateisystem des Präfixes.

## Status

Lauffähig als Kommandozeile; die Oberfläche steht und wird gerade an Index und
Quellen angeschlossen. Gebaut und getestet:

| Teil | Stand |
|---|---|
| Präfix-Erkennung (`crates/core`) | Steam, Faugus/UMU, Heroic, Lutris, Bottles, PortProton, manuelle Ordner — 48 Tests |
| Quellen (`crates/sources`) | GitHub, Tukui, WoWInterface ohne Schlüssel; Wago, CurseForge mit Schlüssel — 50 Tests, echter Netztest über `examples/probe` |
| Index (`crates/index`) | SQLite + FTS5 für Clients, Katalog, installierte Addons, Update-Erkennung — 37 Tests |
| Installation (`crates/install`) | ZIP mit Ordnerkorrektur, Sicherung, Papierkorb, Zip-Slip-Schutz — 30 Tests |
| Oberfläche (`crates/ui-gtk`) | fünf Bildschirme, Dunkel/Hell, Scan im Hintergrund; baut gegen GTK 4.22.5 / libadwaita 1.9.4 |
| Marke und Pakete | Logo, Icons 16–512, symbolisches Tray-Icon, Desktop-Eintrag, AppStream-Metainfo, Flatpak-Manifest, CI |

## Bauen und starten

```bash
cargo build --release                        # Kommandozeile
./target/release/hexwerk scan                # Installationen finden
./target/release/hexwerk index               # lokalen Index bauen (ohne Netz)
./target/release/hexwerk index --net bags    # Quellen befragen, Katalog füllen
./target/release/hexwerk search weakauras
./target/release/hexwerk keys test           # Quellen mit Schlüssel prüfen
./target/release/hexwerk keys set curseforge <key>
```

Die Oberfläche braucht GTK4- und libadwaita-Header, die auf Bazzite fehlen.
Gebaut wird deshalb im Container (Image mit `gtk4-devel`, `libadwaita-devel`):

```bash
podman run -d --name hexwerk-build -v "$PWD":/work:z localhost/hexwerk-buildenv:1 sleep infinity
podman exec -w /work hexwerk-build sh -lc 'cd crates/ui-gtk && cargo build'
```


## Quellen — gemessen am 2026-10-30

| Quelle | Endpunkt | Ergebnis |
|---|---|---|
| WowUp Hub (Index) | `GET https://hub.wowup.io/addons/search?query=…` | `200`, JSON, unauthentifiziert, Feld `source` (u. a. `GitHub`) + `repository` |
| WowUp Hub (Fingerprint) | `POST https://hub.wowup.io/addons/fingerprint` `{"fingerprints":[…]}` | `200` → `{"exactMatches":[]}` |
| Tukui | `GET https://api.tukui.org/v1/addons` | `200`, 3.770 Byte Gesamtliste |
| WoWInterface | `GET https://api.mmoui.com/v4/game/WOW/filelist.json` (Fallback `v3`) | `200`, 2.744.891 Byte, mit `UIMD5` und CDN-URL |
| CurseForge | `GET https://api.curseforge.com/v1/games/1` | `403` ohne API-Key |
| Wago | `https://addons.wago.io/api/...` | `302` auf die Website, ohne Token nicht nutzbar |

Damit sind drei Quellen ohne Zugangsdaten nutzbar: **GitHub-Releases**,
**Tukui**, **WoWInterface**. CurseForge scheidet aus, solange kein eigener
API-Key vergeben wird (Keys sind an den einzelnen Entwickler gebunden und nicht
übertragbar).

## Verfahren, das übernommen wird

**Erkennung statt Registrierung.** Installierte Addon-Ordner werden gehasht und
über einen Fingerprint-Index einem Addon zugeordnet. Nur so funktionieren
Updates für Addons, die der Nutzer nie explizit hinzugefügt hat. WowUp macht
das in `wowup-lib/src/addon-providers/wowup-addon-provider.ts`
(`getAddonIdsByFingerprints`) und `addon-fingerprint.service.ts`.

**Externe IDs stehen im `.toc`.** `X-Curse-Project-ID`, `X-WoWI-ID`,
`X-Tukui-Project-ID` — daraus leitet der Manager die Quelle ab, ohne den
Addon-Namen zu raten. WowUp bildet das in `addon-install.service.ts` ab
(`ADDON_PROVIDER_TOC_EXTERNAL_ID_MAP`).

**Installation ist Entpacken mit Ordnerkorrektur.** ZIP holen, oberste
Verzeichnisebene bestimmen, nach `Interface/AddOns/<Ordner>` entpacken,
`__MACOSX` ignorieren, `.toc` neu einlesen für `## Version` und `## Interface`.

**Präfix-Erkennung.** Steam-Bibliotheken aus `libraryfolders.vdf` lesen, dann
`<lib>/steamapps/compatdata/<appid>/pfx/drive_c/Program Files (x86)/World of Warcraft`
prüfen und je Client-Ordner die `## Interface`-Nummer aus dem `.toc` als
Versionsmaßstab nehmen.

## Gemessene Umgebung (2026-10-30, dieser Rechner)

Damit wird entwickelt und getestet — keine Attrappen:

| Ort | Inhalt |
|---|---|
| `~/Faugus/battlenet` | UMU-Präfix von Faugus. Enthält `drive_c/` **und** `pfx/drive_c/` auf derselben Ebene — beide Formen muss der Scanner kennen |
| `.../World of Warcraft/_classic_era_` | 11 Addons, 2,6 GB, Interface `11509 · 20506 · 16001` |
| `.../_classic_beta_` | 1 Addon: `AggroMatrix` 1.9.0, `## X-Website: local` |
| `.../_forever_` | `Interface/AddOns` vorhanden, leer |
| `_retail_` | **nicht installiert** |
| Steam | 1 Bibliothek, 6 `compatdata`-Präfixe, keins mit WoW |
| Heroic | 2 Präfixe (Divinity OS 2, Satisfactory), kein WoW |
| Lutris, Bottles, PortProton | nicht installiert |

Zwei Dinge, die daraus folgen: `## Interface` ist **kein Einzelwert**, sondern kann eine
Komma-Liste sein (`11509, 20506, 16001`) — der Vergleich mit dem Client muss also Menge gegen
Menge prüfen. Und ein UMU-Präfix ist nicht immer `pfx/drive_c/...`; Faugus legt `drive_c`
zusätzlich direkt in den Präfix-Ordner.

## Entwurf

`docs/design/entwurf-2026-10-30.html` — Fensterentwurf mit fünf Bildschirmen (Scan, Clients,
Installiert, Katalog, Addon-Detail). Client-Daten darin stammen aus dem echten Präfix oben,
Katalogwerte sind Platzhalter.

## Risiko

Der Fingerprint-Index gehört WowUp, nicht uns. Genau diese Abhängigkeit hat
Ajour gekostet (archiviert 2024), nachdem Overwolf den CurseForge-Zugang
gestrichen hat. Hexwerk muss ohne den Hub lauffähig bleiben: direkte
Tukui- und WoWInterface-IDs aus dem `.toc`, GitHub-Repository aus dem
`.toc`-Feld `X-Website` bzw. aus dem Release-ZIP, Hub nur als Beschleuniger.

## Name

`Hexwerk` — Hex (deutsch: Hexer, die WoW-Klasse) + Werk. Kommando `hexwerk`,
Flatpak-Ziel `de.weppelmann.Hexwerk`. Frei auf crates.io, PyPI und npm.
