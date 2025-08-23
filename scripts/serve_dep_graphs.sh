#!/usr/bin/env bash
set -euo pipefail

# Serve docs/dep-graphs over HTTP (for the interactive viewer) and optionally regenerate graphs.
# Usage:
#   scripts/serve_dep_graphs.sh [--port 8844] [--regen] [--focus-dir PATH] [--max-depth N] [--bin NAME]
# Examples:
#   scripts/serve_dep_graphs.sh --regen --focus-dir src-tauri/src/crawl_engine --max-depth 2
#   scripts/serve_dep_graphs.sh --port 8080

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DOCS_DIR="$ROOT_DIR/docs/dep-graphs"
GEN_SCRIPT="$ROOT_DIR/scripts/generate_dep_graphs.sh"
PORT=8844
REGEN=0
FOCUS_DIR=""
MAX_DEPTH=""
BIN_NAME=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      PORT="$2"; shift 2 ;;
    --regen)
      REGEN=1; shift ;;
    --focus-dir)
      FOCUS_DIR="$2"; shift 2 ;;
    --max-depth)
      MAX_DEPTH="$2"; shift 2 ;;
    --bin)
      BIN_NAME="$2"; shift 2 ;;
    -h|--help)
      cat <<USAGE
Serve dep-graphs

Usage: $0 [--port 8844] [--regen] [--focus-dir PATH] [--max-depth N] [--bin NAME]

Options:
  --port PORT         Port to serve (default: 8844)
  --regen             Regenerate graph JSON/DOT/SVG before serving
  --focus-dir PATH    Focus directory to pass to generator (e.g., src-tauri/src/crawl_engine)
  --max-depth N       Max depth to pass to generator
  --bin NAME          Use a specific bin target for structure/graph generation

Then open: http://127.0.0.1:
  $PORT/index.html
USAGE
      exit 0 ;;
    *)
      echo "[warn] Unknown arg: $1"; shift ;;
  esac
done

if [[ ! -d "$DOCS_DIR" ]]; then
  echo "[info] Creating $DOCS_DIR"
  mkdir -p "$DOCS_DIR"
fi

if [[ $REGEN -eq 1 ]]; then
  echo "[regen] Generating graphs before serve"
  gen_args=()
  [[ -n "$FOCUS_DIR" ]] && gen_args+=(--focus-dir "$FOCUS_DIR")
  [[ -n "$MAX_DEPTH" ]] && gen_args+=(--max-depth "$MAX_DEPTH")
  [[ -n "$BIN_NAME" ]] && gen_args+=(--bin "$BIN_NAME")
  (cd "$ROOT_DIR" && bash "$GEN_SCRIPT" "${gen_args[@]}")
fi

# Find a free port if needed
is_port_busy() {
  if command -v lsof >/dev/null 2>&1; then
    lsof -iTCP -sTCP:LISTEN -P 2>/dev/null | grep -q ":$PORT\>" || return 1
    return 0
  fi
  return 1 # assume free if no lsof
}

if is_port_busy; then
  base=$PORT
  for p in $(seq $((base+1)) $((base+50))); do
    PORT=$p
    if ! is_port_busy; then
      echo "[serve] Port $base busy; using $PORT"
      break
    fi
  done
fi

serve_with_python3() {
  (cd "$DOCS_DIR" && python3 -m http.server "$PORT")
}
serve_with_python2() {
  (cd "$DOCS_DIR" && python -m SimpleHTTPServer "$PORT")
}
serve_with_php() {
  (cd "$DOCS_DIR" && php -S 127.0.0.1:"$PORT" -t .)
}
serve_with_ruby() {
  (cd "$DOCS_DIR" && ruby -run -e httpd . -p "$PORT")
}

URL="http://127.0.0.1:$PORT/index.html"
echo "[serve] Serving $DOCS_DIR at $URL"

# Try to open the browser on macOS
if [[ "${OSTYPE:-}" == darwin* ]]; then
  command -v open >/dev/null 2>&1 && open "$URL" || true
fi

# Start server with best available runtime
if command -v python3 >/dev/null 2>&1; then
  serve_with_python3
elif command -v python >/dev/null 2>&1; then
  serve_with_python2
elif command -v php >/dev/null 2>&1; then
  serve_with_php
elif command -v ruby >/dev/null 2>&1; then
  serve_with_ruby
else
  echo "[error] No simple HTTP server found (python3/python/php/ruby)."
  echo "        Please install Python 3 or use VS Code Live Preview to open $DOCS_DIR/index.html"
  exit 1
fi
