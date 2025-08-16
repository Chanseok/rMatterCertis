#!/usr/bin/env bash
# verify_runtime_plan.sh
# Runtime crawling plan & execution path validator (log-based)
#
# Goals:
#  1. Single CrawlingPlan emission
#  2. ListPageCrawling phase count == Stage 2 start count == Stage 2 completion count
#  3. No RangeLoopAnomaly warnings
#  4. Optional partial-page reinclusion appears <=1 (planner unified)
#  5. No duplicate legacy range logs ("[RangeLoop] ENTER" without corresponding plan phases mismatch)
#  6. (Optional) Pages in each ListPageCrawling phase descending newest→oldest
#
# Usage:
#   ./scripts/verify_runtime_plan.sh /path/to/logfile
#   LOG_FILE=... ./scripts/verify_runtime_plan.sh
#
# Exit codes:
#   0 success
#   10 missing log file
#   11 zero plan lines
#   12 multiple plan lines (duplicate planning)
#   13 list phase / stage start mismatch
#   14 list phase / stage completion mismatch
#   15 anomaly warnings present
#   16 partial page reinclusion duplicated
#   17 descending order violation (experimental)
#   18 db analysis cache hit but no page/index enrichment
#   19 mismatch flags present in session_summary (structured)
#   20 batch_start / batch_complete count mismatch
#   21 batch pages sum > completed_pages or logical mismatch
#   22 missing session_summary structured event
#   23 plan_id mismatch between events or missing plan_hash_assigned
#   24 inconsistent plan_id across batch events
#
# Limitations:
#   - Descending order check is heuristic (simple regex parse per phase)
#   - Assumes whole CrawlingPlan printed on a single line
#
set -euo pipefail
LOG_FILE="${1:-${LOG_FILE:-}}"
if [[ -z "${LOG_FILE}" ]]; then
  echo "[ERR] Provide log file path as argument or LOG_FILE env" >&2; exit 10
fi
if [[ ! -f "${LOG_FILE}" ]]; then
  echo "[ERR] Log file not found: ${LOG_FILE}" >&2; exit 10
fi

# Derive sibling logs to support split logging (events.log + back_front.log)
LOG_DIR=$(dirname "${LOG_FILE}")
EVENTS_LOG="${LOG_DIR}/events.log"
BACK_FRONT_LOG="${LOG_DIR}/back_front.log"

# Choose files for different checks
# - KPI/structured events: prefer events.log if it exists
# - Stage textual lines: prefer back_front.log if it exists
KPI_FILE="${LOG_FILE}"
STAGE_FILE="${LOG_FILE}"
if [[ -f "${EVENTS_LOG}" ]]; then
  KPI_FILE="${EVENTS_LOG}"
fi
if [[ -f "${BACK_FRONT_LOG}" ]]; then
  STAGE_FILE="${BACK_FRONT_LOG}"
fi

plan_lines=$(grep -a "📋 CrawlingPlan created:" "${LOG_FILE}" || true)
plan_count=$(printf "%s" "${plan_lines}" | grep -c "CrawlingPlan created" || true)

# Prefer structured plan_created event for robustness; fall back to legacy line parsing
structured_plan_line=$(grep -a '"event":"plan_created"' "${KPI_FILE}" | tail -n1 || true)
if [[ -n "${structured_plan_line}" ]]; then
  structured_mode=1
  list_phase_count=$(echo "${structured_plan_line}" | sed -E 's/.*"list_phase_count":([0-9]+).*/\1/' | head -n1)
  if ! [[ ${list_phase_count} =~ ^[0-9]+$ ]]; then list_phase_count=0; fi
else
  structured_mode=0
  if [[ ${plan_count} -eq 0 ]]; then
    echo "[FAIL] No CrawlingPlan line (legacy) nor structured plan_created event found"; exit 11
  fi
  if [[ ${plan_count} -gt 1 ]]; then
    echo "[FAIL] Duplicate plan creation entries: ${plan_count}"; exit 12
  fi
  # Count list phases inside the single legacy plan line
  list_phase_count=$(printf "%s" "${plan_lines}" | grep -o "phase_type: ListPageCrawling" | wc -l | tr -d ' ')
fi
# Stage 2 start & completion counts (from stage/text log file)
stage2_start_count=$(grep -a "Starting Stage 2: ListPageCrawling" "${STAGE_FILE}" | wc -l | tr -d ' ')
stage2_done_count=$(grep -a "Stage 2 (ListPageCrawling) completed" "${STAGE_FILE}" | wc -l | tr -d ' ')

if [[ ${list_phase_count} -ne ${stage2_start_count} ]]; then
  echo "[FAIL] Phase/start mismatch: phases=${list_phase_count} starts=${stage2_start_count}"; exit 13
fi
if [[ ${list_phase_count} -ne ${stage2_done_count} ]]; then
  echo "[FAIL] Phase/completion mismatch: phases=${list_phase_count} completions=${stage2_done_count}"; exit 14
fi

# Anomaly warnings
if grep -a -q "RangeLoopAnomaly" "${LOG_FILE}"; then
  echo "[FAIL] RangeLoopAnomaly warning present"; exit 15
fi

# Partial page reinclusion occurrences
ppi_count=$(grep -a "Partial page re-included" "${LOG_FILE}" | wc -l | tr -d ' ' || true)
if [[ ${ppi_count} -gt 1 ]]; then
  echo "[FAIL] Partial page reinclusion logged more than once (${ppi_count})"; exit 16
fi

# Experimental: ensure descending order within each ListPageCrawling phase's pages array in plan line
# Extract bracket segments pages: [n1, n2, ...]
# We only inspect the plan line; if truncated, skip check.
if [[ ${list_phase_count} -gt 0 ]]; then
  pages_snippets=$(printf "%s" "${plan_lines}" | grep -o "pages: \[[^]]*\]" || true)
  # Only take those that belong to ListPageCrawling (order preserved)
  idx=0
  while IFS= read -r segment; do
    # Extract numbers
    nums=$(printf "%s" "$segment" | sed -E 's/.*pages: \[([^]]*)\].*/\1/' | tr ',' ' ')
    # Build array and test descending
    prev=""
    for n in $nums; do
      n_trim=$(echo "$n" | xargs)
      [[ -z "$n_trim" ]] && continue
      if [[ -n "$prev" ]]; then
        if (( n_trim > prev )); then
          echo "[FAIL] Descending order violated in phase index ${idx}: ${segment}"; exit 17
        fi
      fi
      prev=$n_trim
    done
    idx=$((idx+1))
  done <<< "$pages_snippets"
fi

# Legacy range loop duplication check: more than one "[RangeLoop] ENTER" while only 1 phase would be suspicious.
range_loop_enters=$(grep -a "\[RangeLoop\] ENTER" "${LOG_FILE}" | wc -l | tr -d ' ' || true)
if [[ ${range_loop_enters} -gt 0 && ${list_phase_count} -eq 0 ]]; then
  echo "[WARN] RangeLoop ENTER logs present but no ListPageCrawling phases (legacy path?)";
fi

# Non-fatal warning if structured incomplete event present
if grep -a -q '"event":"range_loop_incomplete"' "${KPI_FILE}"; then
  echo "[WARN] range_loop_incomplete KPI present (some ranges not executed)";
fi

# ──────────────────────────────────────────────
# Structured KPI validation (Step 7)
# Skip if disabled explicitly
if [[ "${DISABLE_STRUCTURED_CHECKS:-0}" != "1" ]]; then
  plan_created_line=$(grep -a 'kpi.plan' "${KPI_FILE}" | grep '"event":"plan_created"' | tail -n1 || true)
  plan_hash_line=$(grep -a 'kpi.plan' "${KPI_FILE}" | grep '"event":"plan_hash_assigned"' | tail -n1 || true)
  session_summary_line=$(grep -a 'kpi.session' "${KPI_FILE}" | grep '"event":"session_summary"' | tail -n1 || true)
  batch_start_lines=$(grep -a 'kpi.batch' "${KPI_FILE}" | grep '"event":"batch_start"' || true)
  batch_complete_lines=$(grep -a 'kpi.batch' "${KPI_FILE}" | grep '"event":"batch_complete"' || true)

  # Scope KPI batch events to the latest session_id to avoid cross-session mismatches in aggregated logs
  session_id_session=$(echo "${session_summary_line}" | sed -E 's/.*"session_id":"([^"]+)".*/\1/' )
  if [[ -n "${session_id_session}" ]]; then
    batch_start_lines=$(printf "%s" "${batch_start_lines}" | grep -F '"session_id":"' | grep -F "\"session_id\":\"${session_id_session}\"" || true)
    batch_complete_lines=$(printf "%s" "${batch_complete_lines}" | grep -F '"session_id":"' | grep -F "\"session_id\":\"${session_id_session}\"" || true)
  fi

  if [[ -z "${session_summary_line}" ]]; then
    echo "[FAIL] Missing session_summary structured event"; exit 22
  fi

  # Extract core fields (sed tolerant if field missing -> empty)
  plan_id_created=$(echo "${plan_created_line}" | sed -E 's/.*"plan_id":"([^"]+)".*/\1/' )
  plan_id_hash=$(echo "${plan_hash_line}" | sed -E 's/.*"plan_id":"([^"]+)".*/\1/' )
  plan_id_session=$(echo "${session_summary_line}" | sed -E 's/.*"plan_id":"([^"]+)".*/\1/' )
  plan_hash=$(echo "${plan_hash_line}" | sed -E 's/.*"plan_hash":"([^"]+)".*/\1/' )
  mismatch_flags=$(echo "${session_summary_line}" | sed -E 's/.*"mismatch_flags":(\[[^]]*\]).*/\1/' )
  failed_count=$(echo "${session_summary_line}" | sed -E 's/.*"failed_count":([0-9]+).*/\1/' )
  completed_pages_struct=$(echo "${session_summary_line}" | sed -E 's/.*"completed_pages":([0-9]+).*/\1/' )
  expected_pages_struct=$(echo "${session_summary_line}" | sed -E 's/.*"expected_pages":([0-9]+).*/\1/' )

  # Plan ID consistency
  if [[ -n "${plan_created_line}" ]]; then
    if [[ -z "${plan_hash_line}" || -z "${plan_hash}" || "${plan_hash}" == "${plan_hash_line}" && "${plan_hash}" == "" ]]; then
      echo "[FAIL] plan_hash_assigned event missing or empty"; exit 23
    fi
  fi
  if [[ -n "${plan_id_created}" && -n "${plan_id_hash}" && -n "${plan_id_session}" ]]; then
    if [[ "${plan_id_created}" != "${plan_id_hash}" || "${plan_id_created}" != "${plan_id_session}" ]]; then
      echo "[FAIL] plan_id mismatch across events"; exit 23
    fi
  fi

  # Mismatch flags failure if not empty array
  if [[ -n "${mismatch_flags}" && "${mismatch_flags}" != "[]" ]]; then
    echo "[FAIL] mismatch_flags present: ${mismatch_flags}"; exit 19
  fi

  # Batch counts
  batch_start_count=$(echo "${batch_start_lines}" | grep -c 'batch_start' || true)
  batch_complete_count=$(echo "${batch_complete_lines}" | grep -c 'batch_complete' || true)
  if [[ ${batch_start_count} -ne ${batch_complete_count} ]]; then
    echo "[FAIL] batch_start (${batch_start_count}) != batch_complete (${batch_complete_count})"; exit 20
  fi

  # Ensure plan_id consistent in batch events (if present)
  if [[ ${batch_start_count} -gt 0 ]]; then
    bad_plan_batch=$(echo "${batch_start_lines}" | awk -F '"plan_id":"' '{if(NF>1){split($2,a,"\""); if(a[1] != "'"${plan_id_session}"'") print a[1];}}' | head -n1 || true)
    if [[ -n "${bad_plan_batch}" ]]; then
      echo "[FAIL] Inconsistent plan_id in batch_start events (${bad_plan_batch})"; exit 24
    fi
  fi

  # Sum pages from batch_start lines (pages":N)
  batch_pages_sum=$(echo "${batch_start_lines}" | grep -o '"pages":[0-9]*' | sed -E 's/"pages"://' | awk '{s+=$1} END{print s+0}')
  if [[ -n "${completed_pages_struct}" && ${batch_pages_sum:-0} -gt ${completed_pages_struct:-0} ]]; then
    echo "[FAIL] Sum of batch pages (${batch_pages_sum}) > completed_pages (${completed_pages_struct})"; exit 21
  fi
  if [[ -n "${expected_pages_struct}" && ${batch_pages_sum:-0} -gt ${expected_pages_struct:-0} ]]; then
    echo "[FAIL] Sum of batch pages (${batch_pages_sum}) > expected_pages (${expected_pages_struct})"; exit 21
  fi
fi

# Per-product detail lifecycle basic sanity (non-fatal warnings first run)
detail_started=$(grep -a '"status":"detail_started"' "${KPI_FILE}" | wc -l | tr -d ' ' || true)
detail_completed=$(grep -a '"status":"detail_completed"' "${KPI_FILE}" | wc -l | tr -d ' ' || true)
detail_failed=$(grep -a '"status":"detail_failed"' "${KPI_FILE}" | wc -l | tr -d ' ' || true)
if [[ ${detail_started} -gt 0 ]]; then
  if [[ $((detail_completed + detail_failed)) -ne ${detail_started} ]]; then
    echo "[WARN] detail_started (${detail_started}) != completed+failed ($((detail_completed + detail_failed)))"
  fi
  if [[ ${detail_failed} -gt 0 ]]; then
    echo "[WARN] detail_failed count=${detail_failed} (investigate if unusually high)"
  fi
fi

# If events.log exists suggest using it
events_log_dir=$(dirname "${LOG_FILE}")
if [[ -f "${events_log_dir}/events.log" ]]; then
  echo "[INFO] events.log detected (actor-event + kpi targets separated)"
fi

# DB analysis cache enrichment check
if grep -a -q '"db_analysis_cache_hit"' "${KPI_FILE}" 2>/dev/null || true; then
  # If cache hit but DB snapshot shows None for both metrics -> fail
  if (grep -a '🧾 DB snapshot:' "${LOG_FILE}" | tail -n1 || true) | grep -q 'max_page_id=None'; then
    echo "[FAIL] DB analysis cache hit but max_page_id still None (no enrichment)"; exit 18
  fi
fi

echo "[OK] Plan verification passed: plan=1 list_phases=${list_phase_count} starts=${stage2_start_count} completes=${stage2_done_count} partial_page_reinclusions=${ppi_count}";
exit 0
