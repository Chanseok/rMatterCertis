use crate::application::error::AppError;
use crate::application::event_emitters::AppEvent;
use crate::application::services::CrawlService;
use crate::application::services::SiteAnalysisService;
use crate::domain::product::ProductDetail;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct AppContext {
    pub session_id: String,
    pub app_handle: tauri::AppHandle,
    pub db_pool: sqlx::SqlitePool,
    pub http_client: reqwest::Client,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub actor_event_tx: broadcast::Sender<AppEvent>,
    pub site_analysis_service: Arc<SiteAnalysisService>,
    pub crawl_service: Arc<CrawlService>,
    pub event_bridge_handle: JoinHandle<()>,
}

impl AppContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        app_handle: tauri::AppHandle,
        db_pool: sqlx::SqlitePool,
        http_client: reqwest::Client,
        event_tx: mpsc::Sender<AppEvent>,
        actor_event_tx: broadcast::Sender<AppEvent>,
        site_analysis_service: Arc<SiteAnalysisService>,
        crawl_service: Arc<CrawlService>,
        event_bridge_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            session_id,
            app_handle,
            db_pool,
            http_client,
            event_tx,
            actor_event_tx,
            site_analysis_service,
            crawl_service,
            event_bridge_handle,
        }
    }

    /// AppContext 복제 (특정 필드 제외)
    pub fn clone_without_services(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            app_handle: self.app_handle.clone(),
            db_pool: self.db_pool.clone(),
            http_client: self.http_client.clone(),
            event_tx: self.event_tx.clone(),
            actor_event_tx: self.actor_event_tx.clone(),
            site_analysis_service: Arc::new(SiteAnalysisService::default()),
            crawl_service: Arc::new(CrawlService::default()),
            event_bridge_handle: self.event_bridge_handle.clone(),
        }
    }
}