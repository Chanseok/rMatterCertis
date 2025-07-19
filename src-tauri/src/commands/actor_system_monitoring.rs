//! Actor System 모니터링을 위한 Tauri 명령어들
//! Phase C: UI 개선 - OneShot Actor 시스템 상태 조회 API

use crate::new_architecture::actor_system::*;
use crate::new_architecture::services::crawling_integration::CrawlingIntegrationService;
// use crate::commands::crawling_session_manager::{CrawlingSessionManager, CrawlingSessionStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::State;
use tracing::{info, warn, error};
use std::sync::Mutex;
use ts_rs::TS;

// 전역 크롤링 상태 관리
static CRAWLING_SESSION_STATE: std::sync::OnceLock<Mutex<CrawlingSessionState>> = std::sync::OnceLock::new();

#[derive(Debug, Clone)]
struct CrawlingSessionState {
    session_id: Option<String>,
    status: String,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    last_update: chrono::DateTime<chrono::Utc>,
    total_items: u32,
    processed_items: u32,
}

impl Default for CrawlingSessionState {
    fn default() -> Self {
        Self {
            session_id: None,
            status: "idle".to_string(),
            start_time: None,
            last_update: chrono::Utc::now(),
            total_items: 100, // Mock data for demo
            processed_items: 0,
        }
    }
}

fn get_session_state() -> &'static Mutex<CrawlingSessionState> {
    CRAWLING_SESSION_STATE.get_or_init(|| {
        Mutex::new(CrawlingSessionState::default())
    })
}

// Actor 시스템 상태 타입 정의 (UI와 동일)
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ActorSystemStatus {
    pub session_actor: SessionActorStatus,
    pub batch_actors: Vec<BatchActorStatus>,
    pub stage_actors: Vec<StageActorStatus>,
    pub channel_status: ChannelStatus,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct SessionActorStatus {
    pub id: String,
    pub status: String, // 'idle' | 'running' | 'completed' | 'error'
    pub active_batches: u32,
    pub total_processed: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct BatchActorStatus {
    pub id: String,
    pub status: String, // 'idle' | 'processing' | 'waiting' | 'completed' | 'error'
    pub current_stage: Option<String>,
    pub processed_items: u32,
    pub success_rate: f64,
    pub error_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct StageActorStatus {
    pub id: String,
    pub stage_type: String,
    pub status: String, // 'idle' | 'executing' | 'completed' | 'error'
    pub current_batch_size: u32,
    pub avg_processing_time_ms: u64,
    pub total_executions: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct ChannelStatus {
    pub control_channel_pending: u32,
    pub event_channel_pending: u32,
    pub oneshot_channels_active: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct PerformanceMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub throughput_items_per_second: f64,
    pub error_rate_percent: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct SystemHealth {
    pub overall_status: String, // 'healthy' | 'warning' | 'critical' | 'offline'
    pub health_score: u32, // 0-100
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

// 크롤링 진행률 타입 정의
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct CrawlingProgress {
    pub session_id: String,
    pub status: String, // 'preparing' | 'running' | 'paused' | 'completed' | 'error' | 'cancelled'
    pub overall_progress: OverallProgress,
    pub stage_progress: Vec<StageProgress>,
    pub performance_stats: CrawlingPerformanceStats,
    pub recent_events: Vec<CrawlingEvent>,
    pub time_stats: TimeStats,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct OverallProgress {
    pub total_items: u32,
    pub processed_items: u32,
    pub success_items: u32,
    pub failed_items: u32,
    pub progress_percentage: f64,
    pub estimated_remaining_time_secs: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct StageProgress {
    pub stage_type: String,
    pub status: String, // 'pending' | 'running' | 'completed' | 'error'
    pub processed_items: u32,
    pub total_items: u32,
    pub success_rate: f64,
    pub avg_processing_time_ms: u64,
    pub current_batch_size: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct CrawlingPerformanceStats {
    pub items_per_second: f64,
    pub memory_usage_mb: f64,
    pub active_connections: u32,
    pub error_rate_percent: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct CrawlingEvent {
    pub timestamp: String,
    pub stage: String,
    pub event_type: String, // 'started' | 'completed' | 'error' | 'retry'
    pub message: String,
    #[ts(skip)]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct TimeStats {
    pub start_time: String,
    pub elapsed_time_secs: u64,
    pub estimated_total_time_secs: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrawlingConfig {
    pub target_pages: Vec<u32>,
    pub concurrency_limit: u32,
    pub batch_size: u32,
    pub retry_attempts: u32,
    pub timeout_secs: u32,
}

/// Actor 시스템 상태 조회
#[tauri::command]
pub async fn get_actor_system_status() -> Result<ActorSystemStatus, String> {
    info!("🎭 Actor system status requested");
    
    // TODO: 실제 Actor 시스템에서 상태 조회
    // 현재는 모킹 데이터 반환
    
    let mock_status = ActorSystemStatus {
        session_actor: SessionActorStatus {
            id: "session-001".to_string(),
            status: "idle".to_string(),
            active_batches: 0,
            total_processed: 348,
            uptime_seconds: 3600,
        },
        batch_actors: vec![
            BatchActorStatus {
                id: "batch-001".to_string(),
                status: "idle".to_string(),
                current_stage: None,
                processed_items: 0,
                success_rate: 100.0,
                error_count: 0,
            }
        ],
        stage_actors: vec![
            StageActorStatus {
                id: "stage-list-collection".to_string(),
                stage_type: "ListCollection".to_string(),
                status: "idle".to_string(),
                current_batch_size: 0,
                avg_processing_time_ms: 250,
                total_executions: 15,
            },
            StageActorStatus {
                id: "stage-detail-collection".to_string(),
                stage_type: "DetailCollection".to_string(),
                status: "idle".to_string(),
                current_batch_size: 0,
                avg_processing_time_ms: 450,
                total_executions: 8,
            }
        ],
        channel_status: ChannelStatus {
            control_channel_pending: 0,
            event_channel_pending: 0,
            oneshot_channels_active: 0,
        },
        performance_metrics: PerformanceMetrics {
            memory_usage_mb: 45.2,
            cpu_usage_percent: 2.1,
            throughput_items_per_second: 0.0,
            error_rate_percent: 0.0,
        },
    };
    
    Ok(mock_status)
}

/// 시스템 건강 상태 조회 (Actor 시스템용)
#[tauri::command]
pub async fn get_actor_system_health() -> Result<SystemHealth, String> {
    info!("🏥 System health check requested");
    
    // TODO: 실제 시스템 건강 상태 분석
    // 현재는 모킹 데이터 반환
    
    let mock_health = SystemHealth {
        overall_status: "healthy".to_string(),
        health_score: 85,
        issues: vec![],
        recommendations: vec![
            "메모리 사용량이 안정적입니다.".to_string(),
            "모든 Actor가 정상 상태입니다.".to_string(),
        ],
    };
    
    Ok(mock_health)
}

/// 크롤링 진행률 조회 (Actor 시스템용)
#[tauri::command]
pub async fn get_actor_crawling_progress() -> Result<CrawlingProgress, String> {
    info!("📊 Crawling progress requested");
    
    // 실제 상태 조회
    let (session_id, status, start_time, total_items, processed_items) = 
        if let Ok(state) = get_session_state().lock() {
            (
                state.session_id.clone().unwrap_or_else(|| "no-session".to_string()),
                state.status.clone(),
                state.start_time,
                state.total_items,
                state.processed_items,
            )
        } else {
            ("no-session".to_string(), "idle".to_string(), None, 0, 0)
        };
    
    // 진행률 계산
    let progress_percentage = if total_items > 0 {
        (processed_items as f64 / total_items as f64) * 100.0
    } else {
        0.0
    };
    
    // 경과 시간 계산
    let elapsed_time_secs = if let Some(start) = start_time {
        (chrono::Utc::now() - start).num_seconds() as u64
    } else {
        0
    };
    
    // Mock implementation with dynamic state
    let mock_progress = CrawlingProgress {
        session_id: session_id.clone(),
        status: status.clone(),
        overall_progress: OverallProgress {
            total_items,
            processed_items,
            success_items: processed_items,
            failed_items: 0,
            progress_percentage,
            estimated_remaining_time_secs: if processed_items > 0 && processed_items < total_items {
                ((total_items - processed_items) as f64 * elapsed_time_secs as f64 / processed_items as f64) as u64
            } else {
                0
            },
        },
        stage_progress: vec![
            StageProgress {
                stage_type: "ListCollection".to_string(),
                status: if status == "running" { "running".to_string() } else { "pending".to_string() },
                processed_items,
                total_items,
                success_rate: if processed_items > 0 { 100.0 } else { 0.0 },
                avg_processing_time_ms: 1500,
                current_batch_size: 10,
            }
        ],
        performance_stats: CrawlingPerformanceStats {
            items_per_second: if elapsed_time_secs > 0 { processed_items as f64 / elapsed_time_secs as f64 } else { 0.0 },
            memory_usage_mb: 45.2,
            active_connections: if status == "running" { 3 } else { 0 },
            error_rate_percent: 0.0,
        },
        recent_events: vec![
            CrawlingEvent {
                timestamp: chrono::Utc::now().to_rfc3339(),
                stage: "ListCollection".to_string(),
                event_type: "started".to_string(),
                message: "크롤링 세션이 시작되었습니다".to_string(),
                details: None,
            }
        ],
        time_stats: TimeStats {
            start_time: start_time.map(|t| t.to_rfc3339()).unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            elapsed_time_secs,
            estimated_total_time_secs: if processed_items > 0 && processed_items < total_items {
                (total_items as f64 * elapsed_time_secs as f64 / processed_items as f64) as u64
            } else {
                0
            },
        },
    };
    
    Ok(mock_progress)
}

/// 크롤링 구성 조회 (Actor 시스템용)
#[tauri::command]
pub async fn get_actor_crawling_config() -> Result<CrawlingConfig, String> {
    info!("⚙️ Crawling config requested");
    
    // TODO: 실제 크롤링 구성 조회
    // 현재는 모킹 데이터 반환
    
    let mock_config = CrawlingConfig {
        target_pages: vec![1, 2, 3, 4, 5],
        concurrency_limit: 3,
        batch_size: 10,
        retry_attempts: 3,
        timeout_secs: 30,
    };
    
    Ok(mock_config)
}

/// 크롤링 세션 시작
#[tauri::command]
pub async fn start_crawling_session() -> Result<String, String> {
    info!("🚀 Starting actor-based crawling session");
    
    // 상태 업데이트
    let session_id = format!("session_{}", chrono::Utc::now().timestamp());
    
    if let Ok(mut state) = get_session_state().lock() {
        state.session_id = Some(session_id.clone());
        state.status = "running".to_string();
        state.start_time = Some(chrono::Utc::now());
        state.processed_items = 0;
        state.total_items = 100; // Mock data
    }
    
    // 백그라운드에서 진행률 시뮬레이션
    tokio::spawn(async {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            let should_continue = if let Ok(mut state) = get_session_state().lock() {
                if state.status == "running" && state.processed_items < state.total_items {
                    state.processed_items += 1;
                    
                    // 완료 체크
                    if state.processed_items >= state.total_items {
                        state.status = "completed".to_string();
                        info!("✅ Crawling session completed");
                        false
                    } else {
                        true
                    }
                } else {
                    false
                }
            } else {
                false
            };
            
            if !should_continue {
                break;
            }
        }
    });
    
    info!("✅ Crawling session started: {}", session_id);
    Ok(session_id)
}

/// 크롤링 세션 일시정지
#[tauri::command]
pub async fn pause_crawling_session() -> Result<String, String> {
    info!("⏸️ Pausing actor-based crawling session");
    
    // Mock implementation
    Ok("Session paused".to_string())
}

/// 크롤링 세션 재개
#[tauri::command]
pub async fn resume_crawling_session() -> Result<String, String> {
    info!("▶️ Resuming actor-based crawling session");
    
    // Mock implementation
    Ok("Session resumed".to_string())
}

/// 크롤링 세션 중단
#[tauri::command]
pub async fn stop_crawling_session() -> Result<String, String> {
    info!("⏹️ Stopping actor-based crawling session");
    
    // 상태 업데이트 및 실제 취소 처리
    if let Ok(mut state) = get_session_state().lock() {
        // 상태가 실행 중인 경우에만 취소
        if state.status == "running" {
            state.status = "cancelling".to_string();
            state.last_update = chrono::Utc::now();
            
            // 실제 크롤링 작업 취소를 위한 플래그 설정
            // TODO: 실제 취소 로직 연결 (백그라운드 태스크 중단 등)
            
            // 취소 완료 상태로 변경
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if let Ok(mut state) = get_session_state().lock() {
                    state.status = "cancelled".to_string();
                    state.last_update = chrono::Utc::now();
                }
            });
            
            info!("✅ Crawling session cancellation initiated");
            Ok("Session cancellation initiated".to_string())
        } else {
            info!("⚠️ No active session to cancel");
            Ok("No active session to cancel".to_string())
        }
    } else {
        Err("Failed to access session state".to_string())
    }
}
