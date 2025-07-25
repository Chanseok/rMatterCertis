//! # Mock Database Saver for Development
//!
//! Modern Rust 2024 개발용 Mock 구현
//! - SQLX 의존성 제거
//! - 테스트 가능한 구조
//! - Clean Architecture 준수

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;

use crate::crawling::tasks::{TaskResult, TaskOutput, CrawlingTask};
use crate::crawling::state::SharedState;
use crate::crawling::workers::{Worker, WorkerError};

/// Mock Database Saver for development
#[derive(Debug, Clone)]
pub struct MockDbSaver {
    batch_size: usize,
    max_concurrency: usize,
}

impl MockDbSaver {
    /// Create new mock database saver
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            max_concurrency: 4,
        }
    }

    /// 개발 용이성을 위한 간단한 생성자
    pub fn new_simple() -> Self {
        Self {
            batch_size: 100,
            max_concurrency: 4,
        }
    }
}

#[async_trait]
impl Worker<CrawlingTask> for MockDbSaver {
    type Task = CrawlingTask;
    
    fn worker_id(&self) -> &'static str {
        "MockDbSaver"
    }

    fn worker_name(&self) -> &'static str {
        "MockDbSaver"
    }

    fn max_concurrency(&self) -> usize {
        10 // Mock implementation, no real I/O constraints
    }

    async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = Instant::now();

        match task {
            CrawlingTask::SaveProduct { task_id, product_data } => {
                // Mock save operation - just log and update stats
                tracing::info!(
                    "Mock saving product: {} ({})",
                    product_data.name,
                    product_data.product_id
                );

                // Simulate database save delay
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Update shared state statistics
                {
                    let mut stats = shared_state.stats.write().await;
                    stats.products_saved += 1;
                }

                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::SaveConfirmation {
                        product_id: product_data.product_id,
                        saved_at: chrono::Utc::now(),
                    },
                    duration: start_time.elapsed(),
                })
            }
            _ => Err(WorkerError::ValidationError(
                "MockDbSaver can only process SaveProduct tasks".to_string()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawling::tasks::TaskId;
    use crate::crawling::state::CrawlingConfig;
    use crate::domain::ProductData;

    #[tokio::test]
    async fn test_mock_db_saver() {
        let saver = MockDbSaver::new(10);
        let shared_state = Arc::new(SharedState::new(CrawlingConfig::default()));
        
        let product_data = ProductData {
            product_id: "test123".to_string(),
            name: "Test Product".to_string(),
            category: None,
            manufacturer: None,
            model: None,
            certification_number: None,
            certification_date: None,
            technical_details: std::collections::HashMap::new(),
            compliance_details: std::collections::HashMap::new(),
            confidence_score: 0.8,
            extracted_at: chrono::Utc::now(),
            source_url: crate::domain::ValidatedUrl::new("https://example.com/test".to_string()).unwrap(),
        };

        let task_product_data = crate::crawling::tasks::TaskProductData {
            product_id: "test123".to_string(),
            name: "Test Product".to_string(),
            category: Some("Electronics".to_string()),
            manufacturer: Some("Test Company".to_string()),
            model: None,
            certification_number: None,
            certification_date: None,
            details: std::collections::HashMap::new(),
            extracted_at: chrono::Utc::now(),
            source_url: "https://example.com/test".to_string(),
        };

        let task = CrawlingTask::SaveProduct {
            task_id: TaskId::new(),
            product_data: task_product_data,
        };

        let result = saver.process_task(task, shared_state).await;
        assert!(result.is_ok());

        if let Ok(TaskResult::Success { output: TaskOutput::SaveConfirmation { product_id, .. }, .. }) = result {
            assert_eq!(product_id, "test123");
        }
    }
}
