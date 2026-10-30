# Farben

Stand: 2026-10-30. Gemessen am Bildschirmabzug und am Kontrastverhaeltnis
(WCAG-Formel, 1,0 = kein Unterschied, 21,0 = Schwarz auf Weiss).

## Regel

Ein Farbton, zwei Helligkeiten. Marke, Auswahl und Hinweise benutzen denselben
Ton; geaendert wird nur die Helligkeit, damit der Ton auf hellem und dunklem
Grund gleich viel Gewicht hat. Kein zweiter Akzent, kein Farbverlauf im
Zeichen.

| Rolle | dunkles Thema | helles Thema | Regel |
|---|---|---|---|
| Flaeche | `#060A12` | `#F5F7FB` | Fenster und Leiste |
| Karte | `#0A111C` / `#0C1523` | `#FFFFFF` / `#F0F3F8` | abgesetzt, nie schwebend |
| Linie | `#16233A` | `#D7DEE9` | 1 px, keine Schatten |
| Schrift | `#DCE6F5` | `#0B1520` | 13 zu 1 auf der Flaeche |
| Schrift schwach | `#7C8CA6` | `#55637A` | fuer Nebenzeilen |
| **Marke** | `#39DD93` | `#0B7A52` | immer `--hw-signal` |
| Warnung | `#F2B344` | `#8A5A00` | Update verfuegbar |
| Fehler | `#E2657A` | `#A32238` | Loeschen, Abbruch |

## Warum das Zeichen die Palettenfarbe traegt

Das Zeichen in der Leiste ist ein **symbolisches** Symbol und nimmt seine Farbe
aus der CSS-Eigenschaft `color`, also aus `--hw-signal` der aktiven Palette.
Eine farbige PNG-Datei kann das nicht: sie brachte denselben Ton in beide
Themen.

Gemessen auf der Leiste:

| Fassung | Kontrast | Bewertung |
|---|---|---|
| farbige Datei, `#39DD93` auf `#F5F7FB` | **1,64 : 1** | praktisch unsichtbar |
| symbolisch, `#0B7A52` auf `#F5F7FB` | **4,99 : 1** | gut lesbar |
| symbolisch, `#39DD93` auf `#0A111C` | **10,76 : 1** | gut lesbar |

Deshalb: `data/icons/symbolic/...` wird als **gefuellter** Pfad gezeichnet
(Sechseck als Ring mit `fill-rule="evenodd"`, H aus drei Rechtecken). GTK
faerbt nur Fuellungen um; ein gestricheltes Symbol wurde beim Umfaerben zu
einem vollen Block, das H verschwand.

## Was fest bleibt

Das **Programmsymbol** (Taskleiste, Dock, Flatpak-Datei) bleibt die farbige
Fassung mit `#39DD93`. Eine einzelne Bilddatei kann dem Thema nicht folgen, und
der Signalgruen auf dunklem Dock ist das Markenzeichen. In der Leiste, also dort
wo die App selbst zeichnet, folgt das Zeichen der Palette.
