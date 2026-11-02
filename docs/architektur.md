# Architektur

Stand 2026-10-30. Diese Datei beschreibt den Aufbau des Programms und den Weg
der Daten. Sie ist die verbindliche Übersicht; die Details stehen in
`PLAN.md` und in den Modulköpfen des Quelltexts.

## Zuschnitt

Hexwerk ist ein Cargo-Workspace mit einem Kern und drei Fachpaketen. Die
Oberfläche ist ein eigenes Paket mit eigener `[workspace]`-Tabelle, damit sie
sich unabhängig bauen lässt und GTK nur dann im Abhängigkeitsbaum landet, wenn
jemand sie anfasst.

```mermaid
flowchart TD
    subgraph Anwendung
        CLI[hexwerk<br/>Kommandozeile]
        UI[hexwerk-desktop<br/>Tauri + Vue-Frontend<br/>crates/desktop]
    end

    subgraph Fachpakete
        CORE[hexwerk-core<br/>Präfixe, TOC, Addons]
        SOURCES[hexwerk-sources<br/>GitHub, Tukui, WoWInterface,<br/>Wago, CurseForge]
        INDEX[hexwerk-index<br/>SQLite und FTS5]
        INSTALL[hexwerk-install<br/>ZIP, Sicherung, Papierkorb]
    end

    subgraph Aussenwelt
        FS[(Dateisystem<br/>Wine-Präfixe)]
        NET([HTTP-Quellen])
        DB[(index.db)]
        DATA[(Sicherungen<br/>Papierkorb)]
    end

    CLI --> CORE
    CLI --> INDEX
    CLI --> SOURCES
    UI --> CORE
    UI --> INDEX
    UI --> SOURCES
    UI --> INSTALL

    SOURCES --> CORE
    INDEX --> CORE
    INDEX --> SOURCES
    INSTALL --> CORE
    INSTALL --> SOURCES

    CORE --> FS
    SOURCES --> NET
    INDEX --> DB
    INSTALL --> FS
    INSTALL --> DATA
```

Regel für die Richtung: `core` kennt niemanden, `sources`, `index` und
`install` kennen `core`, die Oberfläche kennt alle. Kein Paket ruft ein
späteres auf, damit kein Zyklus entsteht.

## Datenfluss beim Start

Der Scan liest das Dateisystem und dauert auf einem gefüllten Addon-Ordner
Sekunden. Er läuft deshalb nicht im GTK-Hauptthread.

```mermaid
sequenceDiagram
    autonumber
    participant U as Benutzer
    participant UI as Fenster (GTK-Hauptthread)
    participant SC as Scan-Thread
    participant IDX as hexwerk-index
    participant CAT as Katalog-Thread
    participant SRC as hexwerk-sources
    participant NET as HTTP

    U->>UI: Fenster öffnen
    UI->>SC: discover() starten
    SC->>SC: Steam, Faugus, Heroic,<br/>Lutris, Bottles, PortProton
    SC->>SC: je Client die .toc lesen
    SC-->>UI: Discovery mit Clients und Notizen
    UI->>IDX: sync_clients(Discovery)
    IDX-->>UI: Anzahl Clients und Addons
    UI->>CAT: sync_catalogue starten
    CAT->>SRC: Registry::build aus den Schlüsseln
    CAT->>NET: Suche je Quelle
    NET-->>SRC: JSON
    SRC-->>CAT: Einträge je Quelle
    CAT->>CAT: merge nach Projektkennung
    CAT->>IDX: sync_catalogue(Einträge)
    CAT-->>UI: Katalog steht
    UI->>IDX: search(Filters)
    IDX-->>UI: Treffer aus FTS5
    UI-->>U: Katalog, Installiertes, Quellenzustand
```

Zwei Zusagen daraus:

1. Das Fenster fasst im Hauptthread keine Festplatte und kein Netz an.
2. Der Katalog wird nur ergänzt, nie geleert. Fällt eine Quelle aus, bleibt der
   vorherige Stand sichtbar und der Fehler steht daneben.

## Datenfluss einer Installation

```mermaid
sequenceDiagram
    autonumber
    participant U as Benutzer
    participant UI as Fenster
    participant SRC as hexwerk-sources
    participant INS as hexwerk-install
    participant FS as Addon-Ordner
    participant IDX as hexwerk-index

    U->>UI: Addon öffnen, Client und Version wählen
    UI->>SRC: download(Release)
    SRC-->>UI: ZIP als Bytes
    UI->>INS: apply(InstallRequest, bytes)
    INS->>INS: ZIP prüfen (Zip-Slip, Symlinks)
    INS->>FS: nach .hexwerk-tmp entpacken
    INS->>FS: vorhandenen Ordner sichern
    INS->>FS: umbenennen auf Zielordner
    INS-->>UI: Ordner und gelesene TOC-Version
    UI->>IDX: sync_clients für diesen Client
    IDX-->>UI: neuer Stand
    UI-->>U: Liste aktualisiert
```

Ein Abbruch zu jedem Zeitpunkt lässt den alten Stand stehen: entpackt wird in
einen temporären Ordner, gesichert wird vor dem Umbenennen.

## Datenmodell des Index

```mermaid
erDiagram
    clients ||--o{ installed : "hat"
    addons ||--o{ installed : "ist installiert als"
    addons ||--o{ addon_sources : "wird angeboten von"
    addon_sources ||--o{ releases : "hat Versionen"

    clients {
        int id PK
        text key
        text origin_label
        text game_dir
        text addons_dir
        int interface_no
        text interfaces_json
    }
    addons {
        int id PK
        text key
        text name
        text author
        text summary
        text homepage
    }
    addon_sources {
        int addon_id FK
        text source
        text external_id
        text version
        text download_url
    }
    releases {
        int addon_id FK
        text source
        text version
        text interface_json
        text url
    }
    installed {
        int client_id FK
        int addon_id FK
        text folder
        text version
        text fingerprint
    }
```

`addons` trägt einen FTS5-Index über Name, Autor und Kurztext. Die Suche läuft
über `bm25`, nicht über `LIKE`, damit sie auch bei einigen tausend Einträgen
sofort antwortet. Der Index ist eine Datei unter
`~/.local/share/hexwerk/index.db`, im WAL-Modus, damit Lesen und Schreiben
gleichzeitig gehen.

## Erkennung eines installierten Addons

Ein Addon wird nicht eingetragen, sondern erkannt. Die Reihenfolge der Beweise:

```mermaid
flowchart LR
    A[Addon-Ordner im Client] --> B{.toc vorhanden}
    B -- nein --> X[nicht erfassbar]
    B -- ja --> C[Projektkennung aus X-WoWI,<br/>X-Tukui, X-Curse, X-Wago]
    C --> D{Treffer im Katalog}
    D -- ja --> E[verknüpft, Update prüfbar]
    D -- nein --> F[Fingerprint des Ordners]
    F --> G{Treffer im Fingerprint-Index}
    G -- ja --> E
    G -- nein --> H[Titel aus der .toc]
    H --> I{Treffer}
    I -- ja --> E
    I -- nein --> J[angezeigt, aber ohne Quelle]
```

Was ohne Quelle bleibt, wird angezeigt und nie angefasst. Löschen setzt eine
sichere Zuordnung voraus.

## Schlüssel

```mermaid
flowchart TD
    S[Quelle braucht einen Schlüssel] --> E{Umgebungsvariable<br/>HEXWERK_WAGO_TOKEN<br/>HEXWERK_CURSEFORGE_KEY}
    E -- gesetzt --> OK[verwenden]
    E -- leer --> F{keys.json<br/>Modus 0600}
    F -- vorhanden --> OK
    F -- leer --> G{secret-tool}
    G -- vorhanden --> OK
    G -- leer --> OFF[Quelle bleibt abgeschaltet]
```

Ausgegeben wird ein Schlüssel nie vollständig, nur die letzten vier Zeichen.
Ein mitgelieferter CurseForge-Schlüssel ist nicht zulässig, weil Schlüssel bei
CurseForge an den einzelnen Entwickler gebunden sind; Hexwerk liefert deshalb
keinen fremden Schlüssel aus.

## Verzeichnisse zur Laufzeit

| Zweck | Pfad |
|---|---|
| Index | `~/.local/share/hexwerk/index.db` |
| Sicherungen | `~/.local/share/hexwerk/backup` |
| Papierkorb | `~/.local/share/Trash` (der des Desktops) |
| Schlüssel | `~/.config/hexwerk/keys.json` |
| Fensterzustand | `~/.config/hexwerk/ui.json` |
| HTTP-Zwischenspeicher | `~/.cache/hexwerk` |

## Entscheidungen und ihre Gründe

| Entscheidung | Grund |
|---|---|
| GTK4 mit libadwaita statt eigenem Widgetsatz | `AdwSpinner`, `AdwToast` und `AdwStyleManager` bringen Animation, Rückmeldung und Dunkel/Hell mit; dieselbe GTK-Version wie im Nachbarprojekt |
| SQLite mit FTS5 statt eigenem Suchindex | Ein Dateiformat, Transaktionen, Suche mit Rangfolge; keine zusätzliche Abhängigkeit im Betrieb |
| Eigene Fingerprint-Definition | WowUps Verfahren ist nicht offengelegt; die eigene Definition ist dokumentiert und reproduzierbar |
| Kein Netz im GTK-Hauptthread | Sonst friert das Fenster bei langsamen Quellen ein |
| Cargo-Quellen im Flatpak eingefroren | Ohne `new-sources.json` prüft der Sandbox-Bau keine Prüfsummen; dann bricht er ab statt still durchzulaufen |

## Grenzen

* Wago und CurseForge sind vorbereitet, aber ohne eigenen Schlüssel nicht
  erreichbar; ihre Antwortformen sind deshalb nur aus der Dokumentation
  bekannt, nicht gemessen.
* Ein Präfix, der auf einem anderen Datenträger per Symlink liegt, wird nicht
  gefunden; der Scanner folgt keinen Symlinks unter `drive_c`.
* Der Fingerprint-Index von WowUp wird nur als Beschleuniger benutzt. Fällt er
  weg, bleibt die Erkennung über die Projektkennungen der `.toc` bestehen.
