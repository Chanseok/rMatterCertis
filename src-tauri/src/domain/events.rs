//! Event types for real-time communication between backend and frontend
//! 
//! This module defines all event types that will be emitted from the Rust backend
//! to the SolidJS frontend for real-time updates during crawling operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Represents the current stage of the crawling process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlingStage {
    /// System is idle, no crawling in progress
    Idle,
    /// Checking site status and accessibility
    StatusCheck,
    /// Analyzing current database state
    DatabaseAnalysis,
    /// Discovering total number of pages to crawl
    TotalPages,
    /// Collecting product list from pages
    ProductList,
    /// Collecting detailed product information
    ProductDetails,
    /// Saving data to database
    DatabaseSave,
    /// Legacy database stage (keeping for backward compatibility)
    Database,
}

impl std::fmt::Display for CrawlingStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrawlingStage::Idle => write!(f, "대기"),
            CrawlingStage::StatusCheck => write!(f, "사이트 상태 확인"),
            CrawlingStage::DatabaseAnalysis => write!(f, "데이터베이스 분석"),
            CrawlingStage::TotalPages => write!(f, "총 페이지 수 확인"),
            CrawlingStage::ProductList => write!(f, "제품 목록 수집"),
            CrawlingStage::ProductDetails => write!(f, "제품 상세정보 수집"),
            CrawlingStage::DatabaseSave => write!(f, "데이터베이스 저장"),
            CrawlingStage::Database => write!(f, "데이터베이스 저장"), // 레거시 호환성
        }
    }
}

/// Overall status of the crawling operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlingStatus {
    /// No crawling operation is running
    Idle,
    /// Crawling is actively running
    Running,
    /// Crawling is temporarily paused
    Paused,
    /// Crawling completed successfully
    Completed,
    /// Crawling stopped due to error
    Error,
    /// Crawling was cancelled by user
    Cancelled,
}

/// Detailed progress information for the entire crawling operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgress {
    /// Current progress count
    pub current: u32,
    /// Total expected items to process
    pub total: u32,
    /// Progress percentage (0.0 to 100.0)
    pub percentage: f64,
    /// Current stage of crawling
    pub current_stage: CrawlingStage,
    /// Human-readable description of current step
    pub current_step: String,
    /// Overall status
    pub status: CrawlingStatus,
    /// Status message for display
    pub message: String,
    /// Estimated remaining time in seconds
    pub remaining_time: Option<u64>,
    /// Elapsed time in seconds since start
    pub elapsed_time: u64,
    /// Number of new items discovered
    pub new_items: u32,
    /// Number of items updated
    pub updated_items: u32,
    /// Current batch being processed
    pub current_batch: Option<u32>,
    /// Total number of batches
    pub total_batches: Option<u32>,
    /// Number of errors encountered
    pub errors: u32,
    /// Timestamp of this progress update
    pub timestamp: DateTime<Utc>,
}

impl Default for CrawlingProgress {
    fn default() -> Self {
        Self {
            current: 0,
            total: 0,
            percentage: 0.0,
            current_stage: CrawlingStage::Idle,
            current_step: "대기 중".to_string(),
            status: CrawlingStatus::Idle,
            message: "크롤링이 시작되지 않았습니다".to_string(),
            remaining_time: None,
            elapsed_time: 0,
            new_items: 0,
            updated_items: 0,
            current_batch: None,
            total_batches: None,
            errors: 0,
            timestamp: Utc::now(),
        }
    }
}

impl CrawlingProgress {
    /// Calculate and update derived fields based on current progress
    pub fn calculate_derived_fields(&mut self, start_time: DateTime<Utc>) {
        // Calculate percentage
        if self.total > 0 {
            self.percentage = (self.current as f64 / self.total as f64) * 100.0;
        } else {
            self.percentage = 0.0;
        }
        
        // Calculate elapsed time
        let now = Utc::now();
        self.elapsed_time = (now - start_time).num_seconds().max(0) as u64;
        
        // Estimate remaining time based on current progress
        if self.current > 0 && self.elapsed_time > 0 {
            let items_per_second = self.current as f64 / self.elapsed_time as f64;
            if items_per_second > 0.0 {
                let remaining_items = self.total.saturating_sub(self.current) as f64;
                self.remaining_time = Some((remaining_items / items_per_second) as u64);
            }
        }
        
        // Update timestamp
        self.timestamp = now;
    }
    
    /// Create a new progress instance with calculated fields
    pub fn new_with_calculation(
        current: u32,
        total: u32,
        stage: CrawlingStage,
        step: String,
        status: CrawlingStatus,
        message: String,
        start_time: DateTime<Utc>,
        new_items: u32,
        updated_items: u32,
        errors: u32,
    ) -> Self {
        let mut progress = Self {
            current,
            total,
            percentage: 0.0,
            current_stage: stage,
            current_step: step,
            status,
            message,
            remaining_time: None,
            elapsed_time: 0,
            new_items,
            updated_items,
            current_batch: None,
            total_batches: None,
            errors,
            timestamp: Utc::now(),
        };
        
        progress.calculate_derived_fields(start_time);
        progress
    }
}

/// Individual task status within a parallel crawling operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingTaskStatus {
    /// Unique identifier for the task
    pub task_id: String,
    /// URL being processed by this task
    pub url: String,
    /// Current status of the task
    pub status: TaskStatus,
    /// Status message
    pub message: String,
    /// Timestamp when status was updated
    pub timestamp: DateTime<Utc>,
    /// Current stage this task is in
    pub stage: CrawlingStage,
    /// Additional details specific to the task
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// Status of an individual crawling task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be processed
    Pending,
    /// Task is currently being processed
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Database statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total number of products in database
    pub total_products: u64,
    /// Total number of devices in database  
    pub total_devices: u64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Estimated storage size
    pub storage_size: String,
    /// Number of incomplete records
    pub incomplete_records: u64,
    /// Database health status
    pub health_status: DatabaseHealth,
}

/// Database health indicators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseHealth {
    /// Database is operating normally
    Healthy,
    /// Database has minor issues
    Warning,
    /// Database has serious issues
    Critical,
}

/// Summary of crawling results after completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingResult {
    /// Total number of items processed
    pub total_processed: u32,
    /// Number of new items added
    pub new_items: u32,
    /// Number of existing items updated
    pub updated_items: u32,
    /// Number of errors encountered
    pub errors: u32,
    /// Duration of crawling operation in milliseconds
    pub duration_ms: u64,
    /// Stages that were completed
    pub stages_completed: Vec<CrawlingStage>,
    /// Start time of the operation
    pub start_time: DateTime<Utc>,
    /// End time of the operation
    pub end_time: DateTime<Utc>,
    /// Performance metrics
    pub performance_metrics: PerformanceMetrics,
}

/// Performance metrics for crawling operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average processing time per item in milliseconds
    pub avg_processing_time_ms: f64,
    /// Items processed per second
    pub items_per_second: f64,
    /// Memory usage in MB
    pub memory_usage_mb: f64,
    /// Network requests made
    pub network_requests: u64,
    /// Cache hit rate percentage
    pub cache_hit_rate: f64,
}

/// Event types that can be emitted to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CrawlingEvent {
    /// Progress update event
    ProgressUpdate(CrawlingProgress),
    /// Individual task status update
    TaskUpdate(CrawlingTaskStatus),
    /// Stage change notification
    StageChange {
        from: CrawlingStage,
        to: CrawlingStage,
        message: String,
    },
    /// Error notification
    Error {
        error_id: String,
        message: String,
        stage: CrawlingStage,
        recoverable: bool,
    },
    /// Database statistics update
    DatabaseUpdate(DatabaseStats),
    /// Final results notification
    Completed(CrawlingResult),
    /// 🔥 독립적인 사이트 상태 체크 이벤트 (크롤링 세션과 무관)
    SiteStatusCheck {
        is_standalone: bool,  // true면 독립적인 체크, false면 크롤링 세션 내 체크
        status: SiteCheckStatus,
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// 🔥 크롤링 세션 이벤트
    SessionEvent {
        session_id: String,
        event_type: SessionEventType,
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// 🔥 세션 라이프사이클 이벤트 (UI 표시용)
    SessionLifecycle {
        session_id: String,
        event_type: SessionEventType,
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// 🔥 배치 이벤트 (각 스테이지별 배치)
    BatchEvent {
        session_id: String,
        batch_id: String,
        stage: CrawlingStage,
        event_type: BatchEventType,
        message: String,
        timestamp: DateTime<Utc>,
        metadata: Option<BatchMetadata>,
    },
    /// 🔥 ProductList 페이지별 이벤트
    ProductListPageEvent {
        session_id: String,
        batch_id: String,
        page_number: u32,
        event_type: PageEventType,
        message: String,
        timestamp: DateTime<Utc>,
        metadata: Option<PageMetadata>,
    },
    /// 🔥 제품별 상세 정보 수집 이벤트 (기존 TaskUpdate 보완)
    ProductDetailEvent {
        session_id: String,
        batch_id: String,
        product_id: String,
        product_url: String,
        event_type: ProductEventType,
        message: String,
        timestamp: DateTime<Utc>,
        metadata: Option<ProductMetadata>,
    },
}

/// 사이트 상태 체크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SiteCheckStatus {
    /// 체크 시작
    Started,
    /// 체크 중
    InProgress,
    /// 체크 성공
    Success,
    /// 체크 실패
    Failed,
}

/// 🔥 세션 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEventType {
    /// 크롤링 세션 시작 (사용자가 크롤링 버튼 클릭)
    Started,
    /// 사이트 상태 확인 및 캐시 검증
    SiteStatusCheck,
    /// 배치 계획 수립 (총 페이지 수, 배치 분할 계획)
    BatchPlanning,
    /// 크롤링 세션 완료
    Completed,
    /// 크롤링 세션 실패
    Failed,
    /// 크롤링 세션 취소
    Cancelled,
    /// 크롤링 세션 일시정지
    Paused,
    /// 크롤링 세션 재개
    Resumed,
}

/// 🔥 배치 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchEventType {
    /// 배치 생성됨
    Created,
    /// 배치 처리 시작
    Started,
    /// 배치 진행 중
    Progress,
    /// 배치 완료
    Completed,
    /// 배치 실패
    Failed,
    /// 배치 재시도
    Retrying,
}

/// 🔥 페이지 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PageEventType {
    /// 페이지 처리 시작
    Started,
    /// 페이지 처리 중
    Progress,
    /// 페이지 처리 완료
    Completed,
    /// 페이지 처리 실패
    Failed,
    /// 페이지 재시도
    Retrying,
}

/// 🔥 제품 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductEventType {
    /// 제품 상세정보 수집 시작
    Started,
    /// 제품 상세정보 수집 중
    Progress,
    /// 제품 상세정보 수집 완료
    Completed,
    /// 제품 상세정보 수집 실패
    Failed,
    /// 제품 상세정보 수집 재시도
    Retrying,
}

/// 🔥 배치 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// 배치에 포함된 아이템 수
    pub total_items: u32,
    /// 처리된 아이템 수
    pub processed_items: u32,
    /// 성공한 아이템 수
    pub successful_items: u32,
    /// 실패한 아이템 수
    pub failed_items: u32,
    /// 처리 시작 시간
    pub start_time: DateTime<Utc>,
    /// 예상 완료 시간
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// 🔥 페이지 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMetadata {
    /// 페이지에서 발견된 제품 수
    pub products_found: u32,
    /// 처리된 제품 수
    pub products_processed: u32,
    /// 페이지 로드 시간 (밀리초)
    pub load_time_ms: u64,
    /// 페이지 크기 (바이트)
    pub page_size_bytes: u64,
}

/// 🔥 제품 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMetadata {
    /// 제품명
    pub product_name: Option<String>,
    /// 제품 카테고리
    pub category: Option<String>,
    /// 인증 번호
    pub certification_number: Option<String>,
    /// 처리 시간 (밀리초)
    pub processing_time_ms: u64,
    /// 페이지 크기 (바이트)
    pub page_size_bytes: u64,
    /// 재시도 횟수
    pub retry_count: u32,
}

impl CrawlingEvent {
    /// Get the event type as a string for Tauri event emission
    pub fn event_name(&self) -> &'static str {
        match self {
            CrawlingEvent::ProgressUpdate(_) => "crawling-progress",
            CrawlingEvent::TaskUpdate(_) => "crawling-task-update",
            CrawlingEvent::StageChange { .. } => "crawling-stage-change",
            CrawlingEvent::Error { .. } => "crawling-error",
            CrawlingEvent::DatabaseUpdate(_) => "database-update",
            CrawlingEvent::Completed(_) => "crawling-completed",
            CrawlingEvent::SiteStatusCheck { .. } => "site-status-check",
            CrawlingEvent::SessionEvent { .. } => "session-event",
            CrawlingEvent::BatchEvent { .. } => "batch-event",
            CrawlingEvent::ProductListPageEvent { .. } => "product-list-page-event",
            CrawlingEvent::ProductDetailEvent { .. } => "product-detail-event",
            CrawlingEvent::SessionLifecycle { .. } => "session-lifecycle",
        }
    }
}
