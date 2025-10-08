-- Migration 027: Remove unused columns from product_details table
-- This migration removes columns that are either 100% NULL or contain only identical values

-- Drop related indexes first
DROP INDEX IF EXISTS idx_product_details_program_type;

-- Remove columns with 100% NULL values (never used)
ALTER TABLE product_details DROP COLUMN software_version;
ALTER TABLE product_details DROP COLUMN description;
ALTER TABLE product_details DROP COLUMN compliance_document_url;

-- Remove column with 99.71% NULL and only 1 distinct value
ALTER TABLE product_details DROP COLUMN program_type;

-- Remove column where all rows have identical value "Yes" (no variation, no analytical value)
ALTER TABLE product_details DROP COLUMN tis_trp_tested;

-- Note: Keeping family_sku, family_variant_sku, family_id as they have ~85% data coverage
