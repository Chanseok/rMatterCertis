#!/usr/bin/env bash
# Audit Tauri #[tauri::command] functions and report which ones are not referenced by the frontend.
# This is a heuristic static check: it looks for string literals used in invoke('...') etc.
# Usage: bash scripts/audit_tauri_commands_usage.sh

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
BACKEND_DIR="$ROOT_DIR/src-tauri/src"
FRONTEND_DIR="$ROOT_DIR/src"

if [[ ! -d "$BACKEND_DIR" ]]; then
  echo "error: backend dir not found: $BACKEND_DIR" >&2
  exit 1
fi

if [[ ! -d "$FRONTEND_DIR" ]]; then
  echo "error: frontend dir not found: $FRONTEND_DIR" >&2
  exit 1
fi

# Collect command names by scanning for #[tauri::command] and the next function signature line
COMMANDS_CSV="$(
  find "$BACKEND_DIR" -type f -name '*.rs' -print0 \
  | xargs -0 awk '
      /#\[tauri::command\]/ { capture=1; next }
      capture==1 {
        # Look ahead for the fn signature line (POSIX BRE)
        if ($0 ~ /fn[[:space:]]+[a-zA-Z0-9_][a-zA-Z0-9_]*[[:space:]]*\(/) {
          fnline = $0
          # Extract name by removing up to "fn ", then take the first token
          sub(/^.*fn[[:space:]]+/, "", fnline)
          sub(/\(.*/, "", fnline)
          # trim trailing spaces
          sub(/[[:space:]]+$/, "", fnline)
          # take identifier characters only
          if (fnline ~ /^[a-zA-Z0-9_]+$/) { print fnline }
          capture=0
        } else if ($0 ~ /#\[tauri::command\]/) {
          # consecutive annotations, keep capture
        } else if ($0 ~ /\{/ || $0 ~ /;/) {
          # give up if we hit a block start or semicolon without finding fn
          capture=0
        }
      }
    ' \
  | sort -u | tr '\n' ','
)"

if [[ -z "$COMMANDS_CSV" ]]; then
  echo "No Tauri commands found."
  exit 0
fi

IFS=',' read -r -a COMMANDS <<< "$COMMANDS_CSV"
used=()
unused=()
for cmd in "${COMMANDS[@]}"; do
  [[ -z "$cmd" ]] && continue
  if grep -R -n --exclude-dir=node_modules --exclude-dir=dist --include='*.ts' --include='*.tsx' --include='*.js' -- "'${cmd}'\|\"${cmd}\"" "$FRONTEND_DIR" >/dev/null 2>&1; then
    used+=("$cmd")
  else
    unused+=("$cmd")
  fi
done

echo
echo "== Tauri Commands Usage Audit =="
printf "Total commands: %d\n" "${#COMMANDS[@]}"
printf "Used by FE:     %d\n" "${#used[@]}"
printf "Not referenced: %d\n\n" "${#unused[@]}"

echo "-- Used --"
{ for x in "${used[@]}"; do echo "$x"; done; } | sort
echo
echo "-- Not Referenced in FE (candidates to de-expose or gate) --"
{ for x in "${unused[@]}"; do echo "$x"; done; } | sort

# Exit with 0 to avoid failing CI; this is informational
exit 0
