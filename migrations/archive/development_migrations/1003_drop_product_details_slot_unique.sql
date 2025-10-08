-- 1003_drop_product_details_slot_unique.sql
-- Purpose: Remove UNIQUE constraint on product_details(page_id, index_in_page) to allow dynamic repositioning
--          when site order changes. URL is the true identifier; coordinates are reference metadata only.
-- 
-- PROBLEM CONTEXT:
--   When the CSA-IOT site reorders products (e.g., adds new products, deletes old ones),
--   the same physical page position (page_id, index_in_page) may contain different products.
--   The UNIQUE constraint prevents updating these coordinates correctly, causing:
--   1. Duplicate detection failures
--   2. Missed products during partial crawls
--   3. Coordinate mismatches in diagnostics
--
-- SOLUTION:
--   Remove UNIQUE constraint; treat (page_id, index_in_page) as "last observed position"
--   URL remains the authoritative identifier (PRIMARY KEY)
--
-- Safe to run multiple times (IF EXISTS) and idempotent.

PRAGMA foreign_keys=OFF;
BEGIN;

DROP INDEX IF EXISTS ux_product_details_slot;

-- Verify removal
-- Expected: No rows returned after this migration
-- SELECT name FROM sqlite_master WHERE type='index' AND name='ux_product_details_slot';

COMMIT;

-- Bump user_version
PRAGMA user_version = 1003;
