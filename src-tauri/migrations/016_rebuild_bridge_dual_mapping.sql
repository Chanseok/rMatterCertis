-- Migration 016: Rebuild product_primary_device_types using dual mapping (type_id OR id)
-- Also backfill missing type_id values.
-- Idempotent and safe to rerun.
BEGIN TRANSACTION;
-- 1) Backfill missing type_id from id
UPDATE device_types SET type_id = id WHERE type_id IS NULL;
-- 2) Populate bridge rows (dual mapping) without deleting existing (INSERT OR IGNORE)
WITH vals AS (
  SELECT pd.url AS url, CAST(json_each.value AS INTEGER) AS dt_numeric
  FROM product_details pd
  JOIN json_each(pd.primary_device_type_ids)
  WHERE pd.primary_device_type_ids IS NOT NULL
    AND json_valid(pd.primary_device_type_ids)
    AND json_each.value GLOB '[0-9]*'
), map AS (
  SELECT v.url, dt.type_id AS device_type_id
  FROM vals v
  JOIN device_types dt ON dt.type_id = v.dt_numeric
  UNION
  SELECT v.url, dt.type_id AS device_type_id
  FROM vals v
  JOIN device_types dt ON dt.id = v.dt_numeric
)
INSERT OR IGNORE INTO product_primary_device_types(product_detail_id, device_type_id)
SELECT url, device_type_id FROM map;
-- 3) Recreate analytics view to ensure join sees new rows
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
