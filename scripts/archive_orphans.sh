#!/usr/bin/env bash
set -euo pipefail

# Move cargo-modules orphans to _archive/ (dry-run by default)
# Usage:
#   bash scripts/archive_orphans.sh [--apply] [--bin NAME]
# Notes:
#   - By default, processes library target; use --bin to select a binary.
#   - Requires cargo-modules.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_TAURI_DIR="$ROOT_DIR/src-tauri"
APPLY=0
BIN_NAME=""
INCLUDE_UNGUARDED=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply) APPLY=1; shift ;;
    --bin) BIN_NAME="$2"; shift 2 ;;
    --include-unguarded) INCLUDE_UNGUARDED=1; shift ;;
    *) echo "[warn] Unknown arg: $1"; shift ;;
  esac
done

if ! command -v cargo-modules >/dev/null 2>&1; then
  echo "[error] cargo-modules not installed. Run scripts/install_dep_vis_tools.sh" >&2
  exit 1
fi

cd "$SRC_TAURI_DIR"

cmd=(cargo modules orphans --all-features)
if [[ -n "$BIN_NAME" ]]; then
  cmd+=(--bin "$BIN_NAME")
else
  cmd+=(--lib)
fi

lines_tmp="$("${cmd[@]}" 2>/dev/null || true)"
if [[ -z "$lines_tmp" ]]; then
  echo "[info] No orphans output (maybe none found)."
  exit 0
fi

# Parse `orphaned module` lines to extract paths like `src/foo/bar.rs`.
files=()
while IFS= read -r ln; do
  if [[ "$ln" =~ orphaned\ module ]] && [[ "$ln" =~ at\ src/([^[:space:]]+) ]]; then
    rel="${BASH_REMATCH[1]}"
    files+=("$SRC_TAURI_DIR/src/$rel")
  fi
  if [[ "$ln" =~ orphaned\ module ]] && [[ "$ln" =~ at\ (src/[^[:space:]]+) ]]; then
    rel2="${BASH_REMATCH[1]}"
    files+=("$SRC_TAURI_DIR/$rel2")
  fi
done <<< "$lines_tmp"

# Deduplicate
unique_tmp="$(printf '%s\n' "${files[@]}" | sort -u)"
files=()
while IFS= read -r f; do
  files+=("$f")
done <<< "$unique_tmp"

if [[ ${#files[@]} -eq 0 ]]; then
  echo "[info] No orphan file paths parsed."
  exit 0
fi

ARCHIVE_DIR="$SRC_TAURI_DIR/src/_archive/orphans_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$ARCHIVE_DIR"

echo "[plan] Orphan files to archive (dry-run=$((1-APPLY))):"
kept=()
skipped=()
for f in "${files[@]}"; do
  echo " - $f"
  if [[ $APPLY -eq 1 ]]; then
    # Safety: only move guarded orphans unless --include-unguarded is set
    if [[ $INCLUDE_UNGUARDED -eq 0 ]]; then
      if ! grep -qE '^#!\[cfg\(any\(\)\)\]' "$f" 2>/dev/null; then
        echo "   [skip] Not guarded with #![cfg(any())]; use --include-unguarded to move" >&2
        skipped+=("$f")
        continue
      fi
    fi
    dest="$ARCHIVE_DIR/${f#*$SRC_TAURI_DIR/src/}"
    mkdir -p "$(dirname "$dest")"
    if git mv -f "$f" "$dest" 2>/dev/null; then
      kept+=("$dest")
    else
      mv -f "$f" "$dest"
      kept+=("$dest")
    fi
  fi
done

if [[ $APPLY -eq 1 ]]; then
  echo "[done] Moved to $ARCHIVE_DIR"
  echo "[summary] moved: ${#kept[@]}, skipped: ${#skipped[@]}"
else
  echo "[info] Dry-run only. Re-run with --apply to move files."
fi
