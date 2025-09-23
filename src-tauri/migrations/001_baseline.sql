-- 001_baseline.sql
-- Unified fresh-install schema equivalent to state after legacy migrations 003-025
-- Generated baseline: removes legacy bridge table; uses final analytics view; device_types.type_id TEXT
-- Intent: Applied ONLY on brand new empty databases. Existing databases continue to use incremental legacy migrations.

PRAGMA foreign_keys=ON;

-- Core tables
CREATE TABLE IF NOT EXISTS products (
    url TEXT PRIMARY KEY,
    manufacturer TEXT,
    model TEXT,
    certificate_id TEXT,
    page_id INTEGER,
    index_in_page INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

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
    application_categories TEXT,
    description TEXT,
    compliance_document_url TEXT,
    program_type TEXT DEFAULT 'Matter',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS vendors (
    vendor_id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor_number INTEGER UNIQUE,
    vendor_name TEXT NOT NULL,
    company_legal_name TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    stage TEXT NOT NULL,
    total_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    details_fetched INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    execution_time_seconds INTEGER,
    config_snapshot TEXT,
    error_details TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sync_sessions (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    coverage_text TEXT,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at DATETIME
);

CREATE TABLE IF NOT EXISTS sync_observed (
    session_id TEXT NOT NULL,
    url TEXT NOT NULL,
    page_id INTEGER,
    index_in_page INTEGER,
    PRIMARY KEY (session_id, url)
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

-- Indexes
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_products_certificate_id ON products (certificate_id);
CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id);
CREATE INDEX IF NOT EXISTS idx_products_created_at ON products (created_at);

CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details (manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_device_type ON product_details (device_type);
CREATE INDEX IF NOT EXISTS idx_product_details_certificate_id ON product_details (certificate_id);
CREATE INDEX IF NOT EXISTS idx_product_details_certification_date ON product_details (certification_date);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specification_version);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);

CREATE INDEX IF NOT EXISTS idx_vendors_vendor_name ON vendors (vendor_name);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_number ON vendors (vendor_number);

CREATE INDEX IF NOT EXISTS idx_crawling_results_status ON crawling_results (status);
CREATE INDEX IF NOT EXISTS idx_crawling_results_started_at ON crawling_results (started_at);
CREATE INDEX IF NOT EXISTS idx_crawling_results_stage ON crawling_results (stage);

CREATE INDEX IF NOT EXISTS idx_sync_observed_session_page ON sync_observed (session_id, page_id);

CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_name ON device_types(name);
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_code_hex ON device_types(code_hex);
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_type_id ON device_types(type_id);

-- Triggers (updated_at maintenance)
CREATE TRIGGER IF NOT EXISTS products_updated_at
AFTER UPDATE ON products
FOR EACH ROW BEGIN
    UPDATE products SET updated_at = CURRENT_TIMESTAMP WHERE url = NEW.url;
END;

CREATE TRIGGER IF NOT EXISTS product_details_updated_at
AFTER UPDATE ON product_details
FOR EACH ROW BEGIN
    UPDATE product_details SET updated_at = CURRENT_TIMESTAMP WHERE url = NEW.url;
END;

CREATE TRIGGER IF NOT EXISTS vendors_updated_at
AFTER UPDATE ON vendors
FOR EACH ROW BEGIN
    UPDATE vendors SET updated_at = CURRENT_TIMESTAMP WHERE vendor_id = NEW.vendor_id;
END;

CREATE TRIGGER IF NOT EXISTS device_types_updated_at
AFTER UPDATE ON device_types
FOR EACH ROW BEGIN
    UPDATE device_types SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Normalize introduced_in blanks to 'Unknown' (harmless on empty table)
UPDATE device_types SET introduced_in = 'Unknown' WHERE introduced_in IS NULL OR introduced_in = '';

-- Final analytics view (derived from migration 023 logic; no bridge dependency)
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

-- Baseline complete marker (optional no-op select)
SELECT 'Baseline schema applied' AS status;
