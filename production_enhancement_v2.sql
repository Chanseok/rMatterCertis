-- Production Database Enhancement Script (SQLite Compatible)
-- Adds new features from integrated schema to existing production database
-- Preserves all existing 5,582 products and their details

BEGIN TRANSACTION;

-- Add missing audit fields to products table (nullable)
ALTER TABLE products ADD COLUMN created_at DATETIME;
ALTER TABLE products ADD COLUMN updated_at DATETIME;

-- Set current timestamp for all existing records
UPDATE products SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP;

-- Add missing audit fields to product_details table (nullable)
ALTER TABLE product_details ADD COLUMN created_at DATETIME;
ALTER TABLE product_details ADD COLUMN updated_at DATETIME;

-- Set current timestamp for all existing records
UPDATE product_details SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP;

-- Add new enhanced fields to product_details
ALTER TABLE product_details ADD COLUMN description TEXT;
ALTER TABLE product_details ADD COLUMN compliance_document_url TEXT;
ALTER TABLE product_details ADD COLUMN program_type TEXT;

-- Set Matter as default program type for existing records
UPDATE product_details SET program_type = 'Matter';

-- Add missing audit fields to vendors table
ALTER TABLE vendors ADD COLUMN vendor_number INTEGER;
ALTER TABLE vendors ADD COLUMN created_at DATETIME;
ALTER TABLE vendors ADD COLUMN updated_at DATETIME;

-- Set current timestamp for all existing vendors
UPDATE vendors SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP;

-- Create crawling_results table for session tracking
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

COMMIT;

-- Verification query
SELECT 
    'Production Database Enhanced' as status,
    (SELECT COUNT(*) FROM products) as total_products,
    (SELECT COUNT(*) FROM product_details) as total_details,
    (SELECT COUNT(*) FROM vendors) as total_vendors,
    ROUND(
        (SELECT COUNT(*) FROM product_details) * 100.0 / (SELECT COUNT(*) FROM products), 2
    ) as completion_percentage;
