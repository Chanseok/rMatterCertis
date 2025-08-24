#!/usr/bin/env zsh

# Quick DB consistency check for id <-> (page_id, index_in_page)
# Verifies two rules on both tables (products, product_details):
# 1) When page_id and index_in_page are NOT NULL, id must equal printf('p%04di%02d', page_id, index_in_page)
# 2) When either page_id or index_in_page is NULL, id must be NULL

# zsh-safe: avoid set -e; handle errors manually

print_usage() {
  echo "Usage: $0 [DB_PATH]"
  echo "  DB_PATH (optional): path to SQLite DB file."
  echo "  Default (macOS): $HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"
}

if [[ "$1" == "-h" || "$1" == "--help" ]]; then
  print_usage
  exit 0
fi

DEFAULT_DB="$HOME/Library/Application Support/matter-certis-v2/database/matter_certis.db"
DB_PATH="${1:-${DB:-$DEFAULT_DB}}"

echo "[Info] Using DB: $DB_PATH"

# Check sqlite3
if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "[Error] sqlite3 not found. Install with: brew install sqlite"
  exit 2
fi

# Check DB exists
if [[ ! -f "$DB_PATH" ]]; then
  echo "[Error] DB file not found: $DB_PATH"
  print_usage
  exit 3
fi

run_sql() {
  local sql="$1"
  sqlite3 -batch -noheader "$DB_PATH" "$sql"
}

echo "\n[Check 1] Non-NULL coordinates must have matching id (derived = p%04di%02d)"
PD_MISMATCH=$(run_sql "SELECT COUNT(*) FROM product_details WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL AND (id IS NULL OR id != printf('p%04di%02d', page_id, index_in_page));")
P_MISMATCH=$(run_sql "SELECT COUNT(*) FROM products WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL AND (id IS NULL OR id != printf('p%04di%02d', page_id, index_in_page));")

echo "  product_details mismatches: $PD_MISMATCH"
echo "  products        mismatches: $P_MISMATCH"

if [[ "$PD_MISMATCH" == "0" && "$P_MISMATCH" == "0" ]]; then
  echo "  => PASS"
else
  echo "  => FAIL"
  echo "  Sample product_details mismatches (up to 10):"
  run_sql "SELECT url, page_id, index_in_page, id, printf('p%04di%02d', page_id, index_in_page) AS expected FROM product_details WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL AND (id IS NULL OR id != printf('p%04di%02d', page_id, index_in_page)) LIMIT 10;"
  echo "  Sample products mismatches (up to 10):"
  run_sql "SELECT url, page_id, index_in_page, id, printf('p%04di%02d', page_id, index_in_page) AS expected FROM products WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL AND (id IS NULL OR id != printf('p%04di%02d', page_id, index_in_page)) LIMIT 10;"
fi

echo "\n[Check 2] NULL coordinates must NOT have id (id should be NULL)"
PD_NULL_VIOL=$(run_sql "SELECT COUNT(*) FROM product_details WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL;")
P_NULL_VIOL=$(run_sql "SELECT COUNT(*) FROM products WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL;")

echo "  product_details violations: $PD_NULL_VIOL"
echo "  products        violations: $P_NULL_VIOL"

if [[ "$PD_NULL_VIOL" == "0" && "$P_NULL_VIOL" == "0" ]]; then
  echo "  => PASS"
else
  echo "  => FAIL"
  echo "  Sample product_details violations (up to 10):"
  run_sql "SELECT url, page_id, index_in_page, id FROM product_details WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL LIMIT 10;"
  echo "  Sample products violations (up to 10):"
  run_sql "SELECT url, page_id, index_in_page, id FROM products WHERE (page_id IS NULL OR index_in_page IS NULL) AND id IS NOT NULL LIMIT 10;"
fi

echo "\n[Summary]"
echo "  Check1 mismatches -> product_details: $PD_MISMATCH, products: $P_MISMATCH"
echo "  Check2 violations -> product_details: $PD_NULL_VIOL, products: $P_NULL_VIOL"

# Exit code: 0 if both checks pass; 1 otherwise
if [[ "$PD_MISMATCH" == "0" && "$P_MISMATCH" == "0" && "$PD_NULL_VIOL" == "0" && "$P_NULL_VIOL" == "0" ]]; then
  exit 0
else
  exit 1
fi
