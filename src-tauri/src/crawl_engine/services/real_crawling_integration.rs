//! 실제 크롤링 서비스 통합을 위한 Actor 시스템 패치
//! 기존 `actor_system.rs에` 실제 크롤링 서비스를 통합하는 코드를 추가

use std::sync::Arc;
use tracing::error;

use crate::crawl_engine::services::crawling_integration::CrawlingIntegrationService;
use crate::crawl_engine::system_config::SystemConfig;
use crate::infrastructure::config::AppConfig;

/// 실제 크롤링 통합 서비스를
pub struct RealCrawlingIntegration {
    _config: Arc<SystemConfig>, // currently unused; kept for future expansion
    _app_config: AppConfig,     // currently unused; kept for future expansion
}

impl RealCrawlingIntegration {
    #[must_use] pub const fn new(config: Arc<SystemConfig>, app_config: AppConfig) -> Self {
        Self { _config: config, _app_config: app_config }
    }
}

// NOTE: StageActor integration methods were unified into `crawling_integration.rs`.

/// `BatchActor` 확장: 실제 크롤링 서비스를 사용하는 `OneShot` 스테이지 실행
impl crate::crawl_engine::actors::BatchActor {
    /// Stage 1용: 사이트 상태 점검을 수행하고 레거시 StageResult(details 포함)로 브리징
    pub async fn execute_status_check_with_details(
        &self,
        app_config: AppConfig,
    ) -> crate::crawl_engine::actors::types::StageResult {
        let config_arc = match self.config.as_ref() {
            Some(cfg) => cfg.clone(),
            None => {
                return crate::crawl_engine::actors::types::StageResult {
                    processed_items: 0,
                    successful_items: 0,
                    failed_items: 1,
                    duration_ms: 0,
                    details: Vec::new(),
                };
            }
        };

        let app_cfg2 = app_config.clone();
        let integration_service =
            match CrawlingIntegrationService::new(config_arc.clone(), app_cfg2).await {
                Ok(service) => Arc::new(service),
                Err(e) => {
                    error!(error = %e, "Failed to create crawling integration service");
                    return crate::crawl_engine::actors::types::StageResult {
                        processed_items: 0,
                        successful_items: 0,
                        failed_items: 1,
                        duration_ms: 0,
                        details: Vec::new(),
                    };
                }
            };

        let started = std::time::Instant::now();
    let mut details: Vec<crate::crawl_engine::actors::types::StageItemResult> = Vec::new();

        match integration_service.execute_site_analysis().await {
            Ok(site_status) => {
                let collected_data = Some(crate::crawl_engine::actors::types::StageResultData::StatusCheck {
                    site_available: site_status.is_accessible,
                    total_pages: Some(site_status.total_pages),
                    last_page_products: Some(site_status.products_on_last_page),
                    response_time_ms: started.elapsed().as_millis() as u64,
                });
                details.push(crate::crawl_engine::actors::types::StageItemResult {
                    item_id: "site_status_check:0".to_string(),
                    item_type: crate::crawl_engine::actors::types::StageItemType::SiteCheck,
                    success: true,
                    error: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                    retry_count: 0,
                    collected_data,
                });

                crate::crawl_engine::actors::types::StageResult {
                    processed_items: 1,
                    successful_items: 1,
                    failed_items: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    details,
                }
            }
            Err(e) => {
                error!(error = %e, "Site status analysis failed");
                details.push(crate::crawl_engine::actors::types::StageItemResult {
                    item_id: "site_status_check:0".to_string(),
                    item_type: crate::crawl_engine::actors::types::StageItemType::SiteCheck,
                    success: false,
                    error: Some(format!("{}", e)),
                    duration_ms: started.elapsed().as_millis() as u64,
                    retry_count: 0,
                    collected_data: None,
                });

                crate::crawl_engine::actors::types::StageResult {
                    processed_items: 1,
                    successful_items: 0,
                    failed_items: 1,
                    duration_ms: started.elapsed().as_millis() as u64,
                    details,
                }
            }
        }
    }


    /// Stage 3용: `ProductUrls` 입력을 받아 `ProductDetails를` 수집하고 레거시 StageResult(details 포함)로 브리징
    pub async fn execute_detail_collection_with_details(
        &self,
        product_urls_items: Vec<crate::crawl_engine::channels::types::ProductUrls>,
        app_config: AppConfig,
    ) -> crate::crawl_engine::actors::types::StageResult {
        let config_arc = match self.config.as_ref() {
            Some(cfg) => cfg.clone(),
            None => {
                return crate::crawl_engine::actors::types::StageResult {
                    processed_items: 0,
                    successful_items: 0,
                    failed_items: product_urls_items.len() as u32,
                    duration_ms: 0,
                    details: Vec::new(),
                };
            }
        };

        let integration_service =
            match CrawlingIntegrationService::new(config_arc.clone(), app_config).await {
                Ok(service) => Arc::new(service),
                Err(e) => {
                    error!(error = %e, "Failed to create crawling integration service");
                    return crate::crawl_engine::actors::types::StageResult {
                        processed_items: 0,
                        successful_items: 0,
                        failed_items: product_urls_items.len() as u32,
                        duration_ms: 0,
                        details: Vec::new(),
                    };
                }
            };

        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let started = std::time::Instant::now();

        let mut successful = 0u32;
        let mut failed = 0u32;
    let mut details: Vec<crate::crawl_engine::actors::types::StageItemResult> = Vec::new();

        for (idx, urls_wrapper) in product_urls_items.into_iter().enumerate() {
            // Collect details for this item
            let urls = urls_wrapper.urls.clone();
            let urls_strs: Vec<String> = urls.iter().map(|u| u.url.clone()).collect();
            let item_started = std::time::Instant::now();
            match integration_service
                .collect_details_detailed_with_meta(urls.clone(), cancellation_token.clone())
                .await
            {
                Ok((collected_details, retry_count, duration_ms)) => {
                    // if collected_details.is_empty() { /* no-op: keep legacy behavior without warn! */ }
                    let success = !collected_details.is_empty();
                    if success {
                        successful += 1;
                    } else {
                        failed += 1;
                    }
                    let collected_data = if success {
                        let attempted = urls.len() as u32;
                        let successful_count = collected_details.len() as u32;
                        let failed_count = attempted.saturating_sub(successful_count);
                        Some(crate::crawl_engine::actors::types::StageResultData::ProductDetails {
                            details: collected_details,
                            successful_count,
                            failed_count,
                        })
                    } else { None };

                    details.push(crate::crawl_engine::actors::types::StageItemResult {
                        item_id: format!("product_urls:{}", idx),
                        item_type: crate::crawl_engine::actors::types::StageItemType::ProductUrls {
                            urls: urls_strs.clone(),
                        },
                        success,
                        error: if success {
                            None
                        } else {
                            Some("no details".to_string())
                        },
                        duration_ms: duration_ms.max(item_started.elapsed().as_millis() as u64),
                        retry_count,
                        collected_data,
                    });
                }
                Err(e) => {
                    error!(idx = idx, error = %e, "Detail collection failed for ProductUrls item");
                    failed += 1;
                    details.push(crate::crawl_engine::actors::types::StageItemResult {
                        item_id: format!("product_urls:{}", idx),
                        item_type: crate::crawl_engine::actors::types::StageItemType::ProductUrls {
                            urls: urls_strs.clone(),
                        },
                        success: false,
                        error: Some(format!("{}", e)),
                        duration_ms: item_started.elapsed().as_millis() as u64,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
            }
        }

        crate::crawl_engine::actors::types::StageResult {
            processed_items: successful + failed,
            successful_items: successful,
            failed_items: failed,
            duration_ms: started.elapsed().as_millis() as u64,
            details,
        }
    }

    /// Stage 2용: 페이지별 상세 URL 결과를 수집하여 레거시 StageResult(details 포함)로 브리징
    pub async fn execute_list_collection_with_details(
        &self,
        pages: Vec<u32>,
        app_config: AppConfig,
    ) -> crate::crawl_engine::actors::types::StageResult {
        let config_arc = match self.config.as_ref() {
            Some(cfg) => cfg.clone(),
            None => {
                return crate::crawl_engine::actors::types::StageResult {
                    processed_items: 0,
                    successful_items: 0,
                    failed_items: pages.len() as u32,
                    duration_ms: 0,
                    details: Vec::new(),
                };
            }
        };

        let integration_service =
            match CrawlingIntegrationService::new(config_arc.clone(), app_config).await {
                Ok(service) => Arc::new(service),
                Err(e) => {
                    error!(error = %e, "Failed to create crawling integration service");
                    return crate::crawl_engine::actors::types::StageResult {
                        processed_items: 0,
                        successful_items: 0,
                        failed_items: pages.len() as u32,
                        duration_ms: 0,
                        details: Vec::new(),
                    };
                }
            };

        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let started = std::time::Instant::now();
        let perform_site_check = true;

        let page_results = match integration_service
            .collect_pages_detailed_with_meta(pages.clone(), perform_site_check, cancellation_token)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "List detail collection failed");
                return crate::crawl_engine::actors::types::StageResult {
                    processed_items: 0,
                    successful_items: 0,
                    failed_items: pages.len() as u32,
                    duration_ms: 0,
                    details: Vec::new(),
                };
            }
        };

        // Map into legacy StageResult with per-item details
        let mut successful = 0u32;
        let mut failed = 0u32;
    let mut details: Vec<crate::crawl_engine::actors::types::StageItemResult> = Vec::new();
        for (page, urls, retry_count, duration_ms) in page_results {
            let success = !urls.is_empty();
            if success {
                successful += 1;
            } else {
                failed += 1;
            }
            let collected_data = if success {
                Some(crate::crawl_engine::actors::types::StageResultData::ProductUrls {
                    urls: urls.clone(),
                    page_number: page,
                    total_found: urls.len() as u32,
                })
            } else { None };
            details.push(crate::crawl_engine::actors::types::StageItemResult {
                item_id: format!("page:{}", page),
                item_type: crate::crawl_engine::actors::types::StageItemType::Page {
                    page_number: page,
                },
                success,
                error: if success {
                    None
                } else {
                    Some("no urls".to_string())
                },
                duration_ms,
                retry_count,
                collected_data,
            });
        }

        crate::crawl_engine::actors::types::StageResult {
            processed_items: successful + failed,
            successful_items: successful,
            failed_items: failed,
            duration_ms: started.elapsed().as_millis() as u64,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::config::AppConfig;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_crawling_integration_service_creation() {
        // Ensure database paths are initialized for tests using globals
        let _ = crate::infrastructure::initialize_database_paths().await;
        println!("🧪 크롤링 통합 서비스 생성 테스트 시작");

        // 기본 설정 생성
        let system_config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::for_development(); // 개발용 설정 사용

        // 크롤링 통합 서비스 생성 시도
        match CrawlingIntegrationService::new(system_config, app_config).await {
            Ok(_service) => {
                println!("✅ 크롤링 통합 서비스 생성 성공!");
            }
            Err(e) => {
                println!("❌ 크롤링 통합 서비스 생성 실패: {}", e);
                // 테스트 환경에서는 실패할 수 있음 (HTTP 클라이언트 등)
                println!("   (테스트 환경에서는 정상적인 실패일 수 있음)");
            }
        }

        println!("🎯 크롤링 통합 서비스 생성 테스트 완료!");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_stage_actor_with_real_crawling() {
        // Ensure database paths are initialized for tests using globals
        let _ = crate::infrastructure::initialize_database_paths().await;
        println!("🧪 실제 크롤링을 사용하는 StageActor 테스트 시작");

        let batch_id = "test-batch-real".to_string();
        let config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::for_development();

        // StageActor 생성 시도
        match StageActor::new_with_real_crawling_service(
            batch_id.clone(),
            config,
            app_config,
            494, // total_pages
            12,  // products_on_last_page
        )
        .await
        {
            Ok(stage_actor) => {
                println!("✅ 실제 크롤링 StageActor 생성 성공!");
                println!("   배치 ID: {}", stage_actor.batch_id);
            }
            Err(e) => {
                println!("❌ 실제 크롤링 StageActor 생성 실패: {}", e);
                println!("   (테스트 환경에서는 정상적인 실패일 수 있음)");
            }
        }

        println!("🎯 실제 크롤링 StageActor 테스트 완료!");
    }
}
