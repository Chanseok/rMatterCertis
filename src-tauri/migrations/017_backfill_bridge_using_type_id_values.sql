-- Migration 017: Backfill product_primary_device_types using primary_device_type_ids as type_id codes
-- Assumption: product_details.primary_device_type_ids is a JSON array of decimal Matter device type codes
-- that correspond directly to device_types.type_id. Earlier migrations failed because of FK mismatch / timing.
-- This migration:
--   1. Ensures device_types.type_id populated.
--   2. Inserts bridge rows mapping pd.url -> device_types.type_id (only when both exist).
--   3. Recreates analytics view (adds transport_interface passthrough if not already present).
-- Idempotent: uses INSERT OR IGNORE and does not delete existing rows.

BEGIN TRANSACTION;

UPDATE device_types SET type_id = id WHERE type_id IS NULL;

INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, dt.type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids) je
JOIN device_types dt ON dt.type_id = CAST(je.value AS INTEGER)
WHERE pd.primary_device_type_ids IS NOT NULL
  AND json_valid(pd.primary_device_type_ids)
  AND je.value GLOB '[0-9]*';

-- Refresh analytics view to ensure joins reflect new bridge rows (structure kept consistent)
DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
SELECT
  pd.url                               AS product_detail_url,
  pd.model                             AS model,
  pd.certification_date                AS certification_date,
  pd.family_sku                        AS family_sku,
  pd.transport_interface               AS transport_interface,
  pd.vid                               AS vid,
  v.vendor_number                      AS vendor_number,
  v.vendor_name                        AS vendor_name,
  v.company_legal_name                 AS company_legal_name,
  d.type_id                            AS device_type_id,
  d.name                               AS device_type_name,
  d.category                           AS device_category,
  d.introduced_in                      AS device_introduced_in,
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN product_primary_device_types ppt ON ppt.product_detail_id = pd.url
LEFT JOIN device_types d ON d.type_id = ppt.device_type_id;

COMMIT;