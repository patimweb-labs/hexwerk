# Datenfluss

Was passiert, wenn der Anwender etwas tut — vom Klick bis zur Datei auf der
Platte.

## „Addon installieren"

1. **Oberfläche** (`ui/src/…`, Vue): ruft den Tauri-Befehl über die IPC-Schicht
   (`crates/desktop/src/…`); sie kennt weder Pfade noch ZIP-Formate.
2. **Befehl** (`crates/desktop`): reicht an `hexwerk-install` weiter, das die
   Quellen-Antwort (ZIP), Ziel-Client und Sicherung kennt.
3. **Installation** (`crates/install`): entpackt nach dem Ordner, den der
   `.toc`-Name vorgibt, legt eine Sicherung an und räumt den Papierkorb
   (Freedesktop-Desktop-Papierkorb, nicht ein eigener).
4. **Index** (`crates/index`): Der Eintrag wird in SQLite/FTS5 nachgeführt —
   Suche und Bestand lesen aus dem Index, nicht aus dem Dateisystem.
5. **Ereignis**: Der Befehl meldet Erfolg oder Fehler zurück; die Oberfläche
   zeichnet daraus den neuen Zustand (`Installiert`, `Katalog`, `Bestand`).

## „Katalog durchsuchen"

1. UI tippt in die Suche; die Anfrage geht an den Index (FTS5), nicht an die
   Quellen im Netz.
2. `hexwerk-index` liefert Treffer mit Quelle, Version und Interface.
3. Der Katalog selbst kommt als gepackter Snapshot vom Dienst
   `hexwerk-catalog` (eigenes Repo): Die App kennt nur ihren Schlüssel, der
   CurseForge-Schlüssel bleibt beim Dienst.

## „Präfix finden"

1. `hexwerk-core` liest die Herkünfte (Steam, Faugus, Heroic, Lutris, Bottles,
   PortProton, beliebiger Ordner) und sucht `_retail_`/`_classic_`/`_era_`.
2. Der `.toc`-Leser sammelt `## Interface`, `## Version`, `X-*` und Notizen je
   Sprache.
3. Das Ergebnis geht als Modell an die Oberfläche — kein Pfad wird im Frontend
   zusammengesetzt.

## Grundsatz

Ein Weg: Oberfläche → Befehl → Fachpaket → Index/Dateisystem → Ereignis →
Darstellung. `crates/domain`-Regeln (hier: `core`, `index`, `install`, `sources`)
rechnen, die Oberfläche zeigt.
