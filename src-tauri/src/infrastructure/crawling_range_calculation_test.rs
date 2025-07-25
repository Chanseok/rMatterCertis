// Test file for crawling range calculation logic
// This test verifies the implementation matches the prompts6 specification

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
    use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
    use crate::infrastructure::config::AppConfig;
    use sqlx::SqlitePool;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_crawling_range_calculation_prompts6_example() {
        // prompts6 example data:
        // - max_page_id = 10, max_index_in_page = 6
        // - total_pages_on_site = 481, products_on_last_page = 10
        // - crawl_page_limit = 10, products_per_page = 12
        // Expected result: pages 471 to 462
        
        // Create in-memory database for testing
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        
        // Create tables
        sqlx::query(r"
            CREATE TABLE IF NOT EXISTS products (
                url TEXT PRIMARY KEY,
                manufacturer TEXT,
                model TEXT,
                certificate_id TEXT,
                page_id INTEGER,
                index_in_page INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
        ")
        .execute(&pool)
        .await
        .unwrap();
        
        // Insert test data to simulate the prompts6 scenario
        // We need some products with max page_id=10 and max index_in_page=6
        let now = chrono::Utc::now().to_rfc3339();
        
        // Insert products from page_id 0 to 10
        for page_id in 0..=10 {
            let max_index = if page_id == 10 { 6 } else { 11 }; // Last page has index 0-6, others have 0-11
            
            for index_in_page in 0..=max_index {
                let url = format!("https://example.com/product/{}/{}", page_id, index_in_page);
                
                sqlx::query(r"
                    INSERT INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ")
                .bind(&url)
                .bind("Test Manufacturer")
                .bind("Test Model")
                .bind("CERT123")
                .bind(page_id)
                .bind(index_in_page)
                .bind(&now)
                .bind(&now)
                .execute(&pool)
                .await
                .unwrap();
            }
        }
        
        // Create repository and calculator
        let repo = IntegratedProductRepository::new(pool);
        let config = AppConfig::default();
        let calculator = CrawlingRangeCalculator::new(Arc::new(repo), config);
        
        // Test parameters from prompts6
        let total_pages_on_site = 481;
        let products_on_last_page = 10;
        
        // Calculate next crawling range
        let result = calculator.calculate_next_crawling_range(
            total_pages_on_site,
            products_on_last_page,
        ).await.unwrap();
        
        // Expected result: Some((471, 462))
        match result {
            Some((start_page, end_page)) => {
                println!("📊 Test Result:");
                println!("  Start page: {}", start_page);
                println!("  End page: {}", end_page);
                println!("  Expected: start=471, end=462");
                
                assert_eq!(start_page, 471, "Start page should be 471");
                assert_eq!(end_page, 462, "End page should be 462");
                
                println!("✅ Test passed: crawling range calculation matches prompts6 spec");
            }
            None => {
                panic!("❌ Test failed: expected Some((471, 462)), got None");
            }
        }
    }
    
    #[tokio::test]
    async fn test_step_by_step_calculation() {
        // Test the step-by-step calculation as described in prompts6
        
        // Given data from prompts6
        let max_page_id = 10i32;
        let max_index_in_page = 6i32;
        let total_pages_on_site = 481u32;
        let products_on_last_page = 10u32;
        let crawl_page_limit = 10u32;
        let products_per_page = 12u32;
        
        println!("📊 Step-by-step calculation test:");
        println!("Input data:");
        println!("  max_page_id: {}", max_page_id);
        println!("  max_index_in_page: {}", max_index_in_page);
        println!("  total_pages_on_site: {}", total_pages_on_site);
        println!("  products_on_last_page: {}", products_on_last_page);
        println!("  crawl_page_limit: {}", crawl_page_limit);
        println!("  products_per_page: {}", products_per_page);
        
        // Step 1: Calculate last saved index
        let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
        println!("\nStep 1: lastSavedIndex = ({} * {}) + {} = {}", 
                 max_page_id, products_per_page, max_index_in_page, last_saved_index);
        assert_eq!(last_saved_index, 126, "Last saved index should be 126");
        
        // Step 2: Calculate next product index
        let next_product_index = last_saved_index + 1;
        println!("Step 2: nextProductIndex = {} + 1 = {}", last_saved_index, next_product_index);
        assert_eq!(next_product_index, 127, "Next product index should be 127");
        
        // Step 3: Calculate total products
        let total_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
        println!("Step 3: totalProducts = (({} - 1) * {}) + {} = {}", 
                 total_pages_on_site, products_per_page, products_on_last_page, total_products);
        assert_eq!(total_products, 5770, "Total products should be 5770");
        
        // Step 4: Convert to forward index
        let forward_index = (total_products - 1) - next_product_index;
        println!("Step 4: forwardIndex = ({} - 1) - {} = {}", 
                 total_products, next_product_index, forward_index);
        assert_eq!(forward_index, 5642, "Forward index should be 5642");
        
        // Step 5: Calculate target page number
        let target_page_number = (forward_index / products_per_page) + 1;
        println!("Step 5: targetPageNumber = ({} / {}) + 1 = {}", 
                 forward_index, products_per_page, target_page_number);
        assert_eq!(target_page_number, 471, "Target page number should be 471");
        
        // Step 6: Apply crawl page limit
        let start_page = target_page_number;
        let end_page = if start_page >= crawl_page_limit {
            start_page - crawl_page_limit + 1
        } else {
            1
        };
        println!("Step 6: startPage = {}, endPage = {} - {} + 1 = {}", 
                 start_page, start_page, crawl_page_limit, end_page);
        
        assert_eq!(start_page, 471, "Start page should be 471");
        assert_eq!(end_page, 462, "End page should be 462");
        
        println!("\n✅ All calculation steps match prompts6 specification!");
        println!("🎯 Final result: crawl pages {} to {}", start_page, end_page);
    }
}
