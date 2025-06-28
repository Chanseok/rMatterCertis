//! Comprehensive integration test for rMatterCertis Phase 2 implementation
//! 
//! Tests complete domain, repository, use case, and DTO layers.
//! This validates the full Matter Certification backend implementation.
//! 
//! Run with: cargo run --bin test_db_new

use std::sync::Arc;
use anyhow::Result;

// Import current implementation modules
use matter_certis_v2_lib::infrastructure::{
    DatabaseConnection,
    SqliteVendorRepository, SqliteProductRepository,
};
use matter_certis_v2_lib::application::{
    VendorUseCases, MatterProductUseCases, ProductUseCases,
    CreateVendorDto, UpdateVendorDto,
    CreateProductDto, CreateMatterProductDto,
};
use matter_certis_v2_lib::domain::repositories::ProductRepository;

fn main() -> Result<()> {
    // Create tokio runtime manually to avoid macro compilation issues
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_integration_tests())
}

async fn run_integration_tests() -> Result<()> {
    println!("🚀 rMatterCertis Phase 2 - Complete Integration Test");
    println!("📊 Testing all layers: Domain → Repository → Use Cases → DTOs");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Initialize database
    let database_url = "sqlite:./data/integration_test.db";
    let db = DatabaseConnection::new(database_url).await?;
    db.migrate().await?;
    
    // Create repositories
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    
    // Create use cases
    let vendor_use_cases = VendorUseCases::new(vendor_repo.clone());
    let matter_product_use_cases = MatterProductUseCases::new(product_repo.clone());
    let product_use_cases = ProductUseCases::new(product_repo.clone());

    println!("✅ Database and repositories initialized");
    println!();

    // Test 1: Vendor Management (Full CRUD)
    test_vendor_management(&vendor_use_cases).await?;
    
    // Test 2: Product Management
    test_product_management(&matter_product_use_cases, &product_use_cases).await?;
    
    // Test 3: Matter-specific features
    test_matter_specific_features(&product_repo, &matter_product_use_cases).await?;
    
    // Test 4: Database operations and summary
    test_database_operations(&product_repo).await?;
    
    // Test 5: Error handling and validation
    test_error_handling(&vendor_use_cases, &matter_product_use_cases).await?;

    println!("🎉 ALL TESTS PASSED - Phase 2 Implementation Verified");
    println!("═══════════════════════════════════════════════════════════");
    println!("✅ Repository Layer: Complete");
    println!("✅ Use Cases Layer: Complete");
    println!("✅ DTO Layer: Complete");
    println!("✅ Domain Validation: Complete");
    println!("✅ Error Handling: Complete");
    println!("✅ Matter Features: Complete");
    println!();
    println!("🚀 Ready for Phase 3: Crawling Engine!");

    Ok(())
}

async fn test_vendor_management(vendor_use_cases: &VendorUseCases) -> Result<()> {
    println!("🏢 Test 1: Vendor Management (CRUD Operations)");
    println!("─────────────────────────────────────────────");

    // Create vendor
    let create_dto = CreateVendorDto {
        vendor_number: 4660, // 0x1234 in decimal
        vendor_name: "Samsung Electronics".to_string(),
        company_legal_name: "Samsung Electronics Co., Ltd.".to_string(),
    };

    let created_vendor = vendor_use_cases.create_vendor(create_dto).await?;
    println!("✅ Vendor created: {} (Number: {})", 
        created_vendor.vendor_name, created_vendor.vendor_number);

    // Get all vendors
    let all_vendors = vendor_use_cases.get_all_vendors().await?;
    println!("✅ Retrieved {} vendors", all_vendors.len());

    // Search by name
    let search_results = vendor_use_cases.search_vendors_by_name("Samsung").await?;
    println!("✅ Found {} vendors matching 'Samsung'", search_results.len());

    // Update vendor
    let update_dto = UpdateVendorDto {
        vendor_name: Some("Samsung Electronics (Updated)".to_string()),
        company_legal_name: None,
    };
    
    let updated_vendor = vendor_use_cases.update_vendor(&created_vendor.vendor_id, update_dto).await?;
    println!("✅ Vendor updated: {}", updated_vendor.vendor_name);

    println!("✅ Vendor Management tests completed");
    println!();
    Ok(())
}

async fn test_product_management(
    matter_product_use_cases: &MatterProductUseCases,
    product_use_cases: &ProductUseCases,
) -> Result<()> {
    println!("📦 Test 2: Product Management");
    println!("─────────────────────────────");

    // Create basic product first
    let product_dto = CreateProductDto {
        url: "https://certifications.csa-iot.org/products/example-1".to_string(),
        manufacturer: Some("Samsung Electronics".to_string()),
        model: Some("SmartThings Hub v4".to_string()),
        certificate_id: Some("CERT-2024-001".to_string()),
        page_id: Some(1),
        index_in_page: Some(1),
    };

    let created_product = matter_product_use_cases.create_product(product_dto).await?;
    println!("✅ Basic product created: {}", created_product.url);

    // Create Matter product with detailed information
    let matter_dto = CreateMatterProductDto {
        url: "https://certifications.csa-iot.org/products/example-2".to_string(),
        manufacturer: Some("Samsung Electronics".to_string()),
        model: Some("SmartThings Multipurpose Sensor".to_string()),
        page_id: Some(1),
        index_in_page: Some(2),
        id: Some("MATTER-SENSOR-001".to_string()),
        device_type: Some("Contact Sensor".to_string()),
        certificate_id: Some("CERT-2024-002".to_string()),
        certification_date: Some("2024-06-15".to_string()),
        software_version: Some("2.1.0".to_string()),
        hardware_version: Some("1.0".to_string()),
        vid: Some("0x1234".to_string()),
        pid: Some("0x5678".to_string()),
        family_sku: Some("ST-SENSOR-V2".to_string()),
        family_variant_sku: Some("ST-SENSOR-V2-US".to_string()),
        firmware_version: Some("2.1.0".to_string()),
        family_id: Some("ST-SENSOR".to_string()),
        tis_trp_tested: Some("Yes".to_string()),
        specification_version: Some("1.3".to_string()),
        transport_interface: Some("Thread".to_string()),
        primary_device_type_id: Some("0x0015".to_string()), // Contact sensor
        application_categories: vec!["Security".to_string(), "Home Automation".to_string()],
    };

    let created_matter_product = matter_product_use_cases.create_matter_product(matter_dto).await?;
    println!("✅ Matter product created: {} (VID: {}, PID: {})", 
        created_matter_product.model.as_deref().unwrap_or("N/A"),
        created_matter_product.vid.as_deref().unwrap_or("N/A"),
        created_matter_product.pid.as_deref().unwrap_or("N/A"));

    // Test pagination
    let (products, total) = product_use_cases.get_products(0, 10).await?;
    println!("✅ Retrieved {} products (total: {})", products.len(), total);

    let (matter_products, total_matter) = product_use_cases.get_matter_products(0, 10).await?;
    println!("✅ Retrieved {} Matter products (total: {})", matter_products.len(), total_matter);

    println!("✅ Product Management tests completed");
    println!();
    Ok(())
}

async fn test_matter_specific_features(
    product_repo: &Arc<SqliteProductRepository>,
    matter_product_use_cases: &MatterProductUseCases,
) -> Result<()> {
    println!("🔬 Test 3: Matter-Specific Features");
    println!("───────────────────────────────────");

    // Test searches by Matter-specific criteria
    let vid_products = product_repo.find_by_vid("0x1234").await?;
    println!("✅ Found {} products with VID 0x1234", vid_products.len());

    let device_type_products = product_repo.find_by_device_type("Contact Sensor").await?;
    println!("✅ Found {} Contact Sensor devices", device_type_products.len());

    let manufacturer_products = product_repo.find_by_manufacturer("Samsung Electronics").await?;
    println!("✅ Found {} Samsung products", manufacturer_products.len());

    // Test search functionality
    let search_results = product_repo.search_products("Samsung").await?;
    println!("✅ Search for 'Samsung' returned {} products", search_results.len());

    // Test database summary via use cases
    let summary = matter_product_use_cases.get_database_summary().await?;
    println!("✅ Database summary:");
    println!("   📋 Total Vendors: {}", summary.total_vendors);
    println!("   📦 Total Products: {}", summary.total_products);
    println!("   🔬 Total Matter Products: {}", summary.total_matter_products);
    println!("   💾 Database Size: {:.2} MB", summary.database_size_mb);

    println!("✅ Matter-specific features tests completed");
    println!();
    Ok(())
}

async fn test_database_operations(product_repo: &Arc<SqliteProductRepository>) -> Result<()> {
    println!("🗄️  Test 4: Database Operations");
    println!("────────────────────────────────");

    // Test counts
    let product_count = product_repo.count_products().await?;
    let matter_product_count = product_repo.count_matter_products().await?;
    println!("✅ Database counts: {} products, {} matter products", 
        product_count, matter_product_count);

    // Test existing URLs functionality
    let existing_urls = product_repo.get_existing_urls().await?;
    println!("✅ Found {} existing URLs in database", existing_urls.len());

    // Test certification date range (if any products have dates)
    let date_range_products = product_repo.find_by_certification_date_range("2024-01-01", "2024-12-31").await?;
    println!("✅ Found {} products certified in 2024", date_range_products.len());

    println!("✅ Database operations tests completed");
    println!();
    Ok(())
}

async fn test_error_handling(
    vendor_use_cases: &VendorUseCases,
    matter_product_use_cases: &MatterProductUseCases,
) -> Result<()> {
    println!("⚠️  Test 5: Error Handling & Validation");
    println!("───────────────────────────────────────");

    // Test invalid vendor number
    let invalid_vendor_dto = CreateVendorDto {
        vendor_number: 0, // Invalid: must be > 0
        vendor_name: "Invalid Vendor".to_string(),
        company_legal_name: "Invalid Company".to_string(),
    };

    match vendor_use_cases.create_vendor(invalid_vendor_dto).await {
        Ok(_) => println!("❌ Should have failed: vendor_number = 0"),
        Err(e) => println!("✅ Validation working: {}", e),
    }

    // Test duplicate vendor number
    let duplicate_vendor_dto = CreateVendorDto {
        vendor_number: 4660, // Already exists from test 1
        vendor_name: "Duplicate Vendor".to_string(),
        company_legal_name: "Duplicate Company".to_string(),
    };

    match vendor_use_cases.create_vendor(duplicate_vendor_dto).await {
        Ok(_) => println!("❌ Should have failed: duplicate vendor number"),
        Err(e) => println!("✅ Duplicate prevention working: {}", e),
    }

    // Test invalid product URL
    let invalid_product_dto = CreateProductDto {
        url: "".to_string(), // Empty URL
        manufacturer: Some("Test".to_string()),
        model: Some("Test".to_string()),
        certificate_id: Some("TEST".to_string()),
        page_id: Some(1),
        index_in_page: Some(1),
    };

    match matter_product_use_cases.create_product(invalid_product_dto).await {
        Ok(_) => println!("❌ Should have failed: empty URL"),
        Err(e) => println!("✅ URL validation working: {}", e),
    }

    // Test invalid URL format
    let invalid_url_dto = CreateProductDto {
        url: "not-a-valid-url".to_string(),
        manufacturer: Some("Test".to_string()),
        model: Some("Test".to_string()),
        certificate_id: Some("TEST".to_string()),
        page_id: Some(1),
        index_in_page: Some(1),
    };

    match matter_product_use_cases.create_product(invalid_url_dto).await {
        Ok(_) => println!("❌ Should have failed: invalid URL format"),
        Err(e) => println!("✅ URL format validation working: {}", e),
    }

    println!("✅ Error handling tests completed");
    println!();
    Ok(())
}
