// Database connection and pool management
// This module handles SQLite database connections using sqlx

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use anyhow::Result;
use std::path::Path;

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
        // Enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON").execute(&self.pool).await?;
        
        // Create Matter certification vendors table
        let create_vendors_sql = r#"
            CREATE TABLE IF NOT EXISTS vendors (
                vendor_id TEXT PRIMARY KEY,
                vendor_number INTEGER NOT NULL,
                vendor_name TEXT NOT NULL,
                company_legal_name TEXT NOT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        // Create basic products table (Stage 1 collection result)
        let create_products_sql = r#"
            CREATE TABLE IF NOT EXISTS products (
                url TEXT PRIMARY KEY,
                manufacturer TEXT,
                model TEXT,
                certificate_id TEXT,
                page_id INTEGER,
                index_in_page INTEGER,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        // Create detailed Matter products table (Stage 2 collection result)
        let create_matter_products_sql = r#"
            CREATE TABLE IF NOT EXISTS matter_products (
                url TEXT PRIMARY KEY,
                page_id INTEGER,
                index_in_page INTEGER,
                id TEXT,
                manufacturer TEXT,
                model TEXT,
                device_type TEXT,
                certificate_id TEXT,
                certification_date TEXT,
                software_version TEXT,
                hardware_version TEXT,
                vid TEXT,
                pid TEXT,
                family_sku TEXT,
                family_variant_sku TEXT,
                firmware_version TEXT,
                family_id TEXT,
                tis_trp_tested TEXT,
                specification_version TEXT,
                transport_interface TEXT,
                primary_device_type_id TEXT,
                application_categories TEXT, -- JSON array as string
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
            )
        "#;

        // Create crawling sessions table
        let create_sessions_sql = r#"
            CREATE TABLE IF NOT EXISTS crawling_sessions (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'Idle',
                current_stage TEXT NOT NULL DEFAULT 'Idle',
                total_pages INTEGER,
                processed_pages INTEGER NOT NULL DEFAULT 0,
                products_found INTEGER NOT NULL DEFAULT 0,
                errors_count INTEGER NOT NULL DEFAULT 0,
                started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at DATETIME,
                config_snapshot TEXT -- JSON snapshot of crawler config
            )
        "#;

        // Create crawling results table for final session outcomes
        let create_crawling_results_sql = r#"
            CREATE TABLE IF NOT EXISTS crawling_results (
                session_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                stage TEXT NOT NULL,
                total_pages INTEGER NOT NULL,
                products_found INTEGER NOT NULL,
                errors_count INTEGER NOT NULL,
                started_at DATETIME NOT NULL,
                completed_at DATETIME NOT NULL,
                execution_time_seconds INTEGER NOT NULL,
                config_snapshot TEXT,
                error_details TEXT
            )
        "#;

        // Create indexes for performance
        let create_indexes_sql = r#"
            CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id);
            CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer);
            CREATE INDEX IF NOT EXISTS idx_matter_products_vid ON matter_products (vid);
            CREATE INDEX IF NOT EXISTS idx_matter_products_pid ON matter_products (pid);
            CREATE INDEX IF NOT EXISTS idx_matter_products_device_type ON matter_products (device_type);
            CREATE INDEX IF NOT EXISTS idx_matter_products_created_at ON matter_products (created_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_status ON crawling_sessions (status);
            CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON crawling_sessions (started_at);
            CREATE INDEX IF NOT EXISTS idx_vendors_vendor_name ON vendors (vendor_name);
        "#;

        // Execute all table creation and index creation
        sqlx::query(create_vendors_sql).execute(&self.pool).await?;
        sqlx::query(create_products_sql).execute(&self.pool).await?;
        sqlx::query(create_matter_products_sql).execute(&self.pool).await?;
        sqlx::query(create_sessions_sql).execute(&self.pool).await?;
        sqlx::query(create_crawling_results_sql).execute(&self.pool).await?;
        sqlx::query(create_indexes_sql).execute(&self.pool).await?;

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
