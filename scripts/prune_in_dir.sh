#!/usr/bin/env sh
# Archive (git mv) a list of files (stdin) and run cargo check.
# Usage examples:
#   scripts/list_unused_in_dir.sh commands | scripts/prune_in_dir.sh
# or
#   scripts/prune_in_dir.sh <<EOF
#    - src-tauri/src/commands/foo.rs
#   EOF

set -eu
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

STAMP=$(date +%Y%m%dT%H%M%S)
ARCHIVE_DIR="src-tauri/src/_archive/dir_sweep_${STAMP}"
mkdir -p "$ARCHIVE_DIR"

TMP_CANDIDATES=$(mktemp)
# Normalize input lines: remove leading " - " and trailing risk annotations like " (risk=...)"
sed -n 's/^ - //p' | sed 's/ (risk=[^)]*)$//' > "$TMP_CANDIDATES"

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[info] No candidates on stdin. Nothing to do."
  exit 0
fi

COUNT=$(wc -l < "$TMP_CANDIDATES" | tr -d ' ')
echo "[archive] Moving $COUNT files to $ARCHIVE_DIR"

while IFS= read -r f; do
  [ -f "$f" ] || { echo "[skip] Not found: $f" >&2; continue; }
  case "$f" in
    src-tauri/src/*) :;;
    *) echo "[skip] Outside src-tauri/src: $f" >&2; continue;;
  esac
  dest_dir="$ARCHIVE_DIR/$(dirname "${f#src-tauri/src/}")"
  mkdir -p "$dest_dir"
  git mv -f "$f" "$dest_dir/"
done < "$TMP_CANDIDATES"

echo "[check] cargo check (bins)"
( cd src-tauri && cargo check --bins )

echo "[done] Archived and verified."
