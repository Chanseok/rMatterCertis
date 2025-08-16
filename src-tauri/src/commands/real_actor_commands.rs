//! 🎭 Real Actor System Commands
//!
//! SessionActor → BatchActor → StageActor 계층 구조로 병렬 크롤링 실행
//! - SessionActor: CrawlingPlanner 기반 전체 세션 관리
//! - BatchActor: 병렬 배치 실행 (sequential이 아닌 parallel)
//! - StageActor: Stage 2,3,4를 순차 실행 (SessionActor가 아닌 BatchActor가 관리)
//! - Frontend Events: 실시간 진행 상황 브로드캐스트

use chrono::Utc;
use futures;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Manager, command};
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

// 내부 모듈 임포트
use crate::infrastructure::{HttpClient, MatterDataExtractor};
use crate::new_architecture::actors::ActorError;
// use crate::infrastructure::service_based_crawling_engine::ServiceBasedBatchCrawlingEngine; // Deprecated
use crate::application::AppState;
use crate::domain::product::ProductDetail;
use crate::domain::services::StatusChecker;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository; // StatusChecker trait 임포트

/// 🎭 실제 Actor 크롤링 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealActorCrawlingRequest {
    /// 전체 크롤링 강제 실행
    pub force_full_crawl: Option<bool>,
    /// 전략 오버라이드
    pub override_strategy: Option<String>,
}

/// 🎯 Legacy Service-Based 크롤링 시작 (참고용 보존)
///
/// 이 커맨드는 순수 ServiceBasedBatchCrawlingEngine만 사용하는 레거시 구현입니다.
/// NOTE: Deprecated – use unified Analysis-Plan-Execute (ExecutionPlan + SessionActor) instead.
#[command]
pub async fn start_legacy_service_based_crawling(
    app: AppHandle,
    request: RealActorCrawlingRequest,
) -> Result<String, String> {
    info!("🎭 Starting Real Actor System Crawling");
    info!(
        "📋 Request: force_full_crawl={:?}, override_strategy={:?}",
        request.force_full_crawl, request.override_strategy
    );

    let start_time = Instant::now();
    let session_id = Uuid::new_v4().to_string();

    // AppState 가져오기
    let app_state = app.state::<AppState>();
    let app_config = app_state.config.read().await;

    // 🔧 설정 기반 concurrency (하드코딩 3 대신)
    let max_concurrency = app_config.user.max_concurrent_requests;
    info!("⚙️ Using concurrency from config: {}", max_concurrency);

    // 🌐 공유 자원 생성
    let http_client = Arc::new(HttpClient::create_from_global_config().map_err(|e| e.to_string())?);
    let data_extractor = Arc::new(MatterDataExtractor::new().map_err(|e| e.to_string())?);

    // 📡 Frontend 이벤트 채널 생성
    let (event_tx, _event_rx) = broadcast::channel::<FrontendEvent>(500);

    // 📊 실제 크롤링 범위 계산
    info!("🔧 Creating intelligent crawling plan using CrawlingPlanner");

    // 🎯 사이트 상태 확인 및 분석
    let _http_client_for_engine = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let _data_extractor_for_engine = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    let _product_repo_for_engine = Arc::new(IntegratedProductRepository::new(
        app_state
            .database_pool
            .read()
            .await
            .as_ref()
            .ok_or("Database pool not initialized")?
            .clone(),
    ));

    // LEGACY ENGINE DISABLED: ServiceBasedBatchCrawlingEngine removed
    // let _advanced_engine = ServiceBasedBatchCrawlingEngine::new(...);

    // 🎯 실제 크롤링 플래너를 사용하여 최적 범위 계산
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard
            .as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };

    // StatusChecker 생성
    let status_checker_impl = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        (*http_client).clone(),
        (*data_extractor).clone(),
        app_config.clone(),
    );
    let status_checker = Arc::new(status_checker_impl);

    // 🔍 사이트 분석 수행
    let site_status = status_checker
        .check_site_status()
        .await
        .map_err(|e| format!("Failed to check site status: {}", e))?;

    info!(
        "📊 Site analysis: {} pages, {} products",
        site_status.total_pages, site_status.estimated_products
    );

    // DatabaseAnalyzer 생성
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
    let db_analyzer = Arc::new(
        crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
            product_repo.clone(),
        ),
    );

    let crawling_planner =
        crate::new_architecture::services::crawling_planner::CrawlingPlanner::new(
            status_checker.clone(),
            db_analyzer.clone(),
            Arc::new(crate::new_architecture::context::SystemConfig::default()),
        );

    // 🔍 다음 크롤링 범위 계산 (실제 사이트 정보 사용)
    let (site_status, db_analysis) = crawling_planner
        .analyze_system_state()
        .await
        .map_err(|e| format!("Failed to analyze system state: {}", e))?;

    let (range_recommendation, processing_strategy) = crawling_planner
        .determine_crawling_strategy(&site_status, &db_analysis)
        .map_err(|e| format!("Failed to determine crawling strategy: {}", e))?;

    info!(
        "📊 Calculated crawling strategy: {:?}",
        range_recommendation
    );
    info!(
        "⚙️ Processing strategy: batch_size={}, concurrency={}",
        processing_strategy.recommended_batch_size, processing_strategy.recommended_concurrency
    );

    // � CrawlingRangeRecommendation을 실제 페이지 범위로 변환
    let (start_page, end_page) = match range_recommendation.to_page_range(site_status.total_pages) {
        Some((s, e)) => {
            // 역순 크롤링으로 변환 (최신 페이지부터)
            if s > e { (s, e) } else { (e, s) }
        }
        None => {
            // 크롤링이 필요 없는 경우 최신 5페이지만 확인
            let verification_pages = 5;
            let start = site_status.total_pages;
            let end = if start >= verification_pages {
                start - verification_pages + 1
            } else {
                1
            };
            (start, end)
        }
    };

    info!("📊 Final page range: {} to {}", start_page, end_page);

    // �🏗️ 계산된 범위로 배치 생성
    let batch_configs = vec![crate::new_architecture::actors::types::BatchConfig {
        batch_size: (start_page - end_page + 1).min(5), // 최대 5페이지씩
        concurrency_limit: max_concurrency,
        batch_delay_ms: 100,
        retry_on_failure: true,
        start_page: Some(start_page),
        end_page: Some(end_page),
    }];

    info!("📋 Created {} batches for testing", batch_configs.len());

    // 📢 Session 시작 이벤트 발송
    let _ = event_tx.send(FrontendEvent::SessionStarted {
        session_id: session_id.clone(),
        total_batches: batch_configs.len() as u32,
        timestamp: Utc::now(),
    });

    // 🎭 SessionActor 실행 (병렬 BatchActor 스폰)
    match execute_session_with_parallel_batches(
        session_id.clone(),
        batch_configs,
        max_concurrency,
        app_config.advanced.request_timeout_seconds * 1000,
        http_client,
        data_extractor,
        event_tx.clone(),
        app_config.clone(),
    )
    .await
    {
        Ok(_) => {
            let duration = start_time.elapsed();
            info!("✅ Real Actor Crawling completed in {:?}", duration);

            // 📢 Session 완료 이벤트 발송
            let _ = event_tx.send(FrontendEvent::SessionCompleted {
                session_id: session_id.clone(),
                duration_ms: duration.as_millis() as u64,
                timestamp: Utc::now(),
            });

            Ok(format!(
                "Real Actor crawling completed successfully in {:?}",
                duration
            ))
        }
        Err(e) => {
            error!("❌ Real Actor Crawling failed: {:?}", e);
            Err(format!("Actor crawling failed: {:?}", e))
        }
    }
}

/// 🔥 핵심: 순차 Batch 실행 (SessionActor 역할)
///
/// ⚠️ 중요: 배치는 병렬이 아닌 순차 실행!
/// - 이유: 메모리 과부하 및 DB 저장 실패 방지
/// - Batch1 완료 → Batch2 시작 → Batch3 시작
/// - 각 배치 내부에서만 HTTP 요청을 concurrent하게 처리
async fn execute_session_with_parallel_batches(
    session_id: String,
    batches: Vec<crate::new_architecture::actors::types::BatchConfig>,
    max_concurrency: u32,
    timeout_ms: u64,
    _http_client: Arc<HttpClient>,
    _data_extractor: Arc<MatterDataExtractor>,
    event_tx: broadcast::Sender<FrontendEvent>,
    _app_config: AppConfig,
) -> Result<(), ActorError> {
    info!(
        "🎭 SessionActor executing {} batches SEQUENTIALLY (not parallel)",
        batches.len()
    );

    // 🔄 배치들을 순차적으로 실행 (병렬 아님!)
    for (batch_index, batch) in batches.iter().enumerate() {
        let batch_id = format!("{}_batch_{}", session_id, batch_index);
        let batch_config = batch.clone();
        let concurrency = max_concurrency;
        let timeout = timeout_ms;

        info!(
            "📦 Starting Batch {}/{}: {}",
            batch_index + 1,
            batches.len(),
            batch_id
        );

        // 📢 Batch 시작 이벤트
        let _ = event_tx.send(FrontendEvent::BatchStarted {
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            batch_index: batch_index as u32,
            timestamp: Utc::now(),
        });

        // 🎯 BatchActor 순차 실행 (한 번에 하나씩)
        let result = execute_batch_actor_complete_pipeline_simulation(
            batch_id.clone(),
            batch_config,
            concurrency,
            timeout,
        )
        .await;

        // 📢 Batch 완료 이벤트
        let batch_success = result.is_ok();
        let _ = event_tx.send(FrontendEvent::BatchCompleted {
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            batch_index: batch_index as u32,
            success: batch_success,
            timestamp: Utc::now(),
        });

        // 배치 실패 시 전체 세션 중단
        if let Err(e) = result {
            error!("❌ Batch {} failed, stopping session: {:?}", batch_id, e);
            return Err(e);
        }

        info!("✅ Batch {} completed successfully", batch_id);

        // 배치 간 간격 (시스템 안정성을 위해)
        if batch_index < batches.len() - 1 {
            info!("⏳ Waiting between batches for system stability...");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    info!("✅ All {} batches completed sequentially", batches.len());
    Ok(())
}

/// 🎯 BatchActor 전체 파이프라인 실제 구현 (Stage 2→3→4)
#[allow(unused_variables)]
async fn execute_batch_actor_complete_pipeline_simulation(
    batch_id: String,
    batch_config: crate::new_architecture::actors::types::BatchConfig,
    concurrency: u32,
    _timeout_ms: u64,
) -> Result<(), ActorError> {
    info!(
        "🎯 BatchActor {} executing complete Stage 2-3-4 pipeline (REAL IMPLEMENTATION)",
        batch_id
    );

    // Stage 2: List Page Collection (실제 구현)
    info!(
        "🔍 Stage 2: List Page Collection (concurrency: {})",
        concurrency
    );
    let stage2_urls =
        execute_real_stage_2_list_collection(&batch_id, &batch_config, concurrency).await?;

    info!(
        "✅ Stage 2 completed: {} URLs collected ",
        stage2_urls.len()
    );

    // Stage 3: Product Detail Collection (실제 구현)
    info!("🔍 Stage 3: Product Detail Collection ");
    let stage3_items =
        execute_real_stage_3_detail_collection(&batch_id, stage2_urls, concurrency).await?;

    info!(
        "✅ Stage 3 completed: {} items processed ",
        stage3_items.len()
    );

    // Stage 4: Database Storage (실제 구현)
    info!("💾 Stage 4: Database Storage ");
    execute_real_stage_4_storage(&batch_id, stage3_items).await?;

    info!("✅ Stage 4 completed: Database storage finished ");
    info!(
        "🎯 BatchActor {} pipeline completed successfully ",
        batch_id
    );

    Ok(())
}

/// Stage 2: 실제 List Page Collection 구현
async fn execute_real_stage_2_list_collection(
    batch_id: &str,
    batch_config: &crate::new_architecture::actors::types::BatchConfig,
    concurrency: u32,
) -> Result<Vec<String>, ActorError> {
    info!("🔍 BatchActor {} executing REAL Stage 2", batch_id);

    // 실제 HttpClient와 MatterDataExtractor 생성
    let http_client = Arc::new(
        crate::infrastructure::simple_http_client::HttpClient::create_from_global_config()?,
    );
    let data_extractor = Arc::new(crate::infrastructure::html_parser::MatterDataExtractor::new()?);

    // 실제 페이지 범위 (설정에서 가져오거나 기본값 사용)
    let start_page = batch_config.start_page.unwrap_or(291);
    let end_page = batch_config.end_page.unwrap_or(287);

    info!(
        "📄 Collecting pages {} to {} with concurrency {}",
        start_page, end_page, concurrency
    );

    // 세마포어로 동시성 제어
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
    let mut tasks = Vec::new();

    // 페이지 범위 생성 (역순 - 최신부터)
    let pages: Vec<u32> = if start_page > end_page {
        (end_page..=start_page).rev().collect()
    } else {
        (start_page..=end_page).collect()
    };

    for page in pages {
        let http_client_clone = Arc::clone(&http_client);
        let data_extractor_clone = Arc::clone(&data_extractor);
        let semaphore_clone = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Semaphore acquire failed: {}", e))
            })?;

            let url = format!(
                "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver",
                page
            );
            info!("🌐 HTTP GET: {}", url);

            let response = http_client_clone.fetch_response(&url).await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("HTTP request failed: {}", e))
            })?;

            let html_string: String = response.text().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Response text failed: {}", e))
            })?;

            let doc = scraper::Html::parse_document(&html_string);
            let product_urls = data_extractor_clone
                .extract_product_urls(&doc, "https://csa-iot.org")
                .map_err(|e| {
                    ActorError::CommandProcessingFailed(format!("URL extraction failed: {}", e))
                })?;

            info!(
                "📄 Page {} completed with {} URLs",
                page,
                product_urls.len()
            );
            Ok::<Vec<String>, ActorError>(product_urls)
        });

        tasks.push(task);
    }

    info!(
        "✅ Created {} tasks, waiting for all to complete with concurrent execution",
        tasks.len()
    );

    // 모든 태스크 완료 대기
    let results = futures::future::join_all(tasks).await;

    let mut all_urls = Vec::new();
    for result in results {
        match result {
            Ok(Ok(mut urls)) => {
                all_urls.append(&mut urls);
            }
            Ok(Err(e)) => {
                error!("❌ Page processing failed: {:?}", e);
                return Err(e);
            }
            Err(e) => {
                error!("❌ Task join failed: {:?}", e);
                return Err(ActorError::CommandProcessingFailed(format!(
                    "Task join failed: {}",
                    e
                )));
            }
        }
    }

    Ok(all_urls)
}

/// Stage 3: 실제 Product Detail Collection 구현
async fn execute_real_stage_3_detail_collection(
    batch_id: &str,
    stage2_urls: Vec<String>,
    concurrency: u32,
) -> Result<Vec<serde_json::Value>, ActorError> {
    info!(
        "🔍 BatchActor {} executing REAL Stage 3 detail collection",
        batch_id
    );

    if stage2_urls.is_empty() {
        warn!("⚠️ No URLs from Stage 2 - skipping Stage 3");
        return Ok(Vec::new());
    }

    // 실제 HttpClient와 MatterDataExtractor 생성
    let http_client = Arc::new(
        crate::infrastructure::simple_http_client::HttpClient::create_from_global_config()?,
    );
    let data_extractor = Arc::new(crate::infrastructure::html_parser::MatterDataExtractor::new()?);

    // 설정에서 rate limit 가져오기 (하드코딩 제거)
    // HttpClient는 이미 create_from_global_config()에서 올바른 rate limit으로 초기화됨
    info!("🔄 Using configured rate limit from global config (Stage 3: Product Details)");

    // 세마포어로 동시성 제어
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
    let mut tasks = Vec::new();

    for (_index, url) in stage2_urls.iter().enumerate() {
        let http_client_clone = Arc::clone(&http_client);
        let data_extractor_clone = Arc::clone(&data_extractor);
        let semaphore_clone = Arc::clone(&semaphore);
        let url_clone = url.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Semaphore acquire failed: {}", e))
            })?;

            let task_id = format!("product-{}", url_clone);
            info!("� Product task started: {} ({})", url_clone, task_id);

            let start_time = std::time::Instant::now();

            info!("🌐 HTTP GET (HttpClient): {}", url_clone);
            let response = http_client_clone
                .fetch_response(&url_clone)
                .await
                .map_err(|e| {
                    ActorError::CommandProcessingFailed(format!(
                        "HTTP request failed for {}: {}",
                        url_clone, e
                    ))
                })?;

            let html_string: String = response.text().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!(
                    "Response text failed for {}: {}",
                    url_clone, e
                ))
            })?;

            let product_data_json = data_extractor_clone
                .extract_product_data(&html_string)
                .map_err(|e| {
                    ActorError::CommandProcessingFailed(format!(
                        "Product data extraction failed for {}: {}",
                        url_clone, e
                    ))
                })?;

            let elapsed = start_time.elapsed();
            let field_count = count_extracted_json_fields(&product_data_json);

            info!(
                "✅ Product task completed: {} ({}) - {} fields extracted in {:.3}ms",
                url_clone,
                task_id,
                field_count,
                elapsed.as_millis()
            );

            Ok::<serde_json::Value, ActorError>(product_data_json)
        });

        tasks.push(task);
    }

    info!(
        "✅ Created {} product detail tasks, executing with rate limit",
        tasks.len()
    );

    // 모든 태스크 완료 대기
    let results = futures::future::join_all(tasks).await;

    let mut all_products = Vec::new();
    for (index, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok(product_data_json)) => {
                all_products.push(product_data_json);
            }
            Ok(Err(e)) => {
                error!(
                    "❌ Product detail processing failed for item {}: {:?}",
                    index, e
                );
                // 개별 실패는 전체를 중단하지 않고 계속 진행
            }
            Err(e) => {
                error!(
                    "❌ Product detail task join failed for item {}: {:?}",
                    index, e
                );
            }
        }
    }

    Ok(all_products)
}

/// 추출된 필드 수 계산 (JSON용)
fn count_extracted_json_fields(product_json: &serde_json::Value) -> u32 {
    if let Some(obj) = product_json.as_object() {
        obj.len() as u32
    } else {
        0
    }
}

/// Stage 4: 실제 Database Storage 구현
async fn execute_real_stage_4_storage(
    batch_id: &str,
    stage3_items: Vec<serde_json::Value>,
) -> Result<(), ActorError> {
    info!(
        "💾 BatchActor {} executing REAL Stage 4 Database Storage",
        batch_id
    );

    if stage3_items.is_empty() {
        warn!("⚠️ No items from Stage 3 - skipping Stage 4");
        return Ok(());
    }

    // 🏗️ 데이터베이스 풀 재사용 (중앙집중식 경로 관리 + 글로벌 풀)
    let pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| ActorError::DatabaseError(format!("Database pool error: {}", e)))?;

    let repository = IntegratedProductRepository::new(pool);

    info!(
        "📊 Processing {} items for database storage",
        stage3_items.len()
    );

    for (index, product_json) in stage3_items.iter().enumerate() {
        let url = product_json
            .get("source_url")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!(
            "  💾 Processing item {}/{}: {}",
            index + 1,
            stage3_items.len(),
            url
        );

        // 🔄 JSON을 ProductDetail로 변환
        if let Ok(product_detail) = convert_json_to_product_detail(&product_json) {
            // 🏪 실제 데이터베이스 저장
            match repository
                .create_or_update_product_detail(&product_detail)
                .await
            {
                Ok((was_updated, was_created)) => {
                    if was_created {
                        info!("  ✅ Created new product detail: {}", url);
                    } else if was_updated {
                        info!("  🔄 Updated existing product detail: {}", url);
                    } else {
                        info!("  ℹ️ No changes needed for: {}", url);
                    }
                }
                Err(e) => {
                    error!("  ❌ Failed to save product detail {}: {}", url, e);
                    // 개별 저장 실패는 계속 진행 (전체 배치 실패 방지)
                }
            }
        } else {
            warn!("  ⚠️ Failed to convert JSON to ProductDetail: {}", url);
        }

        // 저장 간격 (DB 부하 방지)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    info!(
        "✅ Stage 4 completed: {} items processed for database storage",
        stage3_items.len()
    );
    Ok(())
}

/// 🔄 JSON을 ProductDetail로 변환하는 헬퍼 함수
fn convert_json_to_product_detail(
    json: &serde_json::Value,
) -> Result<ProductDetail, serde_json::Error> {
    // JSON 구조를 ProductDetail로 매핑
    let url = json
        .get("source_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let device_type = json
        .get("certification_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let certification_date = json
        .get("certification_date")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let software_version = json
        .get("software_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let hardware_version = json
        .get("hardware_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let description = json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let manufacturer = json
        .get("manufacturer")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let model = json
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let certification_id = json
        .get("certification_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(ProductDetail {
        url,
        page_id: None,
        index_in_page: None,
        id: None,
        manufacturer,
        model,
        device_type,
        certificate_id: certification_id,
        certification_date,
        software_version,
        hardware_version,
        vid: None,
        pid: None,
        family_sku: None,
        family_variant_sku: None,
        firmware_version: None,
        family_id: None,
        tis_trp_tested: None,
        specification_version: None,
        transport_interface: None,
        primary_device_type_id: None,
        application_categories: None,
        description,
        compliance_document_url: None,
        program_type: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}
/// 📡 Frontend 이벤트 타입들
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum FrontendEvent {
    SessionStarted {
        session_id: String,
        total_batches: u32,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    BatchStarted {
        session_id: String,
        batch_id: String,
        batch_index: u32,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    BatchCompleted {
        session_id: String,
        batch_id: String,
        batch_index: u32,
        success: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    SessionCompleted {
        session_id: String,
        duration_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}
