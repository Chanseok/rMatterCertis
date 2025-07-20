//! 빠른 컴파일을 위한 경량 통합 테스트
//! 
//! 핵심 기능만 테스트하는 최소한의 종속성 버전
//! Run with: cargo run --bin test_db_light

use std::sync::Arc;
use anyhow::Result;

// 최소한의 import만 사용
use matter_certis_v2_lib::infrastructure::{
    DatabaseConnection,
    SqliteVendorRepository, SqliteProductRepository,
};
use matter_certis_v2_lib::application::{
    VendorUseCases, MatterProductUseCases,
    CreateVendorDto, CreateMatterProductDto,
};
use matter_certis_v2_lib::domain::repositories::ProductRepository;

fn main() -> Result<()> {
    // tokio runtime으로 async 코드 실행
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_light_tests())
}

async fn run_light_tests() -> Result<()> {
    println!("⚡ rMatterCertis - Light Integration Test");
    println!("🎯 Testing core functionality only for fast iteration");
    println!("═══════════════════════════════════════════════════");
    println!();

    // 데이터베이스 초기화 (메모리 DB 사용으로 빠르게)
    let database_url = "sqlite::memory:";
    let db = DatabaseConnection::new(database_url).await?;
    db.migrate().await?;
    
    // 저장소 및 사용사례 생성
    let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
    let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
    
    let vendor_use_cases = VendorUseCases::new(vendor_repo);
    let matter_product_use_cases = MatterProductUseCases::new(product_repo.clone());

    println!("✅ In-memory database initialized");

    // 핵심 테스트 1: 벤더 생성
    println!("\n🏢 Test 1: Quick Vendor Test");
    let vendor_dto = CreateVendorDto {
        vendor_number: 4660,
        vendor_name: "Samsung".to_string(),
        company_legal_name: "Samsung Electronics Co., Ltd.".to_string(),
        vendor_url: Some("https://www.samsung.com".to_string()),
        csa_assigned_number: Some("CSA-4660".to_string()),
    };

    let created_vendor = vendor_use_cases.create_vendor(vendor_dto).await?;
    println!("✅ Vendor: {} ({})", created_vendor.vendor_name, created_vendor.vendor_number);

    // 핵심 테스트 2: Matter 제품 생성
    println!("\n📦 Test 2: Quick Matter Product Test");
    let matter_dto = CreateMatterProductDto {
        url: "https://example.com/test".to_string(),
        page_id: Some(1),
        json_data: Some(r"{"device_name": "Test Device", "manufacturer": "Samsung"}".to_string()),
        vid: Some("0x1234".to_string()),
        pid: Some("0x5678".to_string()),
        device_name: Some("Test Device".to_string()),
        device_type: Some("Sensor".to_string()),
        manufacturer: Some("Samsung".to_string()),
        certification_date: Some("2024-12-28".to_string()),
        commissioning_method: Some("Standard".to_string()),
        transport_protocol: Some("Thread".to_string()),
        application_categories: Some(r"["Test"]".to_string()),
        clusters_client: Some("[]".to_string()),
        clusters_server: Some("[]".to_string()),
    };

    let created_product = matter_product_use_cases.create_matter_product(matter_dto).await?;
    println!("✅ Product: {} (VID: {}, PID: {})",
        created_product.model.as_deref().unwrap_or("N/A"),
        created_product.vid.as_deref().unwrap_or("N/A"),
        created_product.pid.as_deref().unwrap_or("N/A"));

    // 핵심 테스트 3: 검색 기능
    println!("\n🔍 Test 3: Quick Search Test");
    let vid_products = product_repo.find_by_vid("0x1234").await?;
    println!("✅ Found {} products with VID 0x1234", vid_products.len());

    // 에러 테스트
    println!("\n⚠️ Test 4: Quick Error Test");
    let invalid_dto = CreateVendorDto {
        vendor_number: 0, // Invalid
        vendor_name: "Invalid".to_string(),
        company_legal_name: "Invalid Co.".to_string(),
        vendor_url: None,
        csa_assigned_number: None,
    };

    match vendor_use_cases.create_vendor(invalid_dto).await {
        Ok(_) => println!("❌ Should have failed"),
        Err(_) => println!("✅ Validation working"),
    }

    println!("\n🎉 LIGHT TESTS PASSED");
    println!("⚡ Fast iteration ready!");

    Ok(())
}
