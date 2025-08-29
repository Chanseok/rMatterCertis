#!/usr/bin/env bash
set -euo pipefail

# Heuristic audit for likely-unused backend files in src-tauri/src
# - Lists files guarded out of build (#![cfg(any())])
# - Lists backup/scratch files (*.bak, *.backup, *.old, *.new, *.corrupted)
# - Lists candidate orphan modules by stem with no references across workspace (best-effort)

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$ROOT_DIR/src-tauri/src"

echo "[audit] Backend unused files heuristic report"
echo "[audit] Workspace: $ROOT_DIR"

echo "\n==> Guarded-out Rust files (#![cfg(any())])"
if command -v rg >/dev/null 2>&1; then
  rg -n --hidden --no-ignore -g "src-tauri/src/**.rs" "^#!\\[cfg\\(any\\(\\)\\)\\]" || true
else
  grep -R -n -E "^#!\\[cfg\(any\(\)\)\]" "$SRC_DIR" || true
fi

echo "\n==> Backup/scratch candidates (*.bak|*.backup|*.old|*.new|*.corrupted)"
find "$SRC_DIR" -type f \( \
  -name "*.bak" -o -name "*.backup" -o -name "*.old" -o -name "*.new" -o -name "*.corrupted" \
\) -print | sort || true

echo "\n==> Potential orphan module files by stem (heuristic)"
echo "# Note: false positives/negatives possible; review before action."
tmp_candidates=$(mktemp)
trap 'rm -f "$tmp_candidates"' EXIT

# Collect Rust files (exclude _archive and target-like dirs)
find "$SRC_DIR" -type f -name "*.rs" \
  -not -path "*/_archive/*" \
  -not -path "*/target/*" \
  -not -path "*/node_modules/*" \
  > "$tmp_candidates"

while IFS= read -r file; do
  stem="$(basename "$file" .rs)"
  # Skip common stems that are often re-export hubs
  case "$stem" in
    lib|main|mod|types|contract|traits|prelude) continue;;
  esac
  # Skip files already guarded out
  if command -v rg >/dev/null 2>&1 && rg -n "^#!\\[cfg\\(any\\(\\)\\)\\]" "$file" > /dev/null 2>&1; then
    continue
  fi
  # Search for references to the stem elsewhere (mod/use/#[path]/pub use) excluding the file itself and _archive
  if command -v rg >/dev/null 2>&1; then
    if ! rg -n --hidden --no-ignore \
         -g "src-tauri/src/**" \
         -g "!**/_archive/**" \
         -g "!$file" \
         -e "(^|[^A-Za-z0-9_])($stem)($|[^A-Za-z0-9_])" \
         > /dev/null 2>&1; then
      echo "$file"
    fi
  else
    if ! grep -R -n -E "(^|[^A-Za-z0-9_])($stem)($|[^A-Za-z0-9_])" "$SRC_DIR" | grep -v "/_archive/" | grep -v "$file" > /dev/null 2>&1; then
      echo "$file"
    fi
  fi
done < "$tmp_candidates"

echo "\n[audit] Done. Review the lists above to decide on archive/delete."
