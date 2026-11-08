#!/usr/bin/env bash
# ============================================================
#  Domino — lancer / arrêter le serveur facilement
#
#  Usage :
#    ./serveur.sh              # = start (port 8080)
#    ./serveur.sh start [port]
#    ./serveur.sh stop   [port]
#    ./serveur.sh restart[port]
#    ./serveur.sh status [port]
# ============================================================
set -u

ACTION="${1:-start}"
PORT="${2:-8080}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$ROOT/target/release/domino"
LOG="$ROOT/serveur-$PORT.log"

# PID du processus qui écoute sur le port (vide si personne)
pid_on_port() {
  ss -ltnp 2>/dev/null | grep ":$PORT\b" | grep -oP 'pid=\K[0-9]+' | head -1
}

do_start() {
  local pid; pid="$(pid_on_port "$PORT")"
  if [ -n "$pid" ]; then
    echo "⚠️  Un serveur tourne déjà sur le port $PORT (PID $pid)."
    echo "   → ./serveur.sh stop pour l'arrêter, ou ./serveur.sh restart"
    return 1
  fi

  if [ ! -x "$BIN" ]; then
    echo "🔨 Binaire release introuvable — première compilation..."
    (cd "$ROOT" && cargo build --release -p domino-server) || { echo "❌ Échec de la compilation."; return 1; }
  elif [ -n "$(find "$ROOT/crates" \( -name '*.rs' -o -name '*.html' \) -newer "$BIN" 2>/dev/null | head -1)" ]; then
    # Le code (ou le client web embarqué) est plus récent que le binaire :
    # on recompile pour ne jamais servir une vieille version.
    echo "🔨 Code modifié depuis le dernier build — recompilation..."
    (cd "$ROOT" && cargo build --release -p domino-server) || { echo "❌ Échec de la compilation."; return 1; }
  fi

  nohup "$BIN" --port "$PORT" >"$LOG" 2>&1 &
  local pid=$!
  for _ in $(seq 1 50); do
    [ -n "$(pid_on_port "$PORT")" ] && break
    sleep 0.1
  done

  if [ -z "$(pid_on_port "$PORT")" ]; then
    echo "❌ Le serveur n'a pas démarré — derniers lignes du log :"
    tail -5 "$LOG"
    return 1
  fi
  echo "✅ Domino lancé (PID $pid) → http://localhost:$PORT   [log : $LOG]"
}

do_stop() {
  local pid; pid="$(pid_on_port "$PORT")"
  if [ -z "$pid" ]; then
    echo "ℹ️  Aucun serveur sur le port $PORT."
    return 0
  fi
  kill "$pid" && echo "🛑 Serveur arrêté (PID $pid)."
}

do_status() {
  local pid; pid="$(pid_on_port "$PORT")"
  if [ -n "$pid" ]; then
    echo "🟢 Serveur en cours d'exécution sur le port $PORT (PID $pid) → http://localhost:$PORT"
  else
    echo "⚪ Aucun serveur sur le port $PORT."
  fi
}

case "$ACTION" in
  start)   do_start ;;
  stop)    do_stop ;;
  restart) do_stop; sleep 0.5; do_start ;;
  status)  do_status ;;
  *) echo "Usage : ./serveur.sh [start|stop|restart|status] [port]   (défaut : start, port 8080)" ;;
esac
