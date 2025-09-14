-- Migration 024: Change product_details.certification_date column type TEXT -> DATE
-- Rationale:
--  * Normalize column to DATE affinity to enable better date comparisons and ordering
--  * Existing values are preserved (assumed ISO8601 'YYYY-MM-DD' or similar). No transformation beyond direct copy.
--  * Performed via table rebuild (SQLite lacks direct ALTER COLUMN type change).
--  * Idempotent at runtime because Rust layer checks current column type before executing.

BEGIN TRANSACTION;

-- Recreate table with certification_date DATE instead of TEXT
CREATE TABLE IF NOT EXISTS product_details_new (
    url TEXT PRIMARY KEY,
    page_id INTEGER,
    index_in_page INTEGER,
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    device_type TEXT,
    certificate_id TEXT,
    certification_date DATE, -- changed type
    software_version TEXT,
    hardware_version TEXT,
    firmware_version TEXT,
    specification_version TEXT,
    vid INTEGER,
    pid INTEGER,
    family_sku TEXT,
    family_variant_sku TEXT,
    family_id TEXT,
    tis_trp_tested TEXT,
    transport_interface TEXT,
    primary_device_type_ids TEXT,
    application_categories TEXT,
    description TEXT,
    compliance_document_url TEXT,
    program_type TEXT DEFAULT 'Matter',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);

-- Copy data (direct copy; if non-ISO formats exist they remain as-is but now under DATE affinity)
INSERT INTO product_details_new (
    url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id, certification_date,
    software_version, hardware_version, firmware_version, specification_version,
    vid, pid, family_sku, family_variant_sku, family_id,
    tis_trp_tested, transport_interface, primary_device_type_ids, application_categories,
    description, compliance_document_url, program_type, created_at, updated_at
)
SELECT 
    url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id, certification_date,
    software_version, hardware_version, firmware_version, specification_version,
    vid, pid, family_sku, family_variant_sku, family_id,
    tis_trp_tested, transport_interface, primary_device_type_ids, application_categories,
    description, compliance_document_url, program_type, created_at, updated_at
FROM product_details;

-- Drop old table and rename
DROP TABLE product_details;
ALTER TABLE product_details_new RENAME TO product_details;

-- Recreate indexes (only those referencing product_details; names must match originals)
CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details (manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_device_type ON product_details (device_type);
CREATE INDEX IF NOT EXISTS idx_product_details_certificate_id ON product_details (certificate_id);
CREATE INDEX IF NOT EXISTS idx_product_details_certification_date ON product_details (certification_date);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specification_version);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);

COMMIT;

-- Verification (for manual runs)
SELECT 'Migration 024 applied: certification_date -> DATE' AS status;
