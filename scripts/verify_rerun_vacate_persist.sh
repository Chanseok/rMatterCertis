#!/usr/bin/env bash
set -euo pipefail

# Verify after a manual physical page range re-run that:
# - [Vacate] and [Persist] logs are present (if verbose logging enabled)
# - No UNIQUE conflicts occurred
# - Slot duplicates are absent
#
# Usage:
#   ./scripts/verify_rerun_vacate_persist.sh 512 500
# Optionally set LOG_FILE to a recent backend log; otherwise we try to auto-detect.

START_PAGE=${1:-}
END_PAGE=${2:-}

if [[ -z "$START_PAGE" || -z "$END_PAGE" ]]; then
  echo "Usage: $0 <start_page> <end_page>" >&2
  exit 2
fi

DB_PATH="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"
if [[ ! -f "$DB_PATH" ]]; then
  echo "❌ Database not found at: $DB_PATH" >&2
  exit 1
fi

# Try to locate a recent log containing backend output
LOG_FILE=${LOG_FILE:-}
if [[ -z "${LOG_FILE}" ]]; then
  CANDIDATE="$(ls -t src-tauri/target/**/logs/*.log 2>/dev/null | head -n1 || true)"
  if [[ -n "$CANDIDATE" ]]; then
    LOG_FILE="$CANDIDATE"
  fi
fi

echo "== Range re-run verification =="
echo "Range: ${START_PAGE}-${END_PAGE}"
echo "DB:    $DB_PATH"
[[ -n "${LOG_FILE:-}" ]] && echo "Log:   $LOG_FILE" || echo "Log:   (none found; will only run DB checks)"

printf "\n-- Checking for UNIQUE constraint errors around the time window --\n"
if [[ -n "${LOG_FILE:-}" && -f "$LOG_FILE" ]]; then
  grep -E "UNIQUE constraint|constraint failed|UNIQUE\s*\(" -n "$LOG_FILE" || echo "No UNIQUE errors found in log."
else
  echo "(skip log scan)"
fi

printf "\n-- Checking [Vacate]/[Persist] evidence in logs --\n"
if [[ -n "${LOG_FILE:-}" && -f "$LOG_FILE" ]]; then
  grep -E "\[Vacate\]|\[Persist\]|Persist(ed|ing)" -n "$LOG_FILE" || echo "No explicit [Vacate]/[Persist] markers found."
else
  echo "(skip log scan)"
fi

printf "\n-- DB duplicate scan and slot integrity --\n"
sqlite3 "$DB_PATH" <<SQL
.headers on
.mode column
PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=2000;

-- Duplicates by (page_id, index_in_page)
SELECT 'products_slot_dupes' AS target,
       COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM products
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

SELECT 'product_details_slot_dupes' AS target,
       COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM product_details
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

-- ID consistency quick sample within range pages (if present)
WITH range_urls AS (
  SELECT url
  FROM product_details
  WHERE page_id BETWEEN $END_PAGE AND $START_PAGE
),
rows AS (
  SELECT pd.url, pd.page_id, pd.index_in_page, pd.id AS actual_id,
         CASE WHEN pd.page_id IS NOT NULL AND pd.index_in_page IS NOT NULL
              THEN printf('p%04ii%02i', pd.page_id, pd.index_in_page)
              ELSE NULL END AS expected_id,
         CASE WHEN pd.id IS NOT NULL AND pd.page_id IS NOT NULL AND pd.index_in_page IS NOT NULL AND pd.id = printf('p%04ii%02i', pd.page_id, pd.index_in_page) THEN 'OK'
              WHEN pd.page_id IS NULL OR pd.index_in_page IS NULL THEN 'N/A'
              WHEN pd.id IS NULL THEN 'MISSING'
              ELSE 'MISMATCH' END AS verdict
  FROM product_details pd
  WHERE pd.url IN (SELECT url FROM range_urls)
)
SELECT * FROM rows LIMIT 50;
SQL

printf "\n✅ Verification complete.\n"
