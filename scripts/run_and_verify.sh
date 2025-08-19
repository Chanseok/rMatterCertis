#!/usr/bin/env bash
# Relax error handling to avoid early exit during dev runs (remove -e)
set -uo pipefail

# Simple E2E: start dev, wait for plan completion, stop, verify logs.
ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
LOG_DIR="$ROOT_DIR/src-tauri/target/debug/logs"
BACK_LOG="$LOG_DIR/back_front.log"

# Kill leftover dev server if any
lsof -ti:1420 | xargs kill -9 2>/dev/null || true

# Clean old logs
rm -f "$LOG_DIR"/*.log 2>/dev/null || true
mkdir -p "$LOG_DIR"

# Start dev (frontend+tauri via npm script)
echo "[run_and_verify] starting tauri:dev..."
# Capture dev process output optionally; default ON but write inside unified log dir to avoid stray root logs.
if [[ "${DEV_LOG_CAPTURE:-1}" == "1" ]]; then
  DEV_PROC_LOG="$LOG_DIR/dev_process.log"
  echo "[run_and_verify] capturing dev output to $DEV_PROC_LOG"
  # Start in background in current shell (no subshell) so $! is available under set -u
  npm run -s tauri:dev > "$DEV_PROC_LOG" 2>&1 &
else
  echo "[run_and_verify] not capturing dev output (DEV_LOG_CAPTURE=0)"
  npm run -s tauri:dev &
fi
TAURI_PID=${!:-}
echo "[run_and_verify] tauri dev pid=${TAURI_PID:-unknown}"

# Wait until back_front.log exists
TIMEOUT=${TIMEOUT:-180}
for ((i=0; i<TIMEOUT; i++)); do
  [[ -f "$BACK_LOG" ]] && break
  sleep 1
done

# Wait for completion marker
for ((i=0; i<TIMEOUT; i++)); do
  if [[ -f "$BACK_LOG" ]] && grep -qa "ExecutionPlan fully executed" "$BACK_LOG"; then
    echo "[run_and_verify] completion detected"
    break
  fi
  sleep 1
done

# Stop app
lsof -ti:1420 | xargs kill -9 2>/dev/null || true
if [[ -n "${TAURI_PID:-}" ]]; then
  kill -9 "$TAURI_PID" 2>/dev/null || true
fi

# Verify
bash "$ROOT_DIR/scripts/verify_runtime_plan.sh" "$BACK_LOG"

echo "[run_and_verify] verify passed"
