-- Optimized Matter Products Database Schema Integration
-- Combines proven production schema with modern enhancements
-- Based on analysis of 5,582 real Matter products + new design improvements

-- ====================================================================
-- CORE PRODUCTS TABLES (Based on proven production schema)
-- ====================================================================

-- Product listing information (Stage 1: fast listing page crawling)
CREATE TABLE IF NOT EXISTS products (
    url TEXT PRIMARY KEY,                    -- Product detail page URL (proven identifier)
    manufacturer TEXT,                       -- Manufacturer name
    model TEXT,                             -- Product model name  
    certificate_id TEXT,                    -- Certificate ID (snake_case for consistency)
    page_id INTEGER,                        -- Page number where found
    index_in_page INTEGER,                  -- Position within page
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Detailed product specifications (Stage 2: detailed page crawling)
CREATE TABLE IF NOT EXISTS product_details (
    url TEXT PRIMARY KEY,                   -- Links to products.url
    page_id INTEGER,                        -- Page number (for tracking)
    index_in_page INTEGER,                  -- Position within page
    
    -- Core identification
    id TEXT,                               -- Internal Matter product ID
    manufacturer TEXT,                     -- Manufacturer name
    model TEXT,                           -- Product model name
    device_type TEXT,                     -- Device type classification
    certificate_id TEXT,                  -- Certificate ID
    
    -- Certification details  
    certification_date TEXT,              -- Certification date (flexible format)
    software_version TEXT,                -- Software version
    hardware_version TEXT,                -- Hardware version
    firmware_version TEXT,                -- Firmware version
    specification_version TEXT,           -- Matter specification version
    
    -- Technical identifiers (OPTIMIZED: INTEGER for performance)
    vid INTEGER,                          -- Vendor ID (hex converted to int)
    pid INTEGER,                          -- Product ID (hex converted to int)
    
    -- Product family information
    family_sku TEXT,                      -- Family SKU
    family_variant_sku TEXT,              -- Family variant SKU  
    family_id TEXT,                       -- Family ID
    
    -- Testing and compliance
    tis_trp_tested TEXT,                  -- TIS/TRP tested status ("Yes"/"No")
    transport_interface TEXT,             -- Transport interface(s)
    primary_device_type_id TEXT,          -- Primary device type ID
    application_categories TEXT,          -- JSON array of categories
    
    -- ENHANCEMENTS from new design
    description TEXT,                     -- Product description
    compliance_document_url TEXT,         -- Compliance document download URL
    program_type TEXT DEFAULT 'Matter',   -- Certification program type
    
    -- Audit fields
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Foreign key relationship
    FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);

-- ====================================================================
-- VENDOR MANAGEMENT (Enhanced)
-- ====================================================================

CREATE TABLE IF NOT EXISTS vendors (
    vendor_id INTEGER PRIMARY KEY AUTOINCREMENT,  -- Auto-increment for new vendors
    vendor_number INTEGER UNIQUE,                 -- Original vendor number (if exists)
    vendor_name TEXT NOT NULL,                    -- Vendor name
    company_legal_name TEXT,                      -- Legal company name
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ====================================================================
-- CRAWLING SESSION MANAGEMENT (Optimized)
-- ====================================================================

-- Final crawling results (memory-based sessions + persistent results)
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

-- ====================================================================
-- PERFORMANCE OPTIMIZATION INDEXES
-- ====================================================================

-- Products table indexes
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_products_certificate_id ON products (certificate_id);
CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id);
CREATE INDEX IF NOT EXISTS idx_products_created_at ON products (created_at);

-- Product details indexes  
CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details (manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_device_type ON product_details (device_type);
CREATE INDEX IF NOT EXISTS idx_product_details_certificate_id ON product_details (certificate_id);
CREATE INDEX IF NOT EXISTS idx_product_details_certification_date ON product_details (certification_date);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specification_version);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);

-- Vendors table indexes
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_name ON vendors (vendor_name);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_number ON vendors (vendor_number);

-- Crawling results indexes
CREATE INDEX IF NOT EXISTS idx_crawling_results_status ON crawling_results (status);
CREATE INDEX IF NOT EXISTS idx_crawling_results_started_at ON crawling_results (started_at);
CREATE INDEX IF NOT EXISTS idx_crawling_results_stage ON crawling_results (stage);

-- ====================================================================
-- DATA INTEGRITY TRIGGERS (Enhanced)
-- ====================================================================

-- Auto-update timestamp triggers
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
    UPDATE vendors SET updated_at = CURRENT_TIMESTAMP WHERE vendor_id = NEW.vendor_id;
END;

-- ====================================================================
-- MIGRATION COMPATIBILITY VIEWS (For existing frontend)
-- ====================================================================

-- Create view for backward compatibility with existing frontend
CREATE VIEW IF NOT EXISTS matter_products_legacy AS
SELECT 
    pd.url,
    pd.page_id,
    pd.index_in_page,
    pd.id,
    pd.manufacturer,
    pd.model,
    pd.device_type,
    pd.certificate_id,
    pd.certification_date,
    pd.software_version,
    pd.hardware_version,
    pd.vid,
    pd.pid,
    pd.family_sku,
    pd.family_variant_sku,
    pd.firmware_version,
    pd.family_id,
    pd.tis_trp_tested,
    pd.specification_version,
    pd.transport_interface,
    pd.primary_device_type_id,
    pd.application_categories,
    pd.created_at,
    pd.updated_at
FROM product_details pd
INNER JOIN products p ON pd.url = p.url
WHERE pd.program_type = 'Matter' OR pd.program_type IS NULL;

-- ====================================================================
-- SCHEMA SUMMARY
-- ====================================================================

/*
INTEGRATED SCHEMA BENEFITS:

✅ PRODUCTION PROVEN:
- Based on 5,582 real Matter products
- Optimized 2-table design (products + product_details)  
- Performance-optimized INTEGER types for vid/pid
- Complete field coverage from CSA-IoT

✅ MODERN ENHANCEMENTS:
- Consistent snake_case naming
- Strong data constraints (NOT NULL, UNIQUE, FK)
- Comprehensive audit trails (created_at, updated_at)
- Enhanced fields (description, compliance_document_url)
- Automatic timestamp triggers

✅ BACKWARD COMPATIBILITY:
- Legacy view for existing frontend
- Same core structure as production database
- Smooth migration path from existing 5,582 products

✅ PERFORMANCE OPTIMIZED:
- Strategic indexing for common queries
- Memory-based session management
- Efficient data types and constraints

This schema combines the reliability of the production database
with the improvements from modern design principles.
*/
