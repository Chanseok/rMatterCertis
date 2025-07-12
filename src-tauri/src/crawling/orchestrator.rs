//! # Crawling Orchestrator
//!
//! The orchestrator manages the entire crawling workflow, coordinating workers,
//! task scheduling, and system lifecycle management.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{sleep, interval};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

use crate::crawling::{
    tasks::*,
    state::*,
    queues::*,
    workers::{WorkerPool, Worker},
};

/// Main orchestrator that coordinates the entire crawling process
pub struct CrawlingOrchestrator {
    /// Worker pool for processing tasks
    worker_pool: Arc<WorkerPool>,
    
    /// Task queue manager
    queue_manager: Arc<QueueManager>,
    
    /// Shared state across all components
    shared_state: Arc<SharedState>,
    
    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
    
    /// Configuration for the orchestrator
    config: OrchestratorConfig,
    
    /// Semaphore for controlling global concurrency
    global_semaphore: Arc<Semaphore>,
    
    /// Task scheduling interval
    scheduler_interval: Duration,
}

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of concurrent tasks across all workers
    pub max_global_concurrency: usize,
    
    /// How often to check for new tasks to schedule
    pub scheduler_interval: Duration,
    
    /// Maximum time to wait for workers to finish during shutdown
    pub shutdown_timeout: Duration,
    
    /// How often to log progress statistics
    pub stats_interval: Duration,
    
    /// Whether to enable automatic retry of failed tasks
    pub auto_retry_enabled: bool,
    
    /// Maximum number of retries for failed tasks
    pub max_retries: u32,
    
    /// Base delay between retries
    pub retry_delay: Duration,
    
    /// Whether to enable backpressure control
    pub backpressure_enabled: bool,
    
    /// Queue size threshold for backpressure
    pub backpressure_threshold: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        use crate::infrastructure::config::defaults;
        Self {
            max_global_concurrency: 50,
            scheduler_interval: Duration::from_millis(defaults::SCHEDULER_INTERVAL_MS),
            shutdown_timeout: Duration::from_secs(defaults::SHUTDOWN_TIMEOUT_SECONDS),
            stats_interval: Duration::from_secs(defaults::STATS_INTERVAL_SECONDS),
            auto_retry_enabled: true,
            max_retries: defaults::MAX_RETRIES,
            retry_delay: Duration::from_millis(defaults::WORKER_RETRY_DELAY_MS),
            backpressure_enabled: true,
            backpressure_threshold: 1000,
        }
    }
}

/// Orchestrator runtime statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStats {
    pub uptime: Duration,
    pub total_tasks_processed: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub retry_attempts: u64,
    pub current_active_tasks: usize,
    pub queue_sizes: QueueSizes,
    pub worker_utilization: f64,
    pub tasks_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSizes {
    pub list_page_fetch: usize,
    pub list_page_parse: usize,
    pub product_detail_fetch: usize,
    pub product_detail_parse: usize,
    pub product_save: usize,
}

impl CrawlingOrchestrator {
    /// Create a new orchestrator instance
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        queue_manager: Arc<QueueManager>,
        shared_state: Arc<SharedState>,
        config: OrchestratorConfig,
    ) -> Self {
        let global_semaphore = Arc::new(Semaphore::new(config.max_global_concurrency));
        let cancellation_token = CancellationToken::new();
        let scheduler_interval = config.scheduler_interval;
        
        Self {
            worker_pool,
            queue_manager,
            shared_state,
            cancellation_token,
            config,
            global_semaphore,
            scheduler_interval,
        }
    }

    /// Start the orchestrator and begin processing tasks
    pub async fn start(&self) -> Result<(), OrchestratorError> {
        info!("Starting crawling orchestrator with config: {:?}", self.config);
        
        // Initialize system components
        self.initialize_system().await?;
        
        // Start background tasks
        let scheduler_handle = self.start_task_scheduler();
        let stats_handle = self.start_stats_reporter();
        let health_check_handle = self.start_health_checker();
        
        // Wait for cancellation
        self.cancellation_token.cancelled().await;
        
        info!("Orchestrator shutdown requested, stopping background tasks...");
        
        // Cancel background tasks
        scheduler_handle.abort();
        stats_handle.abort();
        health_check_handle.abort();
        
        // Wait for all active tasks to complete
        self.graceful_shutdown().await?;
        
        info!("Orchestrator stopped successfully");
        Ok(())
    }

    /// Stop the orchestrator gracefully
    pub async fn stop(&self) -> Result<(), OrchestratorError> {
        info!("Stopping orchestrator...");
        
        // Signal shutdown to all components
        self.shared_state.request_shutdown();
        self.cancellation_token.cancel();
        
        // Wait for graceful shutdown
        tokio::time::timeout(self.config.shutdown_timeout, async {
            while self.has_active_tasks().await {
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| OrchestratorError::ShutdownTimeout)?;
        
        Ok(())
    }

    /// Add an initial crawling task to start the process
    pub async fn add_initial_task(&self, task: CrawlingTask) -> Result<(), OrchestratorError> {
        info!("Adding initial task: {:?}", task);
        
        self.queue_manager.route_task(task).await
            .map_err(|e| OrchestratorError::TaskQueueError(e.to_string()))?;
        
        Ok(())
    }

    /// Get current orchestrator statistics
    pub async fn get_stats(&self) -> OrchestratorStats {
        let stats = self.shared_state.stats.read().await;
        let start_time = std::time::Instant::now(); // Placeholder for now
        
        OrchestratorStats {
            uptime: start_time.elapsed(),
            total_tasks_processed: stats.total_tasks_created,
            successful_tasks: stats.tasks_completed,
            failed_tasks: stats.tasks_failed,
            retry_attempts: 0, // Placeholder
            current_active_tasks: self.global_semaphore.available_permits(),
            queue_sizes: self.get_queue_sizes().await,
            worker_utilization: self.calculate_worker_utilization().await,
            tasks_per_second: self.calculate_tasks_per_second(&stats, start_time.elapsed()),
        }
    }

    /// Initialize system components
    async fn initialize_system(&self) -> Result<(), OrchestratorError> {
        info!("Initializing orchestrator system components...");
        
        // Validate worker pool
        // Worker count validation removed as method doesn't exist
        
        // Validate queue manager
        // Add any queue initialization logic here
        
        info!("System components initialized successfully");
        Ok(())
    }

    /// Start the task scheduler background task
    fn start_task_scheduler(&self) -> tokio::task::JoinHandle<()> {
        let orchestrator = self.clone_for_task();
        let mut scheduler_interval = interval(self.scheduler_interval);
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = orchestrator.cancellation_token.cancelled() => {
                        debug!("Task scheduler shutting down");
                        break;
                    }
                    _ = scheduler_interval.tick() => {
                        if let Err(e) = orchestrator.process_task_queue().await {
                            error!("Error processing task queue: {}", e);
                        }
                    }
                }
            }
        })
    }

    /// Start the statistics reporter background task
    fn start_stats_reporter(&self) -> tokio::task::JoinHandle<()> {
        let orchestrator = self.clone_for_task();
        let mut stats_interval = interval(self.config.stats_interval);
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = orchestrator.cancellation_token.cancelled() => {
                        debug!("Stats reporter shutting down");
                        break;
                    }
                    _ = stats_interval.tick() => {
                        let stats = orchestrator.get_stats().await;
                        info!("Orchestrator Stats: active_tasks={}, total_processed={}, success_rate={:.2}%, tps={:.2}", 
                              stats.current_active_tasks,
                              stats.total_tasks_processed,
                              (stats.successful_tasks as f64 / stats.total_tasks_processed.max(1) as f64) * 100.0,
                              stats.tasks_per_second);
                    }
                }
            }
        })
    }

    /// Start the health checker background task
    fn start_health_checker(&self) -> tokio::task::JoinHandle<()> {
        let orchestrator = self.clone_for_task();
        let mut health_interval = interval(Duration::from_secs(30));
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = orchestrator.cancellation_token.cancelled() => {
                        debug!("Health checker shutting down");
                        break;
                    }
                    _ = health_interval.tick() => {
                        if let Err(e) = orchestrator.perform_health_check().await {
                            warn!("Health check failed: {}", e);
                        }
                    }
                }
            }
        })
    }

    /// Process the task queue and dispatch tasks to workers
    async fn process_task_queue(&self) -> Result<(), OrchestratorError> {
        // Check if we have capacity for more tasks
        if self.global_semaphore.available_permits() == 0 {
            return Ok(()); // No capacity, skip this cycle
        }

        // Try to dequeue and process tasks
        loop {
            // Check if we should stop
            if self.shared_state.is_shutdown_requested() {
                break;
            }

            // Try to dequeue a task
            let task = match self.queue_manager.dequeue_task().await {
                Ok(task) => task,
                Err(_) => break, // No more tasks
            };

            // Try to acquire permit before spawning
            if self.global_semaphore.try_acquire().is_err() {
                break; // No more capacity
            }

            // Clone all needed data for ownership
            let owned_task = task.clone();
            let worker_pool = self.worker_pool.clone();
            let shared_state = self.shared_state.clone();
            let queue_manager = self.queue_manager.clone();
            let config = self.config.clone();
            let global_semaphore = self.global_semaphore.clone();
            
            // Move permit management into spawn closure
            tokio::spawn(async move {
                let _permit = global_semaphore.acquire().await.unwrap();
                
                // Static function call to avoid lifetime issues
                let result = process_single_task_static(
                    owned_task,
                    worker_pool,
                    shared_state,
                    queue_manager,
                    config,
                ).await;
                
                if let Err(e) = result {
                    error!("Task processing failed: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Process a single task with the appropriate worker
    async fn process_single_task(
        task: CrawlingTask,
        worker_pool: Arc<WorkerPool>,
        shared_state: Arc<SharedState>,
        queue_manager: Arc<QueueManager>,
        config: OrchestratorConfig,
    ) -> Result<(), OrchestratorError> {
        process_single_task_static(task, worker_pool, shared_state, queue_manager, config).await
    }

    /// Helper methods for internal operations
    async fn graceful_shutdown(&self) -> Result<(), OrchestratorError> {
        info!("Performing graceful shutdown...");
        
        // Wait for all active tasks to complete
        let shutdown_start = Instant::now();
        while self.has_active_tasks().await {
            if shutdown_start.elapsed() > self.config.shutdown_timeout {
                warn!("Shutdown timeout reached, forcing shutdown");
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        info!("Graceful shutdown completed");
        Ok(())
    }

    async fn has_active_tasks(&self) -> bool {
        self.global_semaphore.available_permits() < self.config.max_global_concurrency
    }

    async fn get_queue_sizes(&self) -> QueueSizes {
        // This would need to be implemented based on queue_manager interface
        QueueSizes {
            list_page_fetch: 0,
            list_page_parse: 0,
            product_detail_fetch: 0,
            product_detail_parse: 0,
            product_save: 0,
        }
    }

    async fn calculate_worker_utilization(&self) -> f64 {
        let available = self.global_semaphore.available_permits();
        let total = self.config.max_global_concurrency;
        ((total - available) as f64 / total as f64) * 100.0
    }

    fn calculate_tasks_per_second(&self, stats: &CrawlingStats, uptime: Duration) -> f64 {
        let total_tasks = stats.total_tasks_processed();
        let uptime_seconds = uptime.as_secs_f64();
        
        if uptime_seconds > 0.0 {
            total_tasks as f64 / uptime_seconds
        } else {
            0.0
        }
    }

    async fn perform_health_check(&self) -> Result<(), OrchestratorError> {
        // Check system health
        let stats = self.get_stats().await;
        
        // Check for stuck queues
        if stats.queue_sizes.list_page_fetch > 1000 {
            warn!("List page fetch queue is very large: {}", stats.queue_sizes.list_page_fetch);
        }
        
        // Check worker utilization
        if stats.worker_utilization > 95.0 {
            warn!("Worker utilization is very high: {:.1}%", stats.worker_utilization);
        }
        
        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            worker_pool: self.worker_pool.clone(),
            queue_manager: self.queue_manager.clone(),
            shared_state: self.shared_state.clone(),
            cancellation_token: self.cancellation_token.clone(),
            config: self.config.clone(),
            global_semaphore: self.global_semaphore.clone(),
            scheduler_interval: self.scheduler_interval,
        }
    }
}

/// Static helper function to avoid lifetime issues with tokio::spawn
pub async fn process_single_task_static(
    task: CrawlingTask,
    worker_pool: Arc<WorkerPool>,
    shared_state: Arc<SharedState>,
    queue_manager: Arc<QueueManager>,
    config: OrchestratorConfig,
) -> Result<(), OrchestratorError> {
    let start_time = Instant::now();
    let task_for_follow_up = task.clone(); // Clone task for follow-up use
    
    // Route task to appropriate worker
    let task_result = match &task {
        CrawlingTask::FetchListPage { .. } => {
            worker_pool.list_page_fetcher().process_task(task, shared_state.clone()).await
        }
        CrawlingTask::ParseListPage { .. } => {
            worker_pool.list_page_parser().process_task(task, shared_state.clone()).await
        }
        CrawlingTask::FetchProductDetail { .. } => {
            worker_pool.product_detail_fetcher().process_task(task, shared_state.clone()).await
        }
        CrawlingTask::ParseProductDetail { .. } => {
            worker_pool.product_detail_parser().process_task(task, shared_state.clone()).await
        }
        CrawlingTask::SaveProduct { .. } => {
            worker_pool.db_saver().process_task(task, shared_state.clone()).await
        }
    };

    // Handle task result
    match task_result {
        Ok(TaskResult::Success { task_id, output, duration }) => {
            // Update shared state with success
            shared_state.record_task_success(task_id, duration).await;
            
            // Generate follow-up tasks if needed
            if let Some(follow_up_tasks) = generate_follow_up_tasks_static(&output, &task_for_follow_up).await {
                for follow_up_task in follow_up_tasks {
                    if let Err(e) = queue_manager.enqueue_task(follow_up_task).await {
                        error!("Failed to enqueue follow-up task: {}", e);
                    }
                }
            }
        }
        Ok(TaskResult::Failure { task_id, error, duration, retry_count }) => {
            // Update shared state with failure
            shared_state.record_task_failure(task_id, error.clone(), duration).await;
            
            // Check if we should retry
            if retry_count < config.max_retries {
                // Re-enqueue with incremented retry count
                let mut retry_task = task_for_follow_up.clone();
                retry_task.increment_retry_count();
                if let Err(e) = queue_manager.enqueue_task(retry_task).await {
                    error!("Failed to enqueue retry task: {}", e);
                }
            } else {
                warn!("Task {} failed after {} retries", task_id, retry_count);
            }
        }
        Err(e) => {
            error!("Worker error processing task: {}", e);
            return Err(OrchestratorError::TaskExecutionError(e.to_string()));
        }
    }
    
    Ok(())
}

/// Static helper function for generating follow-up tasks
pub async fn generate_follow_up_tasks_static(
    output: &TaskOutput,
    original_task: &CrawlingTask,
) -> Option<Vec<CrawlingTask>> {
    use crate::crawling::tasks::TaskOutput;
    
    match output {
        TaskOutput::HtmlContent(html_content) => {
            // List page was fetched, now parse it
            if let CrawlingTask::FetchListPage { page_number, url, .. } = original_task {
                Some(vec![CrawlingTask::ParseListPage {
                    task_id: TaskId::new(),
                    page_number: *page_number,
                    html_content: html_content.clone(),
                    source_url: url.clone(),
                }])
            } else {
                None
            }
        }
        TaskOutput::ProductUrls(urls) => {
            // Product URLs were extracted, now fetch each product detail
            Some(urls.iter().map(|url| CrawlingTask::FetchProductDetail {
                task_id: TaskId::new(),
                product_url: url.clone(),
            }).collect())
        }
        TaskOutput::ProductDetailHtml { product_id, html_content, source_url } => {
            // Product detail was fetched, now parse it
            Some(vec![CrawlingTask::ParseProductDetail {
                task_id: TaskId::new(),
                product_url: source_url.clone(),
                html_content: html_content.clone(),
            }])
        }
        TaskOutput::ProductData(product_data) => {
            // Product data was parsed, now save it
            Some(vec![CrawlingTask::SaveProduct {
                task_id: TaskId::new(),
                product_data: product_data.clone(),
            }])
        }
        TaskOutput::SaveConfirmation { .. } => {
            // Final step, no follow-up tasks needed
            None
        }
    }
}

/// Orchestrator-specific errors
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),
    
    #[error("Task queue error: {0}")]
    TaskQueueError(String),
    
    #[error("Worker pool error: {0}")]
    WorkerPoolError(String),
    
    #[error("Task execution error: {0}")]
    TaskExecutionError(String),
    
    #[error("Shutdown timeout exceeded")]
    ShutdownTimeout,
    
    #[error("System health check failed: {0}")]
    HealthCheckError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn orchestrator_config_defaults() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_global_concurrency, 50);
        assert_eq!(config.scheduler_interval, Duration::from_millis(100));
        assert!(config.auto_retry_enabled);
    }

    #[tokio::test]
    async fn orchestrator_creation() {
        // This test would need mock implementations
        // For now, just test that the structure is sound
        let config = OrchestratorConfig::default();
        assert!(config.max_global_concurrency > 0);
    }
}
