#!/usr/bin/env bash
set -euo pipefail

# ── Synapse Startup Script ──────────────────────────────────────────────
# Usage:
#   ./start.sh              # Interactive REPL (default)
#   ./start.sh serve        # Gateway server on :3000
#   ./start.sh bot telegram # Telegram bot
#   ./start.sh bot lark     # Lark bot
#   ./start.sh bot discord  # Discord bot
#   ./start.sh bot slack    # Slack bot
#   ./start.sh stop         # Stop all local synapse/vite processes

MODE="${1:-repl}"
shift 2>/dev/null || true

ROOT="$(cd "$(dirname "$0")" && pwd)"

# ── Features ────────────────────────────────────────────────────────────
FEATURES="web,plugins,bot-telegram,bot-discord,bot-slack,bot-lark"

# ── Helpers ─────────────────────────────────────────────────────────────
build_backend() {
  local profile="${1:-debug}"
  if [ "$profile" = "release" ]; then
    echo "Building synapse (release, features: $FEATURES)..."
    cargo build --release --features "$FEATURES"
  else
    echo "Building synapse (debug, features: $FEATURES)..."
    cargo build --features "$FEATURES"
  fi
}

stop_port() {
  local port="$1"
  local pids
  pids=$(lsof -ti:"$port" 2>/dev/null || true)
  if [ -n "$pids" ]; then
    echo "$pids" | xargs kill -9 2>/dev/null || true
    sleep 1
  fi
}

# ── Run ─────────────────────────────────────────────────────────────────
case "$MODE" in
  repl)
    build_backend release
    exec "$ROOT/target/release/synapse" "$@"
    ;;

  serve)
    PORT="${1:-3000}"
    build_backend release
    exec "$ROOT/target/release/synapse" serve --port "$PORT"
    ;;

  dev)
    # Development mode: backend (debug) + vite dev server in parallel.
    # Backend on :3000, frontend on :5173 (proxies API/WS to :3000).
    # Ctrl-C stops both.
    BACKEND_PORT="${1:-3000}"
    FRONTEND_PORT="${2:-5173}"

    build_backend debug

    stop_port "$BACKEND_PORT"

    echo ""
    echo "Starting backend on :$BACKEND_PORT ..."
    "$ROOT/target/debug/synapse" serve --port "$BACKEND_PORT" &
    BACKEND_PID=$!

    echo "Starting frontend on :$FRONTEND_PORT ..."
    cd "$ROOT/web"
    npx vite --port "$FRONTEND_PORT" &
    FRONTEND_PID=$!
    cd "$ROOT"

    echo ""
    echo "  Backend:  http://localhost:$BACKEND_PORT  (API + WS)"
    echo "  Frontend: http://localhost:$FRONTEND_PORT  (HMR, proxies to backend)"
    echo ""
    echo "  Press Ctrl-C to stop both."
    echo ""

    cleanup() {
      echo ""
      echo "Shutting down..."
      kill "$FRONTEND_PID" 2>/dev/null
      kill "$BACKEND_PID" 2>/dev/null
      wait "$FRONTEND_PID" 2>/dev/null
      wait "$BACKEND_PID" 2>/dev/null
      echo "Done."
    }
    trap cleanup INT TERM

    wait
    ;;

  build)
    # Build frontend + backend for production.
    echo "Building frontend..."
    cd "$ROOT/web"
    npm run build
    cd "$ROOT"

    build_backend release
    echo ""
    echo "Done. Run: ./start.sh serve"
    ;;

  bot)
    PLATFORM="${1:?usage: ./start.sh bot <platform>}"
    shift
    build_backend release
    exec "$ROOT/target/release/synapse" bot "$PLATFORM" "$@"
    ;;

  stop)
    echo "Stopping synapse and vite processes..."
    count=0
    # Kill synapse processes
    pids=$(pgrep -f "target/(debug|release)/synapse" 2>/dev/null || true)
    if [ -n "$pids" ]; then
      echo "$pids" | xargs kill 2>/dev/null || true
      count=$((count + $(echo "$pids" | wc -l)))
    fi
    # Kill vite dev server
    pids=$(pgrep -f "vite.*--port" 2>/dev/null || true)
    if [ -n "$pids" ]; then
      echo "$pids" | xargs kill 2>/dev/null || true
      count=$((count + $(echo "$pids" | wc -l)))
    fi
    # Clean up ports 3000 and 5173
    for port in 3000 5173; do
      pids=$(lsof -ti:"$port" 2>/dev/null || true)
      if [ -n "$pids" ]; then
        echo "$pids" | xargs kill -9 2>/dev/null || true
        count=$((count + $(echo "$pids" | wc -l)))
      fi
    done
    if [ "$count" -gt 0 ]; then
      echo "Stopped $count process(es)."
    else
      echo "No running processes found."
    fi
    ;;

  connect)
    URL="${1:?usage: ./start.sh connect <ws://host:port>}"
    shift
    build_backend release
    exec "$ROOT/target/release/synapse" connect "$URL" "$@"
    ;;

  *)
    echo "Synapse startup script"
    echo ""
    echo "Usage: ./start.sh <mode> [args...]"
    echo ""
    echo "Modes:"
    echo "  repl                  Interactive REPL (default)"
    echo "  serve [port]          Gateway server (default: 3000)"
    echo "  dev [port] [fe-port]  Dev mode: backend + vite HMR (3000 + 5173)"
    echo "  build                 Build frontend + backend for production"
    echo "  bot <platform>        Start bot adapter (telegram, lark, etc.)"
    echo "  stop                  Stop all local synapse/vite processes"
    echo "  connect <ws-url>      Connect to remote gateway"
    exit 1
    ;;
esac
