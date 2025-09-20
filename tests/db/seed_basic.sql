-- Basic seed dataset
INSERT INTO device_types(type_id, name, category) VALUES
 (100, 'Smart Plug', 'Energy'),
 (200, 'Light Bulb', 'Lighting');

INSERT INTO product_details(url, product_url, model, vendor_name, primary_device_type_ids, created_at, updated_at) VALUES
 ('detail-1', 'prod-1', 'Model A', 'VendorX', '[100]', datetime('now','-1 day'), datetime('now')),
 ('detail-2', 'prod-2', 'Model B', 'VendorX', '[200]', datetime('now','-2 day'), datetime('now')),
 ('detail-3', 'prod-3', 'Model C', 'VendorY', '[100,200]', datetime('now','-3 day'), datetime('now'));

-- Bridge population (simulate runtime backfill logic normally)
INSERT OR IGNORE INTO product_primary_device_types(product_detail_id, device_type_id)
SELECT 'detail-1', 100 UNION ALL SELECT 'detail-2', 200 UNION ALL SELECT 'detail-3', 100 UNION ALL SELECT 'detail-3', 200;
