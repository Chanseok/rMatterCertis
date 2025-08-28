use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::RwLock;

use crate::application::validated_crawling_config::ValidatedCrawlingConfig;
use crate::infrastructure::config::AppConfig;

/// TTL 기반 캐시 항목을 위한 트레이트
pub trait CacheItem {
    /// 캐시 항목이 만료되었는지 확인
    fn is_expired(&self, ttl: Duration) -> bool;

    /// 캐시 항목이 유효한지 확인
    fn is_valid(&self) -> bool;
}

/// 사이트 분석 결과 캐시
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteAnalysisResult {
    pub total_pages: u32,
    pub products_on_last_page: u32,
    pub estimated_products: u32,
    pub analyzed_at: DateTime<Utc>,
    #[serde(skip, default = "Instant::now")] // Skip serialization and use default
    pub cached_at: Instant,
    pub is_valid: bool,
    pub site_url: String,
    pub health_score: f64,
}

impl CacheItem for SiteAnalysisResult {
    fn is_expired(&self, ttl: Duration) -> bool {
        if !self.is_valid {
            return true;
        }
        // analyzed_at (UTC)을 기준으로 만료 확인 (파일 저장/로드 시에도 유지됨)
        let now = Utc::now();
        let age = now.signed_duration_since(self.analyzed_at);
        let ttl_chrono = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::minutes(5));

        age > ttl_chrono
    }

    fn is_valid(&self) -> bool {
        self.is_valid
    }
}

impl SiteAnalysisResult {
    /// 사이트 분석 결과 생성
    #[must_use]
    pub fn new(
        total_pages: u32,
        products_on_last_page: u32,
        estimated_products: u32,
        site_url: String,
        health_score: f64,
    ) -> Self {
        Self {
            total_pages,
            products_on_last_page,
            estimated_products,
            analyzed_at: Utc::now(),
            cached_at: Instant::now(),
            is_valid: true,
            site_url,
            health_score,
        }
    }

    /// 캐시 무효화
    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }

    /// 캐시 갱신 (새로운 데이터로 업데이트)
    pub fn refresh(
        &mut self,
        total_pages: u32,
        products_on_last_page: u32,
        estimated_products: u32,
        health_score: f64,
    ) {
        self.total_pages = total_pages;
        self.products_on_last_page = products_on_last_page;
        self.estimated_products = estimated_products;
        self.health_score = health_score;
        self.analyzed_at = Utc::now();
        self.cached_at = Instant::now();
        self.is_valid = true;
    }
}

/// DB 분석 결과 캐시
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAnalysisResult {
    pub total_products: u32,
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub quality_score: f64,
    pub analyzed_at: DateTime<Utc>,
    #[serde(skip, default = "Instant::now")] // Skip serialization and use default
    pub cached_at: Instant,
    pub is_empty: bool,
    pub is_valid: bool,
}

impl CacheItem for DbAnalysisResult {
    fn is_expired(&self, ttl: Duration) -> bool {
        if !self.is_valid {
            return true;
        }
        // analyzed_at (UTC)을 기준으로 만료 확인 (파일 저장/로드 시에도 유지됨)
        let now = Utc::now();
        let age = now.signed_duration_since(self.analyzed_at);
        let ttl_chrono = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::minutes(10));

        age > ttl_chrono
    }

    fn is_valid(&self) -> bool {
        self.is_valid
    }
}

impl DbAnalysisResult {
    /// DB 분석 결과 생성
    #[must_use]
    pub fn new(
        total_products: u32,
        max_page_id: Option<i32>,
        max_index_in_page: Option<i32>,
        quality_score: f64,
    ) -> Self {
        Self {
            total_products,
            max_page_id,
            max_index_in_page,
            quality_score,
            analyzed_at: Utc::now(),
            cached_at: Instant::now(),
            is_empty: total_products == 0,
            is_valid: true,
        }
    }

    /// 다음 크롤링 시작점 계산 (pageId, indexInPage 기반)
    #[must_use]
    pub fn calculate_next_start_position(&self, products_per_page: u32) -> Option<(u32, u32)> {
        if self.is_empty {
            return None;
        }

        let page_id = self.max_page_id? as u32;
        let index_in_page = self.max_index_in_page? as u32;

        // 다음 인덱스 계산
        let next_index = index_in_page + 1;

        if next_index >= products_per_page {
            // 다음 페이지로 이동
            Some((page_id + 1, 0))
        } else {
            // 같은 페이지 내에서 다음 인덱스
            Some((page_id, next_index))
        }
    }
}

/// 계산된 크롤링 범위 캐시
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedRange {
    pub start_page: u32,
    pub end_page: u32,
    pub total_pages: u32,
    pub is_complete_crawl: bool,
    pub calculated_at: chrono::DateTime<Utc>,
    #[serde(skip, default = "Instant::now")] // Skip serialization and use default
    pub cached_at: Instant,
    pub is_valid: bool,
    pub calculation_reason: String,
    pub profile_mode: String,
}

impl CacheItem for CalculatedRange {
    fn is_expired(&self, ttl: Duration) -> bool {
        if !self.is_valid {
            return true;
        }
        // calculated_at (UTC)을 기준으로 만료 확인 (파일 저장/로드 시에도 유지됨)
        let now = Utc::now();
        let age = now.signed_duration_since(self.calculated_at);
        let ttl_chrono = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::minutes(3));

        age > ttl_chrono
    }

    fn is_valid(&self) -> bool {
        self.is_valid && self.start_page > 0 && self.end_page > 0
    }
}

impl CalculatedRange {
    /// 계산된 범위 생성
    #[must_use]
    pub fn new(start_page: u32, end_page: u32, total_pages: u32, is_complete_crawl: bool) -> Self {
        Self {
            start_page,
            end_page,
            total_pages,
            is_complete_crawl,
            calculated_at: Utc::now(),
            cached_at: Instant::now(),
            is_valid: true,
            calculation_reason: "Intelligent calculation based on site and DB analysis".to_string(),
            profile_mode: "intelligent".to_string(),
        }
    }

    /// 캐시 무효화
    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }
}

/// 크롤링 세션 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSessionInfo {
    pub session_id: String,
    pub start_time: chrono::DateTime<Utc>,
    pub start_page: u32,
    pub end_page: u32,
    pub status: String,
    pub is_active: bool,
}

/// 실시간 크롤링 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub is_crawling_active: bool,
    pub session_target_items: Option<u32>,
    pub session_collected_items: Option<u32>,
    pub session_eta_seconds: Option<u32>,
    pub items_per_minute: Option<f64>,
    pub current_stage: Option<String>,
    pub analyzed_at: Option<DateTime<Utc>>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            is_crawling_active: false,
            session_target_items: None,
            session_collected_items: None,
            session_eta_seconds: None,
            items_per_minute: None,
            current_stage: None,
            analyzed_at: None,
        }
    }
}

impl CacheItem for CrawlingSessionInfo {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.start_time.naive_utc() + chrono::Duration::from_std(ttl).unwrap_or_default()
            < Utc::now().naive_utc()
    }

    fn is_valid(&self) -> bool {
        true // Sessions are always valid when present
    }
}

/// 애플리케이션 전체에서 공유되는 상태 캐시
///
/// Modern Rust 2024와 Clippy 권고사항을 준수하여 설계된
/// TTL 기반 캐싱과 설정 기반 동작을 지원하는 중앙 상태 관리 시스템
#[derive(Debug)]
pub struct SharedStateCache {
    /// 사이트 분석 결과 캐시 (TTL: 5-10분)
    pub site_analysis: Arc<RwLock<Option<SiteAnalysisResult>>>,

    /// DB 분석 결과 캐시 (TTL: 2-5분)
    pub db_analysis: Arc<RwLock<Option<DbAnalysisResult>>>,

    /// 계산된 크롤링 범위 캐시 (TTL: 1-3분)
    pub calculated_range: Arc<RwLock<Option<CalculatedRange>>>,

    /// 검증된 설정값들 (하드코딩 방지)
    pub validated_config: Arc<RwLock<ValidatedCrawlingConfig>>,

    /// 마지막 크롤링 세션 정보
    pub last_crawling_session: Arc<RwLock<Option<CrawlingSessionInfo>>>,

    /// 실시간 크롤링 상태 정보 (UI 브로드캐스팅용)
    pub runtime_state: Arc<RwLock<RuntimeState>>,

    /// 캐시 생성 시간
    created_at: Instant,
    /// ExecutionPlan 메모리 LRU 캐시 (plan_hash 기반, 최대 5개)
    pub execution_plan_cache:
        Arc<RwLock<Vec<(String, crate::crawl_engine::actors::types::ExecutionPlan)>>>,
    /// 사이트 상태 새로고침 단일-flight 보장용 락
    site_status_refresh_lock: Arc<AsyncMutex<()>>,
}

impl Default for SharedStateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedStateCache {
    /// 새로운 SharedStateCache 인스턴스 생성
    #[must_use]
    pub fn new() -> Self {
        Self {
            site_analysis: Arc::new(RwLock::new(None)),
            db_analysis: Arc::new(RwLock::new(None)),
            calculated_range: Arc::new(RwLock::new(None)),
            validated_config: Arc::new(RwLock::new(ValidatedCrawlingConfig::default())),
            last_crawling_session: Arc::new(RwLock::new(None)),
            runtime_state: Arc::new(RwLock::new(RuntimeState::default())),
            created_at: Instant::now(),
            execution_plan_cache: Arc::new(RwLock::new(Vec::with_capacity(5))),
            site_status_refresh_lock: Arc::new(AsyncMutex::new(())),
        }
    }

    /// 앱 설정에서 검증된 설정으로 초기화
    #[must_use]
    pub fn from_config(app_config: &AppConfig) -> Self {
        let validated_config = ValidatedCrawlingConfig::from_app_config(app_config);

        Self {
            site_analysis: Arc::new(RwLock::new(None)),
            db_analysis: Arc::new(RwLock::new(None)),
            calculated_range: Arc::new(RwLock::new(None)),
            validated_config: Arc::new(RwLock::new(validated_config)),
            last_crawling_session: Arc::new(RwLock::new(None)),
            runtime_state: Arc::new(RwLock::new(RuntimeState::default())),
            created_at: Instant::now(),
            execution_plan_cache: Arc::new(RwLock::new(Vec::with_capacity(5))),
            site_status_refresh_lock: Arc::new(AsyncMutex::new(())),
        }
    }

    /// ExecutionPlan 캐시에 저장 (LRU 방식, 중복 제거)
    pub async fn cache_execution_plan(
        &self,
        plan: crate::crawl_engine::actors::types::ExecutionPlan,
    ) {
        let mut guard = self.execution_plan_cache.write().await;
        if let Some(pos) = guard.iter().position(|(h, _)| *h == plan.plan_hash) {
            guard.remove(pos);
        }
        guard.insert(0, (plan.plan_hash.clone(), plan));
        if guard.len() > 5 {
            guard.pop();
        }
    }

    /// plan_hash 로 ExecutionPlan 조회
    pub async fn get_cached_execution_plan(
        &self,
        hash: &str,
    ) -> Option<crate::crawl_engine::actors::types::ExecutionPlan> {
        let guard = self.execution_plan_cache.read().await;
        guard
            .iter()
            .find(|(h, _)| h == hash)
            .map(|(_, p)| p.clone())
    }

    /// 현재 검증된 설정 가져오기
    pub async fn get_validated_config(&self) -> ValidatedCrawlingConfig {
        self.validated_config.read().await.clone()
    }

    /// 설정 업데이트
    pub async fn update_config(&self, app_config: &AppConfig) {
        let new_validated_config = ValidatedCrawlingConfig::from_app_config(app_config);
        let mut config_lock = self.validated_config.write().await;
        *config_lock = new_validated_config;

        // 설정 변경 시 관련 캐시도 무효화
        self.clear_calculated_range_async().await;
    }

    /// 사이트 분석 결과 저장
    pub async fn set_site_analysis(&self, result: SiteAnalysisResult) {
        tracing::info!(
            "🏢 사이트 분석 결과 캐시에 저장: {} 페이지, {} 제품",
            result.total_pages,
            result.estimated_products
        );
        let mut guard = self.site_analysis.write().await;
        *guard = Some(result);
    }

    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함)
    pub async fn get_valid_site_analysis_async(
        &self,
        ttl_minutes: Option<u64>,
    ) -> Option<SiteAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(5)) * 60);
        let guard = self.site_analysis.read().await;

        if let Some(ref analysis) = *guard {
            let now = Utc::now();
            let age = now.signed_duration_since(analysis.analyzed_at);

            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!(
                    "🎯 Using cached site analysis (analyzed: {}, age: {} minutes)",
                    analysis.analyzed_at.format("%H:%M:%S"),
                    age.num_minutes()
                );
                return Some(analysis.clone());
            }
            tracing::warn!(
                "⏰ Site analysis cache expired or invalid (analyzed: {}, age: {} minutes, TTL: {} minutes)",
                analysis.analyzed_at.format("%H:%M:%S"),
                age.num_minutes(),
                ttl_minutes.unwrap_or(5)
            );
        } else {
            tracing::info!("📭 No cached site analysis found");
        }
        None
    }

    /// DB 분석 결과 저장
    pub async fn set_db_analysis(&self, result: DbAnalysisResult) {
        tracing::info!(
            "💽 DB 분석 결과 캐시에 저장: {} 제품, max_page_id: {:?}, max_index_in_page: {:?}",
            result.total_products,
            result.max_page_id,
            result.max_index_in_page
        );
        let mut guard = self.db_analysis.write().await;
        *guard = Some(result);
    }

    /// 기존 DB 분석 캐시를 page/index 정보로 보강 (None -> Some 으로만 업데이트)
    pub async fn enrich_db_analysis_position(
        &self,
        max_page_id: Option<i32>,
        max_index_in_page: Option<i32>,
    ) {
        if max_page_id.is_none() && max_index_in_page.is_none() {
            return;
        }
        let mut guard = self.db_analysis.write().await;
        if let Some(ref mut existing) = *guard {
            let mut changed = false;
            if existing.max_page_id.is_none() && max_page_id.is_some() {
                existing.max_page_id = max_page_id;
                changed = true;
            }
            if existing.max_index_in_page.is_none() && max_index_in_page.is_some() {
                existing.max_index_in_page = max_index_in_page;
                changed = true;
            }
            if changed {
                tracing::info!(
                    "🔄 Enriched cached DB analysis with position: page_id={:?}, index_in_page={:?}",
                    existing.max_page_id,
                    existing.max_index_in_page
                );
            }
        }
    }

    /// 유효한 DB 분석 결과 가져오기 (TTL 검사 포함)
    pub async fn get_valid_db_analysis_async(
        &self,
        ttl_minutes: Option<u64>,
    ) -> Option<DbAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(2)) * 60);
        let guard = self.db_analysis.read().await;

        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!(
                    "🎯 Using cached db analysis (age: {:?})",
                    analysis.cached_at.elapsed()
                );
                return Some(analysis.clone());
            }
            tracing::warn!(
                "⏰ DB analysis cache expired or invalid (age: {:?})",
                analysis.cached_at.elapsed()
            );
        }

        None
    }

    /// 계산된 범위 설정
    pub async fn set_calculated_range(&self, range: CalculatedRange) {
        let mut guard = self.calculated_range.write().await;
        *guard = Some(range);
    }

    /// 계산된 범위 캐시 비우기
    pub async fn clear_calculated_range_async(&self) {
        let mut guard = self.calculated_range.write().await;
        *guard = None;
    }

    /// 유효한 계산된 범위 가져오기
    pub fn get_valid_calculated_range(&self, _ttl_minutes: u64) -> Option<&CalculatedRange> {
        // Note: 이 메서드는 동기식이므로 실제로는 사용하지 않음
        // async 버전을 사용하는 것을 권장
        None
    }

    /// 유효한 계산된 범위 가져오기 (async)
    pub async fn get_valid_calculated_range_async(
        &self,
        ttl_minutes: u64,
    ) -> Option<CalculatedRange> {
        let ttl = Duration::from_secs(ttl_minutes * 60);
        let guard = self.calculated_range.read().await;

        if let Some(ref range) = *guard {
            if !range.is_expired(ttl) && range.is_valid() {
                return Some(range.clone());
            }
        }
        None
    }

    /// 크롤링 세션 정보 설정
    pub async fn set_crawling_session(&self, session: CrawlingSessionInfo) {
        let mut guard = self.last_crawling_session.write().await;
        *guard = Some(session);
    }

    /// 현재 상태 요약 반환
    pub async fn get_status_summary(&self) -> CacheStatusSummary {
        let site_guard = self.site_analysis.read().await;
        let db_guard = self.db_analysis.read().await;
        let range_guard = self.calculated_range.read().await;
        let session_guard = self.last_crawling_session.read().await;

        CacheStatusSummary {
            has_site_analysis: site_guard.is_some(),
            has_db_analysis: db_guard.is_some(),
            has_calculated_range: range_guard.is_some(),
            has_valid_db_analysis: db_guard.is_some(),
            has_valid_calculated_range: range_guard.is_some(),
            has_active_session: session_guard.as_ref().map_or(false, |s| s.is_active),
            cache_age_minutes: self.created_at.elapsed().as_secs() / 60,
            total_cached_items: [
                site_guard.is_some() as u8,
                db_guard.is_some() as u8,
                range_guard.is_some() as u8,
                session_guard.is_some() as u8,
            ]
            .iter()
            .sum(),
        }
    }

    /// 모든 캐시 클리어
    pub async fn clear_all_caches(&self) {
        let mut site_guard = self.site_analysis.write().await;
        let mut db_guard = self.db_analysis.write().await;
        let mut range_guard = self.calculated_range.write().await;
        let mut session_guard = self.last_crawling_session.write().await;

        *site_guard = None;
        *db_guard = None;
        *range_guard = None;
        *session_guard = None;

        tracing::info!("🗑️ All caches cleared");
    }

    /// 만료된 캐시 정리 (백그라운드에서 주기적으로 실행)
    pub async fn cleanup_expired_caches(&self) {
        let site_ttl = Duration::from_secs(10 * 60); // 10분
        let db_ttl = Duration::from_secs(5 * 60); // 5분
        let range_ttl = Duration::from_secs(3 * 60); // 3분

        // 사이트 분석 캐시 정리
        {
            let mut site_guard = self.site_analysis.write().await;
            if let Some(ref analysis) = *site_guard {
                if analysis.is_expired(site_ttl) {
                    *site_guard = None;
                    tracing::info!("🗑️ Expired site analysis cache cleaned up");
                }
            }
        }

        // DB 분석 캐시 정리
        {
            let mut db_guard = self.db_analysis.write().await;
            if let Some(ref analysis) = *db_guard {
                if analysis.is_expired(db_ttl) {
                    *db_guard = None;
                    tracing::info!("🗑️ Expired DB analysis cache cleaned up");
                }
            }
        }

        // 계산된 범위 캐시 정리
        {
            let mut range_guard = self.calculated_range.write().await;
            if let Some(ref range) = *range_guard {
                if range.is_expired(range_ttl) {
                    *range_guard = None;
                    tracing::info!("🗑️ Expired calculated range cache cleaned up");
                }
            }
        }
    }

    /// 단일-flight 보장으로 SiteStatus를 새로고침하거나 유효 캐시를 반환
    pub async fn get_or_refresh_site_analysis_singleflight(
        &self,
        ttl_minutes: Option<u64>,
        status_checker: Arc<dyn crate::domain::services::StatusChecker + Send + Sync>,
    ) -> Result<SiteAnalysisResult, String> {
        if let Some(current) = self.get_valid_site_analysis_async(ttl_minutes).await {
            return Ok(current);
        }
        let _guard = self.site_status_refresh_lock.lock().await;
        // 더블 체크(락 안쪽)
        if let Some(current) = self.get_valid_site_analysis_async(ttl_minutes).await {
            return Ok(current);
        }
        let site_status = status_checker
            .check_site_status()
            .await
            .map_err(|e| format!("Site status fetch failed: {}", e))?;
        let site_analysis = SiteAnalysisResult::new(
            site_status.total_pages,
            site_status.products_on_last_page,
            site_status.estimated_products,
            crate::infrastructure::config::utils::matter_products_page_url_simple(1),
            site_status.health_score,
        );
        self.set_site_analysis(site_analysis.clone()).await;
        Ok(site_analysis)
    }
}

/// 캐시 상태 요약
#[derive(Debug, Clone, Serialize)]
pub struct CacheStatusSummary {
    pub has_site_analysis: bool,
    pub has_db_analysis: bool,
    pub has_calculated_range: bool,
    pub has_valid_db_analysis: bool,
    pub has_valid_calculated_range: bool,
    pub has_active_session: bool,
    pub cache_age_minutes: u64,
    pub total_cached_items: u8,
}
