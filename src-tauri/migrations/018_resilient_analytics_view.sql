-- Migration 018: Resilient analytics view
-- Combines bridge-based mapping (product_primary_device_types) with direct JSON->device_types mapping
-- so analytics still shows device types even if bridge failed to populate.
-- Idempotent: just recreates the view.
BEGIN TRANSACTION;
DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
WITH direct AS (
  SELECT pd.url AS url, dt.type_id, dt.name, dt.category, dt.introduced_in
  FROM product_details pd
  JOIN json_each(pd.primary_device_type_ids) je
  JOIN device_types dt ON dt.type_id = CAST(je.value AS INTEGER)
  WHERE pd.primary_device_type_ids IS NOT NULL AND json_valid(pd.primary_device_type_ids)
  GROUP BY pd.url
), bridged AS (
  SELECT ppt.product_detail_id AS url, dt.type_id, dt.name, dt.category, dt.introduced_in
  FROM product_primary_device_types ppt
  JOIN device_types dt ON dt.type_id = ppt.device_type_id
  GROUP BY ppt.product_detail_id
)
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
  COALESCE(b.type_id, d.type_id)       AS device_type_id,
  COALESCE(b.name, d.name)             AS device_type_name,
  COALESCE(b.category, d.category)     AS device_category,
  COALESCE(b.introduced_in, d.introduced_in) AS device_introduced_in,
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN bridged b ON b.url = pd.url
LEFT JOIN direct d ON d.url = pd.url;
COMMIT;