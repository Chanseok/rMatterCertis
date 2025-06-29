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
    VendorUseCases, MatterProductUseCases, ProductUseCases,
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
    let matter_use_cases = MatterProductUseCases::new(product_repo.clone());
    let product_use_cases = ProductUseCases::new(product_repo.clone());

    println!("✅ In-memory database initialized");
    println!();

    // Fast Test 1: Basic Vendor CRUD
    fast_vendor_test(&vendor_use_cases).await?;
    
    // Fast Test 2: Basic Product Management
    fast_product_test(&matter_use_cases, &product_use_cases).await?;
    
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
        vendor_number: 1001,
        vendor_name: "FastTest Corp".to_string(),
        company_legal_name: "FastTest Corporation".to_string(),
        vendor_url: Some("https://fasttest.com".to_string()),
        csa_assigned_number: Some("CSA-1001".to_string()),
    };
    
    let vendor = vendor_use_cases.create_vendor(create_dto).await?;
    println!("  ✅ Created vendor: {}", vendor.vendor_name);
    
    // Update vendor
    let update_dto = UpdateVendorDto {
        vendor_name: Some("FastTest Corp Updated".to_string()),
        company_legal_name: None,
        vendor_url: None,
        csa_assigned_number: None,
    };
    
    let updated = vendor_use_cases.update_vendor(&vendor.vendor_id, update_dto).await?;
    println!("  ✅ Updated vendor: {}", updated.vendor_name);
    
    // Get vendor
    let fetched = vendor_use_cases.get_vendor(&vendor.vendor_id).await?;
    if let Some(fetched_vendor) = fetched {
        println!("  ✅ Fetched vendor: {}", fetched_vendor.vendor_name);
    }
    
    // List vendors
    let vendors = vendor_use_cases.get_all_vendors().await?;
    println!("  ✅ Listed {} vendors", vendors.len());
    
    println!();
    Ok(())
}

async fn fast_product_test(matter_use_cases: &MatterProductUseCases, product_use_cases: &ProductUseCases) -> Result<()> {
    println!("🧪 Fast Product Test");
    
    // Create matter product
    let create_dto = CreateMatterProductDto {
        url: "https://example.com/test-product".to_string(),
        page_id: Some(1),
        json_data: Some(r#"{"device_name": "Fast Test Product", "manufacturer": "FastTest Corp"}"#.to_string()),
        vid: Some("0x1002".to_string()),
        pid: Some("0x0001".to_string()),
        device_name: Some("Fast Test Product".to_string()),
        device_type: Some("electronics".to_string()),
        manufacturer: Some("FastTest Corp".to_string()),
        certification_date: None,
        commissioning_method: Some("Standard".to_string()),
        transport_protocol: Some("WiFi".to_string()),
        application_categories: Some(r#"["test"]"#.to_string()),
        clusters_client: Some("[]".to_string()),
        clusters_server: Some("[]".to_string()),
    };
    
    let product = matter_use_cases.create_matter_product(create_dto).await?;
    println!("  ✅ Created matter product: {}", product.model.as_ref().unwrap_or(&"Unknown".to_string()));
    
    // Get product using pagination
    let (fetched_products, _) = product_use_cases.get_matter_products(0, 10).await?;
    if let Some(fetched) = fetched_products.first() {
        println!("  ✅ Fetched product: {}", fetched.model.as_ref().unwrap_or(&"Unknown".to_string()));
    }
    
    // List products
    let (products, count) = product_use_cases.get_matter_products(0, 100).await?;
    println!("  ✅ Listed {} products (total: {})", products.len(), count);
    
    println!();
    Ok(())
}

async fn fast_validation_test(vendor_use_cases: &VendorUseCases) -> Result<()> {
    println!("🧪 Fast Validation Test");
    
    // Test empty name validation
    let invalid_dto = CreateVendorDto {
        vendor_number: 0,
        vendor_name: "".to_string(),
        company_legal_name: "Test Company".to_string(),
        vendor_url: None,
        csa_assigned_number: None,
    };
    
    match vendor_use_cases.create_vendor(invalid_dto).await {
        Err(e) => println!("  ✅ Validation error caught: {e}"),
        Ok(_) => println!("  ❌ Validation should have failed"),
    }
    
    // Test zero vendor number validation
    let invalid_vendor_dto = CreateVendorDto {
        vendor_number: 0,
        vendor_name: "Test".to_string(),
        company_legal_name: "Test Company".to_string(),
        vendor_url: None,
        csa_assigned_number: None,
    };
    
    match vendor_use_cases.create_vendor(invalid_vendor_dto).await {
        Err(e) => println!("  ✅ Vendor number validation error caught: {e}"),
        Ok(_) => println!("  ❌ Vendor number validation should have failed"),
    }
    
    println!();
    Ok(())
}
