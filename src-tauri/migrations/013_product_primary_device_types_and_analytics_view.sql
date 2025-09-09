-- Migration 013: Normalize primary_device_type_ids and create analytics view
-- 1. Bridge table product_primary_device_types (PPT)
-- 2. Backfill from product_details.primary_device_type_ids (JSON array of ints as TEXT)
-- 3. Indexes for analytics
-- 4. Analytics view v_product_detail_analytics

BEGIN TRANSACTION;

CREATE TABLE IF NOT EXISTS product_primary_device_types (
    product_detail_id TEXT NOT NULL,               -- references product_details.url or id? We use url? Need stable PK.
    device_type_id INTEGER NOT NULL,
    -- We link via product_details.url because product_details.id isn't guaranteed UNIQUE/NON-NULL in schema.
    -- If later a numeric surrogate appears, migration can adapt.
    PRIMARY KEY (product_detail_id, device_type_id),
    FOREIGN KEY (product_detail_id) REFERENCES product_details(url) ON DELETE CASCADE,
    FOREIGN KEY (device_type_id) REFERENCES device_types(type_id)
);

CREATE INDEX IF NOT EXISTS idx_ppt_device_type_id ON product_primary_device_types(device_type_id);
CREATE INDEX IF NOT EXISTS idx_ppt_product_detail_id ON product_primary_device_types(product_detail_id);

-- Backfill (skip if already populated). Use json_each; guard for json validity and numeric tokens.
-- Backfill intentionally deferred to runtime after device_types seeding (avoid FK failures on fresh install).
-- Runtime code will populate product_primary_device_types if empty.

-- Recreate analytics view (drop then create for idempotent evolution)
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
  d.type_id                            AS device_type_id,
  d.name                               AS device_type_name,
  d.category                           AS device_category,
  d.introduced_in                      AS device_introduced_in,
  pd.created_at                        AS detail_created_at
FROM product_details pd
LEFT JOIN vendors v ON v.vendor_number = pd.vid
LEFT JOIN product_primary_device_types ppt ON ppt.product_detail_id = pd.url
LEFT JOIN device_types d ON d.type_id = ppt.device_type_id;

COMMIT;
