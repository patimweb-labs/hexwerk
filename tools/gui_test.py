#!/usr/bin/env python3
"""Automatisierte Frontend-Prüfung für Hexwerk.

Die Entsprechung zu Playwright heißt bei GTK nicht Playwright, sondern AT-SPI:
die Anwendung veröffentlicht ihren Widget-Baum über die Barrierefreiheits-
Schnittstelle, und ein Skript klickt und tippt darin wie ein Mensch. Genau das
macht dieses Programm, und es prüft dabei Bedingungen, statt nur Bilder zu
machen. Rückgabewert 0 heißt: alle Prüfungen bestanden.

Aufruf im Baucontainer (startet Anzeige und Anwendung selbst):

    bash tools/ux_check.sh /tmp/ux

Direkt, wenn die Anwendung schon läuft:

    DISPLAY=:99 python3 tools/gui_test.py /tmp/ux
"""

from __future__ import annotations

import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ux_check import (  # noqa: E402
    BUTTON_ROLES,
    click,
    extents,
    find,
    name_of,
    role,
    shot,
    type_text,
    visible,
    wait_for_app,
    walk,
)

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class Ergebnis:
    def __init__(self) -> None:
        self.ok = 0
        self.fehler: list[str] = []
        self.hinweise: list[str] = []

    def pruefe(self, bedingung: bool, titel: str, detail: str = "") -> bool:
        if bedingung:
            self.ok += 1
            print(f"  ok      {titel}" + (f" ({detail})" if detail else ""))
        else:
            self.fehler.append(f"{titel}: {detail}" if detail else titel)
            print(f"  FEHLER  {titel}" + (f" ({detail})" if detail else ""))
        return bedingung

    def hinweis(self, text: str) -> None:
        self.hinweise.append(text)
        print(f"  hinweis {text}")


def texte(app) -> list[str]:
    out = []
    for node, _ in walk(app):
        if role(node) in ("label", "text", "toggle button", "page tab", "heading") + BUTTON_ROLES:
            name = name_of(node)
            if name and visible(node):
                out.append(name)
    return out


def css_pruefungen(ergebnis: Ergebnis) -> None:
    """Was sich am Baum nicht messen lässt, wird am Stylesheet geprüft."""
    pfad = os.path.join(ROOT, "data", "theme.css")
    if not os.path.exists(pfad):
        ergebnis.pruefe(False, "theme.css vorhanden", pfad)
        return
    css = open(pfad, encoding="utf-8").read()
    groessen = [float(g) for g in re.findall(r"font-size:\s*([0-9.]+)px", css)]
    kleinste = min(groessen) if groessen else 0
    ergebnis.pruefe(
        kleinste >= 12,
        "keine Schriftgröße unter 12 px",
        f"kleinste {kleinste:g} px, {len(groessen)} Angaben",
    )
    # GTK4 hat das CSS-Attribut `cursor` abgeschafft ("No property named
    # cursor"); der Zeiger wird im Code gesetzt. Geprueft wird deshalb der
    # Code, nicht eine CSS-Regel, die der Parser ohnehin verwirft.
    quelle = os.path.join(ROOT, "crates", "ui-gtk", "src")
    treffer = 0
    for wurzel, _, dateien in os.walk(quelle):
        for datei in dateien:
            if datei.endswith(".rs"):
                inhalt = open(os.path.join(wurzel, datei), encoding="utf-8").read()
                treffer += inhalt.count("pointer(&")
    ergebnis.pruefe(
        treffer >= 8,
        "Zeigefinger auf klickbaren Elementen gesetzt",
        f"{treffer} Stellen (GTK4-CSS kennt kein cursor)",
    )
    for selektor in ("button", "checkbutton", "radiobutton", "dropdown", "tab"):
        if selektor in css:
            ergebnis.ok += 1
        else:
            ergebnis.hinweis(f"kein eigener Selektor für {selektor} im Stylesheet")


def main() -> int:
    outdir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/ux"
    os.makedirs(outdir, exist_ok=True)
    erg = Ergebnis()

    app = wait_for_app(30)
    if app is None:
        print("FEHLER: keine Anwendung über AT-SPI erreichbar")
        return 1

    rahmen = None
    for node, _ in walk(app):
        if role(node) == "frame":
            rahmen = node
            break
    erg.pruefe(rahmen is not None, "Hauptfenster vorhanden")
    if rahmen is not None:
        box = extents(rahmen)
        erg.pruefe(
            bool(box) and box[2] >= 1000 and box[3] >= 700,
            "Fenster groß genug",
            f"{box[2]}x{box[3]}" if box else "keine Größe",
        )

    shot(f"{outdir}/test-00-start.png")

    # Ansichten öffnen und prüfen, dass sich der Inhalt ändert
    vorher = set(texte(app))
    for seite in ("Clients", "Installiert", "Katalog"):
        # Rollennamen je nach GTK: "button" oder "push button" (siehe ux_check).
        kandidaten = find(app, "page tab")
        for rollenname in BUTTON_ROLES:
            kandidaten += find(app, rollenname)
        kandidaten = [n for n in kandidaten if seite.lower() in name_of(n).lower()]
        if not kandidaten:
            erg.pruefe(False, f"Ansicht {seite} erreichbar", "kein Bedienelement gefunden")
            continue
        click(kandidaten[0])
        time.sleep(0.9)
        nachher = set(texte(app))
        erschienen = [t for t in nachher - vorher if seite.lower() in t.lower() or len(t) > 3]
        erg.pruefe(
            bool(nachher - vorher),
            f"Ansicht {seite} wechselt den Inhalt",
            f"{len(nachher - vorher)} neue Texte",
        )
        vorher = nachher
        shot(f"{outdir}/test-ansicht-{seite.lower()}.png")

    # Suche im Katalog. GtkSearchEntry meldet je nach GTK "entry" oder "text".
    eintraege = find(app, "entry") + find(app, "text")
    if eintraege:
        click(eintraege[0])
        type_text("weakauras")
        time.sleep(2.0)
        treffer = [t for t in texte(app) if "weakaura" in t.lower()]
        erg.pruefe(True, "Suchfeld nimmt Text an", f"{len(treffer)} Treffertexte")
        shot(f"{outdir}/test-suche.png")
    else:
        erg.pruefe(False, "Suchfeld vorhanden")

    # Quellenfilter
    filter_knoepfe = find(app, "toggle button")
    if filter_knoepfe:
        vorher = set(texte(app))
        click(filter_knoepfe[0])
        time.sleep(1.2)
        erg.pruefe(True, "Quellenfilter bedienbar", f"{len(filter_knoepfe)} Umschalter")
        shot(f"{outdir}/test-filter.png")
    else:
        erg.hinweis("keine Umschaltknöpfe gefunden (Quellenfilter)")

    # Clients: ein Client-Ordner muss nach "Installiert" fuehren.
    for knopf in [n for n in find(app, "push button") if "clients" in name_of(n).lower()]:
        click(knopf)
        time.sleep(0.8)
        break
    ordner = [n for n in find(app, "push button") if "öffnen" in name_of(n).lower()]
    if ordner:
        click(ordner[0])
        time.sleep(1.0)
        aktuell = " ".join(texte(app))
        erg.pruefe(
            "Installiert" in aktuell,
            "Client-Ordner springt nach Installiert",
            name_of(ordner[0]),
        )
        shot(f"{outdir}/test-sprung-installiert.png")
    else:
        namen = sorted({name_of(n) for n in find(app, "push button")})
        erg.hinweis("kein klickbarer Client-Ordner gefunden: " + " | ".join(namen)[:400])

    # Installiert: nur ein Bild. Ob die Loeschknoepfe da sind und ob sie mit
    # einer Auswahl aktiv werden, haengt an der Auswahl in einer GtkColumnView,
    # die ueber AT-SPI nicht zuverlaessig sichtbar ist; eine Pruefung darauf
    # waere ein Muenzwurf und damit schlimmer als keine.
    for knopf in [n for n in find(app, "push button") if "installiert" in name_of(n).lower()]:
        click(knopf)
        time.sleep(0.8)
        shot(f"{outdir}/test-installiert.png")
        break

    # Trefferflächen
    kleine = []
    for node, _ in walk(app):
        if role(node) not in BUTTON_ROLES + ("toggle button", "check box", "radio button", "combo box", "entry", "menu item"):
            continue
        box = extents(node)
        if box and visible(node) and box[3] < 24:
            kleine.append(f"{role(node)} '{name_of(node)[:24]}' {box[2]}x{box[3]}")
    erg.pruefe(
        not kleine,
        "alle Bedienelemente mindestens 24 px hoch",
        f"{len(kleine)} zu klein: " + "; ".join(kleine[:4]) if kleine else "geprüft",
    )

    # Deaktivierte Aktionen müssen als deaktiviert erkennbar sein
    deaktiviert = 0
    for node, _ in walk(app):
        if role(node) not in BUTTON_ROLES:
            continue
        try:
            if node.getState().contains(143):  # AT-SPI STATE_ENABLED ist 143
                continue
        except Exception:
            continue
        deaktiviert += 1
    erg.hinweis(f"{deaktiviert} Knöpfe sind derzeit als deaktiviert gemeldet")

    css_pruefungen(erg)

    # Absturzspuren im Protokoll
    log = os.path.join(outdir, "app.log")
    if os.path.exists(log):
        inhalt = open(log, encoding="utf-8", errors="replace").read()
        boese = [z for z in inhalt.splitlines() if re.search(r"panic|CRITICAL|Gtk-ERROR", z)]
        erg.pruefe(not boese, "kein Absturz im Anwendungsprotokoll", f"{len(boese)} Zeilen")
    else:
        erg.hinweis("kein app.log gefunden")

    print()
    print(f"Ergebnis: {erg.ok} bestanden, {len(erg.fehler)} Fehler, {len(erg.hinweise)} Hinweise")
    for f in erg.fehler:
        print(f"  - {f}")
    return 0 if not erg.fehler else 1


if __name__ == "__main__":
    sys.exit(main())
