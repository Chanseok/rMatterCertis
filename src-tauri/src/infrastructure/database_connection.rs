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
        // Create tables manually for now
        let create_vendors_sql = r#"
            CREATE TABLE IF NOT EXISTS vendors (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                crawling_config TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                last_crawled_at DATETIME,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        let create_products_sql = r#"
            CREATE TABLE IF NOT EXISTS products (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL,
                currency TEXT NOT NULL DEFAULT 'USD',
                description TEXT,
                image_url TEXT,
                product_url TEXT NOT NULL,
                vendor_id TEXT NOT NULL,
                category TEXT,
                in_stock BOOLEAN NOT NULL DEFAULT 1,
                collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (vendor_id) REFERENCES vendors (id) ON DELETE CASCADE
            )
        "#;

        let create_sessions_sql = r#"
            CREATE TABLE IF NOT EXISTS crawling_sessions (
                id TEXT PRIMARY KEY,
                vendor_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                total_pages INTEGER,
                processed_pages INTEGER NOT NULL DEFAULT 0,
                products_found INTEGER NOT NULL DEFAULT 0,
                errors_count INTEGER NOT NULL DEFAULT 0,
                started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at DATETIME,
                FOREIGN KEY (vendor_id) REFERENCES vendors (id) ON DELETE CASCADE
            )
        "#;

        let create_indexes_sql = r#"
            CREATE INDEX IF NOT EXISTS idx_products_vendor_id ON products (vendor_id);
            CREATE INDEX IF NOT EXISTS idx_products_collected_at ON products (collected_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_vendor_id ON crawling_sessions (vendor_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_status ON crawling_sessions (status);
        "#;

        sqlx::query(create_vendors_sql).execute(&self.pool).await?;
        sqlx::query(create_products_sql).execute(&self.pool).await?;
        sqlx::query(create_sessions_sql).execute(&self.pool).await?;
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
        println!("🔗 Database URL: {}", database_url);

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

        // 테이블이 생성되었는지 확인
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='vendors'")
            .fetch_optional(db.pool())
            .await?;
        
        assert!(result.is_some());
        println!("✅ Database migration test passed!");
        Ok(())
    }
}
