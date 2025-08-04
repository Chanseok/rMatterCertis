//! 실시간 크롤링 대시보드 서비스
//! Phase C - Option A: UI와 Backend 실시간 연동

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use tokio::sync::{RwLock, broadcast};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::dashboard_types::*;
use crate::new_architecture::services::performance_optimizer::CrawlingPerformanceOptimizer;

/// 실시간 대시보드 서비스
pub struct RealtimeDashboardService {
    /// 현재 대시보드 상태
    state: Arc<RwLock<DashboardState>>,
    /// 활성 세션 정보
    active_sessions: Arc<RwLock<HashMap<String, ActiveCrawlingSession>>>,
    /// 완료된 세션 히스토리
    completed_sessions: Arc<RwLock<VecDeque<CompletedSession>>>,
    /// 실시간 차트 데이터
    chart_data: Arc<RwLock<RealtimeChartData>>,
    /// 알림 목록
    alerts: Arc<RwLock<Vec<DashboardAlert>>>,
    /// 이벤트 브로드캐스트 채널
    event_sender: broadcast::Sender<DashboardEvent>,
    /// 대시보드 설정
    config: DashboardConfig,
    /// 성능 최적화 서비스
    performance_optimizer: Option<Arc<CrawlingPerformanceOptimizer>>,
}

impl RealtimeDashboardService {
    /// 새 대시보드 서비스 생성
    pub fn new(config: DashboardConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        let initial_state = DashboardState {
            active_session: None,
            recent_sessions: Vec::new(),
            performance_metrics: None,
            system_status: SystemStatus {
                server_status: ServerStatus::Healthy,
                database_status: DatabaseStatus {
                    connected: false,
                    total_products: 0,
                    products_today: 0,
                    size_mb: 0.0,
                    last_update: None,
                },
                site_status: SiteStatus {
                    accessible: false,
                    response_time_ms: 0,
                    total_pages: 0,
                    estimated_products: 0,
                    health_score: 0,
                    last_checked: Utc::now(),
                },
                last_health_check: Utc::now(),
            },
            last_updated: Utc::now(),
        };
        
        let chart_data = RealtimeChartData {
            processing_speed: Vec::new(),
            response_time: Vec::new(),
            success_rate: Vec::new(),
            memory_usage: Vec::new(),
            concurrent_connections: Vec::new(),
        };
        
        Self {
            state: Arc::new(RwLock::new(initial_state)),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            completed_sessions: Arc::new(RwLock::new(VecDeque::new())),
            chart_data: Arc::new(RwLock::new(chart_data)),
            alerts: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            config,
            performance_optimizer: None,
        }
    }
    
    /// 성능 최적화 서비스 연결
    pub fn with_performance_optimizer(mut self, optimizer: Arc<CrawlingPerformanceOptimizer>) -> Self {
        self.performance_optimizer = Some(optimizer);
        self
    }
    
    /// 대시보드 서비스 시작
    pub async fn start(&self) {
        info!("🎨 Starting realtime dashboard service");
        
        // 정기적 업데이트 태스크 시작
        let update_interval = Duration::from_millis(self.config.update_interval_ms);
        let state = self.state.clone();
        let chart_data = self.chart_data.clone();
        let event_sender = self.event_sender.clone();
        let performance_optimizer = self.performance_optimizer.clone();
        
        tokio::spawn(async move {
            let mut timer = interval(update_interval);
            
            loop {
                timer.tick().await;
                
                // 시스템 상태 업데이트
                if let Err(e) = Self::update_system_status(&state).await {
                    warn!(error = %e, "Failed to update system status");
                }
                
                // 성능 메트릭 업데이트
                if let Some(optimizer) = &performance_optimizer {
                    if let Err(e) = Self::update_performance_metrics(&state, &chart_data, &event_sender, optimizer).await {
                        warn!(error = %e, "Failed to update performance metrics");
                    }
                }
                
                // 차트 데이터 정리 (오래된 데이터 제거)
                if let Err(e) = Self::cleanup_chart_data(&chart_data, 100).await {
                    warn!(error = %e, "Failed to cleanup chart data");
                }
            }
        });
        
        info!("✅ Realtime dashboard service started");
    }
    
    /// 새 크롤링 세션 시작
    pub async fn start_crawling_session(
        &self,
        session_id: String,
        total_pages: u32,
    ) -> Result<(), String> {
        let session = ActiveCrawlingSession {
            session_id: session_id.clone(),
            started_at: Utc::now(),
            current_stage: "초기화".to_string(),
            overall_progress: 0.0,
            stage_progress: 0.0,
            processed_pages: 0,
            total_pages,
            collected_urls: 0,
            current_speed_ppm: 0.0,
            estimated_completion: None,
            status_message: "크롤링 세션 시작".to_string(),
            error_count: 0,
            last_error: None,
        };
        
        // 활성 세션에 추가
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        
        // 대시보드 상태 업데이트
        {
            let mut state = self.state.write().await;
            state.active_session = Some(session.clone());
            state.last_updated = Utc::now();
        }
        
        // 이벤트 발송
        let _ = self.event_sender.send(DashboardEvent::SessionStarted { session });
        
        info!(session_id = %session_id, total_pages = total_pages, "📊 New crawling session started");
        Ok(())
    }
    
    /// 크롤링 진행 상황 업데이트
    pub async fn update_crawling_progress(
        &self,
        session_id: String,
        stage: String,
        overall_progress: f64,
        stage_progress: f64,
        processed_pages: u32,
        collected_urls: u32,
        status_message: String,
    ) -> Result<(), String> {
        let now = Utc::now();
        
        // 활성 세션 업데이트
        {
            let mut sessions = self.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                let elapsed = now.signed_duration_since(session.started_at);
                let elapsed_minutes = elapsed.num_minutes() as f64;
                
                // 처리 속도 계산
                let current_speed_ppm = if elapsed_minutes > 0.0 {
                    processed_pages as f64 / elapsed_minutes
                } else {
                    0.0
                };
                
                // 예상 완료 시간 계산
                let estimated_completion = if current_speed_ppm > 0.0 {
                    let remaining_pages = session.total_pages.saturating_sub(processed_pages);
                    let remaining_minutes = remaining_pages as f64 / current_speed_ppm;
                    Some(now + chrono::Duration::minutes(remaining_minutes as i64))
                } else {
                    None
                };
                
                session.current_stage = stage;
                session.overall_progress = overall_progress;
                session.stage_progress = stage_progress;
                session.processed_pages = processed_pages;
                session.collected_urls = collected_urls;
                session.current_speed_ppm = current_speed_ppm;
                session.estimated_completion = estimated_completion;
                session.status_message = status_message;
                
                // 대시보드 상태 업데이트
                {
                    let mut state = self.state.write().await;
                    state.active_session = Some(session.clone());
                    state.last_updated = now;
                }
            } else {
                return Err(format!("Session not found: {}", session_id));
            }
        }
        
        // 이벤트 발송
        let _ = self.event_sender.send(DashboardEvent::ProgressUpdate {
            session_id,
            progress: overall_progress,
            stage_progress,
        });
        
        Ok(())
    }
    
    /// 크롤링 세션 완료
    pub async fn complete_crawling_session(
        &self,
        session_id: String,
        success: bool,
        error_count: u32,
        final_message: Option<String>,
    ) -> Result<(), String> {
        let completed_session = {
            let mut sessions = self.active_sessions.write().await;
            if let Some(active_session) = sessions.remove(&session_id) {
                let now = Utc::now();
                let duration = now.signed_duration_since(active_session.started_at);
                let duration_seconds = duration.num_seconds() as u64;
                
                let avg_speed_ppm = if duration_seconds > 0 {
                    (active_session.processed_pages as f64 * 60.0) / duration_seconds as f64
                } else {
                    0.0
                };
                
                CompletedSession {
                    session_id: session_id.clone(),
                    started_at: active_session.started_at,
                    completed_at: now,
                    success,
                    processed_pages: active_session.processed_pages,
                    collected_urls: active_session.collected_urls,
                    duration_seconds,
                    avg_speed_ppm,
                    error_count,
                }
            } else {
                return Err(format!("Active session not found: {}", session_id));
            }
        };
        
        // 완료된 세션 히스토리에 추가
        {
            let mut completed = self.completed_sessions.write().await;
            completed.push_back(completed_session.clone());
            
            // 최대 개수 유지
            while completed.len() > self.config.max_recent_sessions as usize {
                completed.pop_front();
            }
        }
        
        // 대시보드 상태 업데이트
        {
            let mut state = self.state.write().await;
            state.active_session = None;
            state.recent_sessions = self.completed_sessions.read().await.iter().cloned().collect();
            state.last_updated = Utc::now();
        }
        
        // 완료 알림 생성
        if let Some(message) = final_message {
            self.add_alert(
                if success { AlertLevel::Info } else { AlertLevel::Error },
                "크롤링 완료".to_string(),
                message,
                Some(session_id.clone()),
            ).await;
        }
        
        // 이벤트 발송
        let duration_for_log = completed_session.duration_seconds;
        let _ = self.event_sender.send(DashboardEvent::SessionCompleted {
            session: completed_session,
        });
        
        info!(
            session_id = %session_id,
            success = success,
            duration_seconds = duration_for_log,
            "📈 Crawling session completed"
        );
        
        Ok(())
    }
    
    /// 현재 대시보드 상태 조회
    pub async fn get_dashboard_state(&self) -> DashboardState {
        self.state.read().await.clone()
    }
    
    /// 실시간 차트 데이터 조회
    pub async fn get_chart_data(&self) -> RealtimeChartData {
        self.chart_data.read().await.clone()
    }
    
    /// 이벤트 수신자 생성
    pub fn subscribe_events(&self) -> broadcast::Receiver<DashboardEvent> {
        self.event_sender.subscribe()
    }
    
    /// 알림 추가
    async fn add_alert(
        &self,
        level: AlertLevel,
        title: String,
        message: String,
        session_id: Option<String>,
    ) {
        let alert = DashboardAlert {
            id: Uuid::new_v4().to_string(),
            level,
            title,
            message,
            timestamp: Utc::now(),
            session_id,
            auto_resolve: false,
        };
        
        {
            let mut alerts = self.alerts.write().await;
            alerts.push(alert.clone());
            
            // 최대 100개 알림 유지
            if alerts.len() > 100 {
                alerts.remove(0);
            }
        }
        
        let _ = self.event_sender.send(DashboardEvent::NewAlert { alert });
    }
    
    /// 시스템 상태 업데이트
    async fn update_system_status(state: &Arc<RwLock<DashboardState>>) -> Result<(), Box<dyn std::error::Error>> {
        // 실제 시스템 상태 확인 로직
        // 현재는 기본 상태로 설정
        let mut state_lock = state.write().await;
        state_lock.system_status.last_health_check = Utc::now();
        state_lock.last_updated = Utc::now();
        Ok(())
    }
    
    /// 성능 메트릭 업데이트
    async fn update_performance_metrics(
        state: &Arc<RwLock<DashboardState>>,
        chart_data: &Arc<RwLock<RealtimeChartData>>,
        event_sender: &broadcast::Sender<DashboardEvent>,
        optimizer: &Arc<CrawlingPerformanceOptimizer>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(metrics) = optimizer.get_current_metrics().await {
            let realtime_metrics = RealtimePerformanceMetrics {
                cpu_usage_percent: 50.0, // TODO: 실제 CPU 사용률
                memory_usage_mb: 150.0,   // TODO: 실제 메모리 사용량
                network_throughput_kbps: metrics.throughput_rps * 2.0, // 추정치
                avg_response_time_ms: metrics.avg_response_time_ms,
                success_rate_percent: metrics.success_rate * 100.0,
                concurrent_connections: metrics.current_concurrency,
                pending_tasks: 0, // TODO: 실제 큐 대기 작업
                recent_rps: metrics.throughput_rps,
            };
            
            // 차트 데이터 업데이트
            let now = Utc::now().timestamp();
            {
                let mut chart = chart_data.write().await;
                
                chart.processing_speed.push(ChartDataPoint {
                    timestamp: now,
                    value: metrics.throughput_rps,
                    label: None,
                });
                
                chart.response_time.push(ChartDataPoint {
                    timestamp: now,
                    value: metrics.avg_response_time_ms,
                    label: None,
                });
                
                chart.success_rate.push(ChartDataPoint {
                    timestamp: now,
                    value: realtime_metrics.success_rate_percent,
                    label: None,
                });
                
                chart.memory_usage.push(ChartDataPoint {
                    timestamp: now,
                    value: realtime_metrics.memory_usage_mb,
                    label: None,
                });
                
                chart.concurrent_connections.push(ChartDataPoint {
                    timestamp: now,
                    value: metrics.current_concurrency as f64,
                    label: None,
                });
            }
            
            // 상태 업데이트
            {
                let mut state_lock = state.write().await;
                state_lock.performance_metrics = Some(realtime_metrics.clone());
                state_lock.last_updated = Utc::now();
            }
            
            // 이벤트 발송
            let _ = event_sender.send(DashboardEvent::PerformanceUpdate {
                metrics: realtime_metrics,
            });
        }
        
        Ok(())
    }
    
    /// 차트 데이터 정리
    async fn cleanup_chart_data(
        chart_data: &Arc<RwLock<RealtimeChartData>>,
        max_points: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut chart = chart_data.write().await;
        
        if chart.processing_speed.len() > max_points {
            let excess_count = chart.processing_speed.len() - max_points;
            chart.processing_speed.drain(0..excess_count);
        }
        if chart.response_time.len() > max_points {
            let excess_count = chart.response_time.len() - max_points;
            chart.response_time.drain(0..excess_count);
        }
        if chart.success_rate.len() > max_points {
            let excess_count = chart.success_rate.len() - max_points;
            chart.success_rate.drain(0..excess_count);
        }
        if chart.memory_usage.len() > max_points {
            let excess_count = chart.memory_usage.len() - max_points;
            chart.memory_usage.drain(0..excess_count);
        }
        if chart.concurrent_connections.len() > max_points {
            let excess_count = chart.concurrent_connections.len() - max_points;
            chart.concurrent_connections.drain(0..excess_count);
        }
        
        Ok(())
    }
}
