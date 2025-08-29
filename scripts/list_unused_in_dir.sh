#!/usr/bin/env sh
# List likely-unused Rust modules under a given src-tauri/src/<dir> directory.
# Usage: scripts/list_unused_in_dir.sh <dir_name>
# Example: scripts/list_unused_in_dir.sh commands
#
# Heuristic:
# - Build module path as <dir_name>::<subpath>
# - Search for references `crate::<module_path>` or `<module_path>` across repo
# - Exclude the file itself and aggregator file src-tauri/src/<dir_name>.rs and mod.rs in subdirs
# - Also detect sibling references (super::name/self::name)
# - Report risk if the basename appears in lib.rs within a `pub mod <dir_name> { ... }` block

set -eu
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

if [ $# -ne 1 ]; then
  echo "Usage: $0 <dir_name> (relative to src-tauri/src)" >&2
  exit 2
fi
DIR_NAME=$1
ROOT="src-tauri/src/$DIR_NAME"
LIB="src-tauri/src/lib.rs"

if [ ! -d "$ROOT" ]; then
  echo "[error] Not a directory: $ROOT" >&2
  exit 1
fi

# If the directory itself has a lib.rs (aggregator), skip aggressive scan to avoid false positives
if [ -f "$ROOT/lib.rs" ]; then
  echo "[info] $ROOT contains lib.rs; treating modules as managed by local aggregator (skip scan)"
  exit 0
fi

# If there is a top-level gate/aggregator file (e.g., src-tauri/src/<dir>.rs), also skip
TOP_LEVEL_GATE="src-tauri/src/${DIR_NAME}.rs"
if [ -f "$TOP_LEVEL_GATE" ]; then
  echo "[info] Top-level aggregator $TOP_LEVEL_GATE found; skipping aggressive scan for $ROOT"
  exit 0
fi

TMP_FILES=$(mktemp)
find "$ROOT" -type f -name "*.rs" ! -name "mod.rs" ! -path "*/_archive/*" | sort > "$TMP_FILES"
if [ ! -s "$TMP_FILES" ]; then
  echo "[info] No .rs files in $ROOT"
  exit 0
fi

echo "[scan] $ROOT"

while IFS= read -r f; do
  rel=${f#src-tauri/src/}
  mod_path=${rel%.rs}
  mod_path=$(printf '%s' "$mod_path" | sed 's%/%::%g')
  parent_dir=$(dirname "$f")
  parent_mod="src-tauri/src/${DIR_NAME}.rs"
  base=$(basename "$f" .rs)

  # Reference search
  p1="crate::${mod_path}"
  p2="${mod_path}"
  hits=$(grep -RIn --exclude-dir=target --exclude-dir=_archive --exclude-dir=node_modules \
    --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.md' --include='*.toml' \
    -e "$p1" -e "$p2" . 2>/dev/null | awk -F: '{print $1}' | sort -u | grep -v -e "^$f$" -e "^$parent_mod$" || true)

  # Sibling relative references in same folder
  sib_hits=$(grep -RIn --exclude-dir=target --exclude-dir=_archive --exclude-dir=node_modules \
    --include='*.rs' -e "super::${base}\\b" -e "self::${base}\\b" "$parent_dir" 2>/dev/null | awk -F: '{print $1}' | sort -u | grep -v -e "^$f$" -e "^$parent_mod$" || true)

  # Risk: declared in lib.rs under the dir module
  risk="no"
  if grep -q "pub mod ${DIR_NAME}" "$LIB" 2>/dev/null; then
    if grep -n "pub mod ${DIR_NAME}" -n "$LIB" >/dev/null 2>&1; then
      if grep -RIn "pub mod ${base}\s*;" "$LIB" >/dev/null 2>&1; then
        risk="lib-decl"
      fi
    fi
  fi

  if [ -z "$hits" ] && [ -z "$sib_hits" ]; then
    echo " - $f (risk=$risk)"
  fi

done < "$TMP_FILES"
