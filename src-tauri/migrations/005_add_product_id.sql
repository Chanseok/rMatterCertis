-- Add id column to products table for generated unique identifiers
-- Format: "p" + 4-digit page_id + "i" + 2-digit index_in_page

ALTER TABLE products ADD COLUMN id TEXT;

-- Generate IDs for existing products
UPDATE products 
SET id = printf('p%04di%02d', 
    COALESCE(page_id, 0), 
    COALESCE(index_in_page, 0)
) 
WHERE id IS NULL;

-- Create index for better performance
CREATE INDEX IF NOT EXISTS idx_products_id ON products(id);
