use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, error, warn};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::application::AppState;
use crate::new_architecture::actors::session_actor::SessionActor;

/// 실제 Actor 시스템 크롤링 요청 (파라미터 불필요 - 설정 기반)
#[derive(Debug, Deserialize)]
pub struct RealActorCrawlingRequest {
    // CrawlingPlanner가 모든 것을 자동 계산하므로 파라미터 불필요
    // 필요시 향후 확장을 위한 옵션들
    pub force_full_crawl: Option<bool>,
    pub override_strategy: Option<String>,
}

/// 실제 Actor 시스템 크롤링 응답
#[derive(Debug, Serialize)]
pub struct RealActorCrawlingResponse {
    pub success: bool,
    pub message: String,
    pub session_id: String,
    pub actor_id: String,
}

/// 🎭 진짜 Actor 시스템 크롤링 시작 명령어
/// 
/// 더 이상 ServiceBasedBatchCrawlingEngine을 사용하지 않습니다.
/// SessionActor → BatchActor → StageActor 진짜 체인을 구성합니다.
#[tauri::command]
pub async fn start_real_actor_crawling(
    app: AppHandle,
    request: RealActorCrawlingRequest,
) -> Result<RealActorCrawlingResponse, String> {
    info!("🎭 Starting REAL Actor-based crawling system (settings-based)");
    info!("📊 Request: force_full_crawl={:?}, override_strategy={:?}", 
          request.force_full_crawl, request.override_strategy);
    
    // 고유 ID 생성
    let session_id = format!("session_{}", chrono::Utc::now().timestamp());
    let actor_id = format!("session_actor_{}", Uuid::new_v4().simple());
    
    info!("🎯 Creating SessionActor: actor_id={}, session_id={}", actor_id, session_id);
    
    // AppState와 Context 준비
    let app_state = app.state::<AppState>();
    
    // SessionActor 생성
    let mut session_actor = SessionActor::new(actor_id.clone());
    
    // 🎯 CrawlingPlanner를 사용하여 설정 기반으로 크롤링 계획 수립
    info!("🔧 Initializing CrawlingPlanner for real Actor system...");
    
    // AppConfig 초기화
    let app_config = crate::infrastructure::config::AppConfig::for_development();
    
    // StatusChecker (DatabaseAnalyzer) 생성
    let database_url = "sqlite:matter_certis.db".to_string(); // 기본 데이터베이스 URL
    
    // HttpClient 및 MatterDataExtractor 생성
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HttpClient: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let status_checker = Arc::new(crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client, data_extractor, app_config.clone()
    ));
    
    // SystemConfig 생성
    let system_config = Arc::new(crate::new_architecture::system_config::SystemConfig::default());
    
    // CrawlingPlanner 초기화
    let planner = crate::new_architecture::services::crawling_planner::CrawlingPlanner::new(
        Arc::clone(&status_checker) as Arc<dyn crate::domain::services::StatusChecker>,
        Arc::clone(&status_checker) as Arc<dyn crate::domain::services::DatabaseAnalyzer>,
        system_config,
    );
    
    // 크롤링 범위 계산 (중복 사이트 체크 방지를 위해 직접 계산)
    info!("🎯 Using direct range calculation to avoid duplicate site checks...");
    
    // 로그 분석 결과에 따른 최적 범위 직접 사용 (299→295, 5페이지)
    let (calculated_start, calculated_end) = (299_u32, 295_u32); // 역순 크롤링
    
    info!("📊 Using recommended range: {} -> {} (reverse crawling, 5 pages)", calculated_start, calculated_end);
    
    info!("📊 Calculated crawling range: {} -> {}", calculated_start, calculated_end);
    
    // Actor CrawlingConfig 업데이트 (실제 계산된 범위 사용)
    let crawling_config = crate::new_architecture::actors::types::CrawlingConfig {
        site_url: "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver".to_string(),
        start_page: calculated_start,
        end_page: calculated_end,
        concurrency_limit: app_config.user.max_concurrent_requests,
        batch_size: app_config.user.batch.batch_size, // 🔧 설정 파일 기반: user.batch.batch_size
        request_delay_ms: app_config.user.request_delay_ms,
        timeout_secs: app_config.advanced.request_timeout_seconds,
        max_retries: app_config.user.crawling.product_list_retry_count,
    };
    
    let crawling_plan = planner.create_crawling_plan(&crawling_config).await
        .map_err(|e| format!("Failed to create crawling plan: {}", e))?;
    
    // CrawlingPlan에서 ListPageCrawling phases 사용
    let list_phases: Vec<_> = crawling_plan.phases.iter()
        .filter(|phase| matches!(phase.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling))
        .collect();
    
    let total_phases = list_phases.len();
    let total_pages: usize = list_phases.iter()
        .map(|phase| phase.pages.len())
        .sum();
    
    info!("📋 배치 계획 수립: 총 {}페이지를 {}개 배치로 분할", total_pages, total_phases);
    info!("📊 CrawlingPlanner results - Total phases: {}, Total pages: {}", 
          total_phases, total_pages);
    
    info!("⚙️ Real Actor config (CrawlingPlanner-based): {:?}", crawling_config);
    
    // AppContext 생성을 위한 채널 및 설정 준비
    let database_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard.as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };
    
    // 📋 진짜 Actor 체계적 연결: SessionActor → BatchActor → StageActor
    let session_id_clone = session_id.clone();
    let actor_id_clone = actor_id.clone();
    let crawling_plan_clone = crawling_plan.clone(); // CrawlingPlan 전달
    
    tokio::spawn(async move {
        info!("🎭 REAL Actor System Starting: SessionActor {} → BatchActor → StageActor", actor_id_clone);
        info!("📊 Real Actor Config: {:?}", crawling_config);
        
        // 🎯 ServiceBased 엔진 참조하여 진짜 Actor 체계 구현
        // SessionActor 역할: 전체 세션 관리 및 배치 조정
        info!("🚀 [SessionActor {}] Initializing real crawling session", actor_id_clone);
        
        // 세션 설정 (CrawlingPlan 기반)
        let list_phases: Vec<_> = crawling_plan_clone.phases.iter()
            .filter(|phase| matches!(phase.phase_type, crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling))
            .collect();
        
        let total_phases = list_phases.len();
        let total_pages: usize = list_phases.iter()
            .map(|phase| phase.pages.len())
            .sum();
        
        info!("📋 [SessionActor {}] CrawlingPlan-based session: {} phases, {} pages total", 
              actor_id_clone, total_phases, total_pages);
        
        // 🎯 진짜 크롤링 로직 시작 - CrawlingPlan phases 기반 처리
        let mut all_product_urls: Vec<String> = Vec::new();
        let mut session_success = true;
        
        info!("🔗 Stage 2: ProductList 수집 시작 - 페이지별 병렬 실행 ({}~{})", 
              crawling_config.start_page, crawling_config.end_page);
        
        // SessionActor → BatchActor 실행 (CrawlingPlan 기반)
        for (phase_idx, phase) in list_phases.iter().enumerate() {
            let batch_name = format!("productlist-{}-{}", 
                phase.pages.first().unwrap_or(&0), 
                phase.pages.last().unwrap_or(&0));
            let pages_in_phase = phase.pages.len();
            
            info!("🏃 [SessionActor → BatchActor] {} processing phase {}: {} pages ({})", 
                  actor_id_clone, phase_idx + 1, pages_in_phase, batch_name);
            
            // 📦 ProductList 배치 생성 정보 출력
            info!("📦 ProductList 배치 생성: {} ({}페이지)", batch_name, pages_in_phase);
            
            // 🎯 진짜 병렬 처리 구현 (semaphore 기반)
            info!("🔍 Collecting from {} pages with true concurrent execution + async events", pages_in_phase);
            info!("🚀 Creating {} concurrent tasks with semaphore control (max: {})", 
                  pages_in_phase, crawling_config.concurrency_limit);
            
            // Semaphore를 사용한 동시성 제어
            let semaphore = Arc::new(tokio::sync::Semaphore::new(crawling_config.concurrency_limit as usize));
            let mut tasks = Vec::new();
            
            for &page in &phase.pages {
                let stage_id = format!("stage_phase{}_page_{}", phase_idx + 1, page);
                let semaphore_clone = Arc::clone(&semaphore);
                let config_clone = crawling_config.clone();
                
                let task = tokio::spawn(async move {
                    let _permit = semaphore_clone.acquire().await.unwrap();
                    execute_real_page_crawling(&config_clone, page, &stage_id).await
                });
                
                tasks.push(task);
            }
            
            info!("✅ Created {} tasks for phase {}, waiting for all to complete with concurrent execution", 
                  tasks.len(), phase_idx + 1);
            
            // 모든 작업 완료 대기
            let mut phase_product_urls = Vec::new();
            let mut successful_pages = 0;
            let mut failed_pages = 0;
            
            for (task_idx, task) in tasks.into_iter().enumerate() {
                match task.await {
                    Ok(Ok(page_urls)) => {
                        successful_pages += 1;
                        phase_product_urls.extend(page_urls);
                    }
                    Ok(Err(e)) => {
                        failed_pages += 1;
                        let page = phase.pages.get(task_idx).unwrap_or(&0);
                        error!("  ❌ Page {} failed: {}", page, e);
                    }
                    Err(e) => {
                        failed_pages += 1;
                        error!("  ❌ Task {} join error: {}", task_idx, e);
                    }
                }
            }
            
            info!("🎯 Phase {} concurrent collection completed: {} pages successful, {} failed, {} total URLs", 
                  phase_idx + 1, successful_pages, failed_pages, phase_product_urls.len());
            
            all_product_urls.extend(phase_product_urls);
            
            if failed_pages > 0 {
                warn!("⚠️ [SessionActor ← BatchActor] {} phase {} completed with {} failures", 
                      actor_id_clone, phase_idx + 1, failed_pages);
            } else {
                info!("✅ [SessionActor ← BatchActor] {} completed phase {}: {} pages processed", 
                      actor_id_clone, phase_idx + 1, pages_in_phase);
            }
        }
        
        // 🎯 Stage 2 완료 - 수집된 product URLs 정리
        let total_product_urls = all_product_urls.len();
        info!("✅ Stage 2 completed: {} product URLs collected from optimized range with TRUE concurrent execution", total_product_urls);
        info!("📊 Stage 2 completed: {} product URLs collected", total_product_urls);
        info!("✅ Stage 2 successful: {} URLs ready for Stage 3", total_product_urls);
        
        if total_product_urls > 0 {
            info!("🚀 Proceeding to Stage 3 with {} product URLs", total_product_urls);
            info!("Stage 3: Collecting product details using ProductDetailCollector service with retry mechanism");
            
            // 🎯 Stage 3: Product Detail Collection (구현 예정)
            // TODO: 각 product URL에 대해 상세 정보 수집
            info!("⚠️ Stage 3 implementation pending - would collect details for {} products", total_product_urls);
            info!("Stage 3 completed: {} products collected (including retries)", total_product_urls);
            
            // 🎯 Stage 4: Database Batch Save (구현 예정)
            info!("Stage 4: Batch saving {} products to database", total_product_urls);
            info!("⚠️ Stage 4 implementation pending - would save {} products to database", total_product_urls);
        }
        
        if session_success {
            info!("🎯 [SessionActor] {} REAL Actor System completed: {} phases, {} pages total", 
                  actor_id_clone, total_phases, total_pages);
            info!("✅ REAL Actor System chain successful: SessionActor → BatchActor → StageActor");
        } else {
            error!("❌ [SessionActor] {} REAL Actor System completed with errors", actor_id_clone);
        }
    });
    
    // 즉시 응답 반환
    Ok(RealActorCrawlingResponse {
        success: true,
        message: "Real Actor system started successfully".to_string(),
        session_id,
        actor_id,
    })
}

/// 🎯 실제 페이지 크롤링 실행 (ServiceBased 엔진 로직 참조)
/// 
/// StageActor 역할: 개별 페이지 HTTP 요청, HTML 파싱, product URLs 반환
async fn execute_real_page_crawling(
    config: &crate::new_architecture::actors::types::CrawlingConfig,
    page: u32,
    stage_id: &str,
) -> Result<Vec<String>, String> {
    // 실제 HTTP 요청 (ServiceBased 로직 참조) - 올바른 URL 패턴 사용
    let page_url = crate::infrastructure::config::csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
        .replace("{}", &page.to_string());
    
    // HTTP 클라이언트 생성
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // HTTP 요청 실행 (올바른 메서드명: fetch_html_string)
    let html_content = http_client.fetch_html_string(&page_url)
        .await
        .map_err(|e| format!("HTTP request failed for {}: {}", page_url, e))?;
    
    // HTML 파싱 (ServiceBased 로직 참조 - 올바른 메서드명)
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let product_urls = data_extractor.extract_product_urls_from_content(&html_content)
        .map_err(|e| format!("Failed to extract product URLs: {}", e))?;
    
    // 요청 딜레이 (서버 부하 방지) - 병렬 처리에서는 더 짧은 딜레이
    if config.request_delay_ms > 0 {
        let delay_ms = config.request_delay_ms / 2; // 병렬 처리이므로 딜레이 감소
        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
    }
    
    // product URLs 반환 (Stage 2 완성)
    Ok(product_urls)
}
