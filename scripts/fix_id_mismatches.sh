#!/usr/bin/env bash
set -euo pipefail

# Fixer: Align id with page_id/index_in_page => id = printf('p%04ii%02i', page_id, index_in_page)
# - Creates a timestamped backup using sqlite .backup
# - Updates product_details (and products if it has id column)
# - Prints a verification summary after changes

DB_PATH="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"

if [[ ! -f "$DB_PATH" ]]; then
  echo "❌ Database not found at: $DB_PATH" >&2
  exit 1
fi

BACKUP_DIR="$HOME/Library/Application Support/matter-certis-v2/database/backups"
TS=$(date +%Y%m%d_%H%M%S)
BACKUP_PATH="$BACKUP_DIR/matter_certis_$TS.db"
mkdir -p "$BACKUP_DIR"

echo "== Fix id mismatches (product_details → p####i##) =="
echo "DB: $DB_PATH"
echo "Backup: $BACKUP_PATH"

echo "\n-- Creating backup snapshot --"
sqlite3 "$DB_PATH" <<SQL
.timeout 4000
.backup '$BACKUP_PATH'
SQL

# Determine whether products has an id column
HAS_PROD_ID=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name='id';")

echo "\n-- Pre-fix mismatch counts --"
sqlite3 "$DB_PATH" <<'SQL'
.headers on
.mode column
SELECT 'product_details_mismatches' AS target,
       COUNT(*) AS mismatches
FROM product_details
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND id IS NOT NULL
  AND id <> printf('p%04ii%02i', page_id, index_in_page);
SQL

echo "\n-- Applying fixes in a single transaction --"
sqlite3 "$DB_PATH" <<'SQL'
.headers off
.mode list
PRAGMA foreign_keys=ON;
BEGIN;
UPDATE product_details
SET id = printf('p%04ii%02i', page_id, index_in_page)
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND (id IS NULL OR id <> printf('p%04ii%02i', page_id, index_in_page));
SELECT 'product_details_fixed', changes();
-- Also clear ids when coordinates are NULL (must be NULL)
UPDATE product_details
SET id = NULL
WHERE (page_id IS NULL OR index_in_page IS NULL)
  AND id IS NOT NULL;
SELECT 'product_details_cleared_null_coords', changes();
COMMIT;
SQL

if [[ "$HAS_PROD_ID" -gt 0 ]]; then
  echo "\n-- Aligning products.id as well (column present) --"
  sqlite3 "$DB_PATH" <<'SQL'
.headers off
.mode list
PRAGMA foreign_keys=ON;
BEGIN;
UPDATE products
SET id = printf('p%04ii%02i', page_id, index_in_page)
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND (id IS NULL OR id <> printf('p%04ii%02i', page_id, index_in_page));
SELECT 'products_fixed', changes();
-- Also clear ids when coordinates are NULL (must be NULL)
UPDATE products
SET id = NULL
WHERE (page_id IS NULL OR index_in_page IS NULL)
  AND id IS NOT NULL;
SELECT 'products_cleared_null_coords', changes();
COMMIT;
SQL
else
  echo "(skip products table: no id column)"
fi

echo "\n-- Post-fix verification --"
sqlite3 "$DB_PATH" <<'SQL'
.headers on
.mode column
SELECT 'product_details_mismatches' AS target,
       COUNT(*) AS mismatches
FROM product_details
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  AND id IS NOT NULL
  AND id <> printf('p%04ii%02i', page_id, index_in_page);

SELECT 'product_details_null_coord_with_id' AS target,
       COUNT(*) AS violations
FROM product_details
WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL;

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

-- If products has an id column, also show null-coord id violations for products
SELECT 'products_null_coord_with_id' AS target,
       COUNT(*) AS violations
FROM products
WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL;
SQL

printf "\n✅ Done. Backup kept at: %s\n" "$BACKUP_PATH"
