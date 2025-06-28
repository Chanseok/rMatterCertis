//! Ultra-fast integration test with minimal dependencies
//! 
//! This is a lightweight test binary that focuses only on core functionality
//! without heavy dependencies like Tauri, Scraper, etc.
//! 
//! Run with: cargo run --bin test_db_fast

use std::sync::Arc;
use anyhow::Result;

// Only import essential modules for testing
use matter_certis_v2_lib::infrastructure::{
    DatabaseConnection,
    SqliteVendorRepository, SqliteProductRepository,
};
use matter_certis_v2_lib::application::{
    VendorUseCases, MatterProductUseCases,
    CreateVendorDto, UpdateVendorDto,
    CreateMatterProductDto,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    run_fast_tests().await
}

async fn run_fast_tests() -> Result<()> {
    println!("⚡ rMatterCertis Fast Integration Test");
    println!("🎯 Core functionality only - optimized for speed");
    println!("════════════════════════════════════════════════");
    println!();

    // Use in-memory database for speed
    let database_url = "sqlite::memory:";
    let db = DatabaseConnection::new(database_url).await?;
    db.migrate().await?;
    
    // Create repositories
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    
    // Create use cases
    let vendor_use_cases = VendorUseCases::new(vendor_repo.clone());
    let matter_product_use_cases = MatterProductUseCases::new(product_repo.clone());

    println!("✅ In-memory database initialized");
    println!();

    // Fast Test 1: Basic Vendor CRUD
    fast_vendor_test(&vendor_use_cases).await?;
    
    // Fast Test 2: Basic Product Management
    fast_product_test(&matter_product_use_cases).await?;
    
    // Fast Test 3: Core validation
    fast_validation_test(&vendor_use_cases).await?;

    println!("🎉 FAST TESTS COMPLETED");
    println!("⚡ Core functionality verified in minimal time");
    println!();

    Ok(())
}

async fn fast_vendor_test(vendor_use_cases: &VendorUseCases) -> Result<()> {
    println!("🧪 Fast Vendor Test");
    
    // Create vendor
    let create_dto = CreateVendorDto {
        name: "FastTest Corp".to_string(),
        base_url: "https://fasttest.com".to_string(),
        description: Some("Test vendor for speed".to_string()),
    };
    
    let vendor = vendor_use_cases.create_vendor(create_dto).await?;
    println!("  ✅ Created vendor: {}", vendor.name);
    
    // Update vendor
    let update_dto = UpdateVendorDto {
        name: Some("FastTest Corp Updated".to_string()),
        base_url: None,
        description: Some("Updated for speed test".to_string()),
    };
    
    let updated = vendor_use_cases.update_vendor(vendor.id, update_dto).await?;
    println!("  ✅ Updated vendor: {}", updated.name);
    
    // Get vendor
    let fetched = vendor_use_cases.get_vendor(vendor.id).await?;
    println!("  ✅ Fetched vendor: {}", fetched.name);
    
    // List vendors
    let vendors = vendor_use_cases.list_vendors().await?;
    println!("  ✅ Listed {} vendors", vendors.len());
    
    println!();
    Ok(())
}

async fn fast_product_test(matter_use_cases: &MatterProductUseCases) -> Result<()> {
    println!("🧪 Fast Product Test");
    
    // Create matter product
    let create_dto = CreateMatterProductDto {
        vendor_id: 1, // Assuming vendor from previous test
        name: "Fast Test Product".to_string(),
        matter_type: "electronics".to_string(),
        price: Some(99.99),
        description: Some("Fast test product".to_string()),
        matter_data: serde_json::json!({
            "category": "test",
            "features": ["fast", "test"]
        }),
    };
    
    let product = matter_use_cases.create_matter_product(create_dto).await?;
    println!("  ✅ Created matter product: {}", product.name);
    
    // Get product
    let fetched = matter_use_cases.get_matter_product(product.id).await?;
    println!("  ✅ Fetched product: {}", fetched.name);
    
    // List products
    let products = matter_use_cases.list_matter_products().await?;
    println!("  ✅ Listed {} products", products.len());
    
    println!();
    Ok(())
}

async fn fast_validation_test(vendor_use_cases: &VendorUseCases) -> Result<()> {
    println!("🧪 Fast Validation Test");
    
    // Test empty name validation
    let invalid_dto = CreateVendorDto {
        name: "".to_string(),
        base_url: "https://test.com".to_string(),
        description: None,
    };
    
    match vendor_use_cases.create_vendor(invalid_dto).await {
        Err(e) => println!("  ✅ Validation error caught: {}", e),
        Ok(_) => println!("  ❌ Validation should have failed"),
    }
    
    // Test invalid URL validation
    let invalid_url_dto = CreateVendorDto {
        name: "Test".to_string(),
        base_url: "not-a-url".to_string(),
        description: None,
    };
    
    match vendor_use_cases.create_vendor(invalid_url_dto).await {
        Err(e) => println!("  ✅ URL validation error caught: {}", e),
        Ok(_) => println!("  ❌ URL validation should have failed"),
    }
    
    println!();
    Ok(())
}
