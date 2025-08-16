-- MOVED from repository root
-- See docs/sql/README.md for usage details

-- Production Database Enhancement v2 Script
-- Adds new manufacturing related fields and data integrity constraints

BEGIN TRANSACTION;

-- Add certification scopes tracking
ALTER TABLE product_details ADD COLUMN certification_scope TEXT; -- 'Product', 'Firmware', 'Component'
ALTER TABLE product_details ADD COLUMN certification_scope_notes TEXT;

-- Add additional product identity fields
ALTER TABLE product_details ADD COLUMN product_aliases TEXT;     -- CSV of known aliases
ALTER TABLE product_details ADD COLUMN vendor_aliases TEXT;      -- CSV of known vendor aliases
ALTER TABLE product_details ADD COLUMN sku TEXT;                 -- Optional SKU
ALTER TABLE product_details ADD COLUMN gtin TEXT;                -- Global Trade Item Number (UPC/EAN/ISBN)

-- Manufacturer/Vendor linkage improvements
ALTER TABLE product_details ADD COLUMN vendor_number INTEGER;    -- Normalized vendor numeric ID

-- Add spec compliance enhancements
ALTER TABLE product_details ADD COLUMN supported_device_types TEXT; -- CSV of device type IDs
ALTER TABLE product_details ADD COLUMN communication_stacks TEXT;   -- CSV of supported stacks
ALTER TABLE product_details ADD COLUMN capabilities TEXT;           -- CSV of declared capabilities

-- Crawl metrics and provenance
ALTER TABLE product_details ADD COLUMN first_seen_at DATETIME;   -- First time seen in crawl
ALTER TABLE product_details ADD COLUMN last_seen_at DATETIME;    -- Last time seen in crawl
ALTER TABLE product_details ADD COLUMN source_hash TEXT;         -- Hash of source page content

-- Data integrity constraints (SQLite-friendly)
CREATE UNIQUE INDEX IF NOT EXISTS idx_product_details_unique_vid_pid ON product_details (vid, pid);
CREATE INDEX IF NOT EXISTS idx_product_details_vendor_number ON product_details (vendor_number);
CREATE INDEX IF NOT EXISTS idx_product_details_first_seen_at ON product_details (first_seen_at);
CREATE INDEX IF NOT EXISTS idx_product_details_last_seen_at ON product_details (last_seen_at);
CREATE INDEX IF NOT EXISTS idx_product_details_source_hash ON product_details (source_hash);

-- Views for reporting
CREATE VIEW IF NOT EXISTS matter_products_recent AS
SELECT 
    pd.url,
    pd.id,
    pd.manufacturer,
    pd.model,
    pd.vid,
    pd.pid,
    pd.certificationId as certification_id,
    pd.certificationDate as certification_date,
    pd.first_seen_at,
    pd.last_seen_at,
    pd.updated_at
FROM product_details pd
WHERE pd.last_seen_at >= DATE('now', '-90 days') OR pd.updated_at >= DATE('now', '-90 days');

CREATE VIEW IF NOT EXISTS matter_products_by_vendor AS
SELECT 
    pd.vendor_number,
    COUNT(*) as product_count,
    MIN(pd.certificationDate) as first_cert_date,
    MAX(pd.certificationDate) as last_cert_date
FROM product_details pd
GROUP BY pd.vendor_number
ORDER BY product_count DESC;

-- Backfill logic (best-effort)
UPDATE product_details
SET first_seen_at = COALESCE(first_seen_at, created_at),
    last_seen_at  = COALESCE(last_seen_at, updated_at)
WHERE first_seen_at IS NULL OR last_seen_at IS NULL;

COMMIT;

-- Verification query
SELECT 
    'Production Database Enhanced v2' as status,
    (SELECT COUNT(*) FROM product_details) as total_details,
    (SELECT COUNT(*) FROM matter_products_recent) as recent_count,
    (SELECT COUNT(*) FROM matter_products_by_vendor) as vendors_with_products;
