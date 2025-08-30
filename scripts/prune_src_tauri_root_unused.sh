#!/usr/bin/env sh
# Archive (git mv) likely-unused top-level src-tauri/src/*.rs files found by list_unused_src_tauri_root_rs.sh
# After move, run cargo check to verify build.
set -eu
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"
ARCHIVE_DIR="src-tauri/src/_archive/src_root_$(date +%Y%m%dT%H%M%S)"
mkdir -p "$ARCHIVE_DIR"

TMP_CANDIDATES=$(mktemp)
if [ -t 0 ]; then
  scripts/list_unused_src_tauri_root_rs.sh | sed -n 's/^ - //p' > "$TMP_CANDIDATES"
else
  sed -n 's/^ - //p' > "$TMP_CANDIDATES"
fi

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[info] No candidates."
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
  git mv -f "$f" "$ARCHIVE_DIR/"
done < "$TMP_CANDIDATES"

echo "[check] Running cargo check (bins)"
( cd src-tauri && cargo check --bins )

echo "[done] Archived top-level files and verified build."
