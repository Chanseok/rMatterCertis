#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/sqlite_dedup_by_url.sh [path_to_db]
# Default DB path matches the app's runtime on macOS.
DB_DEFAULT="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"
DB_PATH="${1:-$DB_DEFAULT}"

if [ ! -f "$DB_PATH" ]; then
  echo "❌ DB not found: $DB_PATH" >&2
  echo "Pass the DB path as an argument if different." >&2
  exit 1
fi

DB_DIR="$(dirname "$DB_PATH")"
BACKUP_DIR="$DB_DIR/backups"
STAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_FILE="$BACKUP_DIR/matter_certis_${STAMP}.db.bak"

mkdir -p "$BACKUP_DIR"
cp -p "$DB_PATH" "$BACKUP_FILE"
echo "🗄️ Backup created: $BACKUP_FILE"

echo "🧹 Deduplicating by URL (products, product_details)…"

# Note: Use single-quoted heredoc to avoid shell expansion inside SQL.
sqlite3 "$DB_PATH" <<'SQL'
.headers on
.mode column
PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
BEGIN IMMEDIATE;

-- Report duplicate counts before
SELECT 'BEFORE products dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (SELECT url, COUNT(*) AS cnt FROM products WHERE url IS NOT NULL GROUP BY url HAVING COUNT(*) > 1);

SELECT 'BEFORE product_details dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (SELECT url, COUNT(*) AS cnt FROM product_details WHERE url IS NOT NULL GROUP BY url HAVING COUNT(*) > 1);

-- Dedup products, keep minimal rowid per URL
WITH d AS (
  SELECT url, MIN(rowid) AS keep_rowid
  FROM products
  WHERE url IS NOT NULL
  GROUP BY url
  HAVING COUNT(*) > 1
)
DELETE FROM products
WHERE url IN (SELECT url FROM d)
  AND rowid NOT IN (SELECT keep_rowid FROM d);

-- Dedup product_details, keep minimal rowid per URL
WITH d AS (
  SELECT url, MIN(rowid) AS keep_rowid
  FROM product_details
  WHERE url IS NOT NULL
  GROUP BY url
  HAVING COUNT(*) > 1
)
DELETE FROM product_details
WHERE url IN (SELECT url FROM d)
  AND rowid NOT IN (SELECT keep_rowid FROM d);

-- Validate after
SELECT 'AFTER products dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (SELECT url, COUNT(*) AS cnt FROM products WHERE url IS NOT NULL GROUP BY url HAVING COUNT(*) > 1);

SELECT 'AFTER product_details dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (SELECT url, COUNT(*) AS cnt FROM product_details WHERE url IS NOT NULL GROUP BY url HAVING COUNT(*) > 1);

COMMIT;
PRAGMA wal_checkpoint(TRUNCATE);
VACUUM;
SQL

echo "🧹 URL-based dedup finished. Now checking duplicates by (page_id, index_in_page)…"

# A second pass: deduplicate rows that collide on (page_id, index_in_page)
# This addresses cases where URLs differ but represent the same slot on a page.
sqlite3 "$DB_PATH" <<'SQL'
.headers on
.mode column
PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
BEGIN IMMEDIATE;

-- BEFORE: duplicates by (page_id, index_in_page) in products
SELECT 'BEFORE products slot dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM products
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

-- BEFORE: duplicates by (page_id, index_in_page) in product_details
SELECT 'BEFORE product_details slot dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM product_details
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

-- Dedup products by (page_id, index_in_page): keep minimal rowid per slot
WITH d AS (
  SELECT page_id, index_in_page, MIN(rowid) AS keep_rowid
  FROM products
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
)
DELETE FROM products
WHERE rowid IN (
  SELECT p.rowid
  FROM products p
  JOIN d ON p.page_id = d.page_id AND p.index_in_page = d.index_in_page
  WHERE p.rowid <> d.keep_rowid
);

-- Dedup product_details by (page_id, index_in_page): keep minimal rowid per slot
WITH d AS (
  SELECT page_id, index_in_page, MIN(rowid) AS keep_rowid
  FROM product_details
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
)
DELETE FROM product_details
WHERE rowid IN (
  SELECT pd.rowid
  FROM product_details pd
  JOIN d ON pd.page_id = d.page_id AND pd.index_in_page = d.index_in_page
  WHERE pd.rowid <> d.keep_rowid
);

-- AFTER: verify slot duplicates are gone
SELECT 'AFTER products slot dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM products
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

SELECT 'AFTER product_details slot dupes' AS target, COALESCE(SUM(cnt - 1), 0) AS duplicates
FROM (
  SELECT page_id, index_in_page, COUNT(*) AS cnt
  FROM product_details
  WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
  GROUP BY page_id, index_in_page
  HAVING COUNT(*) > 1
);

COMMIT;
PRAGMA wal_checkpoint(TRUNCATE);
VACUUM;
SQL

echo "✅ Deduplication finished (URL + page slot)."
