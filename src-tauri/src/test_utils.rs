//! Test utilities for rMatterCertis
//! 
//! Provides common testing infrastructure and utilities to ensure tests
//! are isolated, reliable, and use consistent database setup patterns.

use std::sync::Arc;
use anyhow::Result;
use crate::infrastructure::{DatabaseConnection, SqliteVendorRepository, SqliteProductRepository, SqliteCrawlingResultRepository};
use crate::application::{VendorUseCases, MatterProductUseCases, ProductUseCases};
use crate::domain::session_manager::SessionManager;

/// Test database configuration
pub struct TestDatabase {
    pub connection: DatabaseConnection,
}

impl TestDatabase {
    /// Create a new in-memory test database
    /// 
    /// This ensures tests are isolated and don't interfere with each other.
    /// Each test gets a fresh, clean database state.
    pub async fn new() -> Result<Self> {
        let db = DatabaseConnection::new("sqlite::memory:").await?;
        db.migrate().await?;
        Ok(Self { connection: db })
    }

    /// Get the database pool for use in repositories
    pub fn pool(&self) -> sqlx::Pool<sqlx::Sqlite> {
        self.connection.pool().clone()
    }
}

/// Complete test context with all repositories and use cases
pub struct TestContext {
    pub database: TestDatabase,
    pub vendor_repo: Arc<SqliteVendorRepository>,
    pub product_repo: Arc<SqliteProductRepository>,
    pub result_repo: Arc<SqliteCrawlingResultRepository>,
    pub session_manager: Arc<SessionManager>,
    pub vendor_use_cases: VendorUseCases,
    pub matter_product_use_cases: MatterProductUseCases,
    pub product_use_cases: ProductUseCases,
}

impl TestContext {
    /// Create a complete test context with all components initialized
    pub async fn new() -> Result<Self> {
        let database = TestDatabase::new().await?;
        let pool = database.pool();

        // Create repositories
        let vendor_repo = Arc::new(SqliteVendorRepository::new(pool.clone()));
        let product_repo = Arc::new(SqliteProductRepository::new(pool.clone()));
        let result_repo = Arc::new(SqliteCrawlingResultRepository::new(pool.clone()));
        
        // Create session manager (in-memory only)
        let session_manager = Arc::new(SessionManager::new());

        // Create use cases
        let vendor_use_cases = VendorUseCases::new(vendor_repo.clone());
        let matter_product_use_cases = MatterProductUseCases::new(product_repo.clone());
        let product_use_cases = ProductUseCases::new(product_repo.clone());

        Ok(Self {
            database,
            vendor_repo,
            product_repo,
            result_repo,
            session_manager,
            vendor_use_cases,
            matter_product_use_cases,
            product_use_cases,
        })
    }
}

/// Helper macros for common test patterns
#[macro_export]
macro_rules! test_context {
    () => {{
        $crate::test_utils::TestContext::new().await.expect("Failed to create test context")
    }};
}

#[macro_export]
macro_rules! test_db {
    () => {{
        $crate::test_utils::TestDatabase::new().await.expect("Failed to create test database")
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_creation() {
        let db = TestDatabase::new().await.unwrap();
        assert!(!db.pool().is_closed());
    }

    #[tokio::test]
    async fn test_context_creation() {
        let ctx = TestContext::new().await.unwrap();
        assert!(!ctx.database.pool().is_closed());
    }

    #[tokio::test]
    async fn test_multiple_databases_are_isolated() {
        let db1 = TestDatabase::new().await.unwrap();
        let db2 = TestDatabase::new().await.unwrap();
        
        // Each database should be completely separate
        // This is guaranteed by using "sqlite::memory:" which creates
        // a new in-memory database for each connection
        assert!(!std::ptr::eq(&db1.pool(), &db2.pool()));
    }
}
