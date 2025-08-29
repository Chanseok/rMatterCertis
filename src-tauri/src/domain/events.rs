//! Event types for real-time communication between backend and frontend
//!
//! This module defines all event types that will be emitted from the Rust backend
//! to the `SolidJS` frontend for real-time updates during crawling operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the current stage of the crawling process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
            Self::Idle => write!(f, "대기"),
            Self::StatusCheck => write!(f, "사이트 상태 확인"),
            Self::DatabaseAnalysis => write!(f, "데이터베이스 분석"),
            Self::TotalPages => write!(f, "총 페이지 수 확인"),
            Self::ProductList => write!(f, "제품 목록 수집"),
            Self::ProductDetails => write!(f, "제품 상세정보 수집"),
            Self::DatabaseSave => write!(f, "데이터베이스 저장"),
            Self::Database => write!(f, "데이터베이스 저장"), // 레거시 호환성
        }
    }
}

/// Overall status of the crawling operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
            self.percentage = (f64::from(self.current) / f64::from(self.total)) * 100.0;
        } else {
            self.percentage = 0.0;
        }

        // Calculate elapsed time
        let now = Utc::now();
        self.elapsed_time = (now - start_time).num_seconds().max(0) as u64;

        // Estimate remaining time based on current progress
        if self.current > 0 && self.elapsed_time > 0 {
            let items_per_second = f64::from(self.current) / self.elapsed_time as f64;
            if items_per_second > 0.0 {
                let remaining_items = f64::from(self.total.saturating_sub(self.current));
                self.remaining_time = Some((remaining_items / items_per_second) as u64);
            }
        }

        // Update timestamp
        self.timestamp = now;
    }

    /// Create a new progress instance with calculated fields
    #[must_use] pub fn new_with_calculation(
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
        is_standalone: bool, // true면 독립적인 체크, false면 크롤링 세션 내 체크
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
    /// 🔥 `ProductList` 페이지별 이벤트
    ProductListPageEvent {
        session_id: String,
        batch_id: String,
        page_number: u32,
        event_type: PageEventType,
        message: String,
        timestamp: DateTime<Utc>,
        metadata: Option<PageMetadata>,
    },
    /// 🔥 제품별 상세 정보 수집 이벤트 (기존 `TaskUpdate` 보완)
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
    /// (옵션) 상세 상태 - 요청/대기/파싱 등 세분화
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_state: Option<PageProcessingState>,
    /// (옵션) 타임아웃 설정값(ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_setting_ms: Option<u64>,
    /// (옵션) 현재 시도 횟수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_attempt: Option<u32>,
    /// (옵션) 전체 처리 소요시간(ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_processing_time_ms: Option<u64>,
    /// (옵션) 요청 전송 시각
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_sent_at: Option<DateTime<Utc>>,
    /// (옵션) 응답 수신 시각
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_received_at: Option<DateTime<Utc>>,
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
    /// (옵션) 현재 시도 횟수(표시용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_attempt: Option<u32>,
    /// (옵션) 요청 전송 시각
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_sent_at: Option<DateTime<Utc>>,
    /// (옵션) 응답 수신 시각
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_received_at: Option<DateTime<Utc>>,
}

impl CrawlingEvent {
    /// Get the event type as a string for Tauri event emission
    #[must_use] pub const fn event_name(&self) -> &'static str {
        match self {
            Self::ProgressUpdate(_) => "crawling-progress",
            Self::TaskUpdate(_) => "crawling-task-update",
            Self::StageChange { .. } => "crawling-stage-change",
            Self::Error { .. } => "crawling-error",
            Self::DatabaseUpdate(_) => "database-update",
            Self::Completed(_) => "crawling-completed",
            Self::SiteStatusCheck { .. } => "site-status-check",
            Self::SessionEvent { .. } => "session-event",
            Self::BatchEvent { .. } => "batch-event",
            Self::ProductListPageEvent { .. } => "product-list-page-event",
            Self::ProductDetailEvent { .. } => "product-detail-event",
            Self::SessionLifecycle { .. } => "session-lifecycle",
        }
    }
}

// ========================================================================
// 확장: 세분화된 상태 및 신규 이벤트 (독립 이벤트 스트림)
// ========================================================================

/// 페이지 처리의 세분화된 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageProcessingState {
    /// 대기열에 있음
    Queued,
    /// 요청 전송됨
    RequestSent,
    /// 응답 대기 중
    AwaitingResponse,
    /// 응답 수신됨
    ResponseReceived,
    /// 파싱/처리 중
    Processing,
    /// 완료됨
    Completed,
    /// 실패(사유 포함 가능)
    Failed,
    /// 재시도(n)
    Retrying,
}

/// 동시성 상태 브로드캐스트 (저주파)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConcurrencyEvent {
    ConcurrentBatchStarted {
        session_id: String,
        batch_id: String,
        stage: CrawlingStage,
        concurrent_tasks: u32,
        max_concurrency: u32,
        timestamp: DateTime<Utc>,
    },
    ConcurrentTaskStatusUpdate {
        session_id: String,
        batch_id: String,
        active_tasks: u32,
        queued_tasks: u32,
        completed_tasks: u32,
        failed_tasks: u32,
        timestamp: DateTime<Utc>,
    },
}

impl ConcurrencyEvent {
    #[must_use] pub const fn event_name(&self) -> &'static str {
        "concurrency-event"
    }
}

/// Validation 단계 상세 이벤트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationEvent {
    ValidationStarted {
        batch_id: String,
        total_items: u32,
        timestamp: DateTime<Utc>,
    },
    ValidationIssueFound {
        batch_id: String,
        item_id: String,
        issue_type: ValidationIssueType,
        details: String,
        timestamp: DateTime<Utc>,
    },
    ValidationCompleted {
        batch_id: String,
        passed: u32,
        failed: u32,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationIssueType {
    MissingRequiredField,
    InvalidDataFormat,
    DuplicateEntry,
    DataInconsistency,
}

impl ValidationEvent {
    #[must_use] pub const fn event_name(&self) -> &'static str {
        "validation-event"
    }
}

/// 데이터베이스 저장 단계 상세 이벤트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseSaveEvent {
    SaveBatchStarted {
        batch_id: String,
        total_items: u32,
        timestamp: DateTime<Utc>,
    },
    SaveItemResult {
        batch_id: String,
        item_id: String,
        page_number: u32,
        index_in_page: u32,
        result: SaveResult,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    SaveBatchCompleted {
        batch_id: String,
        saved: u32,
        skipped: u32,
        failed: u32,
        failed_items: Vec<FailedSaveItem>,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SaveResult {
    Saved,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedSaveItem {
    pub page_number: u32,
    pub index_in_page: u32,
    pub product_url: String,
    pub reason: String,
}

impl DatabaseSaveEvent {
    #[must_use] pub const fn event_name(&self) -> &'static str {
        "db-save-event"
    }
}
