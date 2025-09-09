-- Migration 012: Introduce type_id (spec ID) distinct from internal PK id
-- Adds column, backfills from existing id values, and creates a unique index.
-- Guarded in Rust so it's only run when column missing.
ALTER TABLE device_types ADD COLUMN type_id INTEGER;
UPDATE device_types SET type_id = id WHERE type_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_type_id ON device_types(type_id);
