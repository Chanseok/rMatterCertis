#!/usr/bin/env sh
# Archive (git mv) likely-unused modules under src-tauri/src/new_architecture.
# Input: either a file list via stdin, or auto-run the lister to get candidates.
# After each batch move, runs cargo check to verify.

set -eu

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

ARCHIVE_DIR="scripts/_backups/new_architecture_$(date +%Y%m%dT%H%M%S)"
mkdir -p "$ARCHIVE_DIR"

TMP_CANDIDATES=$(mktemp)
if [ -t 0 ]; then
  scripts/list_unused_new_arch_modules.sh | sed -n 's/^ - //p' > "$TMP_CANDIDATES"
else
  sed -n 's/^ - //p' > "$TMP_CANDIDATES"
fi

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[info] No candidates provided. Nothing to do."
  exit 0
fi

COUNT=$(wc -l < "$TMP_CANDIDATES" | tr -d ' ')
echo "[archive] Moving $COUNT files to $ARCHIVE_DIR"

while IFS= read -r f; do
  # Ensure file exists and is under src-tauri/src/new_architecture
  if [ ! -f "$f" ]; then
    echo "[skip] Not a file: $f" >&2
    continue
  fi
  case "$f" in
    src-tauri/src/new_architecture/*) ;;
    *) echo "[skip] Outside new_architecture: $f" >&2; continue;;
  esac

  dest_dir="$ARCHIVE_DIR/$(dirname "${f#src-tauri/src/new_architecture/}")"
  mkdir -p "$dest_dir"
  git mv -f "$f" "$dest_dir/"
done < "$TMP_CANDIDATES"

echo "[check] Running cargo check (bins) after archival"
pushd src-tauri >/dev/null
cargo check --bins
popd >/dev/null

echo "[done] Archived files. Review git status and run tests as needed."
