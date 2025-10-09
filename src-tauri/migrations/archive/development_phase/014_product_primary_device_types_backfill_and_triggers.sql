-- Migration 014: Backfill product_primary_device_types and add maintenance triggers
-- Idempotent: safe to run multiple times.
-- Purpose:
--  1. Populate bridge rows from product_details.primary_device_type_ids if missing
--  2. Add triggers so future INSERT/UPDATE on product_details keep bridge in sync
--  3. Recreate analytics view to ensure latest definition (defensive)

BEGIN TRANSACTION;

-- 1) Backfill (only inserts rows that do not already exist due to OR IGNORE)
INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, CAST(json_each.value AS INTEGER) AS device_type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids)
WHERE pd.primary_device_type_ids IS NOT NULL
  AND json_valid(pd.primary_device_type_ids)
  AND json_each.value GLOB '[0-9]*';

-- 2) Triggers to keep bridge table updated
CREATE TRIGGER IF NOT EXISTS trg_ppt_after_insert
AFTER INSERT ON product_details
FOR EACH ROW WHEN NEW.primary_device_type_ids IS NOT NULL AND json_valid(NEW.primary_device_type_ids)
BEGIN
  INSERT OR IGNORE INTO product_primary_device_types(product_detail_id, device_type_id)
  SELECT NEW.url, CAST(json_each.value AS INTEGER)
  FROM json_each(NEW.primary_device_type_ids)
  WHERE json_each.value GLOB '[0-9]*';
END;

CREATE TRIGGER IF NOT EXISTS trg_ppt_after_update
AFTER UPDATE OF primary_device_type_ids ON product_details
FOR EACH ROW
BEGIN
  DELETE FROM product_primary_device_types WHERE product_detail_id = NEW.url;
  INSERT OR IGNORE INTO product_primary_device_types(product_detail_id, device_type_id)
  SELECT NEW.url, CAST(json_each.value AS INTEGER)
  FROM json_each(NEW.primary_device_type_ids)
  WHERE NEW.primary_device_type_ids IS NOT NULL
    AND json_valid(NEW.primary_device_type_ids)
    AND json_each.value GLOB '[0-9]*';
END;

-- 3) Recreate analytics view (same definition as 013) to ensure device joins reflect new data
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
