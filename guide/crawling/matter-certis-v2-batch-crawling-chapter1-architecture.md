# Matter Certis v2 - Rust/Tauri 배치 크롤링 구현 가이드
## Chapter 1: 전체 아키텍처 설계

### 개요

본 문서는 기존 TypeScript/Electron 기반 Matter Certis 프로젝트의 크롤링 알고리즘을 분석하여 Rust + Tauri 환경으로 마이그레이션하기 위한 종합적인 구현 가이드입니다.

### 1.1 핵심 아키텍처 컴포넌트

#### 1.1.1 도메인 엔티티 정의

```rust
// src-tauri/src/domain/entities/crawling_session.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSession {
    pub session_id: String,
    pub config: CrawlingConfig,
    pub state: SessionState,
    pub progress: CrawlingProgress,
    pub stages: Vec<StageProgress>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CrawlingSession {
    pub fn new(config: CrawlingConfig) -> Self {
        let now = Utc::now();
        Self {
            session_id: Uuid::new_v4().to_string(),
            config,
            state: SessionState::Idle,
            progress: CrawlingProgress::default(),
            stages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    pub fn update_state(&mut self, new_state: SessionState) {
        self.state = new_state;
        self.updated_at = Utc::now();
    }
    
    pub fn add_stage(&mut self, stage: StageProgress) {
        self.stages.push(stage);
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    pub base_url: String,
    pub matter_filter_url: String,
    pub target_page_count: u32,
    pub max_retries_per_page: u32,
    pub max_retries_per_stage: u32,
    pub concurrent_limit: u32,
    pub request_delay_ms: u64,
    pub timeout_ms: u64,
    pub auto_save_to_db: bool,
    pub export_path: String,
    pub enable_missing_data_recovery: bool,
    pub batch_size: u32,
}

impl Default for CrawlingConfig {
    fn default() -> Self {
        Self {
            base_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            matter_filter_url: "https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=".to_string(),
            target_page_count: 20,
            max_retries_per_page: 3,
            max_retries_per_stage: 2,
            concurrent_limit: 5,
            request_delay_ms: 1000,
            timeout_ms: 30000,
            auto_save_to_db: true,
            export_path: "./export".to_string(),
            enable_missing_data_recovery: true,
            batch_size: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    StatusCheck,
    Running { current_stage: u8 },
    Paused { at_stage: u8 },
    Completed,
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrawlingProgress {
    pub current_stage: u8,
    pub total_stages: u8,
    pub overall_progress: f64,
    pub elapsed_time_seconds: u64,
    pub estimated_remaining_seconds: Option<u64>,
    pub total_products_collected: u32,
    pub successful_pages: u32,
    pub failed_pages: u32,
    pub retry_count: u32,
    pub last_update: Option<DateTime<Utc>>,
}

impl CrawlingProgress {
    pub fn update_progress(&mut self, stage_progress: f64) {
        let stage_weight = 100.0 / self.total_stages as f64;
        let completed_stages = (self.current_stage - 1) as f64;
        self.overall_progress = (completed_stages * stage_weight) + (stage_progress * stage_weight / 100.0);
        self.last_update = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageProgress {
    pub stage_id: u8,
    pub name: String,
    pub state: StageState,
    pub progress: f64,
    pub current_item: u32,
    pub total_items: u32,
    pub successful_items: u32,
    pub failed_items: u32,
    pub retry_count: u32,
    pub elapsed_time_seconds: u64,
    pub estimated_remaining_seconds: Option<u64>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

impl StageProgress {
    pub fn new(stage_id: u8, name: String, total_items: u32) -> Self {
        Self {
            stage_id,
            name,
            state: StageState::Pending,
            progress: 0.0,
            current_item: 0,
            total_items,
            successful_items: 0,
            failed_items: 0,
            retry_count: 0,
            elapsed_time_seconds: 0,
            estimated_remaining_seconds: None,
            start_time: Utc::now(),
            end_time: None,
        }
    }
    
    pub fn start(&mut self) {
        self.state = StageState::Running;
        self.start_time = Utc::now();
    }
    
    pub fn update_progress(&mut self, completed: u32, total: u32) {
        self.current_item = completed;
        self.total_items = total;
        
        if total > 0 {
            self.progress = (completed as f64 / total as f64) * 100.0;
        }
        
        // 예상 남은 시간 계산
        let elapsed = Utc::now().signed_duration_since(self.start_time).num_seconds() as u64;
        self.elapsed_time_seconds = elapsed;
        
        if completed > 0 && self.progress > 0.0 {
            let total_estimated = (elapsed as f64 * 100.0 / self.progress) as u64;
            self.estimated_remaining_seconds = Some(total_estimated.saturating_sub(elapsed));
        }
    }
    
    pub fn complete(&mut self) {
        self.state = StageState::Completed;
        self.progress = 100.0;
        self.end_time = Some(Utc::now());
    }
    
    pub fn fail(&mut self, error: String) {
        self.state = StageState::Failed { error };
        self.end_time = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageState {
    Pending,
    Running,
    Retrying { attempt: u32 },
    Completed,
    Failed { error: String },
    Skipped,
}
```

#### 1.1.2 배치 처리 엔진 인터페이스

```rust
// src-tauri/src/domain/services/batch_engine.rs
use async_trait::async_trait;
use tokio::sync::mpsc;
use std::sync::Arc;

#[async_trait]
pub trait BatchCrawlingEngine: Send + Sync {
    async fn execute_crawling_workflow(&self) -> Result<CrawlingResult, CrawlingError>;
    async fn pause_workflow(&self) -> Result<(), CrawlingError>;
    async fn resume_workflow(&self) -> Result<(), CrawlingError>;
    async fn cancel_workflow(&self) -> Result<(), CrawlingError>;
    async fn get_current_status(&self) -> Result<CrawlingProgress, CrawlingError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingResult {
    Success {
        total_products: u32,
        elapsed_time: u64,
        stages_completed: u8,
    },
    PartialSuccess {
        total_products: u32,
        failed_stages: Vec<u8>,
        elapsed_time: u64,
    },
    Failed {
        error: String,
        completed_stages: u8,
        elapsed_time: u64,
    },
    Cancelled {
        completed_stages: u8,
        elapsed_time: u64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CrawlingError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Parsing error: {0}")]
    Parsing(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Stage failed: {stage} - {reason}")]
    StageFailed { stage: u8, reason: String },
    
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,
    
    #[error("Workflow cancelled")]
    WorkflowCancelled,
    
    #[error("Timeout error: {0}")]
    Timeout(String),
}

impl CrawlingError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CrawlingError::Network(_) | 
            CrawlingError::Timeout(_) |
            CrawlingError::Parsing(_)
        )
    }
}
```

#### 1.1.3 단계별 워크플로우 정의

```rust
// src-tauri/src/domain/workflows/crawling_workflow.rs
use crate::domain::entities::*;

#[derive(Debug, Clone)]
pub struct CrawlingWorkflow {
    pub stages: Vec<WorkflowStage>,
}

impl CrawlingWorkflow {
    pub fn new() -> Self {
        Self {
            stages: vec![
                WorkflowStage::new(1, "Status Check".to_string(), StageType::StatusCheck),
                WorkflowStage::new(2, "Database Analysis".to_string(), StageType::DatabaseAnalysis),
                WorkflowStage::new(3, "Product List Collection".to_string(), StageType::ProductListCollection),
                WorkflowStage::new(4, "Product Detail Collection".to_string(), StageType::ProductDetailCollection),
            ],
        }
    }
    
    pub fn get_stage(&self, stage_id: u8) -> Option<&WorkflowStage> {
        self.stages.iter().find(|s| s.stage_id == stage_id)
    }
    
    pub fn total_stages(&self) -> u8 {
        self.stages.len() as u8
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowStage {
    pub stage_id: u8,
    pub name: String,
    pub stage_type: StageType,
    pub dependencies: Vec<u8>,
    pub is_optional: bool,
}

impl WorkflowStage {
    pub fn new(stage_id: u8, name: String, stage_type: StageType) -> Self {
        Self {
            stage_id,
            name,
            stage_type,
            dependencies: Vec::new(),
            is_optional: false,
        }
    }
    
    pub fn with_dependencies(mut self, dependencies: Vec<u8>) -> Self {
        self.dependencies = dependencies;
        self
    }
    
    pub fn as_optional(mut self) -> Self {
        self.is_optional = true;
        self
    }
}

#[derive(Debug, Clone)]
pub enum StageType {
    StatusCheck,
    DatabaseAnalysis,
    ProductListCollection,
    ProductDetailCollection,
    MissingDataRecovery {
        target_pages: Vec<u32>,
        target_products: Vec<String>,
    },
}
```

### 1.2 데이터 모델 정의

#### 1.2.1 상태 체크 결과

```rust
// src-tauri/src/domain/entities/status_check.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCheckResult {
    pub site_total_pages: u32,
    pub last_page_product_count: u32,
    pub local_db_status: LocalDatabaseStatus,
    pub crawling_range: CrawlingRange,
    pub missing_analysis: MissingDataAnalysis,
    pub check_timestamp: DateTime<Utc>,
}

impl StatusCheckResult {
    pub fn calculate_expected_products(&self) -> u32 {
        let full_pages = self.crawling_range.end_site_page.saturating_sub(
            self.crawling_range.start_site_page
        );
        
        // pageId 0 (첫 번째 페이지)는 offset만큼 적음
        let first_page_products = if self.crawling_range.start_site_page == 0 {
            12_u32.saturating_sub(self.crawling_range.offset)
        } else {
            12
        };
        
        first_page_products + (full_pages * 12)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDatabaseStatus {
    pub total_products: u32,
    pub total_pages: u32,
    pub max_page_id: Option<u32>,
    pub incomplete_pages: Vec<IncompletePageInfo>,
    pub missing_products: Vec<ProductRef>,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRange {
    pub start_site_page: u32,
    pub end_site_page: u32,
    pub offset: u32,
    pub expected_total_products: u32,
    pub target_pages: Vec<u32>,
}

impl CrawlingRange {
    pub fn new(start: u32, end: u32, offset: u32) -> Self {
        let target_pages: Vec<u32> = (start..=end).collect();
        let expected_total_products = Self::calculate_expected_products(start, end, offset);
        
        Self {
            start_site_page: start,
            end_site_page: end,
            offset,
            expected_total_products,
            target_pages,
        }
    }
    
    fn calculate_expected_products(start: u32, end: u32, offset: u32) -> u32 {
        let total_pages = end.saturating_sub(start) + 1;
        let first_page_products = if start == 0 {
            12_u32.saturating_sub(offset)
        } else {
            12
        };
        
        if total_pages <= 1 {
            first_page_products
        } else {
            first_page_products + ((total_pages - 1) * 12)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncompletePageInfo {
    pub page_id: u32,
    pub site_page_number: u32,
    pub current_count: u32,
    pub expected_count: u32,
    pub missing_indices: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRef {
    pub url: String,
    pub page_id: u32,
    pub index_in_page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDataAnalysis {
    pub total_missing_products: u32,
    pub missing_pages: Vec<u32>,
    pub incomplete_pages: Vec<IncompletePageInfo>,
    pub missing_products: Vec<ProductRef>,
    pub collection_strategy: CollectionStrategy,
    pub priority_pages: Vec<u32>,
}

impl MissingDataAnalysis {
    pub fn has_missing_data(&self) -> bool {
        !self.missing_pages.is_empty() || 
        !self.incomplete_pages.is_empty() || 
        !self.missing_products.is_empty()
    }
    
    pub fn calculate_recovery_workload(&self) -> u32 {
        self.missing_pages.len() as u32 + 
        self.incomplete_pages.len() as u32 + 
        (self.missing_products.len() as u32 / 12) // 대략적인 페이지 수로 변환
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CollectionStrategy {
    FullPageCollection,      // 전체 페이지 수집
    IncrementalCollection,   // 점진적 수집
    MixedStrategy,          // 혼합 전략
    RecoveryOnlyStrategy,   // 복구 전용
}
```

### 1.3 이벤트 시스템 설계

#### 1.3.1 진행 상황 이벤트

```rust
// src-tauri/src/domain/events/progress_events.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressUpdate {
    SessionStarted { 
        session_id: String,
        config: CrawlingConfig,
        estimated_duration: Option<u64>,
    },
    
    StageStarted { 
        stage_id: u8, 
        stage_name: String,
        total_items: u32,
    },
    
    StatusCheckProgress { 
        current_step: String, 
        progress: f64,
        details: Option<String>,
    },
    
    PageStarted { 
        site_page_number: u32,
        page_url: String,
    },
    
    PageCompleted { 
        site_page_number: u32, 
        products_count: u32,
        elapsed_time: u64,
    },
    
    PageFailed { 
        site_page_number: u32, 
        error: String, 
        retry_count: u32,
        will_retry: bool,
    },
    
    ProductStarted { 
        url: String,
        page_id: u32,
        index_in_page: u32,
    },
    
    ProductCompleted { 
        url: String,
        has_matter_data: bool,
    },
    
    ProductFailed { 
        url: String, 
        error: String, 
        retry_count: u32,
        will_retry: bool,
    },
    
    StageCompleted { 
        stage_id: u8, 
        success_count: u32, 
        failure_count: u32,
        elapsed_time: u64,
    },
    
    SessionCompleted { 
        total_products: u32, 
        elapsed_time: u64,
        final_statistics: SessionStatistics,
    },
    
    SessionFailed { 
        error: String,
        failed_stage: u8,
    },
    
    RetryStarted { 
        stage_id: u8, 
        attempt: u32,
        max_attempts: u32,
    },
    
    DatabaseSaveStarted {
        total_items: u32,
    },
    
    DatabaseSaveCompleted { 
        saved_count: u32,
        skipped_count: u32,
    },
    
    MissingDataRecoveryStarted {
        missing_pages: Vec<u32>,
        incomplete_pages: Vec<u32>,
        individual_products: u32,
    },
    
    ProgressUpdate {
        stage_id: u8,
        current_item: u32,
        total_items: u32,
        elapsed_time: u64,
        estimated_remaining: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatistics {
    pub total_pages_processed: u32,
    pub total_products_collected: u32,
    pub successful_pages: u32,
    pub failed_pages: u32,
    pub retry_attempts: u32,
    pub average_page_time: f64,
    pub average_product_time: f64,
    pub collection_rate: f64,
}
```

이것으로 Chapter 1이 완료되었습니다. 이 장에서는 전체 아키텍처의 핵심 컴포넌트들을 정의했습니다. 

다음으로 Chapter 2를 생성할까요? Chapter 2에서는 상태 체크 및 범위 계산 알고리즘을 다룰 예정입니다.
