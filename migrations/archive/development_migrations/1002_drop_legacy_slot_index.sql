-- 1002_drop_legacy_slot_index.sql
-- Purpose: Remove legacy UNIQUE index on products(page_id,index_in_page) that causes conflicts with
-- dynamic repositioning strategy. Safe to run multiple times (IF EXISTS) and idempotent.
-- After this migration, coordinate conflicts will no longer raise SQLITE_CONSTRAINT for
-- products; reconciliation handled by repair & vacate logic.

PRAGMA foreign_keys=OFF; -- not strictly needed, defensive
BEGIN;
DROP INDEX IF EXISTS ux_products_slot;
-- Also catch any differently named unique index matching signature (best-effort):
-- We scan sqlite_master and drop all unique indexes whose SQL contains 'products' and '(page_id, index_in_page)'.
-- (SQLite doesn't support dynamic looping in plain SQL; we rely on known name ux_products_slot.)
COMMIT;

-- Bump user_version (1xxx reserved for consolidated milestones)
PRAGMA user_version = 1002;
