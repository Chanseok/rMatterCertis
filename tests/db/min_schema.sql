-- Minimal subset schema for query tests
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS products (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL UNIQUE,
  model TEXT,
  vendor_name TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS product_details (
  url TEXT PRIMARY KEY,
  product_url TEXT,
  model TEXT,
  vendor_name TEXT,
  primary_device_type_ids TEXT,
  created_at TEXT DEFAULT (datetime('now')),
  updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS device_types (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type_id INTEGER UNIQUE,
  name TEXT,
  category TEXT
);

CREATE TABLE IF NOT EXISTS product_primary_device_types (
  product_detail_id TEXT NOT NULL,
  device_type_id INTEGER NOT NULL,
  UNIQUE(product_detail_id, device_type_id)
);

-- Simplified analytics view (or fallback table). Using a view here mapping to product_details join device_types.
DROP VIEW IF EXISTS v_product_detail_analytics;
CREATE VIEW v_product_detail_analytics AS
SELECT 
  pd.url AS product_detail_url,
  pd.model AS model,
  pd.vendor_name AS vendor_name,
  dt.name AS device_type_name,
  dt.type_id AS device_type_id,
  dt.category AS device_category,
  pd.created_at AS detail_created_at,
  pd.created_at AS certification_date,
  NULL AS transport_interface
FROM product_details pd
LEFT JOIN product_primary_device_types ppt ON ppt.product_detail_id = pd.url
LEFT JOIN device_types dt ON dt.type_id = ppt.device_type_id;
