#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
SRC_DIR="$ROOT_DIR/src-tauri/src/commands"
ARCH_DIR="$ROOT_DIR/src-tauri/src/_archive/commands"

mkdir -p "$ARCH_DIR"

FILES=(
  "modern_crawling.rs.bak"
  "modern_crawling.rs.new"
  "real_actor_commands.rs.backup"
  "actor_system_monitoring_backup.rs"
)

moved=()
skipped=()
for f in "${FILES[@]}"; do
  src="$SRC_DIR/$f"
  if [[ -f "$src" ]]; then
    ts=$(date +%Y%m%dT%H%M%S)
    mv "$src" "$ARCH_DIR/${f}.moved_$ts" || true
    moved+=("$f")
  else
    skipped+=("$f")
  fi
done

echo "[CLEANUP] Archive directory: $ARCH_DIR"
if [[ ${#moved[@]} -gt 0 ]]; then
  echo "[CLEANUP] Moved files: ${moved[*]}"
else
  echo "[CLEANUP] No files moved"
fi
if [[ ${#skipped[@]} -gt 0 ]]; then
  echo "[CLEANUP] Skipped (not found): ${skipped[*]}"
fi

exit 0
