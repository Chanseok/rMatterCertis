-- Migration 019: Strict device type mapping (primary_device_type_ids -> device_types.type_id)
-- Updated Requirements:
--  * DO NOT backfill product_details.primary_device_type_ids from legacy textual device_type: leave NULL => results stay NULL.
--  * DO NOT fallback to device_types.id; only device_types.type_id is authoritative.
--  * Records where device_types.type_id IS NULL are NOT joinable and should not be coerced.
--  * No dual mapping logic; pure one-to-one on numeric codes found in primary_device_type_ids JSON to device_types.type_id.
--  * Provide deterministic representative mapping (lowest type_id) when multiple codes exist.
--  * Provide helper view to audit unmatched codes.
--
-- Notes:
--  * We intentionally avoid mutating device_types.type_id or primary_device_type_ids here to preserve raw data integrity.
--  * If upstream data quality needs enforcement (e.g., disallow NULL type_id) that should be handled in earlier schema migrations or ingestion.
--  * This migration can be re-run safely; INSERT OR IGNORE protects bridge table.
--  * Window functions (ROW_NUMBER) required.

BEGIN TRANSACTION;

-- Bridge table insert (strict: only codes matching device_types.type_id)
INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, dt.type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids) je
JOIN device_types dt ON dt.type_id = CAST(je.value AS INTEGER)
WHERE pd.primary_device_type_ids IS NOT NULL
  AND json_valid(pd.primary_device_type_ids)
  AND je.value GLOB '[0-9]*';

-- Analytics view: prefer bridge (ensures historical consistency), fallback to direct parse if bridge not yet populated.
DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
-- exploded: raw codes from JSON
WITH exploded AS (
  SELECT pd.url AS url,
       CAST(je.value AS INTEGER) AS code
  FROM product_details pd
  JOIN json_each(pd.primary_device_type_ids) je
  WHERE pd.primary_device_type_ids IS NOT NULL
    AND json_valid(pd.primary_device_type_ids)
    AND je.value GLOB '[0-9]*'
), direct_ranked AS (
  SELECT e.url,
       dt.type_id,
       dt.name,
       dt.category,
       dt.introduced_in,
       ROW_NUMBER() OVER (PARTITION BY e.url ORDER BY dt.type_id) AS rn
  FROM exploded e
  JOIN device_types dt ON dt.type_id = e.code
), picked_direct AS (
  SELECT url, type_id, name, category, introduced_in
  FROM direct_ranked WHERE rn = 1
), bridge_ranked AS (
  SELECT ppt.product_detail_id AS url,
       dt.type_id,
       dt.name,
       dt.category,
       dt.introduced_in,
       ROW_NUMBER() OVER (PARTITION BY ppt.product_detail_id ORDER BY dt.type_id) AS rn
  FROM product_primary_device_types ppt
  JOIN device_types dt ON dt.type_id = ppt.device_type_id
), picked_bridge AS (
  SELECT url, type_id, name, category, introduced_in
  FROM bridge_ranked WHERE rn = 1
), all_types AS (
  -- union of all type mappings (strict) for aggregation
  SELECT e.url, dt.type_id, dt.name, dt.category
  FROM exploded e
  JOIN device_types dt ON dt.type_id = e.code
  UNION
  SELECT ppt.product_detail_id AS url, dt.type_id, dt.name, dt.category
  FROM product_primary_device_types ppt
  JOIN device_types dt ON dt.type_id = ppt.device_type_id
), aggregated AS (
  SELECT url,
       GROUP_CONCAT(DISTINCT type_id, ',')       AS device_type_ids_all,
       GROUP_CONCAT(DISTINCT name, ',')          AS device_type_names_all,
       GROUP_CONCAT(DISTINCT category, ',')      AS device_categories_all
  FROM all_types
  GROUP BY url
)
SELECT
  pd.url                                AS product_detail_url,
  pd.model                              AS model,
  pd.certification_date                 AS certification_date,
  pd.family_sku                         AS family_sku,
  pd.transport_interface                AS transport_interface,
  pd.vid                                AS vid,
  v.vendor_number                       AS vendor_number,
  v.vendor_name                         AS vendor_name,
  v.company_legal_name                  AS company_legal_name,
  COALESCE(pb.type_id, pd2.type_id)     AS device_type_id,
  COALESCE(pb.name, pd2.name)           AS device_type_name,
  COALESCE(pb.category, pd2.category)   AS device_category,
  COALESCE(pb.introduced_in, pd2.introduced_in) AS device_introduced_in,
  agg.device_type_ids_all               AS device_type_ids_all,
  agg.device_type_names_all             AS device_type_names_all,
  agg.device_categories_all             AS device_categories_all,
  pd.created_at                         AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN picked_bridge pb ON pb.url = pd.url
LEFT JOIN picked_direct pd2 ON pd2.url = pd.url
LEFT JOIN aggregated agg ON agg.url = pd.url;

COMMIT;

-- (Optional helper) View: v_product_detail_device_type_codes
-- Purpose: Expose one row per product_detail x raw JSON code with join resolution outcome.
-- Enables debugging and ad‑hoc analytics without re-writing JSON expansion logic.
-- Columns:
--   product_detail_url  : product_details.url
--   raw_code            : INTEGER extracted from JSON array
--   matched_type_id     : device_types.type_id if matched (NULL if not)
--   device_type_name    : device_types.name (NULL if not matched)
--   device_category     : device_types.category
--   match_source        : 'type_id' | 'id' | 'unmatched' (which column produced the match)
--   introduced_in       : device_types.introduced_in
-- This view can be joined directly to other analytics without relying on the bridge.

DROP VIEW IF EXISTS v_product_detail_device_type_codes;
CREATE VIEW v_product_detail_device_type_codes AS
WITH json_codes AS (
    SELECT pd.url AS product_detail_url,
      CAST(je.value AS INTEGER) AS raw_code
    FROM product_details pd
    JOIN json_each(pd.primary_device_type_ids) je
    WHERE pd.primary_device_type_ids IS NOT NULL
      AND json_valid(pd.primary_device_type_ids)
      AND je.value GLOB '[0-9]*'
), ranked AS (
    SELECT jc.product_detail_url,
      jc.raw_code,
      dt.type_id AS matched_type_id,
      dt.name AS device_type_name,
      dt.category AS device_category,
      dt.introduced_in,
      CASE WHEN dt.type_id IS NOT NULL THEN 'type_id' ELSE 'unmatched' END AS match_source,
      ROW_NUMBER() OVER (
        PARTITION BY jc.product_detail_url, jc.raw_code
        ORDER BY CASE WHEN dt.type_id IS NOT NULL THEN 1 ELSE 2 END, dt.type_id
      ) AS rn
    FROM json_codes jc
    LEFT JOIN device_types dt ON dt.type_id = jc.raw_code
)
SELECT product_detail_url,
  raw_code,
  CASE WHEN match_source = 'type_id' THEN matched_type_id END AS matched_type_id,
  CASE WHEN match_source = 'type_id' THEN device_type_name END AS device_type_name,
  CASE WHEN match_source = 'type_id' THEN device_category END AS device_category,
  match_source,
  CASE WHEN match_source = 'type_id' THEN introduced_in END AS introduced_in
FROM ranked
WHERE rn = 1;

