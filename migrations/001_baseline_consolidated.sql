-- 002_baseline_cleaned.sql
-- Cleaned baseline schema for fresh installations.
-- Version: 2.0 (2025-10-10)
-- Changes from v1.0:
--   - REMOVED: sync_sessions, sync_observed (타입 불일치로 실제 동작 안 함)
--   - REMOVED: crawling_results (사용처 없음)
--   - REMOVED: page_fetch_attempts (선택적 디버깅 기능, MC_ATTEMPT_LOG_SQLITE 환경 변수 필요, 일반 사용 안 함)
--   - REMOVED: application_categories 컬럼 (파싱 로직 없음, 항상 NULL)

PRAGMA foreign_keys = ON;
BEGIN;

-- ============================================================================
-- CORE TABLES
-- ============================================================================

CREATE TABLE IF NOT EXISTS vendors (
  vendor_number INTEGER PRIMARY KEY,
  vendor_name TEXT,
  company_legal_name TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Products: URL is PRIMARY KEY, coordinates (page_id, index_in_page) are reference metadata only
-- NO UNIQUE constraint on coordinates to allow dynamic repositioning when site order changes
CREATE TABLE IF NOT EXISTS products (
  url TEXT PRIMARY KEY,
  manufacturer TEXT,
  model TEXT,
  certificate_id TEXT,
  page_id INTEGER,
  index_in_page INTEGER,
  id TEXT,               -- backfilled composite id (pXXXXiYY)
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Product Details: URL is PRIMARY KEY, coordinates are reference metadata only
-- NO UNIQUE constraint on coordinates to allow dynamic repositioning when site order changes
CREATE TABLE IF NOT EXISTS product_details (
  url TEXT PRIMARY KEY,
  page_id INTEGER,
  index_in_page INTEGER,
  id TEXT,
  manufacturer TEXT,
  model TEXT,
  device_type TEXT,
  certificate_id TEXT,
  certification_date DATE,
  software_version TEXT,
  hardware_version TEXT,
  firmware_version TEXT,
  specification_version TEXT,
  vid INTEGER,
  pid INTEGER,
  family_sku TEXT,
  family_variant_sku TEXT,
  family_id TEXT,
  tis_trp_tested TEXT,
  transport_interface TEXT,
  primary_device_type_ids TEXT,
  -- REMOVED: application_categories TEXT (not parsed, always NULL)
  description TEXT,
  compliance_document_url TEXT,
  program_type TEXT DEFAULT 'Matter',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS device_types (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  code_hex TEXT,
  name TEXT NOT NULL,
  category TEXT,
  introduced_in TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  type_id TEXT
);

-- ============================================================================
-- DIAGNOSTIC TABLES
-- ============================================================================

-- REMOVED: page_fetch_attempts (환경 변수로 제어되는 선택적 기능, 일반 사용자는 사용 안 함)

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details (manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_device_type ON product_details (device_type);
CREATE INDEX IF NOT EXISTS idx_product_details_certificate_id ON product_details (certificate_id);
CREATE INDEX IF NOT EXISTS idx_product_details_certification_date ON product_details (certification_date);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specification_version);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);
CREATE INDEX IF NOT EXISTS idx_device_types_type_id ON device_types (type_id);
CREATE INDEX IF NOT EXISTS idx_device_types_category ON device_types (category);

-- ============================================================================
-- SEED DATA
-- ============================================================================

-- SEED DEVICE TYPES (subset + known comprehensive list from prior migrations)
INSERT OR IGNORE INTO device_types (type_id, name, category, code_hex, introduced_in) VALUES
('0', 'Unknown Device', 'Other', '0x0000', 'Unknown'),
('10', 'Door Lock', 'Security', '0x000A', 'Unknown'),
('14', 'Aggregator', 'Utility', '0x000E', 'Unknown'),
('15', 'Generic Switch', 'Switches & Controls', '0x000F', 'Unknown'),
('17', 'Power Source', 'Utility', '0x0011', 'Unknown'),
('21', 'Contact Sensor', 'Sensors', '0x0015', 'Unknown'),
('24', 'Battery', 'Utility', '0x0018', 'Unknown'),
('40', 'Basic Video Player', 'Media & Entertainment', '0x0028', 'Unknown'),
('43', 'Video Player', 'Media & Entertainment', '0x002B', 'Unknown'),
('44', 'Content App', 'Media & Entertainment', '0x002C', 'Unknown'),
('45', 'Casting Video Client', 'Media & Entertainment', '0x002D', 'Unknown'),
('67', 'Flow Sensor', 'Sensors', '0x0043', 'Unknown'),
('114', 'Thread Border Router', 'Networking', '0x0072', 'Unknown'),
('116', 'Air Quality Sensor', 'Sensors', '0x0074', 'Unknown'),
('117', 'Smoke CO Alarm', 'Safety & Security', '0x0075', 'Unknown'),
('118', 'Dishwasher', 'Appliances', '0x0076', 'Unknown'),
('119', 'Laundry Washer', 'Appliances', '0x0077', 'Unknown'),
('120', 'Robotic Vacuum Cleaner', 'Appliances', '0x0078', 'Unknown'),
('145', 'Matter Bridge', 'Bridges & Hubs', '0x0091', 'Unknown'),
('256', 'On/Off Light', 'Lighting', '0x0100', 'Unknown'),
('257', 'Dimmable Light', 'Lighting', '0x0101', 'Unknown'),
('259', 'On/Off Light Switch', 'Switches & Controls', '0x0103', 'Unknown'),
('260', 'Dimmer Switch', 'Switches & Controls', '0x0104', 'Unknown'),
('261', 'Color Dimmer Switch', 'Switches & Controls', '0x0105', 'Unknown'),
('262', 'Light Sensor', 'Sensors', '0x0106', 'Unknown'),
('263', 'Occupancy Sensor', 'Sensors', '0x0107', 'Unknown'),
('266', 'On/Off Plug-in Unit', 'Smart Plugs / Actuators', '0x010A', 'Unknown'),
('267', 'Dimmable Plug-In Unit', 'Smart Plugs / Actuators', '0x010B', 'Unknown'),
('268', 'Color Temperature Light', 'Lighting', '0x010C', 'Unknown'),
('269', 'Extended Color Light', 'Lighting', '0x010D', 'Unknown'),
('271', 'Smart Plug Type A', 'Smart Plugs / Actuators', '0x010F', 'Unknown'),
('272', 'Smart Plug Type B', 'Smart Plugs / Actuators', '0x0110', 'Unknown'),
('513', 'Dimmable Color Light', 'Lighting', '0x0201', 'Unknown'),
('514', 'Window Covering', 'Window & Shade Control', '0x0202', 'Unknown'),
('515', 'Window Covering Controller', 'Window & Shade Control', '0x0203', 'Unknown'),
('769', 'Thermostat', 'HVAC', '0x0301', 'Unknown'),
('770', 'Temperature Sensor', 'Sensors', '0x0302', 'Unknown'),
('771', 'Pump', 'Smart Plugs / Actuators', '0x0303', 'Unknown'),
('775', 'Humidity Sensor', 'Sensors', '0x0307', 'Unknown'),
('4891', 'Smart LED Bulb', 'Lighting', '0x131B', 'Unknown');

-- Ensure introduced_in not null/empty
UPDATE device_types SET introduced_in='Unknown' WHERE introduced_in IS NULL OR introduced_in='';

-- Remove duplicate type_id rows keeping first
DELETE FROM device_types WHERE id NOT IN (SELECT MIN(id) FROM device_types GROUP BY type_id);

-- ============================================================================
-- DATA NORMALIZATION
-- ============================================================================

-- DATE NORMALIZATION (MM/DD/YYYY -> YYYY-MM-DD)
UPDATE product_details
SET certification_date = substr(certification_date,7,4)||'-'||substr(certification_date,1,2)||'-'||substr(certification_date,4,2)
WHERE certification_date GLOB '[0-9][0-9]/[0-9][0-9]/[0-9][0-9][0-9][0-9]';

-- ============================================================================
-- VIEWS
-- ============================================================================

DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
SELECT
  pd.url                               AS product_detail_url,
  pd.model                             AS model,
  pd.certification_date                AS certification_date,
  pd.family_sku                        AS family_sku,
  COALESCE(NULLIF(pd.transport_interface,''), 'Unknown') AS transport_interface,
  COALESCE(NULLIF(pd.transport_interface,''), 'Unknown') AS transport_if,
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

COMMIT;

-- Set schema version
PRAGMA user_version = 2000;
