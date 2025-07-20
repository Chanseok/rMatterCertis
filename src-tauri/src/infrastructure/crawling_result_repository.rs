//! Repository for crawling results (final outcomes only)
//! 
//! Replaces CrawlingSessionRepository with a simpler results-only approach
//! for better performance following the memory-based state management pattern.

use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use crate::domain::session_manager::{CrawlingResult, SessionStatus, CrawlingStage};

/// Repository trait for crawling results
#[async_trait]
pub trait CrawlingResultRepository {
    /// Save final crawling result to database
    async fn save_result(&self, result: &CrawlingResult) -> Result<()>;
    
    /// Get crawling result by session ID
    async fn get_result(&self, session_id: &str) -> Result<Option<CrawlingResult>>;
    
    /// Get recent crawling results
    async fn get_recent_results(&self, limit: u32) -> Result<Vec<CrawlingResult>>;
    
    /// Get crawling results by status
    async fn get_results_by_status(&self, status: SessionStatus) -> Result<Vec<CrawlingResult>>;
    
    /// Delete old results (cleanup)
    async fn delete_old_results(&self, before: DateTime<Utc>) -> Result<u32>;
    
    /// Get last successful crawling date
    async fn get_last_success_date(&self) -> Result<Option<DateTime<Utc>>>;
}

/// SQLite implementation of CrawlingResultRepository
pub struct SqliteCrawlingResultRepository {
    pool: SqlitePool,
}

impl SqliteCrawlingResultRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method to convert database row to CrawlingResult
    fn row_to_result(row: &sqlx::sqlite::SqliteRow) -> Result<CrawlingResult> {
        let started_at: String = row.try_get("started_at")?;
        let completed_at: String = row.try_get("completed_at")?;
        
        let started_at = DateTime::parse_from_rfc3339(&started_at)
            .map_err(|e| anyhow!("Failed to parse started_at: {}", e))?
            .with_timezone(&Utc);
        
        let completed_at = DateTime::parse_from_rfc3339(&completed_at)
            .map_err(|e| anyhow!("Failed to parse completed_at: {}", e))?
            .with_timezone(&Utc);

        // Parse status and stage from strings
        let status_str: String = row.try_get("status")?;
        let stage_str: String = row.try_get("stage")?;
        
        let status = match status_str.as_str() {
            "Completed" => SessionStatus::Completed,
            "Failed" => SessionStatus::Failed,
            "Stopped" => SessionStatus::Stopped,
            _ => return Err(anyhow!("Unknown status: {}", status_str)),
        };
        
        let stage = match stage_str.as_str() {
            "ProductList" => CrawlingStage::ProductList,
            "ProductDetails" => CrawlingStage::ProductDetails,
            "MatterDetails" => CrawlingStage::MatterDetails,
            _ => return Err(anyhow!("Unknown stage: {}", stage_str)),
        };

        let config_snapshot: String = row.try_get("config_snapshot")?;
        let config_snapshot = serde_json::from_str(&config_snapshot)?;

        Ok(CrawlingResult {
            session_id: row.try_get("session_id")?,
            status,
            stage,
            total_pages: row.try_get::<i64, _>("total_pages")? as u32,
            products_found: row.try_get::<i64, _>("products_found")? as u32,
            errors_count: row.try_get::<i64, _>("errors_count")? as u32,
            started_at,
            completed_at,
            execution_time_seconds: row.try_get::<i64, _>("execution_time_seconds")? as u32,
            config_snapshot,
            error_details: row.try_get("error_details")?,
        })
    }
}

#[async_trait]
impl CrawlingResultRepository for SqliteCrawlingResultRepository {
    async fn save_result(&self, result: &CrawlingResult) -> Result<()> {
        let status_str = match result.status {
            SessionStatus::Completed => "Completed",
            SessionStatus::Failed => "Failed",
            SessionStatus::Stopped => "Stopped",
            _ => return Err(anyhow!("Invalid final status: {:?}", result.status)),
        };

        let stage_str = match result.stage {
            CrawlingStage::ProductList => "ProductList",
            CrawlingStage::ProductDetails => "ProductDetails",
            CrawlingStage::MatterDetails => "MatterDetails",
        };

        let config_json = serde_json::to_string(&result.config_snapshot)?;

        sqlx::query(
            r"
            INSERT INTO crawling_results (
                session_id, status, stage, total_pages, products_found, errors_count,
                started_at, completed_at, execution_time_seconds, config_snapshot, error_details
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "
        )
        .bind(&result.session_id)
        .bind(status_str)
        .bind(stage_str)
        .bind(result.total_pages as i32)
        .bind(result.products_found as i32)
        .bind(result.errors_count as i32)
        .bind(result.started_at.to_rfc3339())
        .bind(result.completed_at.to_rfc3339())
        .bind(result.execution_time_seconds as i32)
        .bind(config_json)
        .bind(&result.error_details)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_result(&self, session_id: &str) -> Result<Option<CrawlingResult>> {
        let row = sqlx::query(
            r"
            SELECT session_id, status, stage, total_pages, products_found, errors_count,
                   started_at, completed_at, execution_time_seconds, config_snapshot, error_details
            FROM crawling_results
            WHERE session_id = ?
            "
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Self::row_to_result(&row)?))
        } else {
            Ok(None)
        }
    }

    async fn get_recent_results(&self, limit: u32) -> Result<Vec<CrawlingResult>> {
        let rows = sqlx::query(
            r"
            SELECT session_id, status, stage, total_pages, products_found, errors_count,
                   started_at, completed_at, execution_time_seconds, config_snapshot, error_details
            FROM crawling_results
            ORDER BY completed_at DESC
            LIMIT ?
            "
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::row_to_result(&row)?);
        }
        Ok(results)
    }

    async fn get_results_by_status(&self, status: SessionStatus) -> Result<Vec<CrawlingResult>> {
        let status_str = match status {
            SessionStatus::Completed => "Completed",
            SessionStatus::Failed => "Failed",
            SessionStatus::Stopped => "Stopped",
            _ => return Err(anyhow!("Invalid final status: {:?}", status)),
        };

        let rows = sqlx::query(
            r"
            SELECT session_id, status, stage, total_pages, products_found, errors_count,
                   started_at, completed_at, execution_time_seconds, config_snapshot, error_details
            FROM crawling_results
            WHERE status = ?
            ORDER BY completed_at DESC
            "
        )
        .bind(status_str)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::row_to_result(&row)?);
        }
        Ok(results)
    }

    async fn delete_old_results(&self, before: DateTime<Utc>) -> Result<u32> {
        let result = sqlx::query(
            "DELETE FROM crawling_results WHERE completed_at < ?"
        )
        .bind(before.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as u32)
    }

    async fn get_last_success_date(&self) -> Result<Option<DateTime<Utc>>> {
        let row = sqlx::query(
            "SELECT completed_at FROM crawling_results WHERE status = 'Completed' ORDER BY completed_at DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let completed_at: String = row.try_get("completed_at")?;
            let completed_at = DateTime::parse_from_rfc3339(&completed_at)
                .map_err(|e| anyhow!("Failed to parse completed_at: {}", e))?
                .with_timezone(&Utc);
            Ok(Some(completed_at))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::DatabaseConnection;
    use chrono::Utc;

    #[tokio::test]
    async fn test_crawling_result_repository() {
        let db = DatabaseConnection::new("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        
        let repo = SqliteCrawlingResultRepository::new(db.pool().clone());
        
        let result = CrawlingResult {
            session_id: "test-session".to_string(),
            status: SessionStatus::Completed,
            stage: CrawlingStage::ProductList,
            total_pages: 100,
            products_found: 50,
            errors_count: 2,
            started_at: Utc::now(),
            completed_at: Utc::now(),
            execution_time_seconds: 300,
            config_snapshot: serde_json::json!({"test": true}),
            error_details: Some("Minor errors".to_string()),
        };

        // Save result
        repo.save_result(&result).await.unwrap();

        // Get result
        let retrieved = repo.get_result("test-session").await.unwrap().unwrap();
        assert_eq!(retrieved.session_id, "test-session");
        assert_eq!(retrieved.products_found, 50);

        // Get recent results
        let recent = repo.get_recent_results(10).await.unwrap();
        assert_eq!(recent.len(), 1);

        // Get by status
        let completed = repo.get_results_by_status(SessionStatus::Completed).await.unwrap();
        assert_eq!(completed.len(), 1);
    }
}
