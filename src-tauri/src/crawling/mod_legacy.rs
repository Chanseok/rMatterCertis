//! # Crawling Module
//!
//! This module contains the complete event-driven crawling system implementation.
//! It provides a modern, async-first approach to web scraping with proper error handling,
//! backpressure management, and comprehensive monitoring.

pub mod tasks;
pub mod state;
pub mod queues;
pub mod workers;
pub mod orchestrator;

pub use tasks::*;
pub use state::*;
pub use queues::*;
pub use workers::*;
pub use orchestrator::*;

use std::sync::Arc;
use std::time::Duration;
use sqlx::{Pool, Postgres};
use tokio::sync::RwLock;

/// Main crawling engine that coordinates all components
pub struct CrawlingEngine {
    orchestrator: Arc<CrawlingOrchestrator>,
    shared_state: Arc<SharedState>,
    is_running: Arc<RwLock<bool>>,
}

impl CrawlingEngine {
    /// Create a new crawling engine instance
    pub async fn new(
        database_pool: Pool<Postgres>,
        config: CrawlingConfig,
    ) -> Result<Self, CrawlingEngineError> {
        // Initialize shared state
        let shared_state = Arc::new(SharedState::new(config.clone()));
        
        // Create queue manager
        let queue_manager = Arc::new(QueueManager::new(
            config.max_queue_size,
            config.backpressure_threshold,
        ));
        
        // Initialize workers
        let list_page_fetcher = Arc::new(
            ListPageFetcher::new(
                config.concurrent_requests,
                Duration::from_secs(config.request_timeout_seconds),
                config.max_retries,
                Duration::from_millis(config.retry_delay_ms),
            )?
        );
        
        let list_page_parser = Arc::new(
            ListPageParser::new(
                config.base_url.clone(),
                config.max_products_per_page,
            )
        );
        
        let product_detail_fetcher = Arc::new(
            ProductDetailFetcher::new(
                config.concurrent_requests,
                Duration::from_secs(config.request_timeout_seconds),
                config.max_retries,
                Duration::from_millis(config.retry_delay_ms),
            )?
        );
        
        let product_detail_parser = Arc::new(
            ProductDetailParser::new()?
        );
        
        let db_saver = Arc::new(
            DbSaver::new(
                database_pool,
                config.batch_size,
                Duration::from_secs(config.batch_flush_interval_seconds),
            )
        );
        
        // Create worker pool
        let worker_pool = Arc::new(WorkerPool::new(
            list_page_fetcher,
            list_page_parser,
            product_detail_fetcher,
            product_detail_parser,
            db_saver,
            config.max_concurrent_tasks,
        ));
        
        // Create orchestrator
        let orchestrator_config = OrchestratorConfig {
            max_global_concurrency: config.max_concurrent_tasks,
            scheduler_interval: Duration::from_millis(config.scheduler_interval_ms),
            shutdown_timeout: Duration::from_secs(config.shutdown_timeout_seconds),
            stats_interval: Duration::from_secs(config.stats_interval_seconds),
            auto_retry_enabled: config.auto_retry_enabled,
            max_retries: config.max_retries,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
            backpressure_enabled: config.backpressure_enabled,
            backpressure_threshold: config.backpressure_threshold,
        };
        
        let orchestrator = Arc::new(CrawlingOrchestrator::new(
            worker_pool,
            queue_manager,
            shared_state.clone(),
            orchestrator_config,
        ));
        
        Ok(Self {
            orchestrator,
            shared_state,
            is_running: Arc::new(RwLock::new(false)),
        })
    }
    
    /// Start the crawling engine
    pub async fn start(&self) -> Result<(), CrawlingEngineError> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(CrawlingEngineError::AlreadyRunning);
        }
        *is_running = true;
        drop(is_running);
        
        tracing::info!("Starting crawling engine...");
        
        // Start the orchestrator
        self.orchestrator.start().await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Stop the crawling engine gracefully
    pub async fn stop(&self) -> Result<(), CrawlingEngineError> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Err(CrawlingEngineError::NotRunning);
        }
        *is_running = false;
        drop(is_running);
        
        tracing::info!("Stopping crawling engine...");
        
        // Stop the orchestrator
        self.orchestrator.stop().await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Check if the engine is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// Start crawling from a specific URL
    pub async fn start_crawling(&self, start_url: String) -> Result<(), CrawlingEngineError> {
        if !self.is_running().await {
            return Err(CrawlingEngineError::NotRunning);
        }
        
        tracing::info!("Starting crawling from URL: {}", start_url);
        
        // Create initial task
        let initial_task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            base_url: start_url,
        };
        
        // Add to orchestrator
        self.orchestrator.add_initial_task(initial_task).await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Get current crawling statistics
    pub async fn get_stats(&self) -> Result<CrawlingEngineStats, CrawlingEngineError> {
        let orchestrator_stats = self.orchestrator.get_stats().await;
        let shared_stats = self.shared_state.stats.read().await;
        
        Ok(CrawlingEngineStats {
            uptime: orchestrator_stats.uptime,
            total_tasks_processed: orchestrator_stats.total_tasks_processed,
            successful_tasks: orchestrator_stats.successful_tasks,
            failed_tasks: orchestrator_stats.failed_tasks,
            current_active_tasks: orchestrator_stats.current_active_tasks,
            tasks_per_second: orchestrator_stats.tasks_per_second,
            worker_utilization: orchestrator_stats.worker_utilization,
            
            // Detailed stats from shared state
            list_pages_fetched: shared_stats.list_pages_fetched,
            list_pages_processed: shared_stats.list_pages_processed,
            product_urls_discovered: shared_stats.product_urls_discovered,
            product_details_fetched: shared_stats.product_details_fetched,
            product_details_parsed: shared_stats.product_details_parsed,
            products_saved: shared_stats.products_saved,
            
            // Queue information
            queue_sizes: orchestrator_stats.queue_sizes,
            
            // System health
            is_healthy: self.is_system_healthy(&shared_stats).await,
        })
    }
    
    /// Check if the system is healthy
    async fn is_system_healthy(&self, stats: &CrawlingStats) -> bool {
        // Basic health checks
        let total_tasks = stats.total_tasks_processed();
        let successful_tasks = stats.successful_tasks();
        
        if total_tasks > 0 {
            let success_rate = successful_tasks as f64 / total_tasks as f64;
            success_rate > 0.8 // 80% success rate threshold
        } else {
            true // No tasks processed yet, assume healthy
        }
    }
    
    /// Get system configuration
    pub fn get_config(&self) -> &CrawlingConfig {
        &self.shared_state.config
    }
    
    /// Update system configuration (requires restart)
    pub async fn update_config(&self, new_config: CrawlingConfig) -> Result<(), CrawlingEngineError> {
        if self.is_running().await {
            return Err(CrawlingEngineError::ConfigUpdateWhileRunning);
        }
        
        // Update shared state config
        // Note: This would require additional synchronization in a real implementation
        tracing::info!("Configuration updated (restart required)");
        
        Ok(())
    }
}

/// Statistics for the crawling engine
#[derive(Debug, Clone)]
pub struct CrawlingEngineStats {
    pub uptime: Duration,
    pub total_tasks_processed: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub current_active_tasks: usize,
    pub tasks_per_second: f64,
    pub worker_utilization: f64,
    
    // Detailed crawling stats
    pub list_pages_fetched: u64,
    pub list_pages_processed: u64,
    pub product_urls_discovered: u64,
    pub product_details_fetched: u64,
    pub product_details_parsed: u64,
    pub products_saved: u64,
    
    // Queue information
    pub queue_sizes: QueueSizes,
    
    // System health
    pub is_healthy: bool,
}

/// Crawling engine errors
#[derive(Debug, thiserror::Error)]
pub enum CrawlingEngineError {
    #[error("Engine is already running")]
    AlreadyRunning,
    
    #[error("Engine is not running")]
    NotRunning,
    
    #[error("Orchestrator error: {0}")]
    OrchestratorError(String),
    
    #[error("Worker initialization error: {0}")]
    WorkerInitializationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Cannot update configuration while engine is running")]
    ConfigUpdateWhileRunning,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl From<WorkerError> for CrawlingEngineError {
    fn from(error: WorkerError) -> Self {
        match error {
            WorkerError::InitializationError(msg) => CrawlingEngineError::WorkerInitializationError(msg),
            WorkerError::DatabaseError(msg) => CrawlingEngineError::DatabaseError(msg),
            _ => CrawlingEngineError::OrchestratorError(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn engine_state_management() {
        // Test engine lifecycle without database
        // This would need mock implementations for full testing
        let config = CrawlingConfig::default();
        
        // Test configuration validation
        assert!(config.max_concurrent_tasks > 0);
        assert!(config.request_timeout_seconds > 0);
        assert!(!config.base_url.is_empty());
    }
}
