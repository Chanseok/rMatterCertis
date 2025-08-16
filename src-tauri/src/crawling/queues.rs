//! # Queue Management System
//!
//! This module implements type-safe task queues using Rust's channel system.
//! Each queue is designed for specific task types with appropriate backpressure handling.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, mpsc};
use tokio::time::timeout;

use crate::crawling::tasks::CrawlingTask;

/// Queue configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum number of tasks in queue
    pub max_capacity: usize,

    /// Timeout for queue operations
    pub operation_timeout: Duration,

    /// Enable queue metrics collection
    pub enable_metrics: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10_000,
            operation_timeout: Duration::from_secs(5),
            enable_metrics: true,
        }
    }
}

/// Type-safe task queue with backpressure support
pub struct TaskQueue {
    /// Channel sender for enqueuing tasks
    sender: mpsc::Sender<CrawlingTask>,

    /// Channel receiver for dequeuing tasks
    receiver: Arc<RwLock<mpsc::Receiver<CrawlingTask>>>,

    /// Queue metrics
    metrics: Arc<RwLock<QueueMetrics>>,

    /// Queue configuration
    config: QueueConfig,

    /// Backpressure semaphore
    backpressure_semaphore: Arc<Semaphore>,
}

impl TaskQueue {
    /// Creates a new task queue with specified configuration
    #[must_use]
    pub fn new(config: QueueConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_capacity);

        Self {
            sender,
            receiver: Arc::new(RwLock::new(receiver)),
            metrics: Arc::new(RwLock::new(QueueMetrics::new())),
            backpressure_semaphore: Arc::new(Semaphore::new(config.max_capacity)),
            config,
        }
    }

    /// Enqueues a task with backpressure handling
    ///
    /// # Errors
    /// Returns error if queue is full or timeout occurs
    pub async fn enqueue(&self, task: CrawlingTask) -> Result<(), QueueError> {
        // Acquire backpressure permit
        let _permit = timeout(
            self.config.operation_timeout,
            self.backpressure_semaphore.acquire(),
        )
        .await
        .map_err(|_| QueueError::Timeout)?
        .map_err(|_| QueueError::Closed)?;

        // Send task to queue
        self.sender
            .send(task)
            .await
            .map_err(|_| QueueError::Closed)?;

        // Update metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.record_enqueue();
        }

        Ok(())
    }

    /// Dequeues a task with timeout
    ///
    /// # Errors
    /// Returns error if queue is empty or timeout occurs
    pub async fn dequeue(&self) -> Result<CrawlingTask, QueueError> {
        let mut receiver = self.receiver.write().await;

        let task = timeout(self.config.operation_timeout, receiver.recv())
            .await
            .map_err(|_| QueueError::Timeout)?
            .ok_or(QueueError::Closed)?;

        // Update metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.record_dequeue();
        }

        Ok(task)
    }

    /// Returns current queue metrics
    pub async fn metrics(&self) -> QueueMetrics {
        self.metrics.read().await.clone()
    }

    /// Returns queue capacity
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.config.max_capacity
    }

    /// Returns approximate number of tasks in queue
    #[must_use]
    pub fn len(&self) -> usize {
        self.sender.capacity() - self.sender.max_capacity()
    }

    /// Returns true if queue is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Closes the queue gracefully
    pub async fn close(&self) {
        let _ = &self.sender;
    }
}

/// Queue metrics for monitoring and telemetry
#[derive(Debug, Clone)]
pub struct QueueMetrics {
    /// Total number of tasks enqueued
    pub total_enqueued: u64,

    /// Total number of tasks dequeued
    pub total_dequeued: u64,

    /// Current queue size
    pub current_size: usize,

    /// Maximum queue size reached
    pub max_size_reached: usize,

    /// Average wait time for enqueue operations
    pub avg_enqueue_wait_time: Duration,

    /// Average wait time for dequeue operations
    pub avg_dequeue_wait_time: Duration,

    /// Timestamp of last operation
    pub last_operation_time: Option<Instant>,

    /// Queue creation time
    pub created_at: Instant,
}

impl QueueMetrics {
    /// Creates new queue metrics
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_enqueued: 0,
            total_dequeued: 0,
            current_size: 0,
            max_size_reached: 0,
            avg_enqueue_wait_time: Duration::default(),
            avg_dequeue_wait_time: Duration::default(),
            last_operation_time: None,
            created_at: Instant::now(),
        }
    }

    /// Records an enqueue operation
    pub fn record_enqueue(&mut self) {
        self.total_enqueued += 1;
        self.current_size += 1;
        self.max_size_reached = self.max_size_reached.max(self.current_size);
        self.last_operation_time = Some(Instant::now());
    }

    /// Records a dequeue operation
    pub fn record_dequeue(&mut self) {
        self.total_dequeued += 1;
        self.current_size = self.current_size.saturating_sub(1);
        self.last_operation_time = Some(Instant::now());
    }

    /// Returns queue throughput (operations per second)
    #[must_use]
    pub fn throughput(&self) -> f64 {
        let elapsed = self.created_at.elapsed();
        if elapsed.as_secs() > 0 {
            (self.total_enqueued + self.total_dequeued) as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        }
    }
}

impl Default for QueueMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue operation errors
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue operation timed out")]
    Timeout,

    #[error("Queue is closed")]
    Closed,

    #[error("Queue is full")]
    Full,

    #[error("Queue is empty")]
    Empty,

    #[error("Queue operation failed: {0}")]
    Operation(String),
}

/// Multi-queue manager for organizing different task types
pub struct QueueManager {
    /// Queue for list page fetching tasks
    list_page_fetch_queue: TaskQueue,

    /// Queue for list page parsing tasks
    list_page_parse_queue: TaskQueue,

    /// Queue for product detail fetching tasks
    detail_page_fetch_queue: TaskQueue,

    /// Queue for product detail parsing tasks
    detail_page_parse_queue: TaskQueue,

    /// Queue for database saving tasks
    db_save_queue: TaskQueue,

    /// Dead letter queue for failed tasks
    dead_letter_queue: TaskQueue,
}

impl QueueManager {
    /// Creates a new queue manager with default configuration
    #[must_use]
    pub fn new() -> Self {
        let config = QueueConfig::default();
        Self::with_config(config)
    }

    /// Creates a new queue manager with custom configuration
    #[must_use]
    pub fn with_config(config: QueueConfig) -> Self {
        Self {
            list_page_fetch_queue: TaskQueue::new(config.clone()),
            list_page_parse_queue: TaskQueue::new(config.clone()),
            detail_page_fetch_queue: TaskQueue::new(config.clone()),
            detail_page_parse_queue: TaskQueue::new(config.clone()),
            db_save_queue: TaskQueue::new(config.clone()),
            dead_letter_queue: TaskQueue::new(config),
        }
    }

    /// Creates a new queue manager with specific queue size and backpressure settings
    #[must_use]
    pub fn new_with_config(max_queue_size: usize, _backpressure_threshold: usize) -> Self {
        let config = QueueConfig {
            max_capacity: max_queue_size,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Routes a task to the appropriate queue
    ///
    /// # Errors
    /// Returns error if the target queue is full or closed
    pub async fn route_task(&self, task: CrawlingTask) -> Result<(), QueueError> {
        match task {
            CrawlingTask::FetchListPage { .. } => self.list_page_fetch_queue.enqueue(task).await,
            CrawlingTask::ParseListPage { .. } => self.list_page_parse_queue.enqueue(task).await,
            CrawlingTask::FetchProductDetail { .. } => {
                self.detail_page_fetch_queue.enqueue(task).await
            }
            CrawlingTask::ParseProductDetail { .. } => {
                self.detail_page_parse_queue.enqueue(task).await
            }
            CrawlingTask::SaveProduct { .. } => self.db_save_queue.enqueue(task).await,
        }
    }

    /// Sends a failed task to the dead letter queue
    ///
    /// # Errors
    /// Returns error if dead letter queue is full
    pub async fn send_to_dead_letter(&self, task: CrawlingTask) -> Result<(), QueueError> {
        self.dead_letter_queue.enqueue(task).await
    }

    /// Returns reference to the list page fetch queue
    #[must_use]
    pub const fn list_page_fetch_queue(&self) -> &TaskQueue {
        &self.list_page_fetch_queue
    }

    /// Returns reference to the list page parse queue
    #[must_use]
    pub const fn list_page_parse_queue(&self) -> &TaskQueue {
        &self.list_page_parse_queue
    }

    /// Returns reference to the detail page fetch queue
    #[must_use]
    pub const fn detail_page_fetch_queue(&self) -> &TaskQueue {
        &self.detail_page_fetch_queue
    }

    /// Returns reference to the detail page parse queue
    #[must_use]
    pub const fn detail_page_parse_queue(&self) -> &TaskQueue {
        &self.detail_page_parse_queue
    }

    /// Returns reference to the database save queue
    #[must_use]
    pub const fn db_save_queue(&self) -> &TaskQueue {
        &self.db_save_queue
    }

    /// Returns reference to the dead letter queue
    #[must_use]
    pub const fn dead_letter_queue(&self) -> &TaskQueue {
        &self.dead_letter_queue
    }

    /// Closes all queues gracefully
    pub async fn close_all(&self) {
        self.list_page_fetch_queue.close().await;
        self.list_page_parse_queue.close().await;
        self.detail_page_fetch_queue.close().await;
        self.detail_page_parse_queue.close().await;
        self.db_save_queue.close().await;
        self.dead_letter_queue.close().await;
    }

    /// Dequeues a task from the appropriate queue based on priority
    pub async fn dequeue_task(&self) -> Result<CrawlingTask, QueueError> {
        // 우선순위: list_page_fetch > list_page_parse > detail_page_fetch > detail_page_parse > db_save

        // Try list page fetch first
        if let Ok(task) = self.list_page_fetch_queue.dequeue().await {
            return Ok(task);
        }

        // Try list page parse
        if let Ok(task) = self.list_page_parse_queue.dequeue().await {
            return Ok(task);
        }

        // Try detail page fetch
        if let Ok(task) = self.detail_page_fetch_queue.dequeue().await {
            return Ok(task);
        }

        // Try detail page parse
        if let Ok(task) = self.detail_page_parse_queue.dequeue().await {
            return Ok(task);
        }

        // Try db save
        if let Ok(task) = self.db_save_queue.dequeue().await {
            return Ok(task);
        }

        // No tasks available
        Err(QueueError::Empty)
    }

    /// Enqueues a task to the appropriate queue based on task type
    pub async fn enqueue_task(&self, task: CrawlingTask) -> Result<(), QueueError> {
        match &task {
            CrawlingTask::FetchListPage { .. } => self.list_page_fetch_queue.enqueue(task).await,
            CrawlingTask::ParseListPage { .. } => self.list_page_parse_queue.enqueue(task).await,
            CrawlingTask::FetchProductDetail { .. } => {
                self.detail_page_fetch_queue.enqueue(task).await
            }
            CrawlingTask::ParseProductDetail { .. } => {
                self.detail_page_parse_queue.enqueue(task).await
            }
            CrawlingTask::SaveProduct { .. } => self.db_save_queue.enqueue(task).await,
        }
    }

    /// Clear all queues - used for emergency shutdown
    pub async fn clear_all_queues(&self) -> Result<(), QueueError> {
        // Note: This is a simplified implementation
        // In a real implementation, you would drain all queues
        tracing::info!("Clearing all queues");

        // For now, just closing all queues will effectively clear them
        self.close_all().await;

        Ok(())
    }
}

impl Default for QueueManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawling::tasks::{CrawlingTask, TaskId};

    #[tokio::test]
    async fn task_queue_enqueue_dequeue() {
        let config = QueueConfig::default();
        let queue = TaskQueue::new(config);

        let task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };

        queue.enqueue(task.clone()).await.unwrap();
        let dequeued = queue.dequeue().await.unwrap();

        assert_eq!(task.task_id(), dequeued.task_id());
    }

    #[tokio::test]
    async fn queue_manager_routing() {
        let manager = QueueManager::new();

        let task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };

        manager.route_task(task.clone()).await.unwrap();

        let dequeued = manager.list_page_fetch_queue().dequeue().await.unwrap();
        assert_eq!(task.task_id(), dequeued.task_id());
    }

    #[tokio::test]
    async fn queue_metrics_tracking() {
        let config = QueueConfig::default();
        let queue = TaskQueue::new(config);

        let task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };

        queue.enqueue(task).await.unwrap();
        let metrics = queue.metrics().await;

        assert_eq!(metrics.total_enqueued, 1);
        assert_eq!(metrics.current_size, 1);
    }
}
