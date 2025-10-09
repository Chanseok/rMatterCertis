-- Migration 015: Fallback backfill for primary_device_type_ids and bridge when original JSON list is empty
-- Strategy:
--  1. For rows where primary_device_type_ids IS NULL or '' attempt to map product_details.device_type to device_types.name (case-insensitive)
--  2. If match found, set primary_device_type_ids = json_array(matched type_id)
--  3. Insert corresponding rows into product_primary_device_types (OR IGNORE to keep idempotent)
--  4. Recreate analytics view (defensive)
--
-- Idempotent: running multiple times will not duplicate bridge rows due to OR IGNORE.

BEGIN TRANSACTION;

-- 1) Backfill primary_device_type_ids from device_type name
UPDATE product_details
SET primary_device_type_ids = (
  SELECT json_array(dt.type_id)
  FROM device_types dt
  WHERE lower(trim(dt.name)) = lower(trim(product_details.device_type))
  LIMIT 1
)
WHERE (primary_device_type_ids IS NULL OR primary_device_type_ids = '')
  AND device_type IS NOT NULL AND device_type <> ''
  AND EXISTS (
    SELECT 1 FROM device_types dt2 WHERE lower(trim(dt2.name)) = lower(trim(product_details.device_type))
  );

-- 2) Bridge insert for any rows now having primary_device_type_ids but missing in bridge
INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, CAST(json_each.value AS INTEGER) AS device_type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids)
LEFT JOIN product_primary_device_types ppt ON ppt.product_detail_id = pd.url AND ppt.device_type_id = CAST(json_each.value AS INTEGER)
WHERE (pd.primary_device_type_ids IS NOT NULL AND json_valid(pd.primary_device_type_ids))
  AND json_each.value GLOB '[0-9]*'
  AND ppt.product_detail_id IS NULL;

-- 3) Recreate analytics view (same definition)
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
