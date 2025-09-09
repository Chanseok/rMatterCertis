-- Migration 011: Add category & introduced_in columns (drop description)
-- Safe to run only if old schema (without category) still present.
-- Idempotent guard should be in Rust before executing.
BEGIN TRANSACTION;
CREATE TABLE IF NOT EXISTS device_types_new (
    id INTEGER PRIMARY KEY,
    code_hex TEXT,
    name TEXT NOT NULL,
    category TEXT,
    introduced_in TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Preserve existing data (ignore duplicates on id)
INSERT OR IGNORE INTO device_types_new (id, code_hex, name, created_at, updated_at)
    SELECT id, code_hex, name, created_at, updated_at FROM device_types;
DROP TABLE device_types;
ALTER TABLE device_types_new RENAME TO device_types;
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_name ON device_types(name);
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_code_hex ON device_types(code_hex);
CREATE TRIGGER IF NOT EXISTS device_types_updated_at
AFTER UPDATE ON device_types
FOR EACH ROW BEGIN
    UPDATE device_types SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;
COMMIT;
