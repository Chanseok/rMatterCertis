//! Actor 시스템 타입 정의
//!
//! Actor 간 통신과 이벤트를 위한 핵심 타입들을 정의합니다.
//! ts-rs를 통해 TypeScript 타입이 자동 생성됩니다.
//!
//! Event timestamp/duration policy (stability: additive-only)
//! - All lifecycle events MUST include a `timestamp: DateTime<Utc>` when serialized.
//! - Duration fields use milliseconds. Preferred key name is `duration_ms`.
//! - Historical exceptions exist for backwards compatibility:
//!   - `BatchCompleted` uses `duration` (milliseconds).
//!   - `SyncPageCompleted` uses `ms` (milliseconds for the page).
//!   These are kept for compatibility; do not rename. New events should use `duration_ms`.
//! - Do not remove or rename fields; add new ones if needed (additive-only contract).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// 도메인 객체 import 추가
use crate::domain::integrated_product::ProductDetail;
use crate::domain::product_url::ProductUrl;

/// Actor-Event 계약 버전 (additive-only 정책)
pub const ACTOR_CONTRACT_VERSION: &str = "v1"; // bump when UI requires new additive schema set

/// Actor 간 통신을 위한 통합 명령 타입
///
/// 시스템의 모든 Actor가 이해할 수 있는 공통 명령 인터페이스입니다.
/// 계층별로 명령을 구분하여 명확한 책임 분리를 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorCommand {
    // === 세션 레벨 명령 ===
    /// 크롤링 세션 시작
    StartCrawling {
        session_id: String,
        config: CrawlingConfig,
    },

    /// 세션 일시정지
    PauseSession { session_id: String, reason: String },

    /// 세션 재개
    ResumeSession { session_id: String },

    /// 세션 취소
    CancelSession { session_id: String, reason: String },

    /// 미리 생성된 `ExecutionPlan을` 그대로 실행 (재계획 금지)
    ExecutePrePlanned {
        session_id: String,
        plan: ExecutionPlan,
    },

    // === 배치 레벨 명령 ===
    /// 배치 처리
    ProcessBatch {
        batch_id: String,
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
        total_pages: u32,
        products_on_last_page: u32,
    },

    // === 스테이지 레벨 명령 ===
    /// 스테이지 실행
    ExecuteStage {
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
    },

    // === 시스템 레벨 명령 ===
    /// 시스템 종료
    Shutdown,

    /// 헬스 체크
    HealthCheck,
}

/// Actor 간 전달되는 이벤트
///
/// 시스템 상태 변화를 알리는 이벤트들입니다.
/// 이벤트 드리븐 아키텍처의 핵심 구성 요소입니다.
/// `ActorContractVersion`: v1
///
/// Field policy (timestamps/durations):
/// - Every Started/Completed/Failed event includes `timestamp`.
/// - Durations are milliseconds; prefer `duration_ms` for new events.
/// - Legacy exceptions: `BatchCompleted.duration` (ms), `SyncPageCompleted.ms` (ms).
///
/// Core field groups (v2 clarification - additive only):
/// - Session lifecycle: SessionStarted/Completed/Failed { `session_id`, timestamp }
/// - Progress: Progress { `session_id`, `current_step`, `total_steps`, percentage }
/// - Batch: BatchStarted/Completed/Failed { `batch_id`, `session_id`, timestamp }
/// - Stage: StageStarted/Completed/Failed { `stage_type`, `session_id`, `batch_id`? }
/// - Persistence diagnostics: `ProductLifecycle` { status, metrics? }, `PersistenceAnomaly` { kind, detail }
/// - Metrics snapshots: `DatabaseStats` { `total_product_details`, `min_page`, `max_page` }
/// UI 소비자는 최소 `session_id` + timestamp 조합을 키로 사용하고, 선택적으로 `batch_id` / `stage_type` 으로 세분화 렌더링.
///
/// 버전 관리 원칙:
/// 1. Additive-only (새 이벤트/필드 추가는 허용)
/// 2. 필드 제거/의미 변경 금지 → 새 필드/이벤트로 교체 후 기존 Deprecated 유지
/// 3. 버전 증가 조건: UI 분기 필수 스키마 변화(추가 필드가 breaking semantic) 또는 요약(summary) 구조 확장
/// 4. TS `actorContractVersion.ts` 와 값 동기화 필요
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AppEvent {
    // === 세션 이벤트 ===
    SessionStarted {
        session_id: String,
        config: CrawlingConfig,
        timestamp: DateTime<Utc>,
    },

    SessionPaused {
        session_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    SessionResumed {
        session_id: String,
        timestamp: DateTime<Utc>,
    },

    SessionCompleted {
        session_id: String,
        summary: SessionSummary,
        timestamp: DateTime<Utc>,
    },

    /// 세션 종료 후, 다음 크롤링 범위를 위한 신규 계획(플래너 결과)을 전달
    NextPlanReady {
        session_id: String,
        plan: crate::crawl_engine::services::crawling_planner::CrawlingPlan,
        timestamp: DateTime<Utc>,
    },

    SessionFailed {
        session_id: String,
        error: String,
        final_failure: bool,
        timestamp: DateTime<Utc>,
    },

    SessionTimeout {
        session_id: String,
        elapsed: u64, // Duration을 milliseconds로 변경
        timestamp: DateTime<Utc>,
    },

    /// 사전 점검(프리플라이트) 진단 이벤트: 로컬 DB 및 사이트 상태 요약
    PreflightDiagnostics {
        session_id: String,
        db_products: i64,
        db_min_page: Option<i32>,
        db_max_page: Option<i32>,
        site_total_pages: Option<u32>,
        site_known_last_page: Option<u32>,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// 저장 단계 이상 탐지 (예: 예상 신규/업데이트 없을 때, `page_id` 역순 불일치 등)
    PersistenceAnomaly {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        kind: String,   // e.g. "all_noop", "page_id_mismatch"
        detail: String, // human readable description
        attempted: u32, // attempted product details in this batch stage
        inserted: u32,
        updated: u32,
        timestamp: DateTime<Utc>,
    },

    /// DB 통계 스냅샷 (진행 중 주기적 보고 용도)
    DatabaseStats {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        total_product_details: i64,
        min_page: Option<i32>,
        max_page: Option<i32>,
        note: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // === 배치 이벤트 ===
    BatchStarted {
        batch_id: String,
        session_id: String,
        pages_count: u32,
        timestamp: DateTime<Utc>,
    },

    BatchCompleted {
        batch_id: String,
        session_id: String,
        success_count: u32,
        failed_count: u32,
        duration: u64, // Duration을 milliseconds로 변경
        timestamp: DateTime<Utc>,
    },

    BatchFailed {
        batch_id: String,
        session_id: String,
        error: String,
        final_failure: bool,
        timestamp: DateTime<Utc>,
    },

    // === 스테이지 이벤트 ===
    StageStarted {
        stage_type: StageType,
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        items_count: u32,
        timestamp: DateTime<Utc>,
    },

    StageCompleted {
        stage_type: StageType,
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        result: StageResult,
        timestamp: DateTime<Utc>,
    },

    StageFailed {
        stage_type: StageType,
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// 스테이지 재시도 알림 (additive v1)
    StageRetrying {
        stage_type: StageType,
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        attempt: u32,
        max_attempts: u32,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // === 스테이지 아이템(세부 단위) 이벤트 ===
    /// 개별 아이템 처리 시작 (페이지 또는 상품 단위)
    StageItemStarted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        stage_type: StageType,
        item_id: String,
        item_type: StageItemType,
        timestamp: DateTime<Utc>,
    },
    /// 개별 아이템 처리 완료
    StageItemCompleted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        stage_type: StageType,
        item_id: String,
        item_type: StageItemType,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        duration_ms: u64,
        retry_count: u32,
        /// 수집된 엔트리 수 (`ListPage`: product URL 개수, `ProductDetail`: 상세 필드 객체 개수)
        #[serde(skip_serializing_if = "Option::is_none")]
        collected_count: Option<u32>,
        timestamp: DateTime<Utc>,
    },

    // === 진행 상황 이벤트 ===
    Progress {
        session_id: String,
        current_step: u32,
        total_steps: u32,
        message: String,
        percentage: f64,
        timestamp: DateTime<Utc>,
    },

    // === 성능 이벤트 ===
    PerformanceMetrics {
        session_id: String,
        metrics: PerformanceMetrics,
        timestamp: DateTime<Utc>,
    },

    // === 리포트 이벤트 ===
    /// 배치 단위 요약 리포트
    BatchReport {
        session_id: String,
        batch_id: String,
        pages_total: u32,
        pages_success: u32,
        pages_failed: u32,
        list_pages_failed: Vec<u32>,
        details_success: u32,
        details_failed: u32,
        retries_used: u32,
        duration_ms: u64,
        /// 중복 제거로 스킵된 Product URL 수 (옵션 - v1 기본 0)
        #[serde(default)]
        duplicates_skipped: u32,
        #[serde(default)]
        products_inserted: u32,
        #[serde(default)]
        products_updated: u32,
        timestamp: DateTime<Utc>,
    },

    /// 세션 전체 요약 리포트
    CrawlReportSession {
        session_id: String,
        batches_processed: u32,
        total_pages: u32,
        total_success: u32,
        total_failed: u32,
        total_retries: u32,
        duration_ms: u64,
        #[serde(default)]
        products_inserted: u32,
        #[serde(default)]
        products_updated: u32,
        timestamp: DateTime<Utc>,
    },

    // Phase* events removed in favor of Stage*/TaskLifecycle

    // === Graceful shutdown events ===
    ShutdownRequested {
        session_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    ShutdownCompleted {
        session_id: String,
        timestamp: DateTime<Utc>,
    },

    // DetailTask* removed previously: use ProductLifecycle/ProductLifecycleGroup instead

    // === Fine-grained lifecycle events (additive v2) ===
    /// Unified task lifecycle that covers both page and product tasks (additive v2.1)
    TaskLifecycle {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        task_kind: TaskKind, // page | product
        /// Origin page number if known
        #[serde(skip_serializing_if = "Option::is_none")]
        page_number: Option<u32>,
        /// Product reference (URL or key) when task_kind is Product
        #[serde(skip_serializing_if = "Option::is_none")]
        product_ref: Option<String>,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metrics: Option<SimpleMetrics>,
        timestamp: DateTime<Utc>,
    },
    /// Page lifecycle state transition (queued -> `fetch_started` -> `fetch_completed` | failed -> `urls_extracted`)
    PageLifecycle {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        page_number: u32,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metrics: Option<SimpleMetrics>,
        timestamp: DateTime<Utc>,
    },
    /// Product lifecycle (optional aggregation level for group fetch) or per-product when available
    ProductLifecycle {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        /// Origin page number if known
        #[serde(skip_serializing_if = "Option::is_none")]
        page_number: Option<u32>,
        product_ref: String, // URL or hashed key
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metrics: Option<SimpleMetrics>,
        timestamp: DateTime<Utc>,
    },
    /// Grouped product lifecycle aggregation (reduces event volume)
    ProductLifecycleGroup {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        page_number: Option<u32>,
        group_size: u32,
        started: u32,
        succeeded: u32,
        failed: u32,
        duplicates: u32,
        duration_ms: u64,
        phase: String, // fetch | persist
        timestamp: DateTime<Utc>,
    },
    /// Fine grained HTTP fetch latency for list or detail product requests
    HttpRequestTiming {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
        request_kind: String, // list_page | detail_page
        target: String,       // URL or logical key
        page_number: Option<u32>,
        attempt: u32,
        latency_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // === Validation (page_id/index_in_page integrity) events (additive v1) ===
    ValidationStarted {
        session_id: String,
        scan_pages: u32,
        total_pages_site: Option<u32>,
        timestamp: DateTime<Utc>,
    },
    ValidationPageScanned {
        session_id: String,
        physical_page: u32,
        products_found: u32,
        assigned_start_offset: u64,
        assigned_end_offset: u64,
        timestamp: DateTime<Utc>,
    },
    ValidationDivergenceFound {
        session_id: String,
        physical_page: u32,
        kind: String,   // first_missing | coord_mismatch | duplicate | gap
        detail: String, // human readable
        expected_offset: u64,
        timestamp: DateTime<Utc>,
    },
    ValidationAnomaly {
        session_id: String,
        code: String, // duplicate_index | sparse_page | out_of_range
        detail: String,
        timestamp: DateTime<Utc>,
    },
    ValidationCompleted {
        session_id: String,
        pages_scanned: u32,
        products_checked: u64,
        divergences: u32,
        anomalies: u32,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // === Sync (partial recrawl + DB upsert) events (additive v1) ===
    SyncStarted {
        session_id: String,
        ranges: Vec<(u32, u32)>, // (start_oldest, end_newest) inclusive per range
        #[serde(skip_serializing_if = "Option::is_none")]
        rate_limit: Option<u32>,
        timestamp: DateTime<Utc>,
    },
    SyncPageStarted {
        session_id: String,
        physical_page: u32,
        timestamp: DateTime<Utc>,
    },
    SyncUpsertProgress {
        session_id: String,
        physical_page: u32,
        inserted: u32,
        updated: u32,
        skipped: u32,
        failed: u32,
        timestamp: DateTime<Utc>,
    },
    SyncPageCompleted {
        session_id: String,
        physical_page: u32,
        inserted: u32,
        updated: u32,
        skipped: u32,
        failed: u32,
        ms: u64,
        timestamp: DateTime<Utc>,
    },
    SyncWarning {
        session_id: String,
        code: String,
        detail: String,
        timestamp: DateTime<Utc>,
    },
    /// Per-attempt retry notification for This Range Sync (additive v1)
    SyncRetrying {
        session_id: String,
        /// Scope of retry: "`list_page`" | "`product_detail`"
        scope: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        physical_page: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        attempt: u32,
        max_attempts: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    SyncCompleted {
        session_id: String,
        pages_processed: u32,
        inserted: u32,
        updated: u32,
        skipped: u32,
        failed: u32,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        deleted: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_pages: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        items_on_last_page: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        anomalies: Option<Vec<SyncAnomalyEntry>>,
        timestamp: DateTime<Utc>,
    },
}

/// Compact anomaly entry for `SyncCompleted` summary
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SyncAnomalyEntry {
    pub page_id: i32,
    pub count: i64,
    pub current_page_number: u32,
}

// Lightweight TS-friendly metrics container (additive, extensible)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", content = "data")]
pub enum SimpleMetrics {
    Page {
        url_count: Option<u32>,
        scheduled_details: Option<u32>,
        error: Option<String>,
    },
    Product {
        fields: Option<u32>,
        size_bytes: Option<u32>,
        error: Option<String>,
    },
    Generic {
        key: String,
        value: String,
    },
}

/// Unified task kind for TaskLifecycle
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TaskKind {
    Page,
    Product,
}

/// High-level crawl phases (extensible)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum CrawlPhase {
    ListPages,
    ProductDetails,
    DataValidation,
    Finalize,
}

/// 크롤링 설정
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingConfig {
    /// 사이트 URL
    pub site_url: String,

    /// 시작 페이지
    pub start_page: u32,

    /// 종료 페이지
    pub end_page: u32,

    /// 동시 실행 제한
    pub concurrency_limit: u32,

    /// 배치 크기
    pub batch_size: u32,

    /// 요청 지연 시간 (밀리초)
    pub request_delay_ms: u64,

    /// 타임아웃 (초)
    pub timeout_secs: u64,

    /// 재시도 횟수
    pub max_retries: u32,

    /// 크롤링 전략 (기본: 최신 페이지 기준 역순)
    pub strategy: CrawlingStrategy,
}

impl Default for CrawlingConfig {
    fn default() -> Self {
        Self {
            site_url: "https://example.com".to_string(),
            start_page: 1,
            end_page: 10,
            concurrency_limit: 5,
            batch_size: 20,
            request_delay_ms: 1000,
            timeout_secs: 30,
            max_retries: 3,
            strategy: CrawlingStrategy::NewestFirst,
        }
    }
}

/// 크롤링 전략 모드
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum CrawlingStrategy {
    /// 사이트 최신 페이지부터 N개 (기존 Planner 기본)
    NewestFirst,
    /// 로컬 DB 저장 상태를 기반으로 이어서 수집 (증분)
    ContinueFromDb,
}

/// 배치 설정
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BatchConfig {
    /// 배치 크기
    pub batch_size: u32,

    /// 동시 실행 제한
    pub concurrency_limit: u32,

    /// 배치 간 지연 시간 (밀리초)
    pub batch_delay_ms: u64,

    /// 실패 시 재시도 여부
    pub retry_on_failure: bool,

    /// 시작 페이지 (옵션)
    pub start_page: Option<u32>,

    /// 종료 페이지 (옵션)
    pub end_page: Option<u32>,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 20,
            concurrency_limit: 3,
            batch_delay_ms: 500,
            retry_on_failure: true,
            start_page: None,
            end_page: None,
        }
    }
}

/// 스테이지 타입
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StageType {
    /// 상태 확인
    StatusCheck,

    /// 리스트 페이지 크롤링
    ListPageCrawling,

    /// 상품 상세 크롤링
    ProductDetailCrawling,

    /// 데이터 검증
    DataValidation,

    /// 데이터 저장
    DataSaving,
}

impl StageType {
    /// `StageType을` 문자열로 변환
    #[must_use] pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StatusCheck => "status_check",
            Self::ListPageCrawling => "list_page_crawling",
            Self::ProductDetailCrawling => "product_detail_crawling",
            Self::DataValidation => "data_validation",
            Self::DataSaving => "data_saving",
        }
    }
}

/// 스테이지 아이템
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageItem {
    /// 아이템 ID
    pub id: String,

    /// 아이템 타입
    pub item_type: StageItemType,

    /// 처리할 URL
    pub url: String,

    /// 메타데이터
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StageItemType {
    Page {
        page_number: u32,
    },
    Product {
        page_number: u32,
    },
    Url {
        url_type: String,
    },
    ProductUrls {
        urls: Vec<String>, // 간단히 URL 문자열 리스트로 변경
    },
    SiteCheck, // 사이트 상태 확인용 아이템 타입
}

/// 스테이지 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageResult {
    /// 처리된 아이템 수
    pub processed_items: u32,

    /// 성공한 아이템 수
    pub successful_items: u32,

    /// 실패한 아이템 수
    pub failed_items: u32,

    /// 처리 시간
    pub duration_ms: u64,

    /// 상세 결과
    pub details: Vec<StageItemResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageItemResult {
    /// 아이템 ID
    pub item_id: String,

    /// 아이템 타입
    pub item_type: StageItemType,

    /// 성공 여부
    pub success: bool,

    /// 에러 메시지 (실패 시)
    pub error: Option<String>,

    /// 처리 시간
    pub duration_ms: u64,

    /// 재시도 횟수
    pub retry_count: u32,

    /// 수집된 데이터 (JSON 문자열)
    /// `ListPageCrawling`: `ProductURL들의` JSON 배열
    /// `ProductDetailCrawling`: `ProductDetail들의` JSON 배열
    /// `DataSaving`: 저장된 데이터의 메타정보
    pub collected_data: Option<String>,
}

// =============================================================================
// 🔥 Phase 2: 도메인 객체 직접 반환을 위한 새로운 타입 정의
// =============================================================================

/// 스테이지 결과 데이터
///
/// JSON 직렬화 대신 타입 안전한 도메인 객체를 직접 반환합니다.
/// 이는 성능 향상과 타입 안전성을 동시에 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StageResultData {
    /// 상태 확인 결과
    StatusCheck {
        site_available: bool,
        total_pages: Option<u32>,
        last_page_products: Option<u32>,
        response_time_ms: u64,
    },

    /// 리스트 페이지 크롤링 결과 - `ProductUrl` 직접 반환
    ProductUrls {
        urls: Vec<ProductUrl>,
        page_number: u32,
        total_found: u32,
    },

    /// 상품 상세 크롤링 결과 - `ProductDetail` 직접 반환
    ProductDetails {
        details: Vec<ProductDetail>,
        successful_count: u32,
        failed_count: u32,
    },

    /// 데이터 검증 결과
    ValidationResult {
        validated_count: u32,
        error_count: u32,
        warnings: Vec<String>,
    },

    /// 데이터 품질 분석 결과
    QualityAnalysis {
        total_analyzed: u32,
        new_products: u32,
        updated_products: u32,
        duplicate_products: u32,
        incomplete_products: u32,
        quality_score: f64,
        field_completeness_score: f64,
        recommendations: Vec<String>,
    },

    /// 데이터 저장 결과
    SavingResult {
        saved_count: u32,
        duplicates_found: u32,
        database_id_range: Option<(i64, i64)>, // (min_id, max_id)
    },

    /// 빈 결과 (처리할 데이터 없음)
    Empty,
}

/// 개선된 스테이지 아이템 결과
///
/// `collected_data를` `StageResultData로` 교체하여 타입 안전성 향상
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EnhancedStageItemResult {
    /// 아이템 ID
    pub item_id: String,

    /// 아이템 타입
    pub item_type: StageItemType,

    /// 성공 여부
    pub success: bool,

    /// 에러 메시지 (실패 시)
    pub error: Option<String>,

    /// 처리 시간
    pub duration_ms: u64,

    /// 재시도 횟수
    pub retry_count: u32,

    /// 수집된 데이터 - 타입 안전한 도메인 객체 직접 반환
    pub collected_data: Option<StageResultData>,
}

/// 세션 요약
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionSummary {
    /// 세션 ID
    pub session_id: String,

    /// 총 처리 시간
    pub total_duration_ms: u64,

    /// 총 처리된 페이지 수
    pub total_pages_processed: u32,

    /// 총 처리된 상품 수
    pub total_products_processed: u32,

    /// 성공률
    pub success_rate: f64,

    /// 평균 처리 시간 (페이지당, 밀리초)
    pub avg_page_processing_time: u64,

    /// 에러 요약
    pub error_summary: Vec<ErrorSummary>,

    /// 재시도 관련 통계 (추가 필드)
    #[serde(default)]
    pub total_retry_events: u32,
    #[serde(default)]
    pub max_retries_single_page: u32,
    #[serde(default)]
    pub pages_retried: u32,
    #[serde(default)]
    pub retry_histogram: Vec<(u32, u32)>, // (retry_count, pages_with_that_count)

    /// 처리된 배치 수
    pub processed_batches: u32,

    /// 총 성공 수
    pub total_success_count: u32,

    /// 세션 전체에서 중복 제거로 스킵된 Product URL 수 (`BatchReport` 합산)
    #[serde(default)]
    pub duplicates_skipped: u32,
    #[serde(default)]
    pub products_inserted: u32,
    #[serde(default)]
    pub products_updated: u32,
    #[serde(default)]
    pub planned_list_batches: u32,
    #[serde(default)]
    pub executed_list_batches: u32,
    #[serde(default)]
    pub failed_pages_count: u32,
    #[serde(default)]
    pub failed_page_ids: Vec<u32>,

    /// 최종 상태
    pub final_state: String,

    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorSummary {
    /// 에러 타입
    pub error_type: String,

    /// 발생 횟수
    pub count: u32,

    /// 첫 번째 발생 시간
    pub first_occurrence: DateTime<Utc>,

    /// 마지막 발생 시간
    pub last_occurrence: DateTime<Utc>,
}

/// 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerformanceMetrics {
    /// 메모리 사용량 (MB)
    pub memory_usage_mb: f64,

    /// CPU 사용률 (%)
    pub cpu_usage_percent: f64,

    /// 활성 작업 수
    pub active_tasks_count: u32,

    /// 큐 대기 중인 작업 수
    pub queued_tasks_count: u32,

    /// 평균 응답 시간 (밀리초)
    pub avg_response_time_ms: f64,

    /// 처리량 (작업/초)
    pub throughput_per_second: f64,
}

// =============================================================================
// 에러 타입 정의
// =============================================================================

/// Stage 처리 중 발생할 수 있는 에러
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StageError {
    /// 네트워크 연결 실패
    NetworkError { message: String },

    /// HTML 파싱 에러
    ParsingError { message: String },

    /// 데이터 검증 실패
    ValidationError { message: String },

    /// 데이터베이스 에러
    DatabaseError { message: String },

    /// 타임아웃 에러
    TimeoutError { timeout_ms: u64 },

    /// 설정 에러
    ConfigurationError { message: String },

    /// 네트워크 타임아웃
    NetworkTimeout { timeout_ms: u64 },

    /// 일반적인 에러
    GenericError { message: String },
}

// =============================================================================
// 성공 결과 타입 정의
// =============================================================================

/// Stage 성공 결과 상세
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageSuccessResult {
    /// 성공적으로 처리된 아이템 수
    pub processed_items: u32,

    /// 처리 소요 시간 (밀리초)
    pub duration_ms: u64,

    /// 스테이지 처리 시간 (밀리초) - 호환성을 위한 별칭
    pub stage_duration_ms: u64,

    /// 처리율 (items/second)
    pub throughput: f64,

    /// 성공률 (0.0 ~ 1.0)
    pub success_rate: f64,

    /// 추가 메타데이터
    pub metadata: String,

    /// 수집 메트릭스
    pub collection_metrics: Option<CollectionMetrics>,

    /// 처리 메트릭스
    pub processing_metrics: Option<ProcessingMetrics>,
}

// =============================================================================
// 메트릭스 타입 정의
// =============================================================================

/// 데이터 수집 메트릭스
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CollectionMetrics {
    /// 수집된 총 아이템 수
    pub total_collected: u32,

    /// 총 아이템 수 (호환성을 위한 별칭)
    pub total_items: u32,

    /// 성공한 아이템 수
    pub successful_items: u32,

    /// 실패한 아이템 수  
    pub failed_items: u32,

    /// 수집 성공률
    pub collection_rate: f64,

    /// 평균 수집 시간 (밀리초)
    pub avg_collection_time_ms: u64,

    /// 처리 시간 (밀리초) - 호환성을 위한 별칭
    pub duration_ms: u64,

    /// 평균 응답 시간 (밀리초)
    pub avg_response_time_ms: u64,

    /// 성공률 (0.0 ~ 1.0)
    pub success_rate: f64,

    /// 데이터 품질 점수 (0.0 ~ 1.0)
    pub data_quality_score: f64,
}

/// 처리 성능 메트릭스
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcessingMetrics {
    /// 처리된 총 아이템 수
    pub total_processed: u32,

    /// 처리 성공률
    pub processing_rate: f64,

    /// 평균 처리 시간 (밀리초)
    pub avg_processing_time_ms: u64,

    /// 에러율
    pub error_rate: f64,

    /// 재시도율
    pub retry_rate: f64,
}

/// 실패한 아이템 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FailedItem {
    /// 아이템 ID
    pub item_id: String,

    /// 아이템 타입
    pub item_type: String,

    /// 실패 사유
    pub error_message: String,

    /// 재시도 횟수
    pub retry_count: u32,

    /// 실패 시각
    pub failed_at: DateTime<Utc>,
}

/// Actor 에러 타입
#[derive(Debug, Clone, Serialize, Deserialize, TS, thiserror::Error)]
#[ts(export)]
pub enum ActorError {
    #[error("이벤트 브로드캐스트 실패: {0}")]
    EventBroadcastFailed(String),

    #[error("명령 처리 실패: {0}")]
    CommandProcessingFailed(String),

    #[error("채널 통신 오류: {0}")]
    ChannelError(String),

    #[error("설정 오류: {0}")]
    ConfigurationError(String),

    #[error("타임아웃 발생: {0}")]
    Timeout(String),

    #[error("취소됨: {0}")]
    Cancelled(String),

    #[error("리소스 부족: {0}")]
    ResourceExhausted(String),

    #[error("HTTP 요청 실패: {0}")]
    RequestFailed(String),

    #[error("데이터 파싱 실패: {0}")]
    ParsingFailed(String),

    #[error("레거시 서비스 오류: {0}")]
    LegacyServiceError(String),

    #[error("데이터베이스 오류: {0}")]
    DatabaseError(String),

    #[error("알 수 없는 오류: {0}")]
    Unknown(String),
}

// From 구현들
impl From<anyhow::Error> for ActorError {
    fn from(err: anyhow::Error) -> Self {
        Self::CommandProcessingFailed(err.to_string())
    }
}

/// 실행 계획 - `CrawlingPlanner에서` 생성되어 `SessionActor에게` 전달
///
/// 분석-계획-실행 워크플로우를 명확히 분리하기 위한 핵심 구조체입니다.
/// `CrawlingPlanner가` 시스템 상태를 분석하여 생성한 최적의 실행 계획을 담습니다.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecutionPlan {
    /// 실행 계획 ID
    pub plan_id: String,

    /// 세션 ID
    pub session_id: String,

    /// 크롤링 범위 목록 (여러 범위를 순차 처리)
    pub crawling_ranges: Vec<PageRange>,

    /// 배치 크기
    pub batch_size: u32,

    /// 동시 실행 제한
    pub concurrency_limit: u32,

    /// 예상 소요 시간
    pub estimated_duration_secs: u64,

    /// 계획 생성 시간
    pub created_at: DateTime<Utc>,

    /// 분석 정보 (디버깅용)
    pub analysis_summary: String,

    /// 원본 최적화 전략 (해시/검증 시 안정적 사용)
    pub original_strategy: String,

    /// 계획 입력 스냅샷 (사이트/DB 상태) - 단일 권위 보장 용도
    pub input_snapshot: PlanInputSnapshot,

    /// 입력 스냅샷 + 핵심 파라미터 직렬화 후 계산된 해시
    pub plan_hash: String,
    /// 중복 상품 URL 스킵 여부 (경량 dedupe 1단계)
    pub skip_duplicate_urls: bool,
    pub kpi_meta: Option<ExecutionPlanKpi>,
    /// API / 이벤트 스키마 계약 버전 (additive-only 변경 추적)
    pub contract_version: u32,
    /// 사전 계산된 논리적 page slot 목록 (역순/정순 혼합 시 순서 유지)
    pub page_slots: Vec<PageSlot>,
}

/// 중복 URL(로컬 DB에 이미 존재) 처리 정책
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export)]
pub enum DuplicatePersistencePolicy {
    /// 기존 동작: 변경 없으면 스킵 (noop)
    Skip,
    /// 수동 크롤링 등에서 사용: `page_id`, `index_in_page`, id 만 업데이트
    UpdateIdIndexOnly,
    /// 상세필드를 포함해 실제 변경이 있는 모든 칼럼 업데이트 시도
    FullUpdate,
}

impl Default for DuplicatePersistencePolicy {
    fn default() -> Self {
        Self::Skip
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PageSlot {
    /// 실제 물리 페이지 번호 (사이트 기준)
    pub physical_page: u32,
    /// 논리 `page_id` (0 = 가장 오래된 페이지의 마지막 제품 그룹)
    pub page_id: i64,
    /// 해당 물리 페이지 내에서의 `index_in_page` (0 기반, 최신→오래된 역순 규칙 반영)
    pub index_in_page: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecutionPlanKpi {
    pub total_ranges: usize,
    pub total_pages: u32,
    pub batches: usize,
    pub strategy: String,
    pub created_at: DateTime<Utc>,
}

impl ExecutionPlan {
    /// Preplanned 실행 시 최소한의 `SiteStatus` 형태를 구성 (페이지 처리 통계용)
    #[must_use] pub fn input_snapshot_to_site_status(&self) -> crate::domain::services::SiteStatus {
        use crate::domain::services::crawling_services::{
            CrawlingRangeRecommendation, SiteDataChangeStatus,
        };
        // 안정 상태 count 산출: DB 총량 >0 이면 사용, 아니면 페이지 * 마지막페이지상품수 (대략치)
        let stable_count: u32 = if self.input_snapshot.db_total_products > 0 {
            // u64 -> u32 캐스팅 (과도한 값은 u32::MAX 로 clamp)
            self.input_snapshot.db_total_products.min(u64::from(u32::MAX)) as u32
        } else {
            
            self.input_snapshot.total_pages * self.input_snapshot.products_on_last_page.max(1)
        };
        crate::domain::services::SiteStatus {
            is_accessible: true,
            response_time_ms: 0,
            total_pages: self.input_snapshot.total_pages,
            // Use stable_count heuristic (DB total if available) as estimated products
            estimated_products: stable_count,
            products_on_last_page: self.input_snapshot.products_on_last_page,
            last_check_time: chrono::Utc::now(),
            health_score: 1.0,
            data_change_status: SiteDataChangeStatus::Stable {
                count: stable_count,
            },
            decrease_recommendation: None,
            crawling_range_recommendation: CrawlingRangeRecommendation::Full,
        }
    }
}

/// `ExecutionPlan` 생성 시의 입력 상태 스냅샷
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanInputSnapshot {
    pub total_pages: u32,
    pub products_on_last_page: u32,
    pub db_max_page_id: Option<i32>,
    pub db_max_index_in_page: Option<i32>,
    pub db_total_products: u64,
    pub page_range_limit: u32,
    pub batch_size: u32,
    pub concurrency_limit: u32,
    pub created_at: DateTime<Utc>,
}

/// 페이지 범위
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PageRange {
    /// 시작 페이지
    pub start_page: u32,

    /// 끝 페이지
    pub end_page: u32,

    /// 이 범위의 예상 제품 수
    pub estimated_products: u32,

    /// 역순 크롤링 여부
    pub reverse_order: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_crawling_config_default() {
        let config = CrawlingConfig::default();
        assert_eq!(config.start_page, 1);
        assert_eq!(config.end_page, 10);
        assert_eq!(config.concurrency_limit, 5);
        assert_eq!(config.batch_size, 20);
    }

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 20);
        assert_eq!(config.concurrency_limit, 3);
        assert!(config.retry_on_failure);
    }

    #[test]
    fn test_actor_command_serialization() {
        let command = ActorCommand::StartCrawling {
            session_id: "test-session".to_string(),
            config: CrawlingConfig::default(),
        };

        let serialized = serde_json::to_string(&command).unwrap();
        let deserialized: ActorCommand = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            ActorCommand::StartCrawling { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("Unexpected command type"),
        }
    }

    #[test]
    fn test_app_event_serialization() {
        let event = AppEvent::SessionStarted {
            session_id: "test-session".to_string(),
            config: CrawlingConfig::default(),
            timestamp: Utc::now(),
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: AppEvent = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            AppEvent::SessionStarted { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_stage_result() {
        let result = StageResult {
            processed_items: 100,
            successful_items: 95,
            failed_items: 5,
            duration_ms: 60000, // 60 seconds in milliseconds
            details: vec![StageItemResult {
                item_id: "item1".to_string(),
                item_type: StageItemType::Url {
                    url_type: "test".to_string(),
                },
                success: true,
                error: None,
                duration_ms: 500,
                retry_count: 0,
                collected_data: None,
            }],
        };

        assert_eq!(result.processed_items, 100);
        assert_eq!(result.successful_items, 95);
        assert_eq!(result.failed_items, 5);
        assert_eq!(result.details.len(), 1);
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics {
            memory_usage_mb: 512.0,
            cpu_usage_percent: 25.5,
            active_tasks_count: 10,
            queued_tasks_count: 5,
            avg_response_time_ms: 150.0,
            throughput_per_second: 50.0,
        };

        assert_eq!(metrics.memory_usage_mb, 512.0);
        assert_eq!(metrics.cpu_usage_percent, 25.5);
        assert_eq!(metrics.active_tasks_count, 10);
    }

    fn get_variant_payload(v: &Value) -> &Value {
        // Externally tagged enum => { "VariantName": { ...fields } }
        let obj = v.as_object().expect("enum serializes to object");
        let (_k, inner) = obj.iter().next().expect("one variant key present");
        inner
    }

    #[test]
    fn test_event_timestamp_presence() {
        let event = AppEvent::SessionStarted {
            session_id: "s1".to_string(),
            config: CrawlingConfig::default(),
            timestamp: Utc::now(),
        };
        let v: Value = serde_json::to_value(&event).unwrap();
        let payload = get_variant_payload(&v);
        assert!(payload.get("timestamp").is_some(), "timestamp must be present on SessionStarted");
    }

    #[test]
    fn test_duration_field_names_policy_examples() {
        // BatchCompleted uses legacy `duration` (ms)
        let batch_completed = AppEvent::BatchCompleted {
            batch_id: "b1".to_string(),
            session_id: "s1".to_string(),
            success_count: 10,
            failed_count: 2,
            duration: 1234,
            timestamp: Utc::now(),
        };
    let v: Value = serde_json::to_value(&batch_completed).unwrap();
    let payload = get_variant_payload(&v);
    assert!(payload.get("duration").is_some(), "BatchCompleted must have legacy `duration` field");
    assert!(payload.get("duration_ms").is_none(), "BatchCompleted must not rename to duration_ms");

        // BatchReport uses `duration_ms`
        let batch_report = AppEvent::BatchReport {
            session_id: "s1".to_string(),
            batch_id: "b1".to_string(),
            pages_total: 5,
            pages_success: 5,
            pages_failed: 0,
            list_pages_failed: vec![],
            details_success: 20,
            details_failed: 0,
            retries_used: 1,
            duration_ms: 4321,
            duplicates_skipped: 0,
            products_inserted: 0,
            products_updated: 0,
            timestamp: Utc::now(),
        };
    let v2: Value = serde_json::to_value(&batch_report).unwrap();
    let payload2 = get_variant_payload(&v2);
    assert!(payload2.get("duration_ms").is_some(), "BatchReport must have duration_ms");

        // SyncPageCompleted uses legacy `ms`
        let sync_page_completed = AppEvent::SyncPageCompleted {
            session_id: "s1".to_string(),
            physical_page: 3,
            inserted: 1,
            updated: 2,
            skipped: 0,
            failed: 0,
            ms: 250,
            timestamp: Utc::now(),
        };
    let v3: Value = serde_json::to_value(&sync_page_completed).unwrap();
    let payload3 = get_variant_payload(&v3);
    assert!(payload3.get("ms").is_some(), "SyncPageCompleted must have legacy `ms` field");
    assert!(payload3.get("duration_ms").is_none(), "SyncPageCompleted must not rename to duration_ms");
    }

    #[test]
    fn test_stage_item_completed_has_duration_ms_and_timestamp() {
        let event = AppEvent::StageItemCompleted {
            session_id: "s1".to_string(),
            batch_id: Some("b1".to_string()),
            stage_type: StageType::ListPageCrawling,
            item_id: "p1".to_string(),
            item_type: StageItemType::Page { page_number: 1 },
            success: true,
            error: None,
            duration_ms: 100,
            retry_count: 0,
            collected_count: Some(30),
            timestamp: Utc::now(),
        };
    let v: Value = serde_json::to_value(&event).unwrap();
    let payload = get_variant_payload(&v);
    assert!(payload.get("duration_ms").is_some());
    assert!(payload.get("timestamp").is_some());
    }
}
