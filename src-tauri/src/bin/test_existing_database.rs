use crate::infrastructure::product_repository::ProductRepository;
use crate::infrastructure::product_schema_adapter::ProductSchemaAdapter;
use crate::domain::product::{Product, ProductDetail};
use anyhow::Result;
use std::sync::Arc;
use sqlx::SqlitePool;

/// Test basic database operations with existing schema
pub async fn test_existing_database() -> Result<()> {
    // Connect to existing database
    let database_url = "sqlite:///Users/chanseok/Codes/rMatterCertis/.local/dev-database.sqlite";
    let pool = sqlx::SqlitePool::connect(database_url).await?;
    let pool = Arc::new(pool);
    
    // Create repository
    let product_repo = ProductRepository::new(pool.clone());
    
    println!("🔍 Testing existing database schema...");
    
    // Test 1: Get statistics
    let (total_products, total_details, unique_manufacturers) = product_repo.get_statistics().await?;
    println!("✅ Statistics:");
    println!("   - Total products: {}", total_products);
    println!("   - Total details: {}", total_details);
    println!("   - Unique manufacturers: {}", unique_manufacturers);
    
    // Test 2: Get sample products
    println!("\n🔍 Sample products:");
    let sample_products = product_repo.get_products_paginated(1, 3).await?;
    for (i, product) in sample_products.iter().enumerate() {
        println!("   {}. {} - {} ({})", 
            i + 1, 
            product.manufacturer.as_deref().unwrap_or("Unknown"), 
            product.model.as_deref().unwrap_or("Unknown"),
            product.certificate_id.as_deref().unwrap_or("No ID")
        );
    }
    
    // Test 3: Get product with details
    if let Some(product) = sample_products.first() {
        println!("\n🔍 Product with details:");
        if let Some(product_with_details) = product_repo.get_product_with_details(&product.url).await? {
            println!("   URL: {}", product_with_details.product.url);
            if let Some(details) = &product_with_details.details {
                println!("   Device Type: {}", details.device_type.as_deref().unwrap_or("Unknown"));
                println!("   Firmware: {}", details.firmware_version.as_deref().unwrap_or("Unknown"));
                println!("   VID: {:?}, PID: {:?}", details.vid, details.pid);
                println!("   Certification Date: {}", details.certification_date.as_deref().unwrap_or("Unknown"));
            }
        }
    }
    
    // Test 4: Search products
    println!("\n🔍 Search test (Tuya products):");
    let search_criteria = crate::domain::product::ProductSearchCriteria {
        manufacturer: Some("Tuya".to_string()),
        device_type: None,
        certification_id: None,
        page: Some(1),
        limit: Some(5),
    };
    
    let search_results = product_repo.search_products(&search_criteria).await?;
    println!("   Found {} products from {} total", 
        search_results.products.len(), 
        search_results.total_count
    );
    
    for product_with_details in search_results.products.iter().take(3) {
        println!("   - {} - {}", 
            product_with_details.product.manufacturer.as_deref().unwrap_or("Unknown"),
            product_with_details.product.model.as_deref().unwrap_or("Unknown")
        );
    }
    
    // Test 5: URLs needing details
    println!("\n🔍 URLs needing detail crawling:");
    let urls_needing_details = product_repo.get_urls_needing_details(5).await?;
    println!("   {} URLs need detail crawling", urls_needing_details.len());
    for url in urls_needing_details.iter().take(3) {
        println!("   - {}", url);
    }
    
    println!("\n✅ All tests passed! Existing database schema is working correctly.");
    println!("🎯 Ready to proceed with Phase 4.2: Listing Page Crawler implementation");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    test_existing_database().await
}
