# Quellen

Gemessen am 2026-10-30. Diese Datei hält fest, welche Schnittstellen Hexwerk
befragt, was sie zurückgeben und was daraus folgt. Wer eine Quelle ergänzt,
trägt hier die Messung ein, nicht die Vermutung.

## Endpunkte

| Quelle | Endpunkt | Ergebnis der Messung |
|---|---|---|
| GitHub | `GET https://api.github.com/search/repositories?q=…` und `/repos/<owner>/<repo>/releases/latest` | `200`, ohne Token 60 Anfragen je Stunde |
| Tukui | `GET https://api.tukui.org/v1/addons` | `200`, 3.770 Byte Gesamtliste, Felder `id`, `slug`, `name`, `author`, `version`, `url`, `small_desc`, `last_update` |
| WoWInterface | `GET https://api.mmoui.com/v4/game/WOW/filelist.json`, Einzelabruf `v3/game/WOW/filedetails/<id>.json` | `200`, 2.744.891 Byte; die Liste trägt `id`, `title`, `author`, `lastUpdate`, `checksum`, aber keine Download-Adresse, die kommt aus dem Einzelabruf |
| Wago | `https://addons.wago.io/api/...` | `302` auf die Website ohne Token; mit Token nicht gemessen |
| CurseForge | `GET https://api.curseforge.com/v1/mods/search` mit Header `x-api-key` | `403` ohne Schlüssel; Antwortform nicht gemessen |
| WowUp Hub | `GET https://hub.wowup.io/addons/search?query=…` | `200`, JSON, ohne Authentifizierung, Feld `source` (etwa `GitHub`) und `repository` |
| WowUp Hub Fingerprint | `POST https://hub.wowup.io/addons/fingerprint` mit `{"fingerprints":[…]}` | `200`, Antwort `{"exactMatches":[]}` |

Damit sind drei Quellen ohne Zugangsdaten nutzbar: GitHub, Tukui und
WoWInterface. Wago und CurseForge sind eingebaut, aber abgeschaltet, solange
kein Schlüssel vorliegt.

## Was von den etablierten Managern übernommen wurde

**Erkennung statt Registrierung.** Installierte Addon-Ordner werden gehasht und
über einen Index einem Addon zugeordnet. Nur so funktionieren Updates für
Addons, die der Nutzer nie ausdrücklich hinzugefügt hat. WowUp macht das in
`wowup-lib/src/addon-providers/wowup-addon-provider.ts` (Aufruf
`getAddonIdsByFingerprints`) und `addon-fingerprint.service.ts`.

**Projektkennungen stehen in der `.toc`.** `X-Curse-Project-ID`, `X-WoWI-ID`,
`X-Tukui-Project-ID`, `X-Wago-ID`. Daraus leitet der Manager die Quelle ab,
ohne den Addon-Namen zu raten. WowUp bildet das in `addon-install.service.ts`
ab (`ADDON_PROVIDER_TOC_EXTERNAL_ID_MAP`).

**Installation ist Entpacken mit Ordnerkorrektur.** ZIP holen, oberste
Verzeichnisebene bestimmen, nach `Interface/AddOns/<Ordner>` entpacken,
`__MACOSX` ignorieren, `.toc` neu einlesen für `## Version` und `## Interface`.

Ajour, der frühere Rust-Vertreter dieser Gattung, ist seit 2024 archiviert. Der
Grund war nicht die Technik, sondern der Zugang zu CurseForge nach dem Verkauf
an Overwolf. Hexwerk hält sich deshalb aus dieser Abhängigkeit heraus: der
Fingerprint-Index von WowUp ist ein Beschleuniger, keine Voraussetzung.

## Risiken

| Risiko | Umgang |
|---|---|
| CurseForge verweigert den Schlüssel | Quelle bleibt abschaltbar, Katalog funktioniert ohne sie |
| Der WowUp Hub verschwindet | Erkennung fällt auf die Projektkennungen der `.toc` zurück |
| Wago ändert die Antwortform | Parser ist tolerant geschrieben, Feldnamen sind dokumentiert; mit Token nachziehen |
| GitHub drosselt ohne Token | Zwischenspeicher mit TTL, Abfragen auf wenige Treffer begrenzt |

## Name

`Hexwerk`: Hex (deutsch: Hexer, die Klasse in WoW) und Werk. Kommando
`hexwerk`, Flatpak-Ziel `de.weppelmann.Hexwerk`. Auf GitHub, crates.io, PyPI und
npm zum Zeitpunkt der Wahl frei.
