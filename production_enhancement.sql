-- Production Database Enhancement Script
-- Adds new features from integrated schema to existing production database
-- Preserves all existing 5,582 products and their details

BEGIN TRANSACTION;

-- Add missing audit fields to products table
ALTER TABLE products ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE products ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP;

-- Update existing records with current timestamp
UPDATE products SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE created_at IS NULL;

-- Add missing audit fields to product_details table  
ALTER TABLE product_details ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE product_details ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP;

-- Update existing records with current timestamp
UPDATE product_details SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE created_at IS NULL;

-- Add new enhanced fields to product_details
ALTER TABLE product_details ADD COLUMN description TEXT;
ALTER TABLE product_details ADD COLUMN compliance_document_url TEXT;
ALTER TABLE product_details ADD COLUMN program_type TEXT DEFAULT 'Matter';

-- Update existing records with Matter program type
UPDATE product_details SET program_type = 'Matter' WHERE program_type IS NULL;

-- Add missing audit fields to vendors table
ALTER TABLE vendors ADD COLUMN vendor_number INTEGER;
ALTER TABLE vendors ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE vendors ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP;

-- Update existing records
UPDATE vendors SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE created_at IS NULL;

-- Create crawling_results table for session tracking
CREATE TABLE IF NOT EXISTS crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,                         -- 'Completed', 'Failed', 'Stopped'
    stage TEXT NOT NULL,                          -- 'ProductList', 'ProductDetails', 'Completed'
    total_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    details_fetched INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    execution_time_seconds INTEGER,
    config_snapshot TEXT,                         -- JSON configuration snapshot
    error_details TEXT,                          -- Detailed error information
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add new performance indexes
CREATE INDEX IF NOT EXISTS idx_products_created_at ON products (created_at);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specificationVersion);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_name ON vendors (vendorName);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_number ON vendors (vendor_number);
CREATE INDEX IF NOT EXISTS idx_crawling_results_status ON crawling_results (status);
CREATE INDEX IF NOT EXISTS idx_crawling_results_started_at ON crawling_results (started_at);
CREATE INDEX IF NOT EXISTS idx_crawling_results_stage ON crawling_results (stage);

-- Create update triggers for audit timestamps
CREATE TRIGGER IF NOT EXISTS products_updated_at
    AFTER UPDATE ON products
    FOR EACH ROW
BEGIN
    UPDATE products SET updated_at = CURRENT_TIMESTAMP WHERE url = NEW.url;
END;

CREATE TRIGGER IF NOT EXISTS product_details_updated_at
    AFTER UPDATE ON product_details  
    FOR EACH ROW
BEGIN
    UPDATE product_details SET updated_at = CURRENT_TIMESTAMP WHERE url = NEW.url;
END;

CREATE TRIGGER IF NOT EXISTS vendors_updated_at
    AFTER UPDATE ON vendors
    FOR EACH ROW  
BEGIN
    UPDATE vendors SET updated_at = CURRENT_TIMESTAMP WHERE vendorId = NEW.vendorId;
END;

-- Create legacy compatibility view
CREATE VIEW IF NOT EXISTS matter_products_legacy AS
SELECT 
    pd.url,
    pd.pageId as page_id,
    pd.indexInPage as index_in_page,
    pd.id,
    pd.manufacturer,
    pd.model,
    pd.deviceType as device_type,
    pd.certificationId as certificate_id,
    pd.certificationDate as certification_date,
    pd.softwareVersion as software_version,
    pd.hardwareVersion as hardware_version,
    pd.vid,
    pd.pid,
    pd.familySku as family_sku,
    pd.familyVariantSku as family_variant_sku,
    pd.firmwareVersion as firmware_version,
    pd.familyId as family_id,
    pd.tisTrpTested as tis_trp_tested,
    pd.specificationVersion as specification_version,
    pd.transportInterface as transport_interface,
    pd.primaryDeviceTypeId as primary_device_type_id,
    pd.applicationCategories as application_categories,
    pd.created_at,
    pd.updated_at
FROM product_details pd
INNER JOIN products p ON pd.url = p.url
WHERE pd.program_type = 'Matter' OR pd.program_type IS NULL;

COMMIT;

-- Verification query
SELECT 
    'Production Database Enhanced' as status,
    (SELECT COUNT(*) FROM products) as total_products,
    (SELECT COUNT(*) FROM product_details) as total_details,
    (SELECT COUNT(*) FROM vendors) as total_vendors,
    (SELECT COUNT(*) FROM matter_products_legacy) as legacy_view_count,
    ROUND(
        (SELECT COUNT(*) FROM product_details) * 100.0 / (SELECT COUNT(*) FROM products), 2
    ) as completion_percentage;
