-- Migration 021: Convert device_types.type_id from INTEGER to TEXT
-- This fixes the JSON string to INTEGER matching issue

-- Step 1: Create a new table with TEXT type_id
CREATE TABLE IF NOT EXISTS device_types_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code_hex TEXT,
    name TEXT NOT NULL,
    category TEXT,
    introduced_in TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    type_id TEXT  -- Changed from INTEGER to TEXT
);

-- Step 2: Copy existing data, converting INTEGER type_id to TEXT
INSERT INTO device_types_new (id, code_hex, name, category, introduced_in, created_at, updated_at, type_id)
SELECT id, code_hex, name, category, introduced_in, created_at, updated_at, 
       CASE 
           WHEN type_id IS NULL THEN NULL 
           ELSE CAST(type_id AS TEXT) 
       END as type_id
FROM device_types;

-- Step 3: Drop the old table and rename the new one
DROP TABLE device_types;
ALTER TABLE device_types_new RENAME TO device_types;

-- Step 4: Recreate any indexes that might have existed
CREATE INDEX IF NOT EXISTS idx_device_types_type_id ON device_types(type_id);
CREATE INDEX IF NOT EXISTS idx_device_types_category ON device_types(category);

-- Step 5: Add any missing device types for unmatched JSON values
INSERT OR IGNORE INTO device_types (type_id, name, category, code_hex) VALUES
('0', 'Unknown Device', 'Other', '0x0000'),
('145', 'Matter Bridge', 'Bridges & Hubs', '0x0091'),
('271', 'Smart Plug Type A', 'Smart Plugs / Actuators', '0x010F'),
('272', 'Smart Plug Type B', 'Smart Plugs / Actuators', '0x0110'),
('513', 'Custom Device Type 513', 'Other', '0x0201'),
('519', 'Wi-Fi Light Bulb', 'Lighting', '0x0207'),
('4891', 'Smart LED Bulb', 'Lighting', '0x131B');

-- Step 6: Update product_primary_device_types bridge table to use TEXT
-- First, recreate the bridge table with TEXT type_id
CREATE TABLE IF NOT EXISTS product_primary_device_types_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    type_id TEXT NOT NULL,  -- Changed from INTEGER to TEXT
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (type_id) REFERENCES device_types(type_id) ON DELETE CASCADE
);

-- Copy existing data if any
INSERT OR IGNORE INTO product_primary_device_types_new (product_id, type_id, created_at)
SELECT product_id, CAST(type_id AS TEXT), created_at 
FROM product_primary_device_types 
WHERE type_id IS NOT NULL;

-- Replace the bridge table
DROP TABLE IF EXISTS product_primary_device_types;
ALTER TABLE product_primary_device_types_new RENAME TO product_primary_device_types;

-- Recreate index
CREATE UNIQUE INDEX IF NOT EXISTS idx_product_device_type_unique 
ON product_primary_device_types(product_id, type_id);

-- Step 7: Populate the bridge table with proper TEXT matching
INSERT OR IGNORE INTO product_primary_device_types (product_id, type_id)
SELECT DISTINCT 
    pd.id as product_id,
    json_value.value as type_id  -- Now direct text matching!
FROM product_details pd
CROSS JOIN json_each(pd.primary_device_type_ids) AS json_value
JOIN device_types dt ON json_value.value = dt.type_id  -- TEXT to TEXT comparison
WHERE json_value.value IS NOT NULL 
AND json_value.value != ''
AND json_value.value != '[]';

-- Step 8: Create a verification view to check the results
CREATE TEMP VIEW bridge_population_check AS
SELECT 
    'Bridge Population Results:' as status,
    COUNT(*) as total_bridge_rows,
    COUNT(DISTINCT product_id) as unique_products_mapped,
    COUNT(DISTINCT type_id) as unique_types_used
FROM product_primary_device_types;

-- Display results
SELECT * FROM bridge_population_check;

-- Step 9: Update the analytics view to use TEXT type_id
DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
SELECT 
    pd.id,
    pd.model,
    pd.url,
    pd.primary_device_type_ids,
    -- Use bridge table approach with TEXT matching
    COALESCE(
        (SELECT GROUP_CONCAT(dt.type_id, ', ') 
         FROM product_primary_device_types ppt 
         JOIN device_types dt ON ppt.type_id = dt.type_id 
         WHERE ppt.product_id = pd.id),
        -- Fallback to direct JSON parsing with TEXT matching
        (SELECT GROUP_CONCAT(dt.type_id, ', ')
         FROM json_each(pd.primary_device_type_ids) AS json_value
         JOIN device_types dt ON json_value.value = dt.type_id
         WHERE json_value.value IS NOT NULL AND json_value.value != '')
    ) as device_type_id,
    
    COALESCE(
        (SELECT GROUP_CONCAT(dt.name, ', ') 
         FROM product_primary_device_types ppt 
         JOIN device_types dt ON ppt.type_id = dt.type_id 
         WHERE ppt.product_id = pd.id),
        (SELECT GROUP_CONCAT(dt.name, ', ')
         FROM json_each(pd.primary_device_type_ids) AS json_value
         JOIN device_types dt ON json_value.value = dt.type_id
         WHERE json_value.value IS NOT NULL AND json_value.value != '')
    ) as device_type_name,
    
    COALESCE(
        (SELECT GROUP_CONCAT(dt.category, ', ') 
         FROM product_primary_device_types ppt 
         JOIN device_types dt ON ppt.type_id = dt.type_id 
         WHERE ppt.product_id = pd.id),
        (SELECT GROUP_CONCAT(dt.category, ', ')
         FROM json_each(pd.primary_device_type_ids) AS json_value
         JOIN device_types dt ON json_value.value = dt.type_id
         WHERE json_value.value IS NOT NULL AND json_value.value != '')
    ) as device_category
FROM product_details pd;
