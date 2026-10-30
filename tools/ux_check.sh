#!/usr/bin/env bash
# Frontend-Prüfung: Oberfläche in einer virtuellen Anzeige starten und
# durchklicken. Gedacht für den Baucontainer, weil auf dem Host die
# GTK-Header fehlen:
#
#   podman exec -w /work hexwerk-build bash tools/ux_check.sh /tmp/ux
#
# Ergebnis: PNG je Schritt unter dem Zielordner, dazu bestand.txt mit allen
# bedienbaren Elementen und die Ausgabe von tools/ux_check.py.
set -uo pipefail

OUT=${1:-/tmp/ux}
BIN=${2:-crates/ui-gtk/target/debug/hexwerk-gtk}
DISPLAY_NUM=${DISPLAY_NUM:-99}

mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/bestand.txt 2>/dev/null || true

export DISPLAY=":$DISPLAY_NUM"
# GTK4 kennt nur "none" oder "atspi"; "1" wird verworfen und dann gibt es
# keinen Baum in der Barrierefreiheits-Schnittstelle (das Skript findet kein
# Fenster und bricht ab).
export GTK_A11Y=atspi
export GDK_BACKEND=x11
export NO_AT_BRIDGE=0

echo "== Anzeige :$DISPLAY_NUM starten"
pkill -f "Xvfb :$DISPLAY_NUM" 2>/dev/null || true
sleep 0.3
Xvfb ":$DISPLAY_NUM" -screen 0 1440x900x24 >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1.5

if [ ! -x "$BIN" ]; then
  echo "FEHLER: $BIN fehlt oder ist nicht ausführbar"
  kill "$XVFB_PID" 2>/dev/null
  exit 1
fi

echo "== Anwendung starten: $BIN"
# Eigene Sitzung mit Sitzungsbus: ohne dbus gibt es keine Barrierefreiheit und
# damit kein Skript, das Knöpfe findet.
dbus-run-session -- bash -c "
  '$BIN' >'$OUT/app.log' 2>&1 &
  APP=\$!
  sleep 4
  python3 tools/ux_check.py '$OUT'
  TOUR=\$?
  python3 tools/gui_test.py '$OUT'
  PRUEF=\$?
  kill \$APP 2>/dev/null
  if [ \$PRUEF -ne 0 ]; then exit \$PRUEF; fi
  exit \$TOUR
"
STATUS=$?

echo
echo "== Mauszeigerform =="
# GTK4 hat das CSS-Attribut \`cursor\` abgeschafft ("No property named cursor").
# Der Zeiger wird deshalb im Code gesetzt; hier wird geprueft, dass es
# tatsaechlich passiert und nicht nur behauptet wird.
if grep -rq "cursor_from_name" crates/ui-gtk/src; then
  echo "Zeiger im Code gesetzt (GTK4-CSS kennt kein \`cursor\`):"
  grep -rhoE "pointer\(&[a-z_]+" crates/ui-gtk/src | sort -u | tr '\n' ' '
  echo
else
  echo "FEHLT: kein set_cursor_from_name in crates/ui-gtk/src"
  STATUS=1
fi

echo
echo "== Kleinste Schriftgrößen in data/theme.css =="
grep -oE "font-size: [0-9.]+px" data/theme.css | sort -u -k2 -n | head -5

echo
echo "== Anwendungsprotokoll (Auszug) =="
tail -12 "$OUT/app.log" 2>/dev/null

echo
echo "== Bilder =="
ls -1 "$OUT"/*.png 2>/dev/null | while read -r f; do
  echo "  $f"
done

kill "$XVFB_PID" 2>/dev/null
exit "$STATUS"
