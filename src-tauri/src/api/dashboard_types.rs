//! 실시간 크롤링 대시보드를 위한 데이터 타입
//! Phase C - Option A: UI 대시보드 타입

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardState {
    pub active_session: Option<ActiveCrawlingSession>,
    pub recent_sessions: Vec<CompletedSession>,
    pub performance_metrics: Option<RealtimePerformanceMetrics>,
    pub system_status: SystemStatus,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActiveCrawlingSession {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub current_stage: String,
    pub overall_progress: f64,
    pub stage_progress: f64,
    pub processed_pages: u32,
    pub total_pages: u32,
    pub collected_urls: u32,
    pub current_speed_ppm: f64,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub status_message: String,
    pub error_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CompletedSession {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub success: bool,
    pub processed_pages: u32,
    pub collected_urls: u32,
    pub duration_seconds: u64,
    pub avg_speed_ppm: f64,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealtimePerformanceMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub network_throughput_kbps: f64,
    pub avg_response_time_ms: f64,
    pub success_rate_percent: f64,
    pub concurrent_connections: u32,
    pub pending_tasks: u32,
    pub recent_rps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStatus {
    pub server_status: ServerStatus,
    pub database_status: DatabaseStatus,
    pub site_status: SiteStatus,
    pub last_health_check: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ServerStatus {
    Healthy,
    Degraded { issues: Vec<String> },
    Critical { error: String },
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DatabaseStatus {
    pub connected: bool,
    pub total_products: u64,
    pub products_today: u64,
    pub size_mb: f64,
    pub last_update: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteStatus {
    pub accessible: bool,
    pub response_time_ms: u64,
    pub total_pages: u32,
    pub estimated_products: u32,
    pub health_score: u32,
    pub last_checked: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChartDataPoint {
    pub timestamp: i64,
    pub value: f64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealtimeChartData {
    pub processing_speed: Vec<ChartDataPoint>,
    pub response_time: Vec<ChartDataPoint>,
    pub success_rate: Vec<ChartDataPoint>,
    pub memory_usage: Vec<ChartDataPoint>,
    pub cpu_usage: Vec<ChartDataPoint>,
    pub pages_processed: Vec<ChartDataPoint>,
    pub products_collected: Vec<ChartDataPoint>,
    pub concurrent_connections: Vec<ChartDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageStatistics {
    pub stage_durations: HashMap<String, u64>,
    pub stage_success_rates: HashMap<String, f64>,
    pub stage_throughput: HashMap<String, f64>,
    pub stage_error_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardConfig {
    pub update_interval_ms: u64,
    pub max_chart_points: u32,
    pub performance_thresholds: PerformanceThresholds,
    pub max_recent_sessions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerformanceThresholds {
    pub response_time_warning_ms: u64,
    pub response_time_critical_ms: u64,
    pub success_rate_warning_percent: f64,
    pub memory_warning_mb: f64,
    pub cpu_warning_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardAlert {
    pub id: String,
    pub level: AlertLevel,
    pub title: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub auto_resolve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DashboardEvent {
    SessionStarted {
        session: ActiveCrawlingSession,
    },
    ProgressUpdate {
        session_id: String,
        progress: f64,
        stage_progress: f64,
    },
    PerformanceUpdate {
        metrics: RealtimePerformanceMetrics,
    },
    SessionCompleted {
        session: CompletedSession,
    },
    SystemStatusChange {
        status: SystemStatus,
    },
    NewAlert {
        alert: DashboardAlert,
    },
    ChartDataUpdate {
        data: RealtimeChartData,
    },
}
