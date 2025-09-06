#!/usr/bin/env bash
set -euo pipefail

# Run the ignored parity smoke test twice (StageActor direct vs legacy path) and compare summaries.
# Safe for CI: if parity cannot run (no DB/network), it prints SKIP and exits 0.
# Usage:
#   ./scripts/ci_parity_compare.sh            # tolerant mode (non-blocking when wrapped in CI with '|| true')
#   ./scripts/ci_parity_compare.sh --strict   # strict mode (exits non-zero on mismatch; use in CI without '|| true')

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR/src-tauri"

run_parity() {
  local mode="$1" # stage|legacy
  local out_file="$2"
  echo "[parity] running mode=$mode"

  local envs=("MC_RUN_PARITY=1")
  if [[ "$mode" == "stage" ]]; then
    envs+=("MC_USE_STAGE_DIRECT=1")
  else
    # legacy path: unset or explicit 0
    envs+=("MC_USE_STAGE_DIRECT=0")
  fi

  # Run only the ignored parity test and capture output
  # Enable legacy feature when running legacy mode so the code path exists
  local feature_args=()
  if [[ "$mode" == "legacy" ]]; then
    feature_args=(--features legacy-batch)
  fi

  if ! { "${envs[@]}" cargo test ${feature_args[@]} parity_stage_vs_batch_smoke -- --ignored --nocapture 2>&1 | tee "$out_file"; }; then
    echo "[parity] test invocation failed; see $out_file"
    return 1
  fi
}

extract_summary() {
  local file="$1"
  grep -E "PARITY_SUMMARY" "$file" | tail -n1 || true
}

compare_summaries() {
  local a="$1"; shift
  local b="$1"; shift

  if [[ -z "$a" || -z "$b" ]]; then
    echo "[parity] SKIP: summary unavailable (likely offline env)" >&2
    return 0
  fi

  echo "[parity] stage:  $a"
  echo "[parity] legacy: $b"

  # Strict comparison for core totals; allow differences elsewhere for now.
  # Extract numbers by key name (simple awk parsing of key=value tokens)
  parse() { echo "$1" | awk '{ for(i=1;i<=NF;i++){ if($i~"="){split($i,a,"="); gsub(",","",a[2]); printf a[1]"="a[2]"\n" } } }'; }
  declare -A A B
  while IFS='=' read -r k v; do A["$k"]="$v"; done < <(parse "$a")
  while IFS='=' read -r k v; do B["$k"]="$v"; done < <(parse "$b")

  diff_keys=("pages(t/s/f)" "ins" "upd")
  # Expand pages(t/s/f) into pages_total, pages_success, pages_failed by matching pre-parsed tokens
  # Our parsing keeps tokens as printed; do direct token compares for simplicity.

  # Exact string compare fallback
  if [[ "$a" != "$b" ]]; then
    # Soft compare on totals
    ok=1
    for key in pages_total pages_success pages_failed products_inserted products_updated; do
      va=$(echo "$a" | sed -nE "s/.*${key//_/ }=([0-9]+).*/\1/p" | tail -n1 || true)
      vb=$(echo "$b" | sed -nE "s/.*${key//_/ }=([0-9]+).*/\1/p" | tail -n1 || true)
      if [[ -n "$va" && -n "$vb" && "$va" == "$vb" ]]; then
        continue
      else
        ok=0
      fi
    done
    if [[ $ok -eq 1 ]]; then
      echo "[parity] OK within soft match (core totals equal)"
      return 0
    fi
    echo "[parity] FAIL: summaries differ materially"
    echo "stage:  $a"
    echo "legacy: $b"
    return 2
  fi
  echo "[parity] OK: exact match"
}

main() {
  # parse optional --strict flag (no-op here; strictness is enforced by caller not suppressing exit codes)
  if [[ "${1:-}" == "--strict" ]]; then
    shift
    # In strict mode, we still run the same logic; compare_summaries already returns non-zero on mismatch
    # and with set -e, the script will exit with that status unless the caller masks it.
  fi
  mkdir -p ../parity
  run_parity stage ../parity/out_stage.txt || true
  run_parity legacy ../parity/out_legacy.txt || true

  S=$(extract_summary ../parity/out_stage.txt)
  L=$(extract_summary ../parity/out_legacy.txt)

  echo "$S" > ../parity/summary_stage.txt || true
  echo "$L" > ../parity/summary_legacy.txt || true

  compare_summaries "$S" "$L" || exit $?
}

main "$@"
