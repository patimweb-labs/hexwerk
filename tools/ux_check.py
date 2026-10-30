#!/usr/bin/env python3
"""Fahrplan durch die Oberfläche: klicken, tippen, abfotografieren.

Das Skript startet Hexwerk nicht selbst. Es erwartet ein laufendes Fenster in
einer Sitzung mit barrierefreier Schnittstelle (AT-SPI) und DISPLAY, also zum
Beispiel so:

    Xvfb :99 -screen 0 1440x900x24 &
    DISPLAY=:99 dbus-run-session -- hexwerk-gtk &
    DISPLAY=:99 python3 tools/ux_check.py /tmp/ux

Danach liegen dort PNG je Schritt und eine Textdatei mit dem Bestand aller
bedienbaren Elemente. Was das Skript nicht kann: Mauszeigerform prüfen (das
macht die CSS-Regel `cursor: pointer`, siehe tools/ux_check.sh) und Urteile
über Geschmack. Es sammelt Belege, bewertet wird am Bild.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time

import pyatspi

SCREENSHOT_TOOL = "import"  # ImageMagick

# Rollennamen der Barrierefreiheit sind je nach GTK/AT-SPI-Fassung verschieden:
# GTK4 meldet „button“, aeltere Staende „push button“. Beides wird gesucht,
# sonst findet das Skript die Bedienleiste nicht und meldet „nicht gefunden“.
BUTTON_ROLES = ("button", "push button")


def log(msg: str) -> None:
    print(msg, flush=True)


def desktop_app(name_hint: str = "hexwerk"):
    desktop = pyatspi.Registry.getDesktop(0)
    for app in desktop:
        try:
            name = (app.name or "").lower()
        except Exception:
            continue
        if name_hint in name:
            return app
    return None


def wait_for_app(seconds: float = 40.0):
    end = time.time() + seconds
    while time.time() < end:
        app = desktop_app()
        if app is not None and app.childCount > 0:
            return app
        time.sleep(0.5)
    return None


def walk(node, depth=0, limit=4000):
    """Alle Knoten des Baums, mit Tiefe. Schutz gegen Ausreisser."""
    count = 0
    stack = [(node, depth)]
    while stack and count < limit:
        current, d = stack.pop()
        yield current, d
        count += 1
        try:
            children = list(current)
        except Exception:
            children = []
        for child in reversed(children):
            stack.append((child, d + 1))


def role(node) -> str:
    try:
        return node.getRoleName()
    except Exception:
        return "?"


def name_of(node) -> str:
    try:
        return node.name or ""
    except Exception:
        return ""


def extents(node):
    try:
        comp = node.queryComponent()
        e = comp.getExtents(pyatspi.DESKTOP_COORDS)
        return e.x, e.y, e.width, e.height
    except Exception:
        return None


def visible(node) -> bool:
    try:
        state = node.getState()
        return state.contains(pyatspi.STATE_SHOWING) and state.contains(pyatspi.STATE_VISIBLE)
    except Exception:
        return False


def click(node) -> bool:
    """Erst echte Aktion, sonst Mausklick in die Mitte (wie ein Mensch)."""
    try:
        action = node.queryAction()
        if action.nActions > 0:
            return action.doAction(0)
    except Exception:
        pass
    box = extents(node)
    if not box:
        return False
    x, y, w, h = box
    subprocess.run(["xdotool", "mousemove", str(x + w // 2), str(y + h // 2), "click", "1"], check=False)
    time.sleep(0.2)
    return True


def type_text(text: str) -> None:
    subprocess.run(["xdotool", "type", "--delay", "40", text], check=False)
    time.sleep(0.3)


def shot(path: str) -> bool:
    r = subprocess.run([SCREENSHOT_TOOL, "-window", "root", path], check=False)
    return r.returncode == 0


def find(app, role_name: str, text: str | None = None):
    hits = []
    for node, _ in walk(app):
        if role(node) != role_name:
            continue
        if text is not None and text.lower() not in name_of(node).lower():
            continue
        if visible(node):
            hits.append(node)
    return hits


def inventory(app, path: str) -> None:
    lines = []
    for node, depth in walk(app):
        r = role(node)
        if r in ("frame", "panel", "filler", "scroll pane", "viewport", "unknown", "redundant object"):
            continue
        box = extents(node)
        lines.append(
            f"{'  ' * depth}{r:<22} {'-'.join(filter(None, [name_of(node)[:48]])):<50} {box}"
        )
    open(path, "w").write("\n".join(lines))
    log(f"Bestand: {len(lines)} Elemente -> {path}")


def main() -> int:
    outdir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/ux"
    os.makedirs(outdir, exist_ok=True)

    app = wait_for_app()
    if app is None:
        log("FEHLER: kein Fenster in der Barrierefreiheits-Schnittstelle gefunden")
        return 1

    log(f"Anwendung: {app.name}")
    shot(f"{outdir}/00-start.png")
    inventory(app, f"{outdir}/bestand.txt")

    steps = []

    # Bedienleiste: jede Ansicht einmal oeffnen
    # Katalog zuletzt: danach wird gesucht und eine Karte geoeffnet, dafuer muss
    # die Katalogseite sichtbar sein.
    for label in ["Clients", "Installiert", "Scan", "Katalog"]:
        candidates = find(app, "page tab") + find(app, "link")
        for role_name in BUTTON_ROLES:
            candidates += find(app, role_name)
        hits = [n for n in candidates if label.lower() in name_of(n).lower()]
        if not hits:
            log(f"  {label}: nicht gefunden")
            continue
        click(hits[0])
        time.sleep(0.8)
        path = f"{outdir}/ansicht-{label.lower()}.png"
        shot(path)
        steps.append((label, path))

    # Katalog: suchen. GtkSearchEntry meldet je nach GTK „entry“ oder „text“ -
    # beides wird gesucht, sonst faellt der Suchschritt still aus.
    entries = find(app, "entry") + find(app, "text")
    if entries:
        click(entries[0])
        type_text("weakauras")
        time.sleep(1.5)
        shot(f"{outdir}/katalog-suche.png")
        steps.append(("Katalogsuche", f"{outdir}/katalog-suche.png"))

    # Quellenfilter umschalten
    toggles = find(app, "toggle button")
    if toggles:
        click(toggles[0])
        time.sleep(1.0)
        shot(f"{outdir}/filter-umschalten.png")
        steps.append(("Quellenfilter", f"{outdir}/filter-umschalten.png"))

    # Detailseite: eine Karte im Katalog oeffnen. Karten sind Knoepfe und haben
    # eine AT-SPI-Aktion - Tabellenzeilen nicht, und die gemeldeten Koordinaten
    # der Zeilen sind (0,0), taugen also nicht fuer einen Mausklick.
    chrome = {
        "scan", "clients", "installiert", "katalog", "addon-detail",
        "hell", "dunkel", "system", "minimize", "maximize", "close",
        "erneut scannen", "alle", "mit quell-id", "ohne quell-id",
        "deinstallieren…", "aktualisieren…", "installieren",
    }
    # Der Suchbegriff von oben ist die praeziseste Marke: die Karte heisst wie
    # das Addon, Bedienleiste und Fensterknoepfe heissen anders.
    cards = find(app, "button", "weakaura")
    if not cards:
        cards = [
            n
            for n in find(app, "button")
            if name_of(n).strip() and name_of(n).strip().lower() not in chrome
        ]
    if cards:
        click(cards[0])
        time.sleep(1.2)
        shot(f"{outdir}/detail.png")
        steps.append((f"Detail aus Karte ({name_of(cards[0])[:20]})", f"{outdir}/detail.png"))
    else:
        log("  Detail: keine Karte gefunden")

    # Themenumschalter: Menueknopf oeffnen, Eintraege auflisten
    menus = find(app, "push button")
    theme_hits = [n for n in menus if any(k in name_of(n).lower() for k in ("theme", "thema", "design", "ansicht"))]
    if theme_hits:
        click(theme_hits[0])
        time.sleep(0.8)
        shot(f"{outdir}/themenmenue.png")
        steps.append(("Themenmenue", f"{outdir}/themenmenue.png"))

    log("\nSchritte und Bilder:")
    for label, path in steps:
        log(f"  {label:<24} {path}")

    log("\nBedienbare Elemente mit Groesse unter 24 px Hoehe (Ziele, die man schlecht trifft):")
    small = 0
    for node, _ in walk(app):
        r = role(node)
        if r not in BUTTON_ROLES + ("toggle button", "check box", "radio button", "combo box", "entry", "menu item"):
            continue
        box = extents(node)
        if box and box[3] < 24 and visible(node):
            small += 1
            log(f"  {r:<16} {name_of(node)[:30]:<32} {box[2]}x{box[3]}")
    if small == 0:
        log("  keins")
    return 0


if __name__ == "__main__":
    sys.exit(main())
