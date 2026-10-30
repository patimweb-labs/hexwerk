#!/usr/bin/env bash
# Erzeugt flatpak/new-sources.json aus Cargo.lock.
#
# Aufruf nach jeder Änderung an Cargo.lock (neues Paket, cargo update):
#   bash flatpak/update-sources.sh
#
# Das Werkzeug kommt aus flatpak-builder-tools und braucht Python mit aiohttp,
# toml und tomlkit. Ein eigenes venv hält das aus dem System-Python heraus.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
venv="${HEXWERK_VENV:-/tmp/venv-fpcg}"
gen="$here/.flatpak-cargo-generator.py"

if [ ! -x "$venv/bin/python" ]; then
  echo "lege venv an: $venv"
  python3 -m venv "$venv"
  "$venv/bin/pip" -q install --upgrade pip
  "$venv/bin/pip" -q install aiohttp toml tomlkit
fi

if [ ! -f "$gen" ]; then
  echo "lade flatpak-cargo-generator.py"
  curl -fsSL -o "$gen" \
    https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
fi

"$venv/bin/python" "$gen" "$root/Cargo.lock" -o "$here/new-sources.json"

# Gegenprobe: die Zahl der Archive muss zu den externen Paketen im Lock passen.
archives=$(grep -c '"type": "archive"' "$here/new-sources.json" || true)
lock=$(grep -c '^name = ' "$root/Cargo.lock" || true)
echo "new-sources.json: $archives Archive, Cargo.lock: $lock Pakete (Differenz sind die eigenen Crates)"
