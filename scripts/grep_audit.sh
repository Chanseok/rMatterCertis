#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

# Find risky direct HTTP/DB usages and backup files
{
  echo "# HTTP/DB direct-use audit (grep)"
  echo "## HTTP"
  rg -n "reqwest::|tauri::api::http|fetch\(|axios|http://|https://" --hidden --glob '!target' || true
  echo
  echo "## DB"
  rg -n "sqlx::|rusqlite|postgres|mysql" --hidden --glob '!target' || true
  echo
  echo "## Backup/Stale Files (*.bak|*.backup|*.moved)"
  rg -n "^" --files-with-matches --iglob "**/*.{bak,backup,moved}" || true
} | tee grep_audit.out

echo "\nWrote grep_audit.out"
