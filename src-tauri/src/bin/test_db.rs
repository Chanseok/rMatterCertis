// Test CLI for database functionality
// Run with: cargo run --bin test_db

use anyhow::Result;
use sqlx::Row;
use matter_certis_v2_lib::infrastructure::database_connection::DatabaseConnection;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Testing rMatterCertis Database...");

    // 1. 데이터베이스 연결 테스트
    println!("📊 Testing database connection...");
    let database_url = "sqlite:./data/test.db";
    let db = DatabaseConnection::new(database_url).await?;
    println!("✅ Database connection successful!");

    // 2. 마이그레이션 테스트
    println!("🔄 Running migrations...");
    db.migrate().await?;
    println!("✅ Migrations completed!");

    // 3. 기본 쿼리 테스트
    println!("🔍 Testing basic queries...");
    let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
        .fetch_all(db.pool())
        .await?;
    
    println!("📋 Created tables:");
    for table in tables {
        let table_name: String = table.get("name");
        println!("  - {}", table_name);
    }

    // 4. 샘플 데이터 삽입 테스트
    println!("💾 Testing sample data insertion...");
    let vendor_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vendors (id, name, base_url, crawling_config, created_at, updated_at) 
         VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&vendor_id)
    .bind("Test Vendor")
    .bind("https://test-vendor.com")
    .bind(r#"{"max_concurrent_requests":10}"#) // JSON string
    .execute(db.pool())
    .await?;
    
    println!("✅ Sample vendor inserted with ID: {}", vendor_id);

    // 5. 데이터 조회 테스트
    println!("🔎 Testing data retrieval...");
    let vendors = sqlx::query("SELECT id, name FROM vendors")
        .fetch_all(db.pool())
        .await?;
    
    println!("📊 Found {} vendors:", vendors.len());
    for vendor in vendors {
        let id: String = vendor.get("id");
        let name: String = vendor.get("name");
        println!("  - {} ({})", name, id);
    }

    println!("🎉 All tests passed successfully!");
    Ok(())
}
