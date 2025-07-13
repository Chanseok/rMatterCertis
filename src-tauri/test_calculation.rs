// Simple calculation test for pageId and indexInPage logic
fn main() {
    // User's scenario parameters
    let total_pages = 482u32;
    let items_per_page = 12u32;  
    let items_on_last_page = 2u32;
    let target_page_size = 12u32;
    
    // Calculate where 116th product (0-based: 115) would be on website
    let total_products = (total_pages - 1) * items_per_page + items_on_last_page;
    println!("Total products on site: {}", total_products);
    
    // 116th product in our DB means products 0-115 are stored
    // This means we've stored the oldest 116 products
    let stored_products = 116u32;
    
    // The 116th product (index 115 in 0-based) would be from which website page?
    // Our products 0-115 correspond to the oldest 116 products on the website
    // The oldest products are on the highest page numbers
    
    // Products are numbered from newest (0) to oldest (total_products-1)
    // Our product 115 (116th product) corresponds to website product index: total_products - 116
    let website_product_index = total_products - stored_products; // This should be 4698 for our example
    println!("Website product index for our 116th product: {}", website_product_index);
    
    // Convert website product index to page and item position
    let website_page = website_product_index / items_per_page + 1; // 1-based page
    let website_item_index = website_product_index % items_per_page + 1; // 1-based item
    println!("Website location: page {}, item {}", website_page, website_item_index);
    
    // Now apply the current calculate_page_index logic
    let current_page = website_page;
    let index_on_page = website_item_index;
    
    // Current implementation (fixed)
    let index_from_newest = (current_page - 1) * items_per_page + (index_on_page - 1);
    let total_index = total_products - 1 - index_from_newest;
    let page_id = total_index / target_page_size;
    let index_in_page = total_index % target_page_size;
    
    println!("Current implementation result:");
    println!("  index_from_newest: {}", index_from_newest);
    println!("  total_index: {}", total_index);
    println!("  pageId: {}, indexInPage: {}", page_id, index_in_page);
    
    // Expected result according to user: pageId=9, indexInPage=7
    println!("Expected: pageId=9, indexInPage=7");
    
    // Check if our direct calculation matches
    let direct_page_id = 115 / 12;
    let direct_index_in_page = 115 % 12;
    println!("Direct calculation: pageId={}, indexInPage={}", direct_page_id, direct_index_in_page);
}
