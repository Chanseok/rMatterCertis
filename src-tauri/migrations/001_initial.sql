-- Initial database schema for rMatterCertis
-- Creates core tables for vendors, products, and crawling sessions

-- Vendor table for CSA-IoT Matter certification database vendors
CREATE TABLE IF NOT EXISTS vendors (
    vendor_id TEXT PRIMARY KEY,
    vendor_number INTEGER NOT NULL UNIQUE,
    vendor_name TEXT NOT NULL,
    company_legal_name TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Product table for basic product information (Stage 1 collection)
CREATE TABLE IF NOT EXISTS products (
    url TEXT PRIMARY KEY,              -- Product detail page URL
    manufacturer TEXT,                 -- Manufacturer name
    model TEXT,                       -- Model name
    certificate_id TEXT,              -- Certificate ID
    page_id INTEGER,                  -- Collected page number
    index_in_page INTEGER,           -- Order within page
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Matter products table for complete Matter certification info (Stage 2 collection)
CREATE TABLE IF NOT EXISTS matter_products (
    url TEXT PRIMARY KEY,                    -- Product detail page URL
    page_id INTEGER,                        -- Collected page number
    index_in_page INTEGER,                 -- Order within page
    id TEXT,                               -- Matter product ID
    manufacturer TEXT,                     -- Manufacturer name
    model TEXT,                           -- Model name
    device_type TEXT,                     -- Device type
    certificate_id TEXT,                  -- Certificate ID
    certification_date TEXT,             -- Certification date
    software_version TEXT,               -- Software version
    hardware_version TEXT,               -- Hardware version
    vid TEXT,                            -- Vendor ID
    pid TEXT,                            -- Product ID
    family_sku TEXT,                     -- Family SKU
    family_variant_sku TEXT,             -- Family variant SKU
    firmware_version TEXT,               -- Firmware version
    family_id TEXT,                      -- Family ID
    tis_trp_tested TEXT,                 -- TIS/TRP tested
    specification_version TEXT,          -- Specification version
    transport_interface TEXT,            -- Transport interface
    primary_device_type_id TEXT,         -- Primary device type ID
    application_categories TEXT,         -- JSON array as TEXT
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Crawling results table for final session outcomes only
-- Replaces crawling_sessions for performance optimization
CREATE TABLE IF NOT EXISTS crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,                    -- Completed, Failed, Stopped
    stage TEXT NOT NULL,                     -- products, matter_products, details
    total_pages INTEGER NOT NULL,
    products_found INTEGER NOT NULL,
    errors_count INTEGER NOT NULL,
    started_at DATETIME NOT NULL,
    completed_at DATETIME NOT NULL,
    execution_time_seconds INTEGER NOT NULL,
    config_snapshot TEXT,                    -- JSON configuration used
    error_details TEXT                       -- Detailed error information if failed
);

-- Indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id);
CREATE INDEX IF NOT EXISTS idx_matter_products_manufacturer ON matter_products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_matter_products_device_type ON matter_products (device_type);
CREATE INDEX IF NOT EXISTS idx_matter_products_vid ON matter_products (vid);
CREATE INDEX IF NOT EXISTS idx_matter_products_certification_date ON matter_products (certification_date);
CREATE INDEX IF NOT EXISTS idx_crawling_results_status ON crawling_results (status);
CREATE INDEX IF NOT EXISTS idx_crawling_results_started_at ON crawling_results (started_at);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_number ON vendors (vendor_number);
