-- Migration 009: Remove legacy primary_device_type_id column
-- Rebuilds product_details without the legacy TEXT column and keeps primary_device_type_ids only.

BEGIN TRANSACTION;

-- Drop triggers that referenced legacy column if any remain
DROP TRIGGER IF EXISTS trg_pd_pdtid_after_insert;
DROP TRIGGER IF EXISTS trg_pd_pdtid_after_update;

-- Recreate product_details without primary_device_type_id
CREATE TABLE IF NOT EXISTS product_details_new (
    url TEXT PRIMARY KEY,
    page_id INTEGER,
    index_in_page INTEGER,
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    device_type TEXT,
    certificate_id TEXT,
    certification_date TEXT,
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
    -- keep only normalized column
    primary_device_type_ids TEXT,
    application_categories TEXT,
    description TEXT,
    compliance_document_url TEXT,
    program_type TEXT DEFAULT 'Matter',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy data to the new table (ignore legacy column)
INSERT INTO product_details_new (
    url, page_id, index_in_page, id, manufacturer, model, device_type,
    certificate_id, certification_date, software_version, hardware_version,
    vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
    tis_trp_tested, specification_version, transport_interface,
    primary_device_type_ids, application_categories, description,
    compliance_document_url, program_type, created_at, updated_at
)
SELECT
    url, page_id, index_in_page, id, manufacturer, model, device_type,
    certificate_id, certification_date, software_version, hardware_version,
    vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
    tis_trp_tested, specification_version, transport_interface,
    primary_device_type_ids, application_categories, description,
    compliance_document_url, program_type, created_at, updated_at
FROM product_details;

-- Replace old table
DROP TABLE product_details;
ALTER TABLE product_details_new RENAME TO product_details;

-- Restore unique slot index and timestamp trigger
CREATE UNIQUE INDEX IF NOT EXISTS ux_product_details_slot
ON product_details(page_id, index_in_page)
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL;

CREATE TRIGGER IF NOT EXISTS product_details_updated_at
    AFTER UPDATE ON product_details
    FOR EACH ROW
BEGIN
    UPDATE product_details SET updated_at = CURRENT_TIMESTAMP WHERE url = NEW.url;
END;

-- Drop legacy compatibility view if it references the removed column
DROP VIEW IF EXISTS matter_products_legacy;

COMMIT;
