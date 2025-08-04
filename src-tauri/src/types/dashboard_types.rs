//! 실시간 크롤링 대시보드를 위한 데이터 타입
//! Phase C - Option A: UI 대시보드와 Backend 연동

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use chrono::{DateTime, Utc};

/// 실시간 대시보드 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardState {
    /// 현재 활성 세션
    pub active_session: Option<ActiveCrawlingSession>,
    /// 최근 완료된 세션들 (최대 5개)
    pub recent_sessions: Vec<CompletedSession>,
    /// 실시간 성능 메트릭
    pub performance_metrics: Option<RealtimePerformanceMetrics>,
    /// 시스템 상태
    pub system_status: SystemStatus,
    /// 대시보드 업데이트 시간
    pub last_updated: DateTime<Utc>,
}

/// 활성 크롤링 세션
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActiveCrawlingSession {
    /// 세션 ID
    pub session_id: String,
    /// 시작 시간
    pub started_at: DateTime<Utc>,
    /// 현재 단계
    pub current_stage: String,
    /// 전체 진행률 (0-100)
    pub overall_progress: f64,
    /// 단계별 진행률 (0-100)
    pub stage_progress: f64,
    /// 처리된 페이지 수
    pub processed_pages: u32,
    /// 전체 페이지 수
    pub total_pages: u32,
    /// 수집된 URL 수
    pub collected_urls: u32,
    /// 현재 처리 속도 (pages/min)
    pub current_speed_ppm: f64,
    /// 예상 완료 시간
    pub estimated_completion: Option<DateTime<Utc>>,
    /// 현재 상태 메시지
    pub status_message: String,
    /// 에러 수
    pub error_count: u32,
    /// 마지막 에러 메시지
    pub last_error: Option<String>,
}

/// 완료된 세션 요약
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CompletedSession {
    /// 세션 ID
    pub session_id: String,
    /// 시작 시간
    pub started_at: DateTime<Utc>,
    /// 완료 시간
    pub completed_at: DateTime<Utc>,
    /// 성공 여부
    pub success: bool,
    /// 처리된 페이지 수
    pub processed_pages: u32,
    /// 수집된 URL 수
    pub collected_urls: u32,
    /// 총 소요 시간 (초)
    pub duration_seconds: u64,
    /// 평균 처리 속도 (pages/min)
    pub avg_speed_ppm: f64,
    /// 에러 수
    pub error_count: u32,
}

/// 실시간 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealtimePerformanceMetrics {
    /// CPU 사용률 (%)
    pub cpu_usage_percent: f64,
    /// 메모리 사용량 (MB)
    pub memory_usage_mb: f64,
    /// 네트워크 처리량 (KB/s)
    pub network_throughput_kbps: f64,
    /// 평균 응답 시간 (ms)
    pub avg_response_time_ms: f64,
    /// 성공률 (%)
    pub success_rate_percent: f64,
    /// 동시 연결 수
    pub concurrent_connections: u32,
    /// 큐 대기 중인 작업 수
    pub pending_tasks: u32,
    /// 최근 처리 속도 (requests/sec)
    pub recent_rps: f64,
}

/// 시스템 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStatus {
    /// 서버 상태
    pub server_status: ServerStatus,
    /// 데이터베이스 상태
    pub database_status: DatabaseStatus,
    /// 크롤링 대상 사이트 상태
    pub site_status: SiteStatus,
    /// 마지막 상태 확인 시간
    pub last_health_check: DateTime<Utc>,
}

/// 서버 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ServerStatus {
    /// 정상 동작
    Healthy,
    /// 부분적 문제
    Degraded { issues: Vec<String> },
    /// 심각한 문제
    Critical { error: String },
    /// 서버 다운
    Down,
}

/// 데이터베이스 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DatabaseStatus {
    /// 연결 상태
    pub connected: bool,
    /// 전체 제품 수
    pub total_products: u64,
    /// 오늘 수집된 제품 수
    pub products_today: u64,
    /// 데이터베이스 크기 (MB)
    pub size_mb: f64,
    /// 마지막 업데이트 시간
    pub last_update: Option<DateTime<Utc>>,
}

/// 크롤링 대상 사이트 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteStatus {
    /// 사이트 접근 가능 여부
    pub accessible: bool,
    /// 응답 시간 (ms)
    pub response_time_ms: u64,
    /// 전체 페이지 수
    pub total_pages: u32,
    /// 예상 제품 수
    pub estimated_products: u32,
    /// 사이트 건강 점수 (0-100)
    pub health_score: u32,
    /// 마지막 확인 시간
    pub last_checked: DateTime<Utc>,
}

/// 차트 데이터 포인트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChartDataPoint {
    /// 시간 (Unix timestamp)
    pub timestamp: i64,
    /// 값
    pub value: f64,
    /// 레이블 (선택적)
    pub label: Option<String>,
}

/// 실시간 차트 데이터
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealtimeChartData {
    /// 처리 속도 차트 (pages/min)
    pub processing_speed: Vec<ChartDataPoint>,
    /// 응답 시간 차트 (ms)
    pub response_time: Vec<ChartDataPoint>,
    /// 성공률 차트 (%)
    pub success_rate: Vec<ChartDataPoint>,
    /// 메모리 사용량 차트 (MB)
    pub memory_usage: Vec<ChartDataPoint>,
    /// 동시 연결 수 차트
    pub concurrent_connections: Vec<ChartDataPoint>,
}

/// 크롤링 단계별 통계
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageStatistics {
    /// 단계별 소요 시간 (ms)
    pub stage_durations: HashMap<String, u64>,
    /// 단계별 성공률 (%)
    pub stage_success_rates: HashMap<String, f64>,
    /// 단계별 처리량
    pub stage_throughput: HashMap<String, f64>,
    /// 단계별 에러 수
    pub stage_error_counts: HashMap<String, u32>,
}

/// 대시보드 설정
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardConfig {
    /// 업데이트 간격 (ms)
    pub update_interval_ms: u64,
    /// 차트 데이터 포인트 최대 개수
    pub max_chart_points: u32,
    /// 성능 알림 임계값
    pub performance_thresholds: PerformanceThresholds,
    /// 표시할 최근 세션 수
    pub max_recent_sessions: u32,
}

/// 성능 알림 임계값
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerformanceThresholds {
    /// 응답 시간 경고 임계값 (ms)
    pub response_time_warning_ms: u64,
    /// 응답 시간 위험 임계값 (ms)
    pub response_time_critical_ms: u64,
    /// 성공률 경고 임계값 (%)
    pub success_rate_warning_percent: f64,
    /// 메모리 사용량 경고 임계값 (MB)
    pub memory_warning_mb: f64,
    /// CPU 사용률 경고 임계값 (%)
    pub cpu_warning_percent: f64,
}

/// 대시보드 알림
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardAlert {
    /// 알림 ID
    pub id: String,
    /// 알림 레벨
    pub level: AlertLevel,
    /// 알림 제목
    pub title: String,
    /// 알림 메시지
    pub message: String,
    /// 발생 시간
    pub timestamp: DateTime<Utc>,
    /// 관련 세션 ID
    pub session_id: Option<String>,
    /// 자동 해결 여부
    pub auto_resolve: bool,
}

/// 알림 레벨
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlertLevel {
    /// 정보
    Info,
    /// 경고
    Warning,
    /// 에러
    Error,
    /// 심각
    Critical,
}

/// 대시보드 이벤트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DashboardEvent {
    /// 세션 시작
    SessionStarted { session: ActiveCrawlingSession },
    /// 진행 상황 업데이트
    ProgressUpdate { session_id: String, progress: f64, stage_progress: f64 },
    /// 성능 메트릭 업데이트
    PerformanceUpdate { metrics: RealtimePerformanceMetrics },
    /// 세션 완료
    SessionCompleted { session: CompletedSession },
    /// 시스템 상태 변경
    SystemStatusChange { status: SystemStatus },
    /// 새 알림
    NewAlert { alert: DashboardAlert },
    /// 차트 데이터 업데이트
    ChartDataUpdate { data: RealtimeChartData },
}
