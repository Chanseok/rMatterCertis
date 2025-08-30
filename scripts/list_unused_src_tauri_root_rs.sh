#!/usr/bin/env sh
# Detect likely-unused top-level .rs files under src-tauri/src (root only, not subfolders).
# Logic: For each file F (excluding lib.rs and main.rs), check if any module declaration
# exists referencing it (mod NAME; or pub mod NAME; or #[path = ".../F"] mod NAME;).
# If no declaration found, mark as candidate. This is conservative and safe.

set -eu

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

ROOT="src-tauri/src"

# list *.rs in root directory only
TMP_FILES=$(mktemp)
find "$ROOT" -maxdepth 1 -type f -name "*.rs" | sort > "$TMP_FILES"

if [ ! -s "$TMP_FILES" ]; then
  echo "[info] No .rs files in $ROOT"
  exit 0
fi

echo "[scan] Checking top-level .rs files in $ROOT"
TMP_CANDIDATES=$(mktemp)

while IFS= read -r f; do
  base=$(basename "$f")
  name=${base%.rs}
  case "$base" in
    lib.rs|main.rs) continue;;
  esac

  # Search for declarations or path inclusions
  # 1) mod name; or pub mod name; anywhere in repo
  # 2) #[path = ".../base"] followed by mod name
  decl_hits=$(grep -RIn --exclude-dir=target --exclude-dir=_archive --exclude-dir=node_modules \
    --include='*.rs' \
    -e "^[[:space:]]*mod[[:space:]]\+$name[[:space:]]*;" \
    -e "^[[:space:]]*pub[[:space:]]\+mod[[:space:]]\+$name[[:space:]]*;" \
    -e "#\[path[[:space:]]*=.*$base.*\][[:space:]]*mod[[:space:]]\+" . 2>/dev/null | awk -F: '{print $1}' | sort -u || true)

  # Filter out self-reference (if any) - shouldn't happen, but safe
  decl_hits=$(echo "$decl_hits" | grep -v -e "^$f$" || true)

  if [ -z "$decl_hits" ]; then
    echo "$f" >> "$TMP_CANDIDATES"
  fi

done < "$TMP_FILES"

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[result] All top-level .rs files appear declared/used."
  exit 0
fi

echo "[result] Likely-unused top-level files (no module declaration found):"
while IFS= read -r c; do
  echo " - $c"
done < "$TMP_CANDIDATES"

cat <<'EOF'

Next steps:
- Review the list. If correct, archive them safely with a companion prune script and run cargo check.
EOF
