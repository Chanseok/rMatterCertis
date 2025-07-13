use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        self.cached_at.elapsed() > ttl
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
    pub total_products: u64,
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
        self.cached_at.elapsed() > ttl
    }
    
    fn is_valid(&self) -> bool {
        self.is_valid
    }
}

impl DbAnalysisResult {
    /// DB 분석 결과 생성
    #[must_use]
    pub fn new(
        total_products: u64,
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
        self.cached_at.elapsed() > ttl
    }
    
    fn is_valid(&self) -> bool {
        self.is_valid && self.start_page > 0 && self.end_page > 0
    }
}

impl CalculatedRange {
    /// 계산된 범위 생성
    #[must_use]
    pub fn new(
        start_page: u32,
        end_page: u32,
        total_pages: u32,
        is_complete_crawl: bool,
    ) -> Self {
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
}

impl CacheItem for CrawlingSessionInfo {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.start_time.naive_utc() + chrono::Duration::from_std(ttl).unwrap_or_default() < Utc::now().naive_utc()
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
    
    /// 캐시 생성 시간
    created_at: Instant,
}

impl Default for SharedStateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedStateCache {
    /// 새로운 SharedStateCache 인스턴스 생성
    /// 
    /// # Returns
    /// 초기화된 SharedStateCache 인스턴스
    #[must_use]
    pub fn new() -> Self {
        Self {
            site_analysis: Arc::new(RwLock::new(None)),
            db_analysis: Arc::new(RwLock::new(None)),
            calculated_range: Arc::new(RwLock::new(None)),
            validated_config: Arc::new(RwLock::new(ValidatedCrawlingConfig::default())),
            last_crawling_session: Arc::new(RwLock::new(None)),
            created_at: Instant::now(),
        }
    }
    
    /// 앱 설정에서 검증된 설정으로 초기화
    /// 
    /// # Arguments
    /// * `app_config` - 애플리케이션 설정
    /// 
    /// # Returns
    /// 설정이 적용된 SharedStateCache 인스턴스
    #[must_use]
    pub fn from_config(app_config: &AppConfig) -> Self {
        let validated_config = ValidatedCrawlingConfig::from_app_config(app_config);
        
        Self {
            site_analysis: Arc::new(RwLock::new(None)),
            db_analysis: Arc::new(RwLock::new(None)),
            calculated_range: Arc::new(RwLock::new(None)),
            validated_config: Arc::new(RwLock::new(validated_config)),
            last_crawling_session: Arc::new(RwLock::new(None)),
            created_at: Instant::now(),
        }
    }
    
    /// 검증된 설정 업데이트
    /// 
    /// # Arguments
    /// * `app_config` - 새로운 애플리케이션 설정
    pub async fn update_config(&self, app_config: &AppConfig) {
        let new_validated_config = ValidatedCrawlingConfig::from_app_config(app_config);
        let mut config_lock = self.validated_config.write().await;
        *config_lock = new_validated_config;
        
        // 설정 변경 시 관련 캐시도 무효화
        self.clear_calculated_range_async().await;
    }
    
    /// 현재 검증된 설정 가져오기
    /// 
    /// # Returns
    /// 검증된 크롤링 설정의 복사본
    pub async fn get_validated_config(&self) -> ValidatedCrawlingConfig {
        self.validated_config.read().await.clone()
    }

    /// 사이트 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_site_analysis(&self, result: SiteAnalysisResult) {
        tracing::info!("🏢 사이트 분석 결과 캐시에 저장: {} 페이지, {} 제품", 
                      result.total_pages, result.estimated_products);
        let mut guard = self.site_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_site_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<SiteAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(5)) * 60);
        let guard = self.site_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached site analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ Site analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함)
    pub fn get_valid_site_analysis(&self) -> Option<SiteAnalysisResult> {
        let result = self.site_analysis.read().unwrap();
        let result = result.as_ref()?;
        
        if result.is_expired(self.cache_ttl.site_analysis_ttl) {
            tracing::debug!("🕒 사이트 분석 결과 캐시가 만료됨");
            None
        } else {
            tracing::debug!("✅ 사이트 분석 결과 캐시 사용 (유효)");
            Some(result.clone())
        }
    }
    
    /// 사이트 분석 결과 강제 무효화
    pub fn invalidate_site_analysis(&mut self) {
        if let Some(ref mut analysis) = Arc::get_mut(&mut self.site_analysis) {
            if let Ok(mut analysis) = analysis.write() {
                analysis.invalidate();
                tracing::info!("🗑️ 사이트 분석 결과 캐시 무효화");
            }
        }
    }
    
    /// DB 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_db_analysis(&self, result: DbAnalysisResult) {
        tracing::info!("🗄️ DB 분석 결과 캐시에 저장: {} 제품, 마지막 위치: {}:{}", 
                      result.total_products, result.max_page_id, result.max_index_in_page);
        let mut guard = self.db_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 DB 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_db_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<DbAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(3)) * 60); // DB 분석은 3분 TTL
        let guard = self.db_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached DB analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ DB analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 계산된 범위 저장 (Modern Rust 2024 스타일)
    pub async fn set_calculated_range(&self, range: CalculatedRange) {
        tracing::info!("📐 계산된 크롤링 범위 캐시에 저장: {} → {} ({} 페이지)", 
                      range.start_page, range.end_page, range.total_pages);
        let mut guard = self.calculated_range.write().await;
        *guard = Some(range);
    }
    
    /// 유효한 계산된 범위 가져오기 (TTL 검사 포함) - async 버전  
    pub async fn get_valid_calculated_range_async(&self, ttl_minutes: Option<u64>) -> Option<CalculatedRange> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(2)) * 60); // 범위 계산은 2분 TTL
        let guard = self.calculated_range.read().await;
        
        if let Some(ref range) = *guard {
            if !range.is_expired(ttl) && range.is_valid() {
                tracing::info!("🎯 Using cached calculated range (age: {:?})", range.cached_at.elapsed());
                return Some(range.clone());
            } else {
                tracing::warn!("⏰ Calculated range cache expired or invalid (age: {:?})", range.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 검증된 설정 업데이트
    /// 
    /// # Arguments
    /// * `app_config` - 새로운 애플리케이션 설정
    pub async fn update_config(&self, app_config: &AppConfig) {
        let new_validated_config = ValidatedCrawlingConfig::from_user_config(app_config);
        let mut config_lock = self.validated_config.write().await;
        *config_lock = new_validated_config;
        
        // 설정 변경 시 관련 캐시도 무효화
        self.clear_calculated_range().await;
    }
    
    /// 현재 검증된 설정 가져오기
    /// 
    /// # Returns
    /// 검증된 크롤링 설정의 복사본
    pub async fn get_validated_config(&self) -> ValidatedCrawlingConfig {
        self.validated_config.read().await.clone()
    }

    /// 사이트 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_site_analysis(&self, result: SiteAnalysisResult) {
        tracing::info!("🏢 사이트 분석 결과 캐시에 저장: {} 페이지, {} 제품", 
                      result.total_pages, result.estimated_products);
        let mut guard = self.site_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_site_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<SiteAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(5)) * 60);
        let guard = self.site_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached site analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ Site analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함)
    pub fn get_valid_site_analysis(&self) -> Option<SiteAnalysisResult> {
        let result = self.site_analysis.read().unwrap();
        let result = result.as_ref()?;
        
        if result.is_expired(self.cache_ttl.site_analysis_ttl) {
            tracing::debug!("🕒 사이트 분석 결과 캐시가 만료됨");
            None
        } else {
            tracing::debug!("✅ 사이트 분석 결과 캐시 사용 (유효)");
            Some(result.clone())
        }
    }
    
    /// 사이트 분석 결과 강제 무효화
    pub fn invalidate_site_analysis(&mut self) {
        if let Some(ref mut analysis) = Arc::get_mut(&mut self.site_analysis) {
            if let Ok(mut analysis) = analysis.write() {
                analysis.invalidate();
                tracing::info!("🗑️ 사이트 분석 결과 캐시 무효화");
            }
        }
    }
    
    /// DB 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_db_analysis(&self, result: DbAnalysisResult) {
        tracing::info!("🗄️ DB 분석 결과 캐시에 저장: {} 제품, 마지막 위치: {}:{}", 
                      result.total_products, result.max_page_id, result.max_index_in_page);
        let mut guard = self.db_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 DB 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_db_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<DbAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(3)) * 60); // DB 분석은 3분 TTL
        let guard = self.db_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached DB analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ DB analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 계산된 범위 저장 (Modern Rust 2024 스타일)
    pub async fn set_calculated_range(&self, range: CalculatedRange) {
        tracing::info!("📐 계산된 크롤링 범위 캐시에 저장: {} → {} ({} 페이지)", 
                      range.start_page, range.end_page, range.total_pages);
        let mut guard = self.calculated_range.write().await;
        *guard = Some(range);
    }
    
    /// 유효한 계산된 범위 가져오기 (TTL 검사 포함) - async 버전  
    pub async fn get_valid_calculated_range_async(&self, ttl_minutes: Option<u64>) -> Option<CalculatedRange> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(2)) * 60); // 범위 계산은 2분 TTL
        let guard = self.calculated_range.read().await;
        
        if let Some(ref range) = *guard {
            if !range.is_expired(ttl) && range.is_valid() {
                tracing::info!("🎯 Using cached calculated range (age: {:?})", range.cached_at.elapsed());
                return Some(range.clone());
            } else {
                tracing::warn!("⏰ Calculated range cache expired or invalid (age: {:?})", range.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 검증된 설정 업데이트
    /// 
    /// # Arguments
    /// * `app_config` - 새로운 애플리케이션 설정
    pub async fn update_config(&self, app_config: &AppConfig) {
        let new_validated_config = ValidatedCrawlingConfig::from_user_config(app_config);
        let mut config_lock = self.validated_config.write().await;
        *config_lock = new_validated_config;
        
        // 설정 변경 시 관련 캐시도 무효화
        self.clear_calculated_range().await;
    }
    
    /// 현재 검증된 설정 가져오기
    /// 
    /// # Returns
    /// 검증된 크롤링 설정의 복사본
    pub async fn get_validated_config(&self) -> ValidatedCrawlingConfig {
        self.validated_config.read().await.clone()
    }

    /// 사이트 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_site_analysis(&self, result: SiteAnalysisResult) {
        tracing::info!("🏢 사이트 분석 결과 캐시에 저장: {} 페이지, {} 제품", 
                      result.total_pages, result.estimated_products);
        let mut guard = self.site_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_site_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<SiteAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(5)) * 60);
        let guard = self.site_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached site analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ Site analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함)
    pub fn get_valid_site_analysis(&self) -> Option<SiteAnalysisResult> {
        let result = self.site_analysis.read().unwrap();
        let result = result.as_ref()?;
        
        if result.is_expired(self.cache_ttl.site_analysis_ttl) {
            tracing::debug!("🕒 사이트 분석 결과 캐시가 만료됨");
            None
        } else {
            tracing::debug!("✅ 사이트 분석 결과 캐시 사용 (유효)");
            Some(result.clone())
        }
    }
    
    /// 사이트 분석 결과 강제 무효화
    pub fn invalidate_site_analysis(&mut self) {
        if let Some(ref mut analysis) = Arc::get_mut(&mut self.site_analysis) {
            if let Ok(mut analysis) = analysis.write() {
                analysis.invalidate();
                tracing::info!("🗑️ 사이트 분석 결과 캐시 무효화");
            }
        }
    }
    
    /// DB 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_db_analysis(&self, result: DbAnalysisResult) {
        tracing::info!("🗄️ DB 분석 결과 캐시에 저장: {} 제품, 마지막 위치: {}:{}", 
                      result.total_products, result.max_page_id, result.max_index_in_page);
        let mut guard = self.db_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 DB 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_db_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<DbAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(3)) * 60); // DB 분석은 3분 TTL
        let guard = self.db_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached DB analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ DB analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 계산된 범위 저장 (Modern Rust 2024 스타일)
    pub async fn set_calculated_range(&self, range: CalculatedRange) {
        tracing::info!("📐 계산된 크롤링 범위 캐시에 저장: {} → {} ({} 페이지)", 
                      range.start_page, range.end_page, range.total_pages);
        let mut guard = self.calculated_range.write().await;
        *guard = Some(range);
    }
    
    /// 유효한 계산된 범위 가져오기 (TTL 검사 포함) - async 버전  
    pub async fn get_valid_calculated_range_async(&self, ttl_minutes: Option<u64>) -> Option<CalculatedRange> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(2)) * 60); // 범위 계산은 2분 TTL
        let guard = self.calculated_range.read().await;
        
        if let Some(ref range) = *guard {
            if !range.is_expired(ttl) && range.is_valid() {
                tracing::info!("🎯 Using cached calculated range (age: {:?})", range.cached_at.elapsed());
                return Some(range.clone());
            } else {
                tracing::warn!("⏰ Calculated range cache expired or invalid (age: {:?})", range.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 검증된 설정 업데이트
    /// 
    /// # Arguments
    /// * `app_config` - 새로운 애플리케이션 설정
    pub async fn update_config(&self, app_config: &AppConfig) {
        let new_validated_config = ValidatedCrawlingConfig::from_user_config(app_config);
        let mut config_lock = self.validated_config.write().await;
        *config_lock = new_validated_config;
        
        // 설정 변경 시 관련 캐시도 무효화
        self.clear_calculated_range().await;
    }
    
    /// 현재 검증된 설정 가져오기
    /// 
    /// # Returns
    /// 검증된 크롤링 설정의 복사본
    pub async fn get_validated_config(&self) -> ValidatedCrawlingConfig {
        self.validated_config.read().await.clone()
    }

    /// 사이트 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_site_analysis(&self, result: SiteAnalysisResult) {
        tracing::info!("🏢 사이트 분석 결과 캐시에 저장: {} 페이지, {} 제품", 
                      result.total_pages, result.estimated_products);
        let mut guard = self.site_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_site_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<SiteAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(5)) * 60);
        let guard = self.site_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached site analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ Site analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 유효한 사이트 분석 결과 가져오기 (TTL 검사 포함)
    pub fn get_valid_site_analysis(&self) -> Option<SiteAnalysisResult> {
        let result = self.site_analysis.read().unwrap();
        let result = result.as_ref()?;
        
        if result.is_expired(self.cache_ttl.site_analysis_ttl) {
            tracing::debug!("🕒 사이트 분석 결과 캐시가 만료됨");
            None
        } else {
            tracing::debug!("✅ 사이트 분석 결과 캐시 사용 (유효)");
            Some(result.clone())
        }
    }
    
    /// 사이트 분석 결과 강제 무효화
    pub fn invalidate_site_analysis(&mut self) {
        if let Some(ref mut analysis) = Arc::get_mut(&mut self.site_analysis) {
            if let Ok(mut analysis) = analysis.write() {
                analysis.invalidate();
                tracing::info!("🗑️ 사이트 분석 결과 캐시 무효화");
            }
        }
    }
    
    /// DB 분석 결과 저장 (Modern Rust 2024 스타일)
    pub async fn set_db_analysis(&self, result: DbAnalysisResult) {
        tracing::info!("🗄️ DB 분석 결과 캐시에 저장: {} 제품, 마지막 위치: {}:{}", 
                      result.total_products, result.max_page_id, result.max_index_in_page);
        let mut guard = self.db_analysis.write().await;
        *guard = Some(result);
    }
    
    /// 유효한 DB 분석 결과 가져오기 (TTL 검사 포함) - async 버전
    pub async fn get_valid_db_analysis_async(&self, ttl_minutes: Option<u64>) -> Option<DbAnalysisResult> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(3)) * 60); // DB 분석은 3분 TTL
        let guard = self.db_analysis.read().await;
        
        if let Some(ref analysis) = *guard {
            if !analysis.is_expired(ttl) && analysis.is_valid() {
                tracing::info!("🎯 Using cached DB analysis (age: {:?})", analysis.cached_at.elapsed());
                return Some(analysis.clone());
            } else {
                tracing::warn!("⏰ DB analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 계산된 범위 저장 (Modern Rust 2024 스타일)
    pub async fn set_calculated_range(&self, range: CalculatedRange) {
        tracing::info!("📐 계산된 크롤링 범위 캐시에 저장: {} → {} ({} 페이지)", 
                      range.start_page, range.end_page, range.total_pages);
        let mut guard = self.calculated_range.write().await;
        *guard = Some(range);
    }
    
    /// 유효한 계산된 범위 가져오기 (TTL 검사 포함) - async 버전  
    pub async fn get_valid_calculated_range_async(&self, ttl_minutes: Option<u64>) -> Option<CalculatedRange> {
        let ttl = Duration::from_secs((ttl_minutes.unwrap_or(2)) * 60); // 범위 계산은 2분 TTL
        let guard = self.calculated_range.read().await;
        
        if let Some(ref range) = *guard {
            if !range.is_expired(ttl) && range.is_valid() {
                tracing::info!("🎯 Using cached calculated range (age: {:?})", range.cached_at.elapsed());
                return Some(range.clone());
            } else {
                tracing::warn!("⏰ Calculated range cache expired or invalid (age: {:?})", range.cached_at.elapsed());
            }
        }
        None
    }
    
    /// 캐시 상태 요약
    #[must_use]
    pub fn cache_status_summary(&self) -> CacheStatusSummary {
        CacheStatusSummary {
            has_valid_site_analysis: self.get_valid_site_analysis().is_some(),
            has_valid_db_analysis: self.get_valid_db_analysis().is_some(),
            has_valid_calculated_range: self.get_valid_calculated_range().is_some(),
            has_active_session: self.last_crawling_session
                .as_ref()
                .map_or(false, |s| s.is_active),
        }
    }
}

/// 캐시 상태 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatusSummary {
    pub has_valid_site_analysis: bool,
    pub has_valid_db_analysis: bool,
    pub has_valid_calculated_range: bool,
    pub has_active_session: bool,
}

impl SharedStateCache {
    
    /// 계산된 범위 저장
    pub fn set_calculated_range(&mut self, range: CalculatedRange) {
        self.calculated_range = Some(range);
    }
    
    /// 유효한 계산된 범위 가져오기 (TTL 검사 포함)
    pub fn get_valid_calculated_range(&self, ttl_minutes: u64) -> Option<&CalculatedRange> {
        self.calculated_range.as_ref().and_then(|range| {
            if range.is_expired(ttl_minutes) {
                None
            } else {
                Some(range)
            }
        })
    }
    
    /// 크롤링 세션 정보 저장
    pub fn set_crawling_session(&mut self, session: CrawlingSessionInfo) {
        self.last_crawling_session = Some(session);
    }
    
    /// 상태 일관성 검증
    pub fn validate_consistency(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        
        if let (Some(site), Some(db)) = (&self.site_analysis, &self.db_analysis) {
            // DB 제품 수와 사이트 추정 제품 수 일관성 검증
            let db_products = db.total_products as u32;
            let site_estimated = site.estimated_products;
            
            if db_products > 0 && site_estimated > 0 {
                let ratio = (db_products as f64) / (site_estimated as f64);
                if ratio > 1.2 || ratio < 0.8 {
                    warnings.push(format!(
                        "Product count inconsistency: DB has {}, site estimates {}",
                        db_products, site_estimated
                    ));
                }
            }
            
            // 시간 동기화 검증
            let time_diff = (site.analyzed_at - db.analyzed_at).num_minutes().abs();
            if time_diff > 10 {
                warnings.push(format!(
                    "Analysis time difference too large: {} minutes",
                    time_diff
                ));
            }
        }
        
        warnings
    }
    
    /// 캐시 전체 클리어
    pub fn clear_all(&mut self) {
        self.site_analysis = None;
        self.db_analysis = None;
        self.calculated_range = None;
        self.last_crawling_session = None;
    }
    
    /// 만료된 캐시 항목들 정리
    pub fn cleanup_expired(&mut self, ttl_minutes: u64) {
        if let Some(ref analysis) = self.site_analysis {
            if analysis.is_expired(ttl_minutes) {
                self.site_analysis = None;
            }
        }
        
        if let Some(ref analysis) = self.db_analysis {
            if analysis.is_expired(ttl_minutes) {
                self.db_analysis = None;
            }
        }
        
        if let Some(ref range) = self.calculated_range {
            if range.is_expired(ttl_minutes) {
                self.calculated_range = None;
            }
        }
    }
    
    /// 캐시 상태 요약 정보
    pub fn get_cache_summary(&self) -> CacheSummary {
        CacheSummary {
            has_site_analysis: self.site_analysis.is_some(),
            has_db_analysis: self.db_analysis.is_some(),
            has_calculated_range: self.calculated_range.is_some(),
            has_active_session: self.last_crawling_session.as_ref()
                .map(|s| s.is_active)
                .unwrap_or(false),
        }
    }
}

/// 캐시 상태 요약
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheSummary {
    pub has_site_analysis: bool,
    pub has_db_analysis: bool,
    pub has_calculated_range: bool,
    pub has_active_session: bool,
}

/// 전역 공유 상태 타입 정의
pub type SharedState = Arc<RwLock<SharedStateCache>>;

/// SharedState 생성 헬퍼 함수
pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(SharedStateCache::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    
    #[test]
    fn test_site_analysis_ttl() {
        let mut analysis = SiteAnalysisResult::new(100, 10, 1000, "https://test.com".to_string());
        
        // 새로 생성된 분석은 만료되지 않음
        assert!(!analysis.is_expired(5));
        
        // 시간을 과거로 설정하여 만료 테스트
        analysis.analyzed_at = Utc::now() - Duration::minutes(10);
        assert!(analysis.is_expired(5));
    }
    
    #[test]
    fn test_shared_state_cache() {
        let mut cache = SharedStateCache::new();
        
        // 사이트 분석 결과 저장 및 조회
        let site_analysis = SiteAnalysisResult::new(100, 10, 1000, "https://test.com".to_string());
        cache.set_site_analysis(site_analysis);
        
        assert!(cache.get_valid_site_analysis(5).is_some());
        assert!(cache.get_valid_site_analysis(0).is_none()); // TTL 0분으로 즉시 만료
    }
    
    #[test]
    fn test_cache_consistency_validation() {
        let mut cache = SharedStateCache::new();
        
        // 일관성 없는 데이터 추가
        let site_analysis = SiteAnalysisResult::new(100, 10, 1000, "https://test.com".to_string());
        let db_analysis = DbAnalysisResult::new(2000, Some(50), Some(5), 0.9); // DB에 더 많은 제품
        
        cache.set_site_analysis(site_analysis);
        cache.set_db_analysis(db_analysis);
        
        let warnings = cache.validate_consistency();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("Product count inconsistency"));
    }
}
