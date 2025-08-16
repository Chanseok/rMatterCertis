//! Task Lifecycle Event System
//!
//! 실시간 동시성 시각화를 위한 세밀한 `Task` 생명주기 이벤트 시스템
//! 각 `AsyncTask`의 모든 상태 변화를 추적하여 프론트엔드에서 완전한 투명성 제공

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::module_name_repetitions)] // TaskLifecycleEvent, TaskExecutionContext 등은 의도적인 반복

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// `Task` 실행 컨텍스트 - 모든 이벤트가 공통으로 가지는 실행 환경 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct TaskExecutionContext {
    /// 세션 식별자
    pub session_id: String,
    /// 배치 식별자
    pub batch_id: String,
    /// 스테이지 이름
    pub stage_name: String,
    /// `Task` 고유 식별자
    pub task_id: String,
    /// `Task`가 처리하는 `URL`
    pub task_url: String,
    /// `Task` 시작 시간
    pub start_time: DateTime<Utc>,
    /// Worker 식별자
    pub worker_id: Option<String>,
}

/// `Task` 우선순위
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum TaskPriority {
    /// 즉시 처리해야 하는 중요 `Task`
    Critical,
    /// 높은 우선순위
    High,
    /// 일반 우선순위
    Normal,
    /// 낮은 우선순위
    Low,
}

/// `Task` 유형 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct TaskTypeInfo {
    /// `Task` 유형 이름
    pub name: String,
    /// 예상 실행 시간 (밀리초)
    pub estimated_duration_ms: Option<u64>,
    /// 의존성 있는 다른 `Task`들
    pub dependencies: Vec<String>,
}

/// `Task`의 생명주기를 나타내는 세밀한 이벤트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
#[serde(tag = "status", content = "details")]
pub enum TaskLifecycleEvent {
    /// `Task`가 생성되고 대기열에 추가됨
    Created {
        url: String,
        task_type: TaskTypeInfo,
        priority: TaskPriority,
        estimated_completion: DateTime<Utc>,
    },

    /// `Task`가 실행 대기열에서 대기 중
    Queued {
        queue_position: u32,
        estimated_start_time: DateTime<Utc>,
        queue_length: u32,
    },

    /// `Task` 실행이 시작됨
    Started {
        worker_id: String,
        retry_attempt: u32,
        allocated_resources: ResourceAllocation,
    },

    /// `Task` 실행 중 진행 상황 업데이트
    Progress {
        stage: String,
        completion_percent: f64,
        current_operation: String,
        items_processed: u32,
        items_total: Option<u32>,
        throughput_per_second: Option<f64>,
    },

    /// `Task`가 성공적으로 완료됨
    Succeeded {
        duration_ms: u64,
        result_summary: String,
        items_processed: u32,
        final_throughput: f64,
        resource_usage: ResourceUsage,
    },

    /// `Task`가 실패함
    Failed {
        error_message: String,
        error_code: String,
        error_category: ErrorCategory,
        is_recoverable: bool,
        stack_trace: Option<String>,
        resource_usage: ResourceUsage,
    },

    /// `Task` 재시도 중
    Retrying {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        reason: String,
        retry_strategy: RetryStrategy,
    },

    /// `Task`가 취소됨
    Cancelled {
        reason: String,
        partial_results: Option<String>,
        completion_percent: f64,
    },

    /// `Task`가 타임아웃됨
    TimedOut {
        timeout_duration_ms: u64,
        partial_results: Option<String>,
        completion_percent: f64,
    },
}

/// 리소스 할당 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ResourceAllocation {
    /// 할당된 메모리 (바이트)
    pub memory_bytes: u64,
    /// 할당된 CPU 시간 (퍼센트)
    pub cpu_percent: f64,
    /// 네트워크 대역폭 제한
    pub network_bandwidth_kbps: Option<u64>,
}

/// 리소스 사용량 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ResourceUsage {
    /// 최대 메모리 사용량 (바이트)
    pub peak_memory_bytes: u64,
    /// 평균 CPU 사용률
    pub avg_cpu_percent: f64,
    /// 총 네트워크 사용량 (바이트)
    pub total_network_bytes: u64,
    /// 디스크 I/O 작업 횟수
    pub disk_io_operations: u64,
}

/// 에러 카테고리
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum ErrorCategory {
    /// 네트워크 관련 오류
    Network,
    /// 파싱 관련 오류
    Parsing,
    /// 데이터베이스 관련 오류
    Database,
    /// 시스템 리소스 관련 오류
    Resource,
    /// 비즈니스 로직 오류
    Business,
    /// 알 수 없는 오류
    Unknown,
}

/// 재시도 전략
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum RetryStrategy {
    /// 고정 간격 재시도
    FixedDelay { interval_ms: u64 },
    /// 지수 백오프
    ExponentialBackoff { base_ms: u64, multiplier: f64 },
    /// 선형 증가
    LinearBackoff { initial_ms: u64, increment_ms: u64 },
    /// 사용자 정의 전략
    Custom { strategy_name: String },
}

/// 동시성 상태의 실시간 스냅샷
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ConcurrencySnapshot {
    /// 스냅샷 생성 시간
    pub timestamp: DateTime<Utc>,
    /// 현재 활성 `Task`들
    pub active_tasks: HashMap<String, TaskState>,
    /// 지난 1초간 완료된 `Task` 수
    pub completed_in_last_second: u32,
    /// 지난 1초간 실패한 `Task` 수
    pub failed_in_last_second: u32,
    /// 재시도 대기열 길이
    pub retry_queue_length: u32,
    /// 전체 처리량 (초당 `Task` 수)
    pub overall_throughput: f64,
    /// 평균 응답 시간 (밀리초)
    pub avg_response_time_ms: f64,
    /// 에러율 (0.0 ~ 1.0)
    pub error_rate: f64,
}

/// `Task` 상태 요약
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct TaskState {
    /// `Task` 식별자
    pub task_id: String,
    /// 현재 상태
    pub current_status: String,
    /// 진행률 (0.0 ~ 1.0)
    pub progress: f64,
    /// 시작 시간
    pub start_time: DateTime<Utc>,
    /// 마지막 업데이트 시간
    pub last_update: DateTime<Utc>,
    /// 에러 정보 (있는 경우)
    pub error_info: Option<String>,
}

/// 동시성 인텔리전스 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ConcurrencyInsight {
    /// 감지된 병목점들
    pub detected_bottlenecks: Vec<BottleneckAlert>,
    /// 최적화 기회들
    pub optimization_opportunities: Vec<OptimizationHint>,
    /// 예측된 완료 시간
    pub predicted_completion_time: DateTime<Utc>,
    /// 리소스 사용률 예측
    pub resource_utilization_forecast: ResourceForecast,
    /// 성능 트렌드
    pub performance_trend: PerformanceTrend,
}

/// 병목점 알림
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct BottleneckAlert {
    /// 병목점 유형
    pub bottleneck_type: BottleneckType,
    /// 심각도 (1-10)
    pub severity: u8,
    /// 영향받는 `Task` 수
    pub affected_task_count: u32,
    /// 설명
    pub description: String,
    /// 권장 조치
    pub recommended_action: String,
}

/// 병목점 유형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum BottleneckType {
    /// `CPU` 병목
    CpuBottleneck,
    /// 메모리 병목
    MemoryBottleneck,
    /// 네트워크 병목
    NetworkBottleneck,
    /// 데이터베이스 병목
    DatabaseBottleneck,
    /// 큐 포화
    QueueSaturation,
}

/// 최적화 힌트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct OptimizationHint {
    /// 최적화 유형
    pub optimization_type: OptimizationType,
    /// 예상 성능 향상 (퍼센트)
    pub expected_improvement_percent: f64,
    /// 구현 난이도 (1-10)
    pub implementation_difficulty: u8,
    /// 설명
    pub description: String,
    /// 구현 방법
    pub implementation_steps: Vec<String>,
}

/// 최적화 유형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum OptimizationType {
    /// 동시성 수준 조정
    ConcurrencyAdjustment,
    /// 배치 크기 최적화
    BatchSizeOptimization,
    /// 재시도 전략 개선
    RetryStrategyImprovement,
    /// 리소스 할당 최적화
    ResourceAllocationOptimization,
    /// 큐 관리 개선
    QueueManagementImprovement,
}

/// 리소스 사용률 예측
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ResourceForecast {
    /// 예측 시간 범위 (분)
    pub forecast_minutes: u32,
    /// CPU 사용률 예측
    pub cpu_usage_forecast: Vec<f64>,
    /// 메모리 사용량 예측
    pub memory_usage_forecast: Vec<u64>,
    /// 네트워크 사용량 예측
    pub network_usage_forecast: Vec<u64>,
}

/// 성능 트렌드
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct PerformanceTrend {
    /// 처리량 트렌드 (증가/감소/안정)
    pub throughput_trend: TrendDirection,
    /// 응답시간 트렌드
    pub response_time_trend: TrendDirection,
    /// 에러율 트렌드
    pub error_rate_trend: TrendDirection,
    /// 트렌드 신뢰도 (0.0 ~ 1.0)
    pub confidence: f64,
}

/// 트렌드 방향
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum TrendDirection {
    /// 증가 추세
    Increasing,
    /// 감소 추세
    Decreasing,
    /// 안정 상태
    Stable,
    /// 변동이 심함
    Volatile,
}

/// 최고 수준의 이벤트 - 모든 동시성 정보를 통합
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
#[serde(tag = "type", content = "payload")]
pub enum ConcurrencyEvent {
    /// 개별 `Task`의 생명주기 이벤트
    TaskLifecycle {
        context: TaskExecutionContext,
        event: TaskLifecycleEvent,
    },

    /// 동시성 상태 스냅샷 (1초마다)
    ConcurrencySnapshot(ConcurrencySnapshot),

    /// `AI` 기반 동시성 분석 결과
    ConcurrencyInsight(ConcurrencyInsight),

    /// 세션 레벨 이벤트 (크롤링 버튼 클릭부터 완료까지)
    SessionEvent {
        session_id: String,
        event_type: SessionEventType,
        metadata: HashMap<String, String>,
        timestamp: DateTime<Utc>,
    },

    /// 배치 레벨 이벤트 (배치 생성, 시작, 완료)
    BatchEvent {
        session_id: String,
        batch_id: String,
        event_type: BatchEventType,
        metadata: HashMap<String, String>,
        timestamp: DateTime<Utc>,
    },

    /// 스테이지별 작업 이벤트 (ProductList, ProductDetails 구분)
    StageEvent {
        session_id: String,
        batch_id: Option<String>,
        stage_type: StageType,
        event_type: TaskLifecycleEvent,
        metadata: HashMap<String, String>,
        timestamp: DateTime<Utc>,
    },
}

/// 크롤링 세션 계획 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct CrawlingSessionPlan {
    /// 세션 ID
    pub session_id: String,
    /// 총 페이지 수
    pub total_pages: u32,
    /// 생성될 배치 수
    pub total_batches: u32,
    /// 예상 완료 시간
    pub estimated_completion_time: DateTime<Utc>,
    /// 캐시된 사이트 상태
    pub cached_site_status: Option<SiteStatusInfo>,
    /// 배치별 페이지 분할 계획
    pub batch_plans: Vec<BatchPlan>,
}

/// 배치 계획 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct BatchPlan {
    /// 배치 ID
    pub batch_id: String,
    /// 시작 페이지
    pub start_page: u32,
    /// 끝 페이지
    pub end_page: u32,
    /// 예상 제품 수
    pub estimated_products: u32,
    /// 우선순위
    pub priority: TaskPriority,
}

/// 사이트 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct SiteStatusInfo {
    /// 사이트 접근 가능 여부
    pub is_accessible: bool,
    /// 응답 시간 (밀리초)
    pub response_time_ms: u64,
    /// 마지막 확인 시간
    pub last_checked: DateTime<Utc>,
    /// 총 페이지 수 (캐시된 값)
    pub total_pages: Option<u32>,
    /// 캐시 유효성
    pub cache_valid: bool,
}

/// 세션 이벤트 유형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum SessionEventType {
    /// 세션 시작 - 크롤링 버튼 클릭 시점
    Started,
    /// 캐시 확인 및 사이트 상태 체크
    SiteStatusCheck,
    /// 배치 계획 수립 (총 페이지 수, 배치 수 계산)
    BatchPlanning,
    /// 세션 완료
    Completed,
    /// 세션 실패
    Failed,
    /// 세션 취소
    Cancelled,
    /// 세션 일시정지
    Paused,
    /// 세션 재개
    Resumed,
}

/// 배치 이벤트 유형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum BatchEventType {
    /// 배치 생성 (계획 단계)
    Created,
    /// 배치 시작 (실제 실행 시작)
    Started,
    /// 배치 진행 중 (중간 업데이트)
    InProgress,
    /// 배치 완료
    Completed,
    /// 배치 실패
    Failed,
    /// 배치 재시도
    Retrying,
}

/// 스테이지별 작업 유형 (ProductList, ProductDetails 구분)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub enum StageType {
    /// 사이트 상태 확인
    SiteStatusCheck,
    /// 데이터베이스 분석
    DatabaseAnalysis,
    /// 제품 목록 수집 (페이지별 병렬 실행)
    ProductList { page_number: u32, batch_id: String },
    /// 제품 상세정보 수집 (제품별 병렬 실행)
    ProductDetails {
        product_id: String,
        batch_id: String,
    },
    /// 데이터베이스 저장
    DatabaseSave,
}
