#!/usr/bin/env bash
# Lancement simplifié du serveur de domino.
# Usage :
#   ./lancer.sh                       # port 8080, 4 places, bots sur les places vides
#   ./lancer.sh --port 9000 --players 2
set -euo pipefail
cd "$(dirname "$0")"

BIN=./target/release/domino

# Compilation (release) si le binaire n'existe pas.
if [ ! -x "$BIN" ]; then
    echo "🔨 Première compilation (release)…"
    cargo build --release
fi

echo "🁫 Lancement du serveur de domino…"
exec "$BIN" --bot-fill "$@"
