//! 배치 크롤링 엔진 - 4단계 파이프라인 구현
//! 
//! 이 모듈은 guide/crawling 문서들의 노하우를 바탕으로 구현된 
//! 엔터프라이즈급 배치 크롤링 엔진입니다.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use anyhow::{Result, anyhow};
use futures::future::try_join_all;

use crate::domain::events::{CrawlingProgress, CrawlingStage, CrawlingStatus};
use crate::application::EventEmitter;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository, csa_iot};

/// 배치 크롤링 설정
#[derive(Debug, Clone)]
pub struct BatchCrawlingConfig {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: u32,
    pub delay_ms: u64,
    pub batch_size: u32,
    pub retry_max: u32,
    pub timeout_ms: u64,
}

impl Default for BatchCrawlingConfig {
    fn default() -> Self {
        Self {
            start_page: 1,
            end_page: 100,
            concurrency: 3,
            delay_ms: 1000,
            batch_size: 10,
            retry_max: 3,
            timeout_ms: 30000,
        }
    }
}

/// 4단계 배치 크롤링 엔진
pub struct BatchCrawlingEngine {
    http_client: Arc<tokio::sync::Mutex<HttpClient>>,
    data_extractor: Arc<MatterDataExtractor>,
    product_repo: Arc<IntegratedProductRepository>,
    event_emitter: Arc<Option<EventEmitter>>,
    config: BatchCrawlingConfig,
    session_id: String,
}

impl BatchCrawlingEngine {
    pub fn new(
        http_client: HttpClient,
        data_extractor: MatterDataExtractor,
        product_repo: Arc<IntegratedProductRepository>,
        event_emitter: Arc<Option<EventEmitter>>,
        config: BatchCrawlingConfig,
        session_id: String,
    ) -> Self {
        Self {
            http_client: Arc::new(tokio::sync::Mutex::new(http_client)),
            data_extractor: Arc::new(data_extractor),
            product_repo,
            event_emitter,
            config,
            session_id,
        }
    }

    /// 4단계 배치 크롤링 실행
    pub async fn execute(&self) -> Result<()> {
        let start_time = Instant::now();
        info!("Starting 4-stage batch crawling for session: {}", self.session_id);

        // Stage 1: 총 페이지 수 확인
        let total_pages = self.stage1_discover_total_pages().await?;
        
        // Stage 2: 제품 목록 수집 (배치 처리)
        let product_urls = self.stage2_collect_product_list(total_pages).await?;
        
        // Stage 3: 제품 상세정보 수집 (병렬 처리)
        let products = self.stage3_collect_product_details(&product_urls).await?;
        
        // Stage 4: 데이터베이스 저장
        let (processed_count, new_items, updated_items, errors) = self.stage4_save_to_database(products).await?;

        let duration = start_time.elapsed();
        info!("Batch crawling completed in {:?}", duration);
        
        // 완료 이벤트 발송
        self.emit_completion_event(duration, processed_count, new_items, updated_items, errors).await?;
        
        Ok(())
    }

    /// Stage 1: 총 페이지 수 발견
    async fn stage1_discover_total_pages(&self) -> Result<u32> {
        info!("Stage 1: Discovering total pages");
        
        self.emit_progress(CrawlingStage::TotalPages, 0, 1, 0.0, 
            "총 페이지 수를 확인하는 중...").await?;

        let url = format!("{}?page=1", csa_iot::PRODUCTS_PAGE_MATTER_ONLY);
        let html = self.fetch_page(&url).await?;
        
        // 총 페이지 수 추출 (MatterDataExtractor 활용)
        let total_pages = match self.data_extractor.extract_total_pages(&html) {
            Ok(pages) => pages.min(self.config.end_page),
            Err(_) => {
                warn!("Could not extract total pages, using configured end_page");
                self.config.end_page
            }
        };

        self.emit_progress(CrawlingStage::TotalPages, 1, 1, 100.0, 
            &format!("총 {}페이지 발견", total_pages)).await?;

        info!("Stage 1 completed: {} total pages", total_pages);
        Ok(total_pages)
    }

    /// Stage 2: 제품 목록 수집 (배치 처리)
    async fn stage2_collect_product_list(&self, total_pages: u32) -> Result<Vec<String>> {
        info!("Stage 2: Collecting product list from {} pages", total_pages);
        
        let effective_start = self.config.start_page;
        let effective_end = total_pages.min(self.config.end_page);
        let total_pages_to_process = effective_end - effective_start + 1;

        self.emit_progress(CrawlingStage::ProductList, 0, total_pages_to_process, 0.0,
            "제품 목록 페이지를 수집하는 중...").await?;

        let semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        let mut all_product_urls = Vec::new();
        let mut completed_pages = 0u32;

        // 배치별로 페이지 처리
        for batch_start in (effective_start..=effective_end).step_by(self.config.batch_size as usize) {
            let batch_end = (batch_start + self.config.batch_size - 1).min(effective_end);
            let batch_pages: Vec<u32> = (batch_start..=batch_end).collect();
            
            debug!("Processing batch: pages {} to {}", batch_start, batch_end);

            let batch_tasks: Vec<_> = batch_pages.into_iter().map(|page_num| {
                let semaphore = Arc::clone(&semaphore);
                let http_client = Arc::clone(&self.http_client);
                let data_extractor = Arc::clone(&self.data_extractor);
                let delay_ms = self.config.delay_ms;

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    
                    if delay_ms > 0 {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }

                    let url = format!("{}?page={}", csa_iot::PRODUCTS_PAGE_MATTER_ONLY, page_num);
                    debug!("Fetching page: {}", url);

                    let mut client = http_client.lock().await;
                    match client.fetch_html_string(&url).await {
                        Ok(html_str) => {
                            match data_extractor.extract_product_urls_from_content(&html_str) {
                                Ok(urls) => {
                                    debug!("Extracted {} URLs from page {}", urls.len(), page_num);
                                    Ok(urls)
                                },
                                Err(e) => {
                                    warn!("Failed to extract URLs from page {}: {}", page_num, e);
                                    Ok(Vec::new())
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to fetch page {}: {}", page_num, e);
                            Err(e)
                        }
                    }
                })
            }).collect();

            // 배치 실행 및 결과 수집
            let batch_results = try_join_all(batch_tasks).await?;
            for result in batch_results {
                match result {
                    Ok(urls) => all_product_urls.extend(urls),
                    Err(e) => warn!("Batch task failed: {}", e),
                }
            }

            completed_pages += batch_end - batch_start + 1;
            let progress = (completed_pages as f64 / total_pages_to_process as f64) * 100.0;
            
            self.emit_progress(CrawlingStage::ProductList, completed_pages, total_pages_to_process, 
                progress, &format!("{}/{} 페이지 처리 완료", completed_pages, total_pages_to_process)).await?;
        }

        info!("Stage 2 completed: {} product URLs collected", all_product_urls.len());
        Ok(all_product_urls)
    }

    /// Stage 3: 제품 상세정보 수집 (병렬 처리)
    async fn stage3_collect_product_details(&self, product_urls: &[String]) -> Result<Vec<serde_json::Value>> {
        info!("Stage 3: Collecting product details from {} URLs", product_urls.len());
        
        let total_products = product_urls.len() as u32;
        self.emit_progress(CrawlingStage::ProductDetail, 0, total_products, 0.0,
            "제품 상세정보를 수집하는 중...").await?;

        let semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        let mut all_products = Vec::new();
        let mut completed_products = 0u32;

        // 배치별로 제품 상세정보 수집
        for batch in product_urls.chunks(self.config.batch_size as usize) {
            let batch_tasks: Vec<_> = batch.iter().enumerate().map(|(idx, url)| {
                let semaphore = Arc::clone(&semaphore);
                let http_client = Arc::clone(&self.http_client);
                let data_extractor = Arc::clone(&self.data_extractor);
                let url = url.clone();
                let delay_ms = self.config.delay_ms;

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    
                    if delay_ms > 0 && idx > 0 {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }

                    debug!("Fetching product details: {}", url);
                    
                    let mut client = http_client.lock().await;
                    match client.fetch_html_string(&url).await {
                        Ok(html_str) => {
                            match data_extractor.extract_product_data(&html_str) {
                                Ok(product) => {
                                    debug!("Successfully extracted product data from: {}", url);
                                    Ok(Some(product))
                                },
                                Err(e) => {
                                    warn!("Failed to extract product data from {}: {}", url, e);
                                    Ok(None)
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to fetch product page {}: {}", url, e);
                            Err(e)
                        }
                    }
                })
            }).collect();

            // 배치 실행 및 결과 수집
            let batch_results = try_join_all(batch_tasks).await?;
            for result in batch_results {
                match result {
                    Ok(Some(product)) => all_products.push(product),
                    Ok(None) => {}, // 추출 실패한 제품은 무시
                    Err(e) => warn!("Product detail task failed: {}", e),
                }
            }

            completed_products += batch.len() as u32;
            let progress = (completed_products as f64 / total_products as f64) * 100.0;
            
            self.emit_progress(CrawlingStage::ProductDetail, completed_products, total_products, 
                progress, &format!("{}/{} 제품 상세정보 수집 완료", completed_products, total_products)).await?;
        }

        info!("Stage 3 completed: {} products detailed collected", all_products.len());
        Ok(all_products)
    }

    /// Stage 4: 데이터베이스 저장
    async fn stage4_save_to_database(&self, products: Vec<serde_json::Value>) -> Result<(u32, u32, u32, u32)> {
        info!("Stage 4: Saving {} products to database", products.len());
        
        let total_products = products.len() as u32;
        self.emit_progress(CrawlingStage::Database, 0, total_products, 0.0,
            "데이터베이스에 저장하는 중...").await?;

        let mut saved_count = 0u32;
        let mut new_items = 0u32;
        let mut updated_items = 0u32;
        let mut error_count = 0u32;

        // 배치별로 데이터베이스 저장
        for (batch_idx, batch) in products.chunks(self.config.batch_size as usize).enumerate() {
            debug!("Saving batch {}: {} products", batch_idx + 1, batch.len());
            
            for (idx, product) in batch.iter().enumerate() {
                match self.product_repo.upsert_product(product.clone()).await {
                    Ok(is_new) => {
                        if is_new {
                            new_items += 1;
                        } else {
                            updated_items += 1;
                        }
                        saved_count += 1;
                    },
                    Err(e) => {
                        error!("Failed to save product {}: {}", idx + 1, e);
                        error_count += 1;
                        
                        // 오류 이벤트 발송
                        let error_id = format!("db_save_error_{}", uuid::Uuid::new_v4());
                        let error_msg = format!("제품 #{} 저장 실패: {}", idx + 1, e);
                        let _ = self.emit_error(&error_id, &error_msg, CrawlingStage::Database, true).await;
                    }
                }

                if idx % 10 == 0 {
                    let progress = (saved_count as f64 / total_products as f64) * 100.0;
                    self.emit_progress(CrawlingStage::Database, saved_count, total_products, 
                        progress, &format!("{}/{} 제품 저장 완료 (신규: {}, 업데이트: {}, 오류: {})", 
                        saved_count, total_products, new_items, updated_items, error_count)).await?;
                }
            }
        }

        info!("Stage 4 completed: {} products saved (new: {}, updated: {}, errors: {})", 
              saved_count, new_items, updated_items, error_count);
        
        self.emit_progress(CrawlingStage::Database, total_products, total_products, 100.0,
            &format!("데이터베이스 저장 완료: 총 {} 제품 (신규: {}, 업데이트: {}, 오류: {})", 
            saved_count, new_items, updated_items, error_count)).await?;

        // 처리된 항목 수, 신규 항목 수, 업데이트된 항목 수, 오류 수 반환
        Ok((saved_count, new_items, updated_items, error_count))
    }

    /// 진행상황 이벤트 발송 (계산된 필드 포함)
    async fn emit_progress(
        &self, 
        stage: CrawlingStage, 
        current: u32, 
        total: u32, 
        _percentage: f64, // 계산된 필드이므로 무시
        message: &str
    ) -> Result<()> {
        // Start time을 현재 시간으로 가정 (실제로는 BatchCrawlingEngine에서 관리해야 함)
        let start_time = chrono::Utc::now() - chrono::Duration::seconds(60); // 임시값
        
        let progress = CrawlingProgress::new_with_calculation(
            current,
            total,
            stage,
            message.to_string(),
            CrawlingStatus::Running,
            message.to_string(),
            start_time,
            0, // new_items - TODO: 실제 값으로 업데이트
            0, // updated_items - TODO: 실제 값으로 업데이트
            0, // errors - TODO: 실제 값으로 업데이트
        );

        if let Some(ref emitter) = *self.event_emitter {
            if let Err(e) = emitter.emit_progress(progress).await {
                warn!("이벤트 발송 실패 (무시됨): {}", e);
                // 이벤트 발송 실패는 크롤링 프로세스를 중단시키지 않음
            }
        }

        Ok(())
    }

    /// 완료 이벤트 발송
    async fn emit_completion_event(&self, duration: Duration, processed_count: u32, new_items: u32, updated_items: u32, errors: u32) -> Result<()> {
        if let Some(ref emitter) = *self.event_emitter {
            let result = crate::domain::events::CrawlingResult {
                total_processed: processed_count,
                new_items,
                updated_items,
                errors,
                duration_ms: duration.as_millis() as u64,
                stages_completed: vec![
                    CrawlingStage::TotalPages,
                    CrawlingStage::ProductList,
                    CrawlingStage::ProductDetail,
                    CrawlingStage::Database,
                ],
                start_time: chrono::Utc::now() - chrono::Duration::milliseconds(duration.as_millis() as i64),
                end_time: chrono::Utc::now(),
                performance_metrics: crate::domain::events::PerformanceMetrics {
                    avg_processing_time_ms: if processed_count > 0 {
                        duration.as_millis() as f64 / processed_count as f64
                    } else {
                        0.0
                    },
                    items_per_second: if duration.as_secs() > 0 {
                        processed_count as f64 / duration.as_secs() as f64
                    } else {
                        0.0
                    },
                    memory_usage_mb: 0.0, // TODO: 실제 메모리 사용량 측정
                    network_requests: 0,  // TODO: 실제 네트워크 요청 수 추적
                    cache_hit_rate: 0.0,  // TODO: 캐시 히트율 계산
                },
            };

            if let Err(e) = emitter.emit_completed(result).await {
                warn!("완료 이벤트 발송 실패: {}", e);
                // 이벤트 발송 실패는 크롤링 프로세스를 중단시키지 않음
            }
        }

        Ok(())
    }

    /// 스테이지 변경 이벤트 발송
    #[allow(dead_code)]
    async fn emit_stage_change(&self, from: CrawlingStage, to: CrawlingStage, message: &str) -> Result<()> {
        if let Some(ref emitter) = *self.event_emitter {
            if let Err(e) = emitter.emit_stage_change(from, to, message.to_string()).await {
                warn!("스테이지 변경 이벤트 발송 실패 (무시됨): {}", e);
            }
        }
        Ok(())
    }

    /// 오류 이벤트 발송
    async fn emit_error(&self, error_id: &str, message: &str, stage: CrawlingStage, recoverable: bool) -> Result<()> {
        if let Some(ref emitter) = *self.event_emitter {
            if let Err(e) = emitter.emit_error(
                error_id.to_string(), 
                message.to_string(), 
                stage, 
                recoverable
            ).await {
                warn!("오류 이벤트 발송 실패 (무시됨): {}", e);
            }
        }
        Ok(())
    }

    /// HTTP 페이지 가져오기 (재시도 포함)
    async fn fetch_page(&self, url: &str) -> Result<String> {
        let mut retries = 0;
        let max_retries = self.config.retry_max;

        loop {
            let mut client = self.http_client.lock().await;
            match client.fetch_html_string(url).await {
                Ok(html) => return Ok(html),
                Err(e) => {
                    retries += 1;
                    if retries > max_retries {
                        return Err(anyhow!("Failed to fetch {} after {} retries: {}", url, max_retries, e));
                    }
                    
                    let delay = Duration::from_millis(1000 * (1 << retries.min(5))); // 지수 백오프
                    warn!("Retrying {} ({}/{}) after {:?}", url, retries, max_retries, delay);
                    sleep(delay).await;
                }
            }
        }
    }
}
