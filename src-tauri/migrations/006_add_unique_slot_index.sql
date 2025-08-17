-- Enforce unique logical slot (page_id, index_in_page) when both are present
-- Applies to both products and product_details to prevent slot collisions.

-- Unique slot for products when both page_id and index_in_page are NOT NULL
CREATE UNIQUE INDEX IF NOT EXISTS ux_products_slot
ON products(page_id, index_in_page)
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL;

-- Unique slot for product_details when both page_id and index_in_page are NOT NULL
CREATE UNIQUE INDEX IF NOT EXISTS ux_product_details_slot
ON product_details(page_id, index_in_page)
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL;
