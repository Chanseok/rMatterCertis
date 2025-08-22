//! 성능 최적화 관련 Tauri Commands
//! Phase C: 실시간 성능 모니터링 및 최적화 제어

use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;
use tracing::info;

use crate::crawl_engine::config::SystemConfig;
use crate::crawl_engine::services::performance_optimizer::{
    CrawlingPerformanceMetrics, CrawlingPerformanceOptimizer, OptimizationRecommendation,
};

/// 성능 최적화 상태 관리
pub struct PerformanceOptimizerState {
    pub optimizer: Arc<RwLock<Option<Arc<CrawlingPerformanceOptimizer>>>>,
}

impl Default for PerformanceOptimizerState {
    fn default() -> Self {
        Self {
            optimizer: Arc::new(RwLock::new(None)),
        }
    }
}

/// 🔧 성능 최적화 서비스 초기화
#[tauri::command]
pub async fn init_performance_optimizer(app: AppHandle) -> Result<String, String> {
    info!("🔧 Initializing performance optimizer");

    // SystemConfig 생성
    let system_config = Arc::new(SystemConfig::default());

    // 성능 최적화 서비스 생성
    let optimizer = Arc::new(CrawlingPerformanceOptimizer::new(system_config));

    // 상태에 저장
    let optimizer_state = app.state::<PerformanceOptimizerState>();
    let mut optimizer_lock = optimizer_state.optimizer.write().await;
    *optimizer_lock = Some(optimizer);

    info!("✅ Performance optimizer initialized successfully");
    Ok("Performance optimizer initialized".to_string())
}

/// 📊 현재 성능 메트릭 조회
#[tauri::command]
pub async fn get_current_performance_metrics(
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<Option<CrawlingPerformanceMetrics>, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        Ok(optimizer.get_current_metrics().await)
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// 💡 최적화 추천사항 조회
#[tauri::command]
pub async fn get_optimization_recommendation(
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<Option<OptimizationRecommendation>, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        Ok(optimizer.get_optimization_recommendation().await)
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// 📈 성능 히스토리 조회
#[tauri::command]
pub async fn get_performance_history(
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<Vec<CrawlingPerformanceMetrics>, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        Ok(optimizer.get_performance_history().await)
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// 🧹 성능 히스토리 초기화
#[tauri::command]
pub async fn clear_performance_history(
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<String, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        optimizer.clear_performance_history().await;
        Ok("Performance history cleared".to_string())
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// 🔄 성능 최적화 세션 시작
#[tauri::command]
pub async fn start_performance_session(
    session_id: String,
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<String, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        optimizer.start_session(session_id.clone()).await;
        info!(session_id = %session_id, "🔄 Performance optimization session started");
        Ok(format!("Performance session started: {}", session_id))
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// ⏹️ 성능 최적화 세션 종료
#[tauri::command]
pub async fn end_performance_session(
    optimizer_state: State<'_, PerformanceOptimizerState>,
) -> Result<String, String> {
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        optimizer.end_session().await;
        info!("⏹️ Performance optimization session ended");
        Ok("Performance session ended".to_string())
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}

/// 📊 성능 메트릭 기록 (내부용)
pub async fn record_performance_metrics(
    app: &AppHandle,
    response_time_ms: u64,
    success: bool,
    concurrency: u32,
    memory_usage_kb: u64,
    network_error: bool,
) -> Result<(), String> {
    let optimizer_state = app.state::<PerformanceOptimizerState>();
    let optimizer_lock = optimizer_state.optimizer.read().await;

    if let Some(optimizer) = optimizer_lock.as_ref() {
        optimizer
            .record_metrics(
                response_time_ms,
                success,
                concurrency,
                memory_usage_kb,
                network_error,
            )
            .await;
        Ok(())
    } else {
        Err("Performance optimizer not initialized".to_string())
    }
}
