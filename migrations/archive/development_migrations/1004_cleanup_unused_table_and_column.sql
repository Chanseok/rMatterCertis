-- Migration 1004: Remove unused table and column
-- Date: 2025-10-08
-- Description: Remove page_repair_queue table and certification_date_normalized column

-- Drop the unused page_repair_queue table
-- This table was created for page repair functionality but is not currently used
DROP TABLE IF EXISTS page_repair_queue;

-- Drop the certification_date_normalized column from product_details
-- This column was added during certification date normalization efforts
-- but is no longer needed as the original certification_date column now holds normalized values
ALTER TABLE product_details DROP COLUMN IF EXISTS certification_date_normalized;
