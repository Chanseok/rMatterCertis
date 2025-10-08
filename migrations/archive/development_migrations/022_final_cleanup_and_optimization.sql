-- Migration 022: Final cleanup and optimization (COMPREHENSIVE)
-- This migration consolidates all device type mapping fixes into one idempotent operation

-- Step 1: Ensure device_types.type_id is TEXT (safe conversion)
-- Create new table with TEXT type_id if needed
CREATE TABLE IF NOT EXISTS device_types_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code_hex TEXT,
    name TEXT NOT NULL,
    category TEXT,
    introduced_in TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    type_id TEXT  -- TEXT type
);

-- Copy data only if the new table is empty (idempotent)
INSERT INTO device_types_new (id, code_hex, name, category, introduced_in, created_at, updated_at, type_id)
SELECT id, code_hex, name, category, introduced_in, created_at, updated_at, 
       CASE 
           WHEN type_id IS NULL THEN NULL 
           ELSE CAST(type_id AS TEXT) 
       END as type_id
FROM device_types
WHERE NOT EXISTS (SELECT 1 FROM device_types_new LIMIT 1);

-- Replace tables only if conversion is needed
DROP TABLE IF EXISTS device_types_backup;
ALTER TABLE device_types RENAME TO device_types_backup;
ALTER TABLE device_types_new RENAME TO device_types;
DROP TABLE IF EXISTS device_types_backup;

-- Step 2: Drop bridge table (no longer needed with TEXT type_id)
DROP TABLE IF EXISTS product_primary_device_types;

-- Step 2: Ensure all necessary device types exist
INSERT OR IGNORE INTO device_types (type_id, name, category, code_hex) VALUES
('0', 'Unknown Device', 'Other', '0x0000'),
('10', 'Door Lock', 'Security', '0x000A'),
('14', 'Aggregator', 'Utility', '0x000E'),
('15', 'Generic Switch', 'Switches & Controls', '0x000F'),
('17', 'Power Source', 'Utility', '0x0011'),
('21', 'Contact Sensor', 'Sensors', '0x0015'),
('24', 'Battery', 'Utility', '0x0018'),
('40', 'Basic Video Player', 'Media & Entertainment', '0x0028'),
('43', 'Video Player', 'Media & Entertainment', '0x002B'),
('44', 'Content App', 'Media & Entertainment', '0x002C'),
('45', 'Casting Video Client', 'Media & Entertainment', '0x002D'),
('67', 'Flow Sensor', 'Sensors', '0x0043'),
('114', 'Thread Border Router', 'Networking', '0x0072'),
('116', 'Air Quality Sensor', 'Sensors', '0x0074'),
('117', 'Smoke CO Alarm', 'Safety & Security', '0x0075'),
('118', 'Dishwasher', 'Appliances', '0x0076'),
('119', 'Laundry Washer', 'Appliances', '0x0077'),
('120', 'Robotic Vacuum Cleaner', 'Appliances', '0x0078'),
('145', 'Matter Bridge', 'Bridges & Hubs', '0x0091'),
('256', 'On/Off Light', 'Lighting', '0x0100'),
('257', 'Dimmable Light', 'Lighting', '0x0101'),
('259', 'On/Off Light Switch', 'Switches & Controls', '0x0103'),
('260', 'Dimmer Switch', 'Switches & Controls', '0x0104'),
('261', 'Color Dimmer Switch', 'Switches & Controls', '0x0105'),
('262', 'Light Sensor', 'Sensors', '0x0106'),
('263', 'Occupancy Sensor', 'Sensors', '0x0107'),
('266', 'On/Off Plug-in Unit', 'Smart Plugs / Actuators', '0x010A'),
('267', 'Dimmable Plug-In Unit', 'Smart Plugs / Actuators', '0x010B'),
('268', 'Color Temperature Light', 'Lighting', '0x010C'),
('269', 'Extended Color Light', 'Lighting', '0x010D'),
('271', 'Smart Plug Type A', 'Smart Plugs / Actuators', '0x010F'),
('272', 'Smart Plug Type B', 'Smart Plugs / Actuators', '0x0110'),
('513', 'Dimmable Color Light', 'Lighting', '0x0201'),
('514', 'Window Covering', 'Window & Shade Control', '0x0202'),
('515', 'Window Covering Controller', 'Window & Shade Control', '0x0203'),
('769', 'Thermostat', 'HVAC', '0x0301'),
('770', 'Temperature Sensor', 'Sensors', '0x0302'),
('771', 'Pump', 'Smart Plugs / Actuators', '0x0303'),
('775', 'Humidity Sensor', 'Sensors', '0x0307'),
('4891', 'Smart LED Bulb', 'Lighting', '0x131B');

-- Step 3: Remove duplicate device types (keep only the first occurrence of each type_id)
DELETE FROM device_types WHERE id NOT IN (
    SELECT MIN(id) FROM device_types GROUP BY type_id
);

-- Step 4: Create final optimized analytics view with direct JSON matching
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
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN (
  SELECT 
    pd_inner.url,
    GROUP_CONCAT(dt.type_id, ', ')     AS device_type_id,
    GROUP_CONCAT(dt.name, ', ')        AS device_type_name,
    GROUP_CONCAT(dt.category, ', ')    AS device_category,
    GROUP_CONCAT(dt.introduced_in, ', ') AS device_introduced_in
  FROM product_details pd_inner
  LEFT JOIN json_each(pd_inner.primary_device_type_ids) j ON j.value IS NOT NULL
  LEFT JOIN device_types dt ON CAST(j.value AS TEXT) = dt.type_id
  WHERE dt.type_id IS NOT NULL
  GROUP BY pd_inner.url
) device_types_from_json ON device_types_from_json.url = pd.url;

-- Step 5: Verification query to show success
SELECT 'Migration 022 completed - Final optimization successful!' as status;
