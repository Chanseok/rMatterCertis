#!/usr/bin/env bash
# Unified-only E2E: start dev with unified flags, wait for plan completion, stop, verify logs.
set -uo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
LOG_DIR="$ROOT_DIR/src-tauri/target/debug/logs"
BACK_LOG="$LOG_DIR/back_front.log"

# Kill leftover dev server if any
lsof -ti:1420 | xargs kill -9 2>/dev/null || true

# Clean old logs
rm -f "$LOG_DIR"/*.log 2>/dev/null || true
mkdir -p "$LOG_DIR"

echo "[run_and_verify_unified] starting tauri:dev with unified flags..."
export VITE_DISABLE_LEGACY_EVENTS=true
export MC_FEATURE_EVENTS_GENERALIZED_ONLY=1
export MC_FEATURE_LEGACY_DOMAIN_EVENTS=0

# Capture dev output to a file by default
if [[ "${DEV_LOG_CAPTURE:-1}" == "1" ]]; then
  DEV_PROC_LOG="$LOG_DIR/dev_process_unified.log"
  echo "[run_and_verify_unified] capturing dev output to $DEV_PROC_LOG"
  npm run -s tauri:dev > "$DEV_PROC_LOG" 2>&1 &
else
  echo "[run_and_verify_unified] not capturing dev output (DEV_LOG_CAPTURE=0)"
  npm run -s tauri:dev &
fi
TAURI_PID=${!:-}
echo "[run_and_verify_unified] tauri dev pid=${TAURI_PID:-unknown}"

# Wait until back_front.log exists
TIMEOUT=${TIMEOUT:-180}
for ((i=0; i<TIMEOUT; i++)); do
  [[ -f "$BACK_LOG" ]] && break
  sleep 1
done

# Wait for completion marker (multiple signals considered)
EVENTS_LOG="$LOG_DIR/events.log"
for ((i=0; i<TIMEOUT; i++)); do
  if [[ -f "$BACK_LOG" ]]; then
    if grep -qa "ExecutionPlan fully executed" "$BACK_LOG"; then
      echo "[run_and_verify_unified] completion detected (legacy execution marker)"
      break
    fi
    if grep -qa "Session Final Summary |" "$BACK_LOG"; then
      echo "[run_and_verify_unified] completion detected (Session Final Summary)"
      break
    fi
    if grep -qa "SessionActor .* execution loop ended" "$BACK_LOG"; then
      echo "[run_and_verify_unified] completion detected (SessionActor loop ended)"
      break
    fi
    if grep -qa "Actor Event Bridge stopped" "$BACK_LOG"; then
      echo "[run_and_verify_unified] completion detected (Bridge stopped)"
      break
    fi
  fi
  if [[ -f "$EVENTS_LOG" ]] && grep -qa '"event":"session_final_summary"' "$EVENTS_LOG"; then
    echo "[run_and_verify_unified] completion detected (kpi.session session_final_summary)"
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
if DISABLE_STRUCTURED_CHECKS=1 bash "$ROOT_DIR/scripts/verify_runtime_plan.sh" "$BACK_LOG"; then
  echo "[run_and_verify_unified] verify passed"
else
  status=$?
  echo "[run_and_verify_unified] verify failed with exit code $status" >&2
  exit $status
fi
