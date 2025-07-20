//! Simple database integration test
//! 
//! Tests core database functionality with proper DTO structures.

use std::sync::Arc;
use anyhow::Result;

use matter_certis_v2_lib::{
    infrastructure::{DatabaseConnection, SqliteVendorRepository, SqliteProductRepository},
    application::{
        VendorUseCases, MatterProductUseCases,
        CreateVendorDto, CreateMatterProductDto,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Simple Database Integration Test");
    println!("==================================");

    // Initialize database
    let db = DatabaseConnection::new("sqlite::memory:").await?;
    db.migrate().await?;

    // Create repositories
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));

    // Create use cases
    let vendor_use_cases = VendorUseCases::new(vendor_repo);
    let matter_product_use_cases = MatterProductUseCases::new(product_repo);

    // Test vendor creation
    println!("\n🏢 Testing Vendor Management");
    let vendor = vendor_use_cases.create_vendor(CreateVendorDto {
        vendor_number: 4660,
        vendor_name: "Samsung Electronics".to_string(),
        company_legal_name: "Samsung Electronics Co., Ltd.".to_string(),
        vendor_url: Some("https://www.samsung.com".to_string()),
        csa_assigned_number: Some("CSA-4660".to_string()),
    }).await?;
    println!("✅ Created vendor: {} (ID: {})", vendor.vendor_name, vendor.vendor_id);

    // Test matter product creation
    println!("\n📦 Testing Matter Product Management");
    let matter_product = matter_product_use_cases.create_matter_product(CreateMatterProductDto {
        url: "https://example.com/test-product".to_string(),
        page_id: Some(1),
        json_data: Some(r"{"device_name": "Test Device", "manufacturer": "Samsung"}".to_string()),
        vid: Some("0x1234".to_string()),
        pid: Some("0x5678".to_string()),
        device_name: Some("Test Smart Sensor".to_string()),
        device_type: Some("Sensor".to_string()),
        manufacturer: Some("Samsung Electronics".to_string()),
        certification_date: Some("2024-12-28".to_string()),
        commissioning_method: Some("Standard".to_string()),
        transport_protocol: Some("Thread".to_string()),
        application_categories: Some(r"["Security", "Home Automation"]".to_string()),
        clusters_client: Some("[]".to_string()),
        clusters_server: Some("[]".to_string()),
    }).await?;
    println!("✅ Created matter product: {} (VID: {}, PID: {})", 
        matter_product.model.as_deref().unwrap_or("N/A"),
        matter_product.vid.as_deref().unwrap_or("N/A"),
        matter_product.pid.as_deref().unwrap_or("N/A"));

    // Test search functionality
    println!("\n🔍 Testing Search Functionality");
    let vendors = vendor_use_cases.search_vendors("Samsung").await?;
    println!("✅ Found {} vendors matching 'Samsung'", vendors.len());

    println!("\n🎉 All tests passed!");
    Ok(())
}
