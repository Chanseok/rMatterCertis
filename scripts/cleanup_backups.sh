#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Remove obvious backups and moved artifacts from the repo working tree
patterns=("*.bak" "*.backup" "*.moved*")
for p in "${patterns[@]}"; do
  git ls-files -co --exclude-standard "**/$p" | while read -r f; do
    echo "Removing $f"
    rm -f "$f"
  done
done

echo "Done. Consider running: git add -A && git commit -m 'cleanup: remove backups'"
