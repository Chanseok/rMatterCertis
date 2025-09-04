//! 실시간 크롤링 대시보드 서비스
//! Phase C - Option A: UI와 Backend 실시간 연동

use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::crawl_engine::services::performance_optimizer::CrawlingPerformanceOptimizer;
use crate::api::dashboard_types::{DashboardState, ActiveCrawlingSession, CompletedSession, RealtimeChartData, DashboardAlert, DashboardEvent, DashboardConfig, SystemStatus, ServerStatus, DatabaseStatus, SiteStatus, AlertLevel};

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

		{
			let mut sessions = self.active_sessions.write().await;
			sessions.insert(session_id.clone(), session.clone());
		}

		{
			let mut state = self.state.write().await;
			state.active_session = Some(session.clone());
			state.last_updated = Utc::now();
		}

		let _ = self
			.event_sender
			.send(DashboardEvent::SessionStarted { session });

		info!(session_id = %session_id, total_pages = total_pages, "📈 New crawling session started");
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

		{
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				let elapsed = now.signed_duration_since(session.started_at);
				#[allow(clippy::cast_precision_loss)]
				let elapsed_minutes = elapsed.num_minutes() as f64;

				let current_speed_ppm = if elapsed_minutes > 0.0 {
					f64::from(processed_pages) / elapsed_minutes
				} else { 0.0 };

				let estimated_completion = if current_speed_ppm > 0.0 {
					let remaining_pages = session.total_pages.saturating_sub(processed_pages);
					let remaining_minutes = f64::from(remaining_pages) / current_speed_ppm;
					#[allow(clippy::cast_possible_truncation)]
					Some(now + chrono::Duration::minutes(remaining_minutes as i64))
				} else { None };

				session.current_stage = stage;
				session.overall_progress = overall_progress;
				session.stage_progress = stage_progress;
				session.processed_pages = processed_pages;
				session.collected_urls = collected_urls;
				session.current_speed_ppm = current_speed_ppm;
				session.estimated_completion = estimated_completion;
				session.status_message = status_message;

				let mut state = self.state.write().await;
				state.active_session = Some(session.clone());
				state.last_updated = now;
			} else {
				let _ = self.event_sender.send(DashboardEvent::ProgressUpdate {
					session_id: session_id.clone(),
					progress: overall_progress,
					stage_progress,
				});
				return Err(format!("Session not found: {}", session_id));
			}
		}

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
				let secs = duration.num_seconds().max(0);
				let duration_seconds = u64::try_from(secs).unwrap_or(0);

				#[allow(clippy::cast_precision_loss)]
				let avg_speed_ppm = if duration_seconds > 0 {
					(f64::from(active_session.processed_pages) * 60.0) / (duration_seconds as f64)
				} else { 0.0 };

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

		{
			let mut completed = self.completed_sessions.write().await;
			completed.push_back(completed_session.clone());
			while completed.len() > self.config.max_recent_sessions as usize { completed.pop_front(); }
			drop(completed);
		}

		{
			let mut state = self.state.write().await;
			state.active_session = None;
			state.recent_sessions = self.completed_sessions.read().await.iter().cloned().collect();
			state.last_updated = Utc::now();
		}

		if let Some(message) = final_message {
			self.add_alert(
				if success { AlertLevel::Info } else { AlertLevel::Error },
				"크롤링 완료".to_string(),
				message,
				Some(session_id.clone()),
			).await;
		}

		let duration_for_log = completed_session.duration_seconds;
		let _ = self.event_sender.send(DashboardEvent::SessionCompleted { session: completed_session });

		info!(session_id = %session_id, success = success, duration_seconds = duration_for_log, "📊 Crawling session completed");
		Ok(())
	}

	pub async fn get_dashboard_state(&self) -> DashboardState { self.state.read().await.clone() }
	pub async fn get_chart_data(&self) -> RealtimeChartData { self.chart_data.read().await.clone() }
	#[must_use] pub fn subscribe_events(&self) -> broadcast::Receiver<DashboardEvent> { self.event_sender.subscribe() }

	async fn add_alert(&self, level: AlertLevel, title: String, message: String, session_id: Option<String>) {
		let alert = DashboardAlert { id: Uuid::new_v4().to_string(), level, title, message, timestamp: Utc::now(), session_id, auto_resolve: false };
		{
			let mut alerts = self.alerts.write().await;
			alerts.push(alert.clone());
			if alerts.len() > 100 { alerts.remove(0); }
		}
		let _ = self.event_sender.send(DashboardEvent::NewAlert { alert });
	}

	async fn update_system_status(state: &Arc<RwLock<DashboardState>>) -> Result<(), Box<dyn std::error::Error>> {
		let mut state_lock = state.write().await;
		state_lock.system_status.last_health_check = Utc::now();
		state_lock.last_updated = Utc::now();
		drop(state_lock);
		Ok(())
	}
}

