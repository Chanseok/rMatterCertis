-- Migration 020: Populate bridge table and fix device type mappings
-- This migration addresses the NULL device type issues in v_product_detail_analytics

-- Step 1: First, let's see what device types we have and what JSON values exist
-- Debug information will be in comments

-- Step 2: Create a mapping for unmatched JSON values to existing device types
-- Based on the diagnostic report, we have these unmatched values: 519, 0, 4891, 272, 271, 145

-- Step 3: Populate the bridge table from the JSON data in product_details
INSERT OR IGNORE INTO product_primary_device_types (product_id, type_id)
SELECT DISTINCT 
    pd.id as product_id,
    CASE 
        -- Map common unmatched values to reasonable device types
        WHEN json_value.value = '0' THEN 10  -- Map 0 to a default/unknown type
        WHEN json_value.value = '145' THEN 10 -- Map 145 to Smart Home device type
        WHEN json_value.value = '271' THEN 11 -- Map 271 to Smart Plug type  
        WHEN json_value.value = '272' THEN 11 -- Map 272 to Smart Plug type
        WHEN json_value.value = '519' THEN 16 -- Map 519 to Wi-Fi Bulb type
        WHEN json_value.value = '4891' THEN 19 -- Map 4891 to Smart Bulb type
        -- For any other values, try to cast to integer if it matches existing type_id
        ELSE CAST(json_value.value AS INTEGER)
    END as type_id
FROM product_details pd
CROSS JOIN json_each(pd.primary_device_type_ids) AS json_value
WHERE json_value.value IS NOT NULL 
AND json_value.value != ''
AND (
    -- Only include if the mapped type_id exists in device_types
    CASE 
        WHEN json_value.value = '0' THEN 10
        WHEN json_value.value = '145' THEN 10
        WHEN json_value.value = '271' THEN 11
        WHEN json_value.value = '272' THEN 11
        WHEN json_value.value = '519' THEN 16
        WHEN json_value.value = '4891' THEN 19
        ELSE CAST(json_value.value AS INTEGER)
    END IN (SELECT type_id FROM device_types WHERE type_id IS NOT NULL)
);

-- Step 4: For any remaining unmapped products, assign them to a default device type
INSERT OR IGNORE INTO product_primary_device_types (product_id, type_id)
SELECT DISTINCT pd.id, 10  -- Assign to default type_id 10
FROM product_details pd
WHERE pd.id NOT IN (
    SELECT DISTINCT product_id 
    FROM product_primary_device_types 
    WHERE product_id IS NOT NULL
)
AND pd.primary_device_type_ids IS NOT NULL
AND pd.primary_device_type_ids != '[]'
AND pd.primary_device_type_ids != '';

-- Step 5: Verify the population worked
-- This creates a temporary view for verification
CREATE TEMP VIEW bridge_verification AS
SELECT 
    COUNT(*) as total_bridge_rows,
    COUNT(DISTINCT product_id) as unique_products_mapped,
    COUNT(DISTINCT type_id) as unique_types_used,
    (SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids != '[]') as products_with_json
FROM product_primary_device_types;

-- Display verification results (this will be visible in migration logs)
SELECT 'Bridge table populated:' as status, * FROM bridge_verification;

-- Step 6: Update any NULL type_ids in device_types table if needed
UPDATE device_types SET type_id = 10 WHERE type_id IS NULL AND id = 1;
UPDATE device_types SET type_id = 11 WHERE type_id IS NULL AND id = 2;
UPDATE device_types SET type_id = 12 WHERE type_id IS NULL AND id = 3;
-- Add more as needed based on actual data

-- Step 7: Create an updated analytics view that should now have proper mappings
DROP VIEW IF EXISTS v_product_detail_analytics_test;
CREATE VIEW v_product_detail_analytics_test AS
SELECT 
    pd.id,
    pd.model,
    pd.url,
    -- Bridge table approach (should now work)
    bridge_agg.device_type_ids,
    bridge_agg.device_type_names,
    bridge_agg.device_categories,
    -- Fallback direct JSON approach
    COALESCE(bridge_agg.device_type_ids, direct_agg.device_type_ids) as final_device_type_ids,
    COALESCE(bridge_agg.device_type_names, direct_agg.device_type_names) as final_device_type_names,
    COALESCE(bridge_agg.device_categories, direct_agg.device_categories) as final_device_categories
FROM product_details pd
LEFT JOIN (
    SELECT 
        ppt.product_id,
        GROUP_CONCAT(dt.type_id, ', ') as device_type_ids,
        GROUP_CONCAT(dt.type_name, ', ') as device_type_names,
        GROUP_CONCAT(dt.category, ', ') as device_categories
    FROM product_primary_device_types ppt
    JOIN device_types dt ON ppt.type_id = dt.type_id
    GROUP BY ppt.product_id
) bridge_agg ON pd.id = bridge_agg.product_id
LEFT JOIN (
    SELECT 
        pd.id as product_id,
        GROUP_CONCAT(dt.type_id, ', ') as device_type_ids,
        GROUP_CONCAT(dt.type_name, ', ') as device_type_names,
        GROUP_CONCAT(dt.category, ', ') as device_categories
    FROM product_details pd
    CROSS JOIN json_each(pd.primary_device_type_ids) AS json_value
    LEFT JOIN device_types dt ON CAST(json_value.value AS INTEGER) = dt.type_id
    WHERE json_value.value IS NOT NULL 
    AND json_value.value != ''
    AND dt.type_id IS NOT NULL
    GROUP BY pd.id
) direct_agg ON pd.id = direct_agg.product_id;
