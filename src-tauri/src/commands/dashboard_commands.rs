//! 실시간 대시보드 Tauri Commands
//! Phase C - Option A: Frontend와 Backend 실시간 연동

use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Emitter};
use tokio::sync::RwLock;
use tracing::{info, error, warn};

use crate::services::dashboard_service::RealtimeDashboardService;
use crate::types::dashboard_types::*;
use crate::new_architecture::services::performance_optimizer::CrawlingPerformanceOptimizer;
use crate::new_architecture::config::SystemConfig;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::database_connection::DatabaseConnection;
use crate::infrastructure::database_paths::get_main_database_url;

/// 대시보드 서비스 상태 관리
pub struct DashboardServiceState {
    pub service: Arc<RwLock<Option<Arc<RealtimeDashboardService>>>>,
}

impl Default for DashboardServiceState {
    fn default() -> Self {
        Self {
            service: Arc::new(RwLock::new(None)),
        }
    }
}

/// 🎨 대시보드 서비스 초기화
#[tauri::command]
pub async fn init_dashboard_service(
    app: AppHandle,
) -> Result<String, String> {
    // 이미 초기화되었는지 확인
    let dashboard_state = app.state::<DashboardServiceState>();
    {
        let service_lock = dashboard_state.service.read().await;
        if service_lock.is_some() {
            info!("✅ Dashboard service already initialized");
            return Ok("Dashboard service already running".to_string());
        }
    }
    
    info!("🎨 Initializing dashboard service");
    
    // 대시보드 설정
    let config = DashboardConfig {
        update_interval_ms: 1000, // 1초마다 업데이트
        max_chart_points: 100,
        performance_thresholds: PerformanceThresholds {
            response_time_warning_ms: 2000,
            response_time_critical_ms: 5000,
            success_rate_warning_percent: 90.0,
            memory_warning_mb: 500.0,
            cpu_warning_percent: 80.0,
        },
        max_recent_sessions: 10,
    };
    
    // 성능 최적화 서비스 생성 (옵션)
    let system_config = Arc::new(SystemConfig::default());
    let performance_optimizer = Arc::new(CrawlingPerformanceOptimizer::new(system_config));
    
    // 데이터베이스 연결 생성 (중앙집중식 경로 관리 사용)
    let database_url = get_main_database_url();
    
    // 제품 리포지토리 생성
    let product_repository = match DatabaseConnection::new(&database_url).await {
        Ok(db_conn) => {
            let repo = Arc::new(IntegratedProductRepository::new(db_conn.pool().clone()));
            info!("✅ Database connection established for dashboard");
            Some(repo)
        },
        Err(e) => {
            error!("❌ Failed to connect to database for dashboard: {}", e);
            None
        }
    };
    
    // 대시보드 서비스 생성
    let dashboard_service = RealtimeDashboardService::new(config)
        .with_performance_optimizer(performance_optimizer);
    
    // TODO: 나중에 product_repository 연결 구현
    // if let Some(repo) = product_repository {
    //     dashboard_service = dashboard_service.with_product_repository(repo);
    // }
    
    let dashboard_service = Arc::new(dashboard_service);
    
    // 서비스 시작
    dashboard_service.start().await;
    
    // 상태에 저장
    let dashboard_state = app.state::<DashboardServiceState>();
    let mut service_lock = dashboard_state.service.write().await;
    *service_lock = Some(dashboard_service.clone());
    
    // 이벤트 리스너 시작
    start_dashboard_event_listener(app.clone(), dashboard_service.clone()).await;
    
    info!("✅ Dashboard service initialized successfully");
    Ok("Dashboard service initialized".to_string())
}

/// 📊 대시보드 상태 조회
#[tauri::command]
pub async fn get_dashboard_state(
    dashboard_state: State<'_, DashboardServiceState>,
) -> Result<DashboardState, String> {
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        Ok(service.get_dashboard_state().await)
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// 📈 실시간 차트 데이터 조회
#[tauri::command]
pub async fn get_chart_data(
    metric_type: String,
    dashboard_state: State<'_, DashboardServiceState>,
) -> Result<Vec<ChartDataPoint>, String> {
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        let chart_data = service.get_chart_data().await;
        
        // 메트릭 타입에 따라 해당 데이터 반환
        let data_points = match metric_type.as_str() {
            "requests_per_second" => chart_data.processing_speed,
            "success_rate" => chart_data.success_rate,
            "response_time" => chart_data.response_time,
            "memory_usage" => chart_data.memory_usage,
            "cpu_usage" => chart_data.cpu_usage,
            "pages_processed" => chart_data.pages_processed,
            "products_collected" => chart_data.products_collected,
            _ => {
                warn!("Unknown metric type: {}", metric_type);
                chart_data.processing_speed // 기본값
            }
        };
        
        Ok(data_points)
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// 🚀 크롤링 세션 시작 (대시보드 연동)
#[tauri::command]
pub async fn start_dashboard_crawling_session(
    session_id: String,
    total_pages: u32,
    dashboard_state: State<'_, DashboardServiceState>,
) -> Result<String, String> {
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        service.start_crawling_session(session_id.clone(), total_pages).await?;
        Ok(format!("Dashboard session started: {}", session_id))
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// 📊 크롤링 진행 상황 업데이트 (대시보드 연동)
#[tauri::command]
pub async fn update_dashboard_progress(
    session_id: String,
    stage: String,
    overall_progress: f64,
    stage_progress: f64,
    processed_pages: u32,
    collected_urls: u32,
    status_message: String,
    dashboard_state: State<'_, DashboardServiceState>,
) -> Result<String, String> {
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        service.update_crawling_progress(
            session_id,
            stage,
            overall_progress,
            stage_progress,
            processed_pages,
            collected_urls,
            status_message,
        ).await?;
        Ok("Progress updated".to_string())
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// ✅ 크롤링 세션 완료 (대시보드 연동)
#[tauri::command]
pub async fn complete_dashboard_crawling_session(
    session_id: String,
    success: bool,
    error_count: u32,
    final_message: Option<String>,
    dashboard_state: State<'_, DashboardServiceState>,
) -> Result<String, String> {
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        service.complete_crawling_session(session_id.clone(), success, error_count, final_message).await?;
        Ok(format!("Dashboard session completed: {}", session_id))
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// 🔄 대시보드와 실제 크롤링 통합 테스트
#[tauri::command]
pub async fn test_dashboard_integration(
    app: AppHandle,
    test_pages: Option<u32>,
) -> Result<String, String> {
    info!("🧪 Starting dashboard integration test");
    
    let pages = test_pages.unwrap_or(3);
    let session_id = format!("dashboard_test_{}", chrono::Utc::now().timestamp());
    
    // 1. 대시보드 세션 시작
    let dashboard_state = app.state::<DashboardServiceState>();
    let service_lock = dashboard_state.service.read().await;
    
    if let Some(service) = service_lock.as_ref() {
        // 세션 시작
        service.start_crawling_session(session_id.clone(), pages).await?;
        
        // 시뮬레이션된 진행 상황 업데이트
        for i in 1..=pages {
            let overall_progress = (i as f64 / pages as f64) * 100.0;
            let stage_progress = 100.0; // 각 페이지는 100% 완료
            
            service.update_crawling_progress(
                session_id.clone(),
                format!("페이지 {} 처리", i),
                overall_progress,
                stage_progress,
                i,
                i * 12, // 페이지당 12개 URL 가정
                format!("페이지 {}/{} 처리 중...", i, pages),
            ).await?;
            
            // 실제와 유사한 지연
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        
        // 세션 완료
        service.complete_crawling_session(
            session_id.clone(),
            true,
            0,
            Some(format!("테스트 완료: {}페이지 처리", pages)),
        ).await?;
        
        info!(session_id = %session_id, pages = pages, "✅ Dashboard integration test completed");
        Ok(format!("Dashboard integration test completed: {} pages", pages))
    } else {
        Err("Dashboard service not initialized".to_string())
    }
}

/// 📡 실시간 대시보드 이벤트 리스너 시작
async fn start_dashboard_event_listener(
    app: AppHandle,
    dashboard_service: Arc<RealtimeDashboardService>,
) {
    let mut event_receiver = dashboard_service.subscribe_events();
    
    tokio::spawn(async move {
        while let Ok(event) = event_receiver.recv().await {
            match &event {
                DashboardEvent::SessionStarted { session } => {
                    info!(session_id = %session.session_id, "📡 Broadcasting session started event");
                    let _ = app.emit("dashboard-session-started", session);
                }
                DashboardEvent::ProgressUpdate { session_id, progress, stage_progress } => {
                    let _ = app.emit("dashboard-progress-update", serde_json::json!({
                        "session_id": session_id,
                        "progress": progress,
                        "stage_progress": stage_progress
                    }));
                }
                DashboardEvent::PerformanceUpdate { metrics } => {
                    let _ = app.emit("dashboard-performance-update", metrics);
                }
                DashboardEvent::SessionCompleted { session } => {
                    info!(session_id = %session.session_id, success = session.success, "📡 Broadcasting session completed event");
                    let _ = app.emit("dashboard-session-completed", session);
                }
                DashboardEvent::SystemStatusChange { status } => {
                    let _ = app.emit("dashboard-system-status", status);
                }
                DashboardEvent::NewAlert { alert } => {
                    let _ = app.emit("dashboard-alert", alert);
                }
                DashboardEvent::ChartDataUpdate { data } => {
                    let _ = app.emit("dashboard-chart-update", data);
                }
            }
        }
    });
    
    info!("📡 Dashboard event listener started");
}

/// 🎯 통합 대시보드 데모 (모든 기능 시연)
#[tauri::command]
pub async fn run_dashboard_demo(
    app: AppHandle,
) -> Result<String, String> {
    info!("🎯 Starting comprehensive dashboard demo");
    
    // 1. 대시보드 서비스 초기화
    init_dashboard_service(app.clone()).await?;
    
    // 잠시 대기
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // 2. 여러 테스트 세션 실행
    let test_configs = vec![
        ("빠른 테스트", 2),
        ("중간 테스트", 5),
        ("긴 테스트", 8),
    ];
    
    for (test_name, pages) in test_configs {
        info!(test_name = test_name, pages = pages, "🔄 Running dashboard test");
        
        match test_dashboard_integration(app.clone(), Some(pages)).await {
            Ok(result) => info!(test_name = test_name, result = %result, "✅ Test completed"),
            Err(e) => error!(test_name = test_name, error = %e, "❌ Test failed"),
        }
        
        // 테스트 간 간격
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
    
    info!("🎉 Dashboard demo completed successfully");
    Ok("Dashboard demo completed with multiple test sessions".to_string())
}
