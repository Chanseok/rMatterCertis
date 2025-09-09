-- Migration 010: Introduce device_types reference table
-- This table is seeded from data/matter_device_types.json at runtime when empty.

CREATE TABLE IF NOT EXISTS device_types (
    id INTEGER PRIMARY KEY,              -- Decimal numeric ID (e.g. 256 for 0x0100)
    code_hex TEXT,                       -- Original hexadecimal code (e.g. 0x0100)
    name TEXT NOT NULL,                  -- Human friendly name
    description TEXT,                    -- Optional description
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_name ON device_types(name);
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_code_hex ON device_types(code_hex);

CREATE TRIGGER IF NOT EXISTS device_types_updated_at
AFTER UPDATE ON device_types
FOR EACH ROW
BEGIN
    UPDATE device_types SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;
