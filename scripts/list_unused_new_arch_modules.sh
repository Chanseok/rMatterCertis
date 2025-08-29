#!/usr/bin/env sh
# List likely-unused Rust modules under src-tauri/src/new_architecture by reference scan.
# Heuristic: if no occurrences of crate::new_architecture::<module_path> are found outside the module file itself
# and parent mod.rs, we mark it as a candidate. Handles nested submodules.
#
# Usage:
#   scripts/list_unused_new_arch_modules.sh [ROOT_DIR]
#
# Notes:
# - This is conservative but still heuristic. Reexports and glob imports are covered via 'use crate::new_architecture::...'
# - False negatives possible if symbols are only referenced via reexports outside new_architecture.
# - We exclude tests and archives by default.

set -eu

ROOT=${1:-"src-tauri/src/new_architecture"}
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

if [[ ! -d "$ROOT" ]]; then
  echo "[error] ROOT not found: $ROOT" >&2
  exit 1
fi

# Collect .rs files excluding mod.rs, tests, examples, archive folders
TMP_FILES=$(mktemp)
find "$ROOT" \
  -type f -name "*.rs" \
  ! -name "mod.rs" \
  ! -path "*/tests/*" \
  ! -path "*/test/*" \
  ! -path "*/_archive/*" \
  | sort > "$TMP_FILES"

if [ ! -s "$TMP_FILES" ]; then
  echo "[info] No .rs files found under $ROOT (excluding mod.rs)."
  exit 0
fi

TMP_CANDIDATES=$(mktemp)

echo "[scan] Searching repo for references to modules under $ROOT" >&2
while IFS= read -r f; do
  rel=${f#src-tauri/src/}             # new_architecture/foo/bar.rs
  mod_path=${rel%.rs}                 # new_architecture/foo/bar
  # replace / with :: in POSIX sh
  mod_path=$(printf '%s' "$mod_path" | sed 's%/%::%g')

  # Compose grep patterns to detect references
  # - use crate::<mod_path>
  # - crate::<mod_path>::
  # - new_architecture::<...> (relative path from other modules)
  # We'll search both absolute (crate::...) and path-anchored occurrences.
  p1="crate::${mod_path}"
  p2="${mod_path}"

  # Build exclude list: the file itself and its immediate parent mod.rs (if any)
  parent_dir=$(dirname "$f")
  parent_mod="$parent_dir/mod.rs"

  # Run ripgrep if available for speed, else fallback to grep -R
  if command -v rg >/dev/null 2>&1; then
    hits=$(rg -n --no-ignore --glob '!target' --glob '!**/_archive/**' --glob '!**/node_modules/**' \
      -e "$p1|$p2" . 2>/dev/null \
      | awk -F: '{print $1}' | grep -v -e "^$f$" -e "^$parent_mod$" | sort -u || true)
  else
    # GNU/BSD grep fallback (search common code/text files)
    hits=$(grep -RIn --exclude-dir=target --exclude-dir=_archive --exclude-dir=node_modules \
      --include='*.rs' --include='*.md' --include='*.toml' --include='*.ts' --include='*.tsx' --include='*.js' --include='*.jsx' --include='*.mjs' \
      -e "$p1" -e "$p2" . 2>/dev/null \
      | awk -F: '{print $1}' | grep -v -e "^$f$" -e "^$parent_mod$" | sort -u || true)
  fi

  # Additional sibling-relative reference detection: super::<name>, self::<name>
  base=$(basename "$f" .rs)
  if command -v rg >/dev/null 2>&1; then
    sib_hits=$(rg -n --no-ignore --glob '!target' --glob '!**/_archive/**' --glob '!**/node_modules/**' \
      -e "(super|self)::${base}(::|\\b)" "$parent_dir" 2>/dev/null \
      | awk -F: '{print $1}' | grep -v -e "^$f$" -e "^$parent_mod$" | sort -u || true)
  else
    sib_hits=$(grep -RIn --exclude-dir=target --exclude-dir=_archive --exclude-dir=node_modules \
      --include='*.rs' -e "super::${base}\\b" -e "self::${base}\\b" "$parent_dir" 2>/dev/null \
      | awk -F: '{print $1}' | grep -v -e "^$f$" -e "^$parent_mod$" | sort -u || true)
  fi

  if [ -z "$hits" ] && [ -z "$sib_hits" ]; then
    echo "$f" >> "$TMP_CANDIDATES"
  fi
done < "$TMP_FILES"

if [ ! -s "$TMP_CANDIDATES" ]; then
  echo "[result] No obvious unused modules detected by reference scan."
  exit 0
fi

echo "[result] Likely-unused modules (no repo references found):"
while IFS= read -r c; do
  echo " - $c"
done < "$TMP_CANDIDATES"

cat <<'EOF'

Next steps:
- Review the list above for false positives (e.g., used via reexports outside new_architecture).
- If it looks right, run the prune helper we can generate to archive them safely and cargo check after each move.
EOF
