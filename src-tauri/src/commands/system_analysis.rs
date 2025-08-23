//! 시스템 상태 분석 커맨드
//!
//! proposal6.md의 워크플로우 재정의에 따라 StatusTab에서 사용하는
//! 사이트 종합 분석 기능을 제공합니다.

use serde_json;
use sqlx::Row;
use tauri::{AppHandle, State};
use tracing::info;

// Legacy CrawlingEngineState/CrawlingResponse removed – define minimal local response
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
use crate::application::shared_state::{DbAnalysisResult, SharedStateCache, SiteAnalysisResult};
use crate::domain::constants::{crawling::ttl, site};

/// 시스템 종합 분석 커맨드 (StatusTab용)
///
/// proposal6.md Section 3.1: StatusTab의 역할 - 분석 및 캐시 업데이트
///
/// 이 커맨드는:
/// 1. 사이트 분석과 DB 분석을 수행합니다
/// 2. 결과를 SharedStateCache에 업데이트합니다  
/// 3. 분석 결과를 UI에 전송하여 화면에 표시합니다
/// 4. 백엔드가 total_pages, DB 커서 위치를 "기억"하게 됩니다
#[tauri::command]
pub async fn analyze_system_status(
    _app: AppHandle,
    shared_state: State<'_, SharedStateCache>,
) -> Result<CrawlingResponse, String> {
    info!("🔍 Starting comprehensive system analysis...");

    // Phase 1 & 2: Perform Site and Database Analysis in Parallel
    info!("📊 Phase 1 & 2: Performing site and database analysis in parallel...");
    let (site_analysis, db_analysis) =
        tokio::try_join!(perform_site_analysis(Some(&*shared_state)), perform_database_analysis())
            .map_err(|e| format!("System analysis failed: {}", e))?;

    // Phase 3: Update SharedStateCache
    info!("💾 Phase 3: Updating SharedStateCache with analysis results");
    {
        shared_state.set_site_analysis(site_analysis.clone()).await;
        shared_state.set_db_analysis(db_analysis.clone()).await;
    }

    // Phase 4: Calculate intelligent range preview (optional)
    let range_preview = if !db_analysis.is_empty {
        calculate_range_preview(&site_analysis, &db_analysis).await
    } else {
        None
    };

    // Phase 5: Prepare comprehensive response for UI
    let analysis_data = serde_json::json!({
        "site_analysis": {
            "total_pages": site_analysis.total_pages,
            "products_on_last_page": site_analysis.products_on_last_page,
            "estimated_products": site_analysis.estimated_products,
            "health_score": site_analysis.health_score,
            "analyzed_at": site_analysis.analyzed_at,
            "site_url": site_analysis.site_url,
        },
        "database_analysis": {
            "total_products": db_analysis.total_products,
            "max_page_id": db_analysis.max_page_id,
            "max_index_in_page": db_analysis.max_index_in_page,
            "quality_score": db_analysis.quality_score,
            "is_empty": db_analysis.is_empty,
            "analyzed_at": db_analysis.analyzed_at,
        },
        "range_preview": range_preview,
        "cache_status": {
            "site_analysis_ttl_minutes": ttl::SITE_ANALYSIS_TTL_MINUTES,
            "db_analysis_ttl_minutes": ttl::DB_ANALYSIS_TTL_MINUTES,
            "products_per_page_constant": site::PRODUCTS_PER_PAGE,
        },
        "analysis_summary": format!(
            "Site: {} pages ({} products), DB: {} products saved, Position: {}:{}",
            site_analysis.total_pages,
            site_analysis.estimated_products,
            db_analysis.total_products,
            db_analysis.max_page_id.unwrap_or(-1),
            db_analysis.max_index_in_page.unwrap_or(-1)
        )
    });

    info!("✅ System analysis completed successfully");
    info!(
        "🧠 Backend now remembers: {} total pages, DB cursor at {}:{}",
        site_analysis.total_pages,
        db_analysis.max_page_id.unwrap_or(-1),
        db_analysis.max_index_in_page.unwrap_or(-1)
    );

    Ok(CrawlingResponse {
        success: true,
        message: "System analysis completed successfully".to_string(),
        data: Some(analysis_data),
    })
}

/// 진단: products / product_details 간 미스매치 및 이상치 탐지/정리
#[tauri::command]
pub async fn diagnose_and_repair_data(
    shared_state: State<'_, SharedStateCache>,
    delete_mismatches: Option<bool>,
    sync_orphans: Option<bool>,
) -> Result<CrawlingResponse, String> {
    let pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("DB init failed: {}", e))?;

    // 1) 최신 site_total_pages 추정(캐시 우선)
    let site_total_pages = shared_state
        .get_valid_site_analysis_async(Some(5))
        .await
        .map(|s| s.total_pages)
        .unwrap_or(0);

    // 2) 이상치/미스매치 수집
    let orphans_products = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM products p LEFT JOIN product_details d ON p.url=d.url WHERE d.url IS NULL"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    // 샘플 orphan URL 목록 (최대 200개)
    let orphan_sample_limit: i64 = 200;
    let orphan_urls: Vec<String> = sqlx::query_scalar::<_, String>(
        r#"SELECT p.url FROM products p LEFT JOIN product_details d ON p.url=d.url
           WHERE d.url IS NULL ORDER BY p.page_id ASC, p.index_in_page ASC LIMIT ?"#,
    )
    .bind(orphan_sample_limit)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let nullish_core = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM products WHERE (manufacturer IS NULL OR model IS NULL OR certificate_id IS NULL)"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let out_of_range_pages = if site_total_pages > 0 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM products WHERE page_id IS NOT NULL AND page_id > ?",
        )
        .bind(site_total_pages as i64)
        .fetch_one(&pool)
        .await
        .unwrap_or(0)
    } else {
        0
    };

    let bad_indices = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM products WHERE (index_in_page IS NOT NULL AND index_in_page <= 0) OR (page_id IS NOT NULL AND page_id <= 0)"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    // 3) 심각한 page_id 연속성 붕괴(break) 탐지: page_id 오름차순에서 인접 차이가 2 이상인 최초 위치
    #[derive(Debug, Default, Clone)]
    struct BreakInfo { first_break_at: Option<i64>, break_next: Option<i64>, gap: i64 }
    let mut break_info: BreakInfo = BreakInfo::default();
    if let Ok(rows) = sqlx::query("SELECT DISTINCT page_id FROM products WHERE page_id IS NOT NULL ORDER BY page_id")
        .fetch_all(&pool)
        .await
    {
        let mut prev: Option<i64> = None;
        for r in rows {
            let pid: i64 = r.get::<i64, _>("page_id");
            if let Some(p) = prev {
                let gap = pid - p;
                if gap >= 2 {
                    break_info.first_break_at = Some(p);
                    break_info.break_next = Some(pid);
                    break_info.gap = gap;
                    break;
                }
            }
            prev = Some(pid);
        }
    }

    // 4) 삭제 옵션 수행
    let mut deleted_rows = 0i64;
    if delete_mismatches.unwrap_or(false) {
        // 안전한 순서: out-of-range → bad-indices → nullish-core (목록-only 무의미 레코드) → orphans (상세 미존재)
        if site_total_pages > 0 {
            if let Ok(res) = sqlx::query("DELETE FROM products WHERE page_id IS NOT NULL AND page_id > ?")
                .bind(site_total_pages as i64)
                .execute(&pool)
                .await
            {
                deleted_rows += res.rows_affected() as i64;
            }
        }
        if let Ok(res) = sqlx::query(
            r#"DELETE FROM products WHERE (index_in_page IS NOT NULL AND index_in_page <= 0) OR (page_id IS NOT NULL AND page_id <= 0)"#,
        )
        .execute(&pool)
        .await
        {
            deleted_rows += res.rows_affected() as i64;
        }
        if let Ok(res) = sqlx::query(
            r#"DELETE FROM products WHERE manufacturer IS NULL AND model IS NULL AND certificate_id IS NULL"#,
        )
        .execute(&pool)
        .await
        {
            deleted_rows += res.rows_affected() as i64;
        }
        // 심각한 break 이후 레코드 일괄 삭제: 기준 = break_next 이상 모든 page_id
        if let Some(threshold) = break_info.break_next {
            if let Ok(res) = sqlx::query("DELETE FROM products WHERE page_id IS NOT NULL AND page_id >= ?")
                .bind(threshold)
                .execute(&pool)
                .await
            {
                deleted_rows += res.rows_affected() as i64;
            }
        }
        // 남은 orphans는 상세 동기화가 금방 따라올 수 있어 선택 삭제 대신 보고만 유지할 수도 있음
    }

    // 5) 요청 시: orphans 상세 동기화(상세 페이지를 조회하여 product_details 채움 + products 코어 필드 보정)
    let mut synced_orphans: i64 = 0;
    if sync_orphans.unwrap_or(false) && !orphan_urls.is_empty() {
        // Infra 생성
        let config_manager = crate::infrastructure::config::ConfigManager::new()
            .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
        let config = config_manager
            .load_config()
            .await
            .map_err(|e| format!("Failed to load configuration: {}", e))?;
        let http_client = crate::infrastructure::HttpClient::create_from_global_config()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        let extractor = crate::infrastructure::MatterDataExtractor::new()
            .map_err(|e| format!("Failed to create extractor: {}", e))?;
        let sync_ua = config.user.crawling.workers.user_agent_sync.clone();

        for url in orphan_urls.iter() {
            // referer는 기본 Matter 필터 목록을 사용(충분히 허용적)
            let referer = crate::infrastructure::config::csa_iot::PRODUCTS_BASE.to_string();
            if let Ok(resp) = http_client
                .fetch_response_with_options(
                    url,
                    &crate::infrastructure::simple_http_client::RequestOptions {
                        user_agent_override: sync_ua.clone(),
                        referer: Some(referer),
                        skip_robots_check: false,
                        attempt: None,
                        max_attempts: None,
                    },
                )
                .await
            {
                if let Ok(body) = resp.text().await {
                    let extracted = {
                        let doc = scraper::Html::parse_document(&body);
                        extractor.extract_product_detail(&doc, url.clone())
                    };
                    if let Ok(detail) = extracted {
                        // Clone fields needed later to avoid move issues
                        let man_clone = detail.manufacturer.clone();
                        let model_clone = detail.model.clone();
                        let cert_clone = detail.certificate_id.clone();
                        // page_id/index_in_page는 알 수 있으면 보정, 없으면 NULL 유지
                        let _ = sqlx::query(
                            r#"INSERT INTO product_details (
                                url, page_id, index_in_page, id, manufacturer, model, device_type,
                                certificate_id, certification_date, software_version, hardware_version, firmware_version,
                                specification_version, vid, pid, family_sku, family_variant_sku, family_id,
                                tis_trp_tested, transport_interface, primary_device_type_id, application_categories,
                                description, compliance_document_url, program_type
                            ) VALUES (
                                ?, ?, ?, ?, ?, ?, ?,
                                ?, ?, ?, ?, ?,
                                ?, ?, ?, ?, ?, ?,
                                ?, ?, ?, ?,
                                ?, ?, ?
                            ) ON CONFLICT(url) DO UPDATE SET
                                page_id=COALESCE(excluded.page_id, product_details.page_id),
                                index_in_page=COALESCE(excluded.index_in_page, product_details.index_in_page),
                                id=COALESCE(excluded.id, product_details.id),
                                manufacturer=COALESCE(excluded.manufacturer, product_details.manufacturer),
                                model=COALESCE(excluded.model, product_details.model),
                                device_type=COALESCE(excluded.device_type, product_details.device_type),
                                certificate_id=COALESCE(excluded.certificate_id, product_details.certificate_id),
                                certification_date=COALESCE(excluded.certification_date, product_details.certification_date),
                                software_version=COALESCE(excluded.software_version, product_details.software_version),
                                hardware_version=COALESCE(excluded.hardware_version, product_details.hardware_version),
                                firmware_version=COALESCE(excluded.firmware_version, product_details.firmware_version),
                                specification_version=COALESCE(excluded.specification_version, product_details.specification_version),
                                vid=COALESCE(excluded.vid, product_details.vid),
                                pid=COALESCE(excluded.pid, product_details.pid),
                                family_sku=COALESCE(excluded.family_sku, product_details.family_sku),
                                family_variant_sku=COALESCE(excluded.family_variant_sku, product_details.family_variant_sku),
                                family_id=COALESCE(excluded.family_id, product_details.family_id),
                                tis_trp_tested=COALESCE(excluded.tis_trp_tested, product_details.tis_trp_tested),
                                transport_interface=COALESCE(excluded.transport_interface, product_details.transport_interface),
                                primary_device_type_id=COALESCE(excluded.primary_device_type_id, product_details.primary_device_type_id),
                                application_categories=COALESCE(excluded.application_categories, product_details.application_categories),
                                description=COALESCE(excluded.description, product_details.description),
                                compliance_document_url=COALESCE(excluded.compliance_document_url, product_details.compliance_document_url),
                                program_type=COALESCE(excluded.program_type, product_details.program_type),
                                updated_at=CURRENT_TIMESTAMP
                        "#,
                        )
                        .bind(&detail.url)
                        .bind(detail.page_id)
                        .bind(detail.index_in_page)
                        .bind(detail.id)
                        .bind(detail.manufacturer)
                        .bind(detail.model)
                        .bind(detail.device_type)
                        .bind(detail.certificate_id)
                        .bind(detail.certification_date)
                        .bind(detail.software_version)
                        .bind(detail.hardware_version)
                        .bind(detail.firmware_version)
                        .bind(detail.specification_version)
                        .bind(detail.vid)
                        .bind(detail.pid)
                        .bind(detail.family_sku)
                        .bind(detail.family_variant_sku)
                        .bind(detail.family_id)
                        .bind(detail.tis_trp_tested)
                        .bind(detail.transport_interface)
                        .bind(detail.primary_device_type_id)
                        .bind(detail.application_categories)
                        .bind(detail.description)
                        .bind(detail.compliance_document_url)
                        .bind(detail.program_type)
                        .execute(&pool)
                        .await;

                        // products의 코어 필드 보정
                        let _ = sqlx::query(
                            r#"UPDATE products SET
                                manufacturer = COALESCE(?, manufacturer),
                                model = COALESCE(?, model),
                                certificate_id = COALESCE(?, certificate_id),
                                updated_at = CURRENT_TIMESTAMP
                              WHERE url = ?"#,
                        )
                        .bind(&man_clone)
                        .bind(&model_clone)
                        .bind(&cert_clone)
                        .bind(&detail.url)
                        .execute(&pool)
                        .await;
                        synced_orphans += 1;
                    }
                }
            }
        }
    }

    let payload = serde_json::json!({
        "site_total_pages": site_total_pages,
        "diagnostics": {
            "orphans_products_without_details": orphans_products,
            "orphans_sample_urls": orphan_urls,
            "products_with_nullish_core_fields": nullish_core,
            "out_of_range_page_id": out_of_range_pages,
            "invalid_indices_or_page_id": bad_indices,
            "severe_break": {
                "first_break_at": break_info.first_break_at,
                "break_next": break_info.break_next,
                "gap": break_info.gap,
            }
        },
        "actions": {
            "delete_mismatches": delete_mismatches.unwrap_or(false),
            "deleted_rows": deleted_rows,
            "sync_orphans": sync_orphans.unwrap_or(false),
            "synced_orphans": synced_orphans,
        }
    });

    Ok(CrawlingResponse { success: true, message: "Diagnosis completed".into(), data: Some(payload) })
}

/// 사이트 분석 수행
async fn perform_site_analysis(shared_cache: Option<&SharedStateCache>) -> Result<SiteAnalysisResult, String> {
    info!("🌐 Analyzing site status...");

    // Create necessary components for site analysis
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to load configuration: {}", e))?;

    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;

    // Create database connection and repository for StatusChecker
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("Failed to obtain database pool: {}", e))?;
    let product_repo = std::sync::Arc::new(
        crate::infrastructure::IntegratedProductRepository::new(db_pool),
    );

    let status_checker: std::sync::Arc<dyn crate::domain::services::crawling_services::StatusChecker> =
        std::sync::Arc::new(
            crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                http_client,
                data_extractor,
                config,
                product_repo,
            ),
        );

    // Perform site status check (single-flight via SharedStateCache if available)
    let site_analysis_cached = if let Some(cache) = shared_cache {
        Some(cache.get_or_refresh_site_analysis_singleflight(Some(5), status_checker.clone()).await.map_err(|e| format!("Failed to refresh site status: {}", e))?)
    } else { None };
    let site_status = if let Some(cached) = site_analysis_cached {
        crate::domain::services::SiteStatus {
            is_accessible: true,
            response_time_ms: 0,
            total_pages: cached.total_pages,
            estimated_products: cached.estimated_products,
            products_on_last_page: cached.products_on_last_page,
            last_check_time: cached.analyzed_at,
            health_score: cached.health_score,
            data_change_status: crate::domain::services::crawling_services::SiteDataChangeStatus::Stable { count: cached.estimated_products },
            decrease_recommendation: None,
            crawling_range_recommendation: crate::domain::services::crawling_services::CrawlingRangeRecommendation::Full,
        }
    } else {
        status_checker
            .check_site_status()
            .await
            .map_err(|e| format!("Failed to check site status: {}", e))?
    };

    info!(
        "📊 Site analysis completed: {} pages, {} products on last page, {} estimated total",
        site_status.total_pages, site_status.products_on_last_page, site_status.estimated_products
    );

    Ok(SiteAnalysisResult::new(
        site_status.total_pages,
        site_status.products_on_last_page,
        site_status.estimated_products,
        site::BASE_URL.to_string(),
        1.0, // health_score - TODO: Calculate actual health score
    ))
}

/// 데이터베이스 분석 수행
async fn perform_database_analysis() -> Result<DbAnalysisResult, String> {
    info!("🗄️ Analyzing database state...");

    // Create database connection and repository
    let db_pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| format!("Failed to obtain database pool: {}", e))?;
    let product_repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);

    // Perform actual database analysis
    let analysis = product_repo
        .analyze_database_state()
        .await
        .map_err(|e| format!("Failed to analyze database: {}", e))?;

    info!(
        "📊 Database analysis completed: {} products, cursor at {}:{}",
        analysis.total_products,
        analysis.max_page_id.unwrap_or(-1),
        analysis.max_index_in_page.unwrap_or(-1)
    );

    Ok(analysis)
}

/// 범위 계산 미리보기 (UI 표시용)
async fn calculate_range_preview(
    site_analysis: &SiteAnalysisResult,
    db_analysis: &DbAnalysisResult,
) -> Option<serde_json::Value> {
    // This is just a preview calculation, not the actual range used for crawling
    if db_analysis.is_empty {
        return Some(serde_json::json!({
            "type": "full_crawl",
            "reason": "Empty database - full site crawl recommended",
            "estimated_start_page": site_analysis.total_pages,
            "estimated_end_page": 1,
            "estimated_total_pages": site_analysis.total_pages,
        }));
    }

    // Calculate incremental crawl preview
    let max_page_id = db_analysis.max_page_id?;
    let max_index_in_page = db_analysis.max_index_in_page?;

    // Use site constants for calculation
    let products_per_page = site::PRODUCTS_PER_PAGE as u32;
    let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
    let next_product_index = last_saved_index + 1;
    let next_page = (next_product_index / products_per_page) + 1;

    Some(serde_json::json!({
        "type": "incremental_crawl",
        "reason": format!("Continue from last saved position: page {} index {}", max_page_id, max_index_in_page),
        "last_saved_absolute_index": last_saved_index,
        "next_product_index": next_product_index,
        "next_page_to_crawl": next_page,
        "database_products": db_analysis.total_products,
    }))
}

/// 캐시 상태 조회 (디버그/관리용)
#[tauri::command]
pub async fn get_analysis_cache_status(
    shared_state: State<'_, SharedStateCache>,
) -> Result<serde_json::Value, String> {
    let site_status = shared_state
        .get_valid_site_analysis_async(Some(ttl::SITE_ANALYSIS_TTL_MINUTES))
        .await;
    let db_status = shared_state
        .get_valid_db_analysis_async(Some(ttl::DB_ANALYSIS_TTL_MINUTES))
        .await;
    let range_status = shared_state
        .get_valid_calculated_range_async(ttl::CALCULATED_RANGE_TTL_MINUTES)
        .await;

    Ok(serde_json::json!({
        "cache_status": {
            "has_valid_site_analysis": site_status.is_some(),
            "has_valid_db_analysis": db_status.is_some(),
            "has_valid_calculated_range": range_status.is_some(),
        },
        "ttl_settings": {
            "site_analysis_ttl_minutes": ttl::SITE_ANALYSIS_TTL_MINUTES,
            "db_analysis_ttl_minutes": ttl::DB_ANALYSIS_TTL_MINUTES,
            "calculated_range_ttl_minutes": ttl::CALCULATED_RANGE_TTL_MINUTES,
        },
        "site_constants": {
            "products_per_page": site::PRODUCTS_PER_PAGE,
            "base_url": site::BASE_URL,
            "page_numbering_base": site::PAGE_NUMBERING_BASE,
        }
    }))
}

/// 분석 캐시 수동 클리어 (디버그/관리용)
#[tauri::command]
pub async fn clear_analysis_cache(
    shared_state: State<'_, SharedStateCache>,
) -> Result<String, String> {
    shared_state.clear_all_caches().await;

    info!("🧹 Analysis cache cleared manually");
    Ok("Analysis cache cleared successfully".to_string())
}
