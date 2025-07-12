// Database connection and pool management
// This module handles SQLite database connections using sqlx

#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use anyhow::Result;
use std::path::Path;

#[derive(Clone)]
pub struct DatabaseConnection {
    pool: SqlitePool,
}

impl DatabaseConnection {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Create database file directory if it doesn't exist
        let db_path = if database_url.starts_with("sqlite://") {
            database_url.trim_start_matches("sqlite://")
        } else if database_url.starts_with("sqlite:") {
            database_url.trim_start_matches("sqlite:")
        } else {
            database_url
        };

        if let Some(parent) = Path::new(db_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Ensure the database file exists by creating it if necessary
        if !Path::new(db_path).exists() {
            if let Some(parent) = Path::new(db_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::File::create(db_path)?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<()> {
        use std::fs;
        use tracing::{info, warn};
        
        // Enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON").execute(&self.pool).await?;
        
        // Load and run the integrated schema SQL (003_integrated_schema.sql)
        info!("📦 Applying integrated schema migration...");
        let schema_path = std::path::Path::new("migrations/003_integrated_schema.sql");
        
        if schema_path.exists() {
            let schema_sql = fs::read_to_string(schema_path)?;
            sqlx::query(&schema_sql).execute(&self.pool).await?;
            info!("✅ Applied integrated schema successfully");
        } else {
            // Fallback to embedded schema if file doesn't exist
            warn!("⚠️ Schema file not found, using embedded schema");
            
            // Read schema from embedded file or resources
            let schema_sql = include_str!("../../migrations/003_integrated_schema.sql");
            sqlx::query(schema_sql).execute(&self.pool).await?;
            info!("✅ Applied embedded integrated schema");
        }
        
        // Check if we need to migrate legacy data
        let has_legacy_data = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='matter_products'"
        )
        .fetch_one(&self.pool)
        .await?;
        
        if has_legacy_data > 0 {
            // Check if there's data to migrate
            let legacy_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM matter_products"
            )
            .fetch_one(&self.pool)
            .await?;
            
            if legacy_count > 0 {
                info!("🔄 Found {} legacy records to migrate", legacy_count);
                
                // Apply data migration script
                let migration_path = std::path::Path::new("migrations/004_migrate_legacy_data.sql");
                
                if migration_path.exists() {
                    let migration_sql = fs::read_to_string(migration_path)?;
                    sqlx::query(&migration_sql).execute(&self.pool).await?;
                    info!("✅ Migrated legacy data successfully");
                } else {
                    // Fallback to embedded migration script
                    let migration_sql = include_str!("../../migrations/004_migrate_legacy_data.sql");
                    sqlx::query(migration_sql).execute(&self.pool).await?;
                    info!("✅ Migrated legacy data using embedded script");
                }
            } else {
                info!("ℹ️ No legacy data to migrate");
            }
        } else {
            info!("ℹ️ No legacy tables found, fresh installation");
        }
        
        // Report on database status
        let product_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
            .fetch_one(&self.pool)
            .await?;
            
        let details_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details")
            .fetch_one(&self.pool)
            .await?;
            
        info!("📊 Database initialized with {} products and {} detailed records", 
            product_count, details_count);
            
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_connection() -> Result<()> {
        // 임시 디렉토리 생성
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        
        println!("🔍 Testing with path: {}", db_path.display());
        
        // 디렉토리가 존재하는지 확인
        println!("📁 Directory exists: {}", temp_dir.path().exists());
        
        // SQLite URL 형식으로 변환 (절대 경로 사용)
        let database_url = format!("sqlite:{}", db_path.to_string_lossy());
        println!("🔗 Database URL: {database_url}");

        // 데이터베이스 연결 테스트
        let db = DatabaseConnection::new(&database_url).await?;
        
        // 연결 풀이 정상적으로 생성되었는지 확인
        assert!(!db.pool().is_closed());

        println!("✅ Database connection test passed with optimized build!");
        Ok(())
    }

    #[tokio::test]
    async fn test_database_migration() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_migration.db");
        let database_url = format!("sqlite:{}", db_path.display());

        let db = DatabaseConnection::new(&database_url).await?;
        
        // 마이그레이션 실행
        db.migrate().await?;

        // Matter certification 테이블들이 생성되었는지 확인
        let vendors_table = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='vendors'")
            .fetch_optional(db.pool())
            .await?;
        
        let products_table = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='products'")
            .fetch_optional(db.pool())
            .await?;
            
        let matter_products_table = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='matter_products'")
            .fetch_optional(db.pool())
            .await?;
            
        let sessions_table = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='crawling_sessions'")
            .fetch_optional(db.pool())
            .await?;
        
        assert!(vendors_table.is_some());
        assert!(products_table.is_some());
        assert!(matter_products_table.is_some());
        assert!(sessions_table.is_some());
        
        println!("✅ Matter certification database migration test passed!");
        Ok(())
    }
}
