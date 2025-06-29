-- Data Migration Script: Legacy to Integrated Schema
-- Migrates data from old matter_products table to new products + product_details structure

-- Step 1: Check if legacy matter_products table exists and has data
-- Step 2: Migrate data to new structure
-- Step 3: Verify migration success

BEGIN TRANSACTION;

-- Migrate from legacy matter_products to new products table
INSERT OR IGNORE INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at)
SELECT 
    url,
    manufacturer,
    model,
    certificate_id,
    page_id,
    index_in_page,
    created_at,
    updated_at
FROM matter_products
WHERE url IS NOT NULL;

-- Migrate from legacy matter_products to new product_details table  
INSERT OR IGNORE INTO product_details (
    url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id,
    certification_date, software_version, hardware_version, firmware_version, specification_version,
    vid, pid, family_sku, family_variant_sku, family_id, tis_trp_tested,
    transport_interface, primary_device_type_id, application_categories,
    description, compliance_document_url, program_type, created_at, updated_at
)
SELECT 
    url,
    page_id,
    index_in_page,
    id,
    manufacturer,
    model,
    device_type,
    certificate_id,
    certification_date,
    software_version,
    hardware_version,
    firmware_version,
    specification_version,
    -- Convert hex strings to integers for performance optimization
    CASE 
        WHEN vid IS NOT NULL AND vid LIKE '0x%' THEN 
            CAST(('0x' || TRIM(vid, '0x')) AS INTEGER)
        WHEN vid IS NOT NULL AND vid NOT LIKE '0x%' THEN 
            CAST(('0x' || vid) AS INTEGER)
        ELSE NULL
    END as vid,
    CASE 
        WHEN pid IS NOT NULL AND pid LIKE '0x%' THEN 
            CAST(('0x' || TRIM(pid, '0x')) AS INTEGER)
        WHEN pid IS NOT NULL AND pid NOT LIKE '0x%' THEN 
            CAST(('0x' || pid) AS INTEGER)
        ELSE NULL
    END as pid,
    family_sku,
    family_variant_sku,
    family_id,
    tis_trp_tested,
    transport_interface,
    primary_device_type_id,
    application_categories,
    NULL as description, -- New field, will be NULL initially
    NULL as compliance_document_url, -- New field, will be NULL initially  
    'Matter' as program_type, -- Default for migrated data
    created_at,
    updated_at
FROM matter_products
WHERE url IS NOT NULL;

-- Verify migration by checking counts
SELECT 
    'Migration Results:' as status,
    (SELECT COUNT(*) FROM matter_products) as legacy_count,
    (SELECT COUNT(*) FROM products) as new_products_count,
    (SELECT COUNT(*) FROM product_details) as new_details_count;

COMMIT;

-- Final verification query
SELECT 
    'Final Verification:' as check_type,
    COUNT(*) as total_products,
    COUNT(CASE WHEN pd.url IS NOT NULL THEN 1 END) as products_with_details,
    COUNT(CASE WHEN pd.url IS NULL THEN 1 END) as products_without_details,
    ROUND(
        (COUNT(CASE WHEN pd.url IS NOT NULL THEN 1 END) * 100.0 / COUNT(*)), 2
    ) as completion_percentage
FROM products p
LEFT JOIN product_details pd ON p.url = pd.url;
