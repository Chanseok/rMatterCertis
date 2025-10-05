-- Migration 026: Enhance analytics view with diagnostic columns
-- Purpose: Add columns to track why products lack category information

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
  
  -- Device type information (existing)
  CASE 
    WHEN device_types_from_json.device_type_id IS NOT NULL 
    THEN device_types_from_json.device_type_id
    ELSE NULL
  END                                  AS device_type_id,
  CASE 
    WHEN device_types_from_json.device_type_name IS NOT NULL 
    THEN device_types_from_json.device_type_name
    ELSE NULL
  END                                  AS device_type_name,
  CASE 
    WHEN device_types_from_json.device_category IS NOT NULL 
    THEN device_types_from_json.device_category
    ELSE NULL
  END                                  AS device_category,
  CASE 
    WHEN device_types_from_json.device_introduced_in IS NOT NULL 
    THEN device_types_from_json.device_introduced_in
    ELSE NULL
  END                                  AS device_introduced_in,
  
  -- DIAGNOSTIC COLUMNS (NEW) --
  pd.primary_device_type_ids           AS raw_type_ids,              -- Raw JSON array from source
  CASE 
    WHEN pd.primary_device_type_ids IS NULL THEN 'NULL'
    WHEN pd.primary_device_type_ids = '' THEN 'EMPTY_STRING'
    WHEN pd.primary_device_type_ids = '[]' THEN 'EMPTY_ARRAY'
    WHEN json_valid(pd.primary_device_type_ids) = 0 THEN 'INVALID_JSON'
    ELSE 'VALID_JSON'
  END                                  AS type_ids_status,           -- Status of raw data
  
  CASE
    WHEN pd.primary_device_type_ids IS NULL 
      OR pd.primary_device_type_ids = '' 
      OR pd.primary_device_type_ids = '[]' THEN 'SOURCE_DATA_MISSING'
    WHEN device_types_from_json.device_type_id IS NULL THEN 'JOIN_FAILED_OR_TYPE_NOT_FOUND'
    ELSE 'OK'
  END                                  AS category_missing_reason,   -- Why category is NULL
  
  device_types_from_json.matched_count AS type_match_count,          -- How many types matched
  
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN (
  SELECT 
    pd_inner.url,
    GROUP_CONCAT(dt.type_id, ', ')     AS device_type_id,
    GROUP_CONCAT(dt.name, ', ')        AS device_type_name,
    GROUP_CONCAT(dt.category, ', ')    AS device_category,
    GROUP_CONCAT(dt.introduced_in, ', ') AS device_introduced_in,
    COUNT(DISTINCT dt.type_id)         AS matched_count  -- NEW: count of matched types
  FROM product_details pd_inner
  LEFT JOIN json_each(pd_inner.primary_device_type_ids) j ON j.value IS NOT NULL
  LEFT JOIN device_types dt ON CAST(j.value AS TEXT) = dt.type_id
  WHERE dt.type_id IS NOT NULL  -- Only include if type was found
  GROUP BY pd_inner.url
) device_types_from_json ON device_types_from_json.url = pd.url;

-- Diagnostic query to analyze category missing reasons
SELECT 
  category_missing_reason,
  COUNT(*) as count,
  ROUND(COUNT(*) * 100.0 / (SELECT COUNT(*) FROM v_product_detail_analytics), 2) as percentage
FROM v_product_detail_analytics
GROUP BY category_missing_reason
ORDER BY count DESC;

-- Show sample products for each missing reason
SELECT 
  '=== SOURCE_DATA_MISSING samples ===' as info;
SELECT product_detail_url, model, raw_type_ids, type_ids_status
FROM v_product_detail_analytics 
WHERE category_missing_reason = 'SOURCE_DATA_MISSING' 
LIMIT 5;

SELECT 
  '=== JOIN_FAILED_OR_TYPE_NOT_FOUND samples ===' as info;
SELECT product_detail_url, model, raw_type_ids, type_ids_status
FROM v_product_detail_analytics 
WHERE category_missing_reason = 'JOIN_FAILED_OR_TYPE_NOT_FOUND' 
LIMIT 5;

SELECT 
  '=== Migration 026 completed - Analytics view enhanced with diagnostics ===' as status;
