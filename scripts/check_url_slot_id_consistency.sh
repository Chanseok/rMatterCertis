#!/usr/bin/env bash
set -euo pipefail

# Quick checker: For a given URL, verify page_id/index_in_page ↔ id (p####i##) consistency
# - If URL not provided, auto-pick the latest updated product_details row with non-null coordinates.
# - Also prints counts of mismatches across tables.

DB_PATH="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"

if [[ ! -f "$DB_PATH" ]]; then
  echo "❌ Database not found at: $DB_PATH" >&2
  echo "Hint: Launch the app once to initialize paths, or adjust DB_PATH in this script." >&2
  exit 1
fi

URL_INPUT=${1:-}

if [[ -z "$URL_INPUT" ]]; then
  URL_INPUT=$(sqlite3 "$DB_PATH" <<'SQL'
.timeout 2000
SELECT url
FROM product_details
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
ORDER BY COALESCE(updated_at, created_at) DESC
LIMIT 1;
SQL
  )
  if [[ -z "$URL_INPUT" ]]; then
    echo "❌ No candidate rows found in product_details." >&2
    exit 2
  fi
  echo "ℹ️ No URL provided. Using most recently updated candidate: $URL_INPUT"
fi

printf "\n== URL slot/id consistency check ==\n"
echo "URL: $URL_INPUT"

# Check if products has an 'id' column
HAS_PROD_ID=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name='id';")

echo "\n-- product_details --"
# Escape single quotes for SQL literal
ESCAPED_URL=${URL_INPUT//\'/''}
sqlite3 "$DB_PATH" <<SQL
.headers on
.mode column
WITH rows AS (
  SELECT 
    url,
    page_id,
    index_in_page,
    id AS actual_id,
    CASE WHEN page_id IS NOT NULL AND index_in_page IS NOT NULL
         THEN printf('p%04ii%02i', page_id, index_in_page)
         ELSE NULL END AS expected_id,
    CASE WHEN id IS NOT NULL AND page_id IS NOT NULL AND index_in_page IS NOT NULL AND id = printf('p%04ii%02i', page_id, index_in_page) THEN 'OK'
         WHEN page_id IS NULL OR index_in_page IS NULL THEN 'N/A'
         WHEN id IS NULL THEN 'MISSING'
         ELSE 'MISMATCH' END AS verdict
  FROM product_details
  WHERE url = '$ESCAPED_URL'
)
SELECT * FROM rows;
SQL

if [[ "$HAS_PROD_ID" -gt 0 ]]; then
  echo "\n-- products (includes id column) --"
  sqlite3 "$DB_PATH" <<SQL
.headers on
.mode column
WITH rows AS (
  SELECT 
    url,
    page_id,
    index_in_page,
    id AS actual_id,
    CASE WHEN page_id IS NOT NULL AND index_in_page IS NOT NULL
         THEN printf('p%04ii%02i', page_id, index_in_page)
         ELSE NULL END AS expected_id,
    CASE WHEN id IS NOT NULL AND page_id IS NOT NULL AND index_in_page IS NOT NULL AND id = printf('p%04ii%02i', page_id, index_in_page) THEN 'OK'
         WHEN page_id IS NULL OR index_in_page IS NULL THEN 'N/A'
         WHEN id IS NULL THEN 'MISSING'
         ELSE 'MISMATCH' END AS verdict
  FROM products
  WHERE url = '$ESCAPED_URL'
)
SELECT * FROM rows;
SQL
else
  echo "\n-- products --"
  echo "(No 'id' column present. Showing coordinates only.)"
  sqlite3 "$DB_PATH" <<SQL
.headers on
.mode column
SELECT url, page_id, index_in_page
FROM products
WHERE url = '$ESCAPED_URL';
SQL
fi

printf "\n== Global mismatch summary ==\n"
sqlite3 "$DB_PATH" <<'SQL'
.headers on
.mode column
SELECT 'product_details_mismatches' AS target,
       COUNT(*) AS mismatches
FROM product_details
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND id IS NOT NULL
  AND id <> printf('p%04ii%02i', page_id, index_in_page);

SELECT 'product_details_missing_id' AS target,
       COUNT(*) AS missing
FROM product_details
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND id IS NULL;

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
SQL

printf "\n✅ Check complete.\n"
