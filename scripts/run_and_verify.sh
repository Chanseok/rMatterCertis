#!/usr/bin/env bash
set -euo pipefail

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
(npm run -s tauri:dev > "$ROOT_DIR/run2.log" 2>&1 &) 
TAURI_PID=$!
echo "[run_and_verify] tauri dev pid=$TAURI_PID"

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
kill -9 "$TAURI_PID" 2>/dev/null || true

# Verify
bash "$ROOT_DIR/scripts/verify_runtime_plan.sh" "$BACK_LOG"

echo "[run_and_verify] verify passed"
