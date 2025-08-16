#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

ARCHIVE_DIR="_archive/backups_$(date +%Y%m%dT%H%M%S)"
mkdir -p "$ARCHIVE_DIR"

echo "[archive] moving backup-like files to $ARCHIVE_DIR"
shopt -s globstar nullglob
for f in **/*.{bak,backup}; do
  # skip node_modules/target
  if [[ "$f" == node_modules/* || "$f" == src-tauri/target/* ]]; then
    continue
  fi
  dest_dir="$ARCHIVE_DIR/$(dirname "$f")"
  mkdir -p "$dest_dir"
  git mv -f "$f" "$dest_dir/" 2>/dev/null || mv -f "$f" "$dest_dir/"
  echo "moved: $f"
done

echo "[archive] done"
