//! Test utilities migrated to integration test space.
//! If reusable across integration tests, consider moving to a helper crate or keeping here with `pub` exports.

use anyhow::Result;
use std::sync::Arc;

pub struct TestDatabase {
    pub connection: matter_certis_v2_lib::infrastructure::DatabaseConnection,
}

impl TestDatabase {
    pub async fn new() -> Result<Self> {
        let db = matter_certis_v2_lib::infrastructure::DatabaseConnection::new("sqlite::memory:")
            .await?;
        db.migrate().await?;
        Ok(Self { connection: db })
    }

    #[must_use] pub fn pool(&self) -> sqlx::Pool<sqlx::Sqlite> {
        self.connection.pool().clone()
    }
}

pub struct TestContext {
    pub database: TestDatabase,
    pub integrated_repo: Arc<matter_certis_v2_lib::infrastructure::IntegratedProductRepository>,
    pub session_manager: Arc<matter_certis_v2_lib::domain::session_manager::SessionManager>,
    pub integrated_use_cases: matter_certis_v2_lib::application::IntegratedProductUseCases,
}

impl TestContext {
    pub async fn new() -> Result<Self> {
        let database = TestDatabase::new().await?;
        let pool = database.pool();

        let integrated_repo = Arc::new(
            matter_certis_v2_lib::infrastructure::IntegratedProductRepository::new(pool),
        );

        let session_manager = Arc::new(matter_certis_v2_lib::domain::session_manager::SessionManager::new());

        let integrated_use_cases =
            matter_certis_v2_lib::application::IntegratedProductUseCases::new(
                integrated_repo.clone(),
            );

        Ok(Self {
            database,
            integrated_repo,
            session_manager,
            integrated_use_cases,
        })
    }
}
