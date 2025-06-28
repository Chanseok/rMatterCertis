-- Initial database schema for rMatterCertis
-- Creates core tables for vendors, products, and crawling sessions

CREATE TABLE IF NOT EXISTS vendors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    crawling_config TEXT NOT NULL, -- JSON string
    is_active BOOLEAN NOT NULL DEFAULT 1,
    last_crawled_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL,
    currency TEXT NOT NULL DEFAULT 'USD',
    description TEXT,
    image_url TEXT,
    product_url TEXT NOT NULL,
    vendor_id TEXT NOT NULL,
    category TEXT,
    in_stock BOOLEAN NOT NULL DEFAULT 1,
    collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS crawling_sessions (
    id TEXT PRIMARY KEY,
    vendor_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, completed, failed
    total_pages INTEGER,
    processed_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id) ON DELETE CASCADE
);

-- Indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_products_vendor_id ON products (vendor_id);
CREATE INDEX IF NOT EXISTS idx_products_collected_at ON products (collected_at);
CREATE INDEX IF NOT EXISTS idx_sessions_vendor_id ON crawling_sessions (vendor_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON crawling_sessions (status);
