#!/usr/bin/env sh
# Archive (git mv) a list of frontend files (stdin) and run TS type-check.
# Usage:
#   node scripts/find_unused_frontend.mjs | scripts/prune_frontend_unused.sh
# Or with a saved list:
#   cat reports/unused_frontend_*.txt | scripts/prune_frontend_unused.sh

set -eu
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

STAMP=$(date +%Y%m%dT%H%M%S)
ARCHIVE_DIR="scripts/_backups/frontend_sweep_${STAMP}"
mkdir -p "$ARCHIVE_DIR"

TMP_CANDIDATES=$(mktemp)
# Normalize input lines: remove leading " - " and ignore headings
sed -n 's/^ - //p' > "$TMP_CANDIDATES"

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[info] No candidates on stdin. Nothing to do."
  exit 0
fi

COUNT=$(wc -l < "$TMP_CANDIDATES" | tr -d ' ')
echo "[archive] Moving $COUNT files to $ARCHIVE_DIR"

while IFS= read -r f; do
  [ -f "$f" ] || { echo "[skip] Not found: $f" >&2; continue; }
  case "$f" in
    src/*) :;;
    *) echo "[skip] Outside src/: $f" >&2; continue;;
  esac
  dest_dir="$ARCHIVE_DIR/$(dirname "${f#src/}")"
  mkdir -p "$dest_dir"
  git mv -f "$f" "$dest_dir/"
done < "$TMP_CANDIDATES"

# Quick safety check
if npm run -s type-check; then
  echo "[done] Archived and type-check passed."
else
  echo "[warn] Type-check failed. You may want to revert the last moves (git restore -S HEAD~1 or manual)." >&2
fi
