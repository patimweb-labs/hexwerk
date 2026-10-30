# Fixtures

Gekürzte echte Antworten, damit `cargo test` ohne Netz läuft (harte Regel aus
PLAN.md §5/§12). Aufgenommen am 2026‑10‑30.

| Datei | Quelle | Aufnahme |
|---|---|---|
| `tukui_addons.json` | `GET https://api.tukui.org/v1/addons` | vollständig (die Gesamtliste hat zwei Einträge, 3,7 KB) |
| `wowi_filelist.json` | `GET https://api.mmoui.com/v4/game/WOW/filelist.json` | erste 3 von 7.766 Einträgen, unverändert |
| `wowi_details_3826.json` | `GET https://api.mmoui.com/v3/game/WOW/filedetails/3826.json` | ein Eintrag, unverändert |
| `gh_search.json` | `GET https://api.github.com/search/repositories?q=weakauras+in:name&per_page=2` | 2 Treffer, auf die benutzten Felder gekürzt |
| `gh_release.json` | `GET https://api.github.com/repos/WeakAuras/WeakAuras2/releases/latest` | erste 3 Assets, `body` auf 300 Zeichen gekürzt |
| `hub_fingerprint.json` | `POST https://hub.wowup.io/addons/fingerprint` (unbekannter Hash) | unverändert (`{"exactMatches":[]}`) |
| `curseforge_search.json` | **nicht gemessen** | ohne Schlüssel antwortet die API mit 403; Form nach Dokumentation, das Feld `_note` sagt es |
| `wago_search.json` | **nicht gemessen** | ohne Token 302 auf die Website; Form nach Dokumentation, Feld `_note` |

Regeln: keine echten Zugangsdaten, keine Nutzerdaten, `_note` nur bei den beiden
nicht gemessenen Dateien. Wer eine Fixture neu aufnimmt, kürzt auf die Felder,
die der Parser liest, nicht auf Werte, die er gern hätte.
