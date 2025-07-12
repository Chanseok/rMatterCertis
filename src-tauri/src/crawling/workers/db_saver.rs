//! # Database Saver Worker (Development Mode)
//!
//! Modern Rust 2024 + Clean Architecture 구현
//! - 개발 환경에서 SQLX 의존성 제거
//! - 테스트 가능한 Mock 구현
//! - Clean Architecture 준수

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;

use crate::crawling::tasks::{TaskResult, TaskOutput, CrawlingTask};
use crate::crawling::state::SharedState;
use crate::crawling::workers::{Worker, WorkerError};

/// Database Saver Worker (Development Mock)
#[derive(Debug, Clone)]
pub struct DbSaver {
    batch_size: usize,
    max_concurrency: usize,
}

impl DbSaver {
    /// Create new database saver
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
impl Worker<CrawlingTask> for DbSaver {
    type Task = CrawlingTask;
    
    fn worker_id(&self) -> &'static str {
        "DbSaver"
    }

    fn worker_name(&self) -> &'static str {
        "DbSaver"
    }

    fn max_concurrency(&self) -> usize {
        2 // Database I/O, more conservative
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
                    "Saving product: {} ({})",
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
                "DbSaver can only process SaveProduct tasks".to_string()
            )),
        }
    }
}
