-- Migration 023: Enhance analytics view
-- Goal:
--  * Ensure transport_interface is always present (fallback 'Unknown')
--  * Provide non-empty device_introduced_in (fallback 'Unknown')
--  * Add legacy compatible alias transport_if
--  * Backfill device_types.introduced_in with 'Unknown' where NULL
--  * Idempotent

-- Backfill introduced_in
UPDATE device_types SET introduced_in = 'Unknown' WHERE introduced_in IS NULL OR introduced_in = '';

DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
SELECT
  pd.url                               AS product_detail_url,
  pd.model                             AS model,
  pd.certification_date                AS certification_date,
  pd.family_sku                        AS family_sku,
  COALESCE(NULLIF(pd.transport_interface,''), 'Unknown') AS transport_interface,
  COALESCE(NULLIF(pd.transport_interface,''), 'Unknown') AS transport_if, -- legacy alias
  pd.vid                               AS vid,
  v.vendor_number                      AS vendor_number,
  v.vendor_name                        AS vendor_name,
  v.company_legal_name                 AS company_legal_name,
  dtg.device_type_id,
  dtg.device_type_name,
  dtg.device_category,
  CASE 
    WHEN dtg.device_introduced_in IS NOT NULL AND dtg.device_introduced_in != '' THEN dtg.device_introduced_in
    ELSE 'Unknown'
  END                                  AS device_introduced_in,
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN (
  SELECT 
    pd_inner.url,
    GROUP_CONCAT(dt.type_id, ', ')      AS device_type_id,
    GROUP_CONCAT(dt.name, ', ')         AS device_type_name,
    GROUP_CONCAT(dt.category, ', ')     AS device_category,
    GROUP_CONCAT(dt.introduced_in, ', ') AS device_introduced_in
  FROM product_details pd_inner
  LEFT JOIN json_each(pd_inner.primary_device_type_ids) j ON j.value IS NOT NULL
  LEFT JOIN device_types dt ON CAST(j.value AS TEXT) = dt.type_id
  WHERE dt.type_id IS NOT NULL
  GROUP BY pd_inner.url
) dtg ON dtg.url = pd.url;

-- Verification (no-op in sqlx execute, kept for manual runs)
SELECT 'Migration 023 applied: analytics view enhanced with transport_if + introduced_in backfill' AS status;
