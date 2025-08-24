#!/usr/bin/env bash
set -euo pipefail

# Report missing index_in_page slots for the first N pages (default: 12 pages => page_id 0..11)
# - Detects items_on_last_page from latest back_front.log (fallback: 12)
# - Prints per-page missing indices for product_details table

DB_PATH="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"
PAGES=${1:-12} # number of pages to check starting at page_id=0

if [[ ! -f "$DB_PATH" ]]; then
  echo "❌ Database not found at: $DB_PATH" >&2
  exit 1
fi

# Find latest unified log to detect items_on_last_page
LOG_FILE=${LOG_FILE:-}
if [[ -z "${LOG_FILE}" ]]; then
  CANDIDATE=$(ls -t src-tauri/target/**/logs/back_front.log 2>/dev/null | head -n1 || true)
  [[ -n "$CANDIDATE" ]] && LOG_FILE="$CANDIDATE"
fi

ITEMS_ON_LAST_PAGE=12
if [[ -n "${LOG_FILE:-}" && -f "${LOG_FILE}" ]]; then
  VAL=$(grep -Eo "items_on_last_page=([0-9]+)" -h "$LOG_FILE" | tail -n1 | sed -E 's/.*=([0-9]+)/\1/') || true
  if [[ -n "${VAL:-}" ]]; then
    ITEMS_ON_LAST_PAGE="$VAL"
  fi
fi

echo "== Missing slots report =="
echo "DB: $DB_PATH"
[[ -n "${LOG_FILE:-}" ]] && echo "Log: ${LOG_FILE}" || echo "Log: (none)"
echo "items_on_last_page(page_id=0): $ITEMS_ON_LAST_PAGE"
echo "pages to check: 0..$((PAGES-1))"

# Build SQL with two expected ranges: page 0 uses ITEMS_ON_LAST_PAGE, others use 12
sqlite3 "$DB_PATH" <<SQL
.headers on
.mode column
WITH RECURSIVE n0(i) AS (
  SELECT 0 UNION ALL SELECT i+1 FROM n0 WHERE i+1 < $ITEMS_ON_LAST_PAGE
),
n12(i) AS (
  SELECT 0 UNION ALL SELECT i+1 FROM n12 WHERE i+1 < 12
),
pages AS (
  SELECT 0 AS page_id
  UNION ALL
  SELECT page_id FROM (
    SELECT 1 AS page_id
    UNION ALL SELECT 2
    UNION ALL SELECT 3
    UNION ALL SELECT 4
    UNION ALL SELECT 5
    UNION ALL SELECT 6
    UNION ALL SELECT 7
    UNION ALL SELECT 8
    UNION ALL SELECT 9
    UNION ALL SELECT 10
    UNION ALL SELECT 11
  ) WHERE page_id < $PAGES
),
expected AS (
  -- page 0
  SELECT 0 AS page_id, i AS idx FROM n0
  UNION ALL
  -- pages 1..(PAGES-1)
  SELECT p.page_id, i AS idx FROM pages p JOIN n12 ON p.page_id > 0
),
present AS (
  SELECT page_id, index_in_page AS idx
  FROM product_details
  WHERE page_id BETWEEN 0 AND $((PAGES-1))
),
missing AS (
  SELECT e.page_id, e.idx
  FROM expected e
  LEFT JOIN present p ON p.page_id = e.page_id AND p.idx = e.idx
  WHERE p.idx IS NULL
)
SELECT page_id, GROUP_CONCAT(idx, ',') AS missing_slots
FROM missing
GROUP BY page_id
ORDER BY page_id;
SQL

printf "\n✅ Report complete.\n"
