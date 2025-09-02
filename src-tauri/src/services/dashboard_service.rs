//! 실시간 크롤링 대시보드 서비스
//! Phase C - Option A: UI와 Backend 실시간 연동

use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::crawl_engine::services::performance_optimizer::CrawlingPerformanceOptimizer;
use crate::types::dashboard_types::{DashboardState, ActiveCrawlingSession, CompletedSession, RealtimeChartData, DashboardAlert, DashboardEvent, DashboardConfig, SystemStatus, ServerStatus, DatabaseStatus, SiteStatus, AlertLevel, RealtimePerformanceMetrics, ChartDataPoint};
use crate::crawl_engine::actors::types::{AppEvent, StageType};

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
    #[must_use] pub fn new(config: DashboardConfig) -> Self {
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
            cpu_usage: Vec::new(),
            pages_processed: Vec::new(),
            products_collected: Vec::new(),
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
    #[must_use] pub fn with_performance_optimizer(
        mut self,
        optimizer: Arc<CrawlingPerformanceOptimizer>,
    ) -> Self {
        self.performance_optimizer = Some(optimizer);
        self
    }

    /// 대시보드 서비스 시작
    pub async fn start(&self) {
        info!("🎨 Starting event-driven dashboard service (no auto-polling)");

        // 초기 시스템 상태만 설정
        if let Err(e) = Self::update_system_status(&self.state).await {
            warn!(error = %e, "Failed to initialize system status");
        }

        info!("✅ Dashboard service ready - waiting for Actor events");

        info!("✅ Realtime dashboard service started");
    }

    /// 새 크롤링 세션 시작
    ///
    /// # Errors
    /// 내부 상태 업데이트 또는 이벤트 전송 중 오류가 발생하면 오류를 반환합니다.
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
        let _ = self
            .event_sender
            .send(DashboardEvent::SessionStarted { session });

        info!(session_id = %session_id, total_pages = total_pages, "📊 New crawling session started");
        Ok(())
    }

    /// 크롤링 진행 상황 업데이트
    ///
    /// # Errors
    /// 세션이 존재하지 않으면 오류를 반환합니다.
    #[allow(clippy::too_many_arguments)]
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
                #[allow(clippy::cast_precision_loss)]
                let elapsed_minutes = elapsed.num_minutes() as f64;

                // 처리 속도 계산
                let current_speed_ppm = if elapsed_minutes > 0.0 {
                    f64::from(processed_pages) / elapsed_minutes
                } else {
                    0.0
                };

                // 예상 완료 시간 계산
                let estimated_completion = if current_speed_ppm > 0.0 {
                    let remaining_pages = session.total_pages.saturating_sub(processed_pages);
                    let remaining_minutes = f64::from(remaining_pages) / current_speed_ppm;
                    #[allow(clippy::cast_possible_truncation)]
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
                // 세션을 찾지 못한 경우에도 프론트가 진행률 갱신 실패를 감지할 수 있도록 이벤트를 보냅니다.
                let _ = self.event_sender.send(DashboardEvent::ProgressUpdate {
                    session_id: session_id.clone(),
                    progress: overall_progress,
                    stage_progress,
                });
                return Err(format!("Session not found: {}", session_id));
            }
        }

        // 이벤트 발송 (정상 경로)
        let _ = self.event_sender.send(DashboardEvent::ProgressUpdate {
            session_id,
            progress: overall_progress,
            stage_progress,
        });

        Ok(())
    }

    /// 크롤링 세션 완료
    ///
    /// # Errors
    /// 활성 세션을 찾지 못하면 오류를 반환합니다.
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
                // Avoid sign-loss on cast; negative durations are clamped to 0
                let secs = duration.num_seconds().max(0);
                let duration_seconds = u64::try_from(secs).unwrap_or(0);

                // Display metric; precision loss is acceptable for UI reporting
                #[allow(clippy::cast_precision_loss)]
                let avg_speed_ppm = if duration_seconds > 0 {
                    (f64::from(active_session.processed_pages) * 60.0)
                        / (duration_seconds as f64)
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
            drop(completed);
        }

        // 대시보드 상태 업데이트
        {
            let mut state = self.state.write().await;
            state.active_session = None;
            state.recent_sessions = self
                .completed_sessions
                .read()
                .await
                .iter()
                .cloned()
                .collect();
            state.last_updated = Utc::now();
        }

        // 완료 알림 생성
        if let Some(message) = final_message {
            self.add_alert(
                if success {
                    AlertLevel::Info
                } else {
                    AlertLevel::Error
                },
                "크롤링 완료".to_string(),
                message,
                Some(session_id.clone()),
            )
            .await;
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
    #[must_use] pub fn subscribe_events(&self) -> broadcast::Receiver<DashboardEvent> {
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
    async fn update_system_status(
        state: &Arc<RwLock<DashboardState>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 실제 시스템 상태 확인 로직
        // 현재는 기본 상태로 설정
        let mut state_lock = state.write().await;
        state_lock.system_status.last_health_check = Utc::now();
        state_lock.last_updated = Utc::now();
    // Tighten lock scope explicitly
    drop(state_lock);
        Ok(())
    }

    /// 성능 메트릭 업데이트
    // REMOVE_CANDIDATE(Phase3): currently unused aggregation routine
    #[allow(dead_code)]
    async fn update_performance_metrics(
        state: &Arc<RwLock<DashboardState>>,
        chart_data: &Arc<RwLock<RealtimeChartData>>,
        event_sender: &broadcast::Sender<DashboardEvent>,
        optimizer: &Arc<CrawlingPerformanceOptimizer>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 타임스탬프 기반 의사-랜덤 값 생성 (메트릭 생성과 UI 추정에 모두 사용)
    // Display-only randomness; allow precision loss on cast
    #[allow(clippy::cast_precision_loss)]
    let timestamp_seed = Utc::now().timestamp_millis() as f64;
    let random_factor = (timestamp_seed % 1000.0) / 1000.0; // 0.0-1.0 범위

        // 옵티마이저에서 메트릭 가져오기, 없으면 기본값 생성
        let metrics = optimizer.get_current_metrics().await.unwrap_or_else(|| {
            crate::crawl_engine::services::performance_optimizer::CrawlingPerformanceMetrics {
                session_id: "default".to_string(),
                throughput_rps: random_factor.mul_add(5.0, 10.0), // 10-15 RPS
                avg_response_time_ms: random_factor.mul_add(200.0, 500.0), // 500-700ms
                success_rate: random_factor.mul_add(0.04, 0.95), // 95-99%
                // Counts are small and for display; clamp and allow truncation/sign-loss
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                current_concurrency: random_factor.mul_add(4.0, 3.0).max(0.0).round() as u32, // 3-7 동시 연결
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                recommended_concurrency: random_factor.mul_add(3.0, 5.0).max(0.0).round() as u32, // 5-8 권장 동시성
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                memory_usage_kb: (random_factor.mul_add(128.0, 256.0).max(0.0).round() as u64)
                    * 1024, // 256-384 MB를 KB로 변환
                network_error_rate: random_factor.mul_add(0.04, 0.01), // 1-5% 에러율
                optimization_status: crate::crawl_engine::services::performance_optimizer::OptimizationStatus::Optimal,
            }
        });

        let realtime_metrics = RealtimePerformanceMetrics {
            cpu_usage_percent: random_factor.mul_add(40.0, 30.0), // CPU 사용률 추정
            // Display metric; allow precision loss
            #[allow(clippy::cast_precision_loss)]
            memory_usage_mb: (metrics.memory_usage_kb as f64) / 1024.0, // KB를 MB로 변환
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
                value: f64::from(metrics.current_concurrency),
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

        Ok(())
    }

    /// 차트 데이터 정리
    // REMOVE_CANDIDATE(Phase3): currently unused retention routine
    #[allow(dead_code)]
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
        if chart.cpu_usage.len() > max_points {
            let excess_count = chart.cpu_usage.len() - max_points;
            chart.cpu_usage.drain(0..excess_count);
        }
        if chart.pages_processed.len() > max_points {
            let excess_count = chart.pages_processed.len() - max_points;
            chart.pages_processed.drain(0..excess_count);
        }
        if chart.products_collected.len() > max_points {
            let excess_count = chart.products_collected.len() - max_points;
            chart.products_collected.drain(0..excess_count);
        }
        if chart.concurrent_connections.len() > max_points {
            let excess_count = chart.concurrent_connections.len() - max_points;
            chart.concurrent_connections.drain(0..excess_count);
        }

        // Tighten lock scope explicitly
        drop(chart);

        Ok(())
    }

    /// Actor 시스템 이벤트 처리 - 실제 크롤링 활동만 차트에 반영
    ///
    /// # Errors
    /// 이벤트 처리 중 내부 상태 접근에 실패하거나 세션이 존재하지 않을 경우 오류 문자열을 반환합니다.
    pub async fn handle_actor_event(&self, event: AppEvent) -> Result<(), String> {
        match event {
            AppEvent::SessionStarted {
                session_id,
                config,
                timestamp,
            } => {
                info!("📊 Dashboard: Session started - {}", session_id);

                // 세션 시작 시점 기록
                let now = timestamp.timestamp();
                {
                    let mut chart = self.chart_data.write().await;
                    chart.processing_speed.push(ChartDataPoint {
                        timestamp: now,
                        value: 0.0,
                        label: Some("Session Started".to_string()),
                    });
                }

                self.start_crawling_session(session_id, config.end_page)
                    .await
            }

            AppEvent::Progress {
                session_id,
                current_step,
                total_steps,
                percentage,
                timestamp,
                ..
            } => {
                info!(
                    "📊 Dashboard: Progress update - {}% ({}/{})",
                    percentage, current_step, total_steps
                );
                // Estimate processed pages from percentage and known total_pages
                let (processed_est, started_at_ts) = {
                    let sessions = self.active_sessions.read().await;
                    sessions.get(&session_id).map_or_else(
                        || (current_step, timestamp.timestamp()),
                        |s| {
                            let total_pages = s.total_pages.max(1);
                            let est_f = ((percentage / 100.0) * f64::from(total_pages)).floor();
                            // Clamp to [0, total_pages] and cast
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            let est = est_f.clamp(0.0, f64::from(total_pages)) as u32;
                            (est, s.started_at.timestamp())
                        },
                    )
                };

                // Push charts with a sensible speed based on elapsed from session start
                let now = timestamp.timestamp();
                #[allow(clippy::cast_precision_loss)]
                let elapsed_minutes = (((now - started_at_ts).max(0)) as f64 / 60.0).max(1e-6);
                let speed_ppm = f64::from(processed_est) / elapsed_minutes;
                {
                    let mut chart = self.chart_data.write().await;
                    chart.processing_speed.push(ChartDataPoint { timestamp: now, value: speed_ppm, label: None });
                    chart.pages_processed.push(ChartDataPoint { timestamp: now, value: f64::from(processed_est), label: None });
                }

                self
                    .update_crawling_progress(
                        session_id,
                        "Processing".to_string(),
                        percentage / 100.0,
                        percentage / 100.0,
                        processed_est,
                        0,
                        format!("Processing ~{}% ({} of {} steps)", percentage, current_step, total_steps),
                    )
                    .await
            }

            AppEvent::SessionCompleted {
                session_id,
                summary,
                timestamp,
            } => {
                info!("📊 Dashboard: Session completed - {}", session_id);

                let now = timestamp.timestamp();
                {
                    let mut chart = self.chart_data.write().await;
                    chart.processing_speed.push(ChartDataPoint {
                        timestamp: now,
                        value: 0.0,
                        label: Some("Session Completed".to_string()),
                    });

                    chart.success_rate.push(ChartDataPoint {
                        timestamp: now,
                        value: summary.success_rate * 100.0,
                        label: None,
                    });
                }

                self.complete_crawling_session(
                    session_id,
                    true,
                    0,
                    Some("Session completed successfully".to_string()),
                )
                .await
            }

            AppEvent::SessionFailed {
                session_id,
                error,
                timestamp,
                ..
            } => {
                warn!("📊 Dashboard: Session failed - {}: {}", session_id, error);

                let now = timestamp.timestamp();
                {
                    let mut chart = self.chart_data.write().await;
                    chart.processing_speed.push(ChartDataPoint {
                        timestamp: now,
                        value: 0.0,
                        label: Some("Session Failed".to_string()),
                    });
                }

                self.complete_crawling_session(session_id, false, 1, Some(error))
                    .await
            }

            // Update stage and counts with Stage* and report events
            AppEvent::StageStarted { stage_type, session_id, .. } => {
                let stage_name = match stage_type {
                    StageType::StatusCheck => "진단".to_string(),
                    StageType::ListPageCrawling => "제품 목록 수집".to_string(),
                    StageType::ProductDetailCrawling => "세부 정보 수집".to_string(),
                    StageType::DataValidation => "검증".to_string(),
                    StageType::DataSaving => "저장".to_string(),
                };
                self
                    .update_crawling_progress(
                        session_id,
                        stage_name,
                        0.0,
                        0.0,
                        0,
                        0,
                        "Stage started".to_string(),
                    )
                    .await
            }

            AppEvent::StageCompleted { stage_type, session_id, result, .. } => {
                // Use processed items to bump counters; keep overall unchanged
                let stage_name = match stage_type {
                    StageType::StatusCheck => "진단".to_string(),
                    StageType::ListPageCrawling => "제품 목록 수집".to_string(),
                    StageType::ProductDetailCrawling => "세부 정보 수집".to_string(),
                    StageType::DataValidation => "검증".to_string(),
                    StageType::DataSaving => "저장".to_string(),
                };
                // Compute new processed_pages as previous + successful_items when applicable
                let processed_pages = {
                    let sessions = self.active_sessions.read().await;
                    sessions
                        .get(&session_id)
                        .map_or(0, |s| s.processed_pages)
                        .saturating_add(result.successful_items)
                };
                self
                    .update_crawling_progress(
                        session_id,
                        stage_name,
                        0.0,
                        1.0,
                        processed_pages,
                        0,
                        format!("Stage completed, processed {} items", result.processed_items),
                    )
                    .await
            }

            AppEvent::BatchReport { session_id, pages_success, pages_failed, details_success, .. } => {
                let processed_pages_delta = pages_success.saturating_add(pages_failed);
                // Update processed_pages and collected_urls cumulatively
                let (processed_pages, collected_urls) = {
                    let sessions = self.active_sessions.read().await;
                    sessions.get(&session_id).map_or(
                        (processed_pages_delta, details_success),
                        |s| (
                            s.processed_pages.saturating_add(processed_pages_delta),
                            s.collected_urls.saturating_add(details_success),
                        ),
                    )
                };
                self
                    .update_crawling_progress(
                        session_id,
                        "배치 처리".to_string(),
                        0.0,
                        0.0,
                        processed_pages,
                        collected_urls,
                        format!("Batch processed: +{} pages, +{} details", processed_pages_delta, details_success),
                    )
                    .await
            }

            AppEvent::PreflightDiagnostics { session_id, site_total_pages, .. } => {
                // Update known total_pages so percentage/estimates are correct
                if let Some(tp) = site_total_pages {
                    let mut sessions = self.active_sessions.write().await;
                    if let Some(s) = sessions.get_mut(&session_id) {
                        s.total_pages = tp;
                        let mut state = self.state.write().await;
                        state.active_session = Some(s.clone());
                        state.last_updated = Utc::now();
                    }
                }
                Ok(())
            }

            _ => {
                // 다른 이벤트는 무시 (실제 크롤링 활동이 아님)
                Ok(())
            }
        }
    }
}
