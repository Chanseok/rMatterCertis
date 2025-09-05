//! Phase C: 실제 크롤링 테스트 및 개발자 도구
//! 개발자가 실제 크롤링을 쉽게 테스트하고 디버깅할 수 있는 명령어들

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use ts_rs::TS;

use crate::application::AppState;
use crate::crawl_engine::actor_system::StageResult;
use crate::crawl_engine::channels::types::{StageItem, StageType};
use crate::crawl_engine::config::SystemConfig;
use crate::crawl_engine::services::crawling_integration::{
    CrawlingIntegrationService, RealCrawlingStageExecutor,
};

/// 간단한 크롤링 테스트 요청
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QuickCrawlingTest {
    /// 테스트할 페이지 수 (기본: 2)
    pub test_pages: Option<u32>,
    /// 동시성 (기본: 2)
    pub concurrency: Option<u32>,
    /// 상세 수집 포함 여부 (기본: false)
    pub include_details: Option<bool>,
}

/// 크롤링 테스트 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingTestResult {
    /// 테스트 성공 여부
    pub success: bool,
    /// 테스트한 페이지 수
    pub tested_pages: u32,
    /// 수집된 URL 수
    pub collected_urls: u32,
    /// 수집된 상세 정보 수
    pub collected_details: u32,
    /// 총 소요 시간 (밀리초)
    pub total_duration_ms: u64,
    /// 평균 페이지당 소요 시간 (밀리초)
    pub avg_time_per_page_ms: u64,
    /// 에러 메시지 (실패 시)
    pub error_message: Option<String>,
    /// 사이트 상태 정보
    pub site_status: Option<SiteStatusInfo>,
    /// 성능 메트릭
    pub performance_metrics: PerformanceMetrics,
}

/// 사이트 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteStatusInfo {
    /// 사이트 접근 가능 여부
    pub accessible: bool,
    /// 응답 시간 (밀리초)
    pub response_time_ms: u64,
    /// 전체 페이지 수
    pub total_pages: u32,
    /// 예상 제품 수
    pub estimated_products: u32,
    /// 건강 점수 (0.0-1.0)
    pub health_score: f64,
}

/// 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerformanceMetrics {
    /// 페이지당 평균 URL 수
    pub avg_urls_per_page: f64,
    /// 네트워크 성공률 (0.0-1.0)
    pub network_success_rate: f64,
    /// 파싱 성공률 (0.0-1.0)
    pub parsing_success_rate: f64,
    /// 메모리 사용량 추정 (KB)
    pub estimated_memory_kb: u64,
}

/// 🧪 빠른 크롤링 테스트 (개발자용)
#[tauri::command]
/// # Errors
/// Returns an error string if test execution fails.
pub async fn quick_crawling_test(
    app: AppHandle,
    test_request: QuickCrawlingTest,
) -> Result<CrawlingTestResult, String> {
    let start_time = std::time::Instant::now();

    info!("🧪 Starting quick crawling test");

    // 기본값 설정
    let test_pages = test_request.test_pages.unwrap_or(2);
    let concurrency = test_request.concurrency.unwrap_or(2);
    let include_details = test_request.include_details.unwrap_or(false);

    // 1. 설정 로드
    let app_state = app.state::<AppState>();
    #![cfg(feature = "dev-tools")]
    //! Deprecated shim: re-exports the devtools crawling test commands.
    //! This file remains only to keep the legacy path `commands::crawling_test_commands`
    //! resolving to the gated devtools implementation when compiled.

    // Re-export all items from the devtools implementation.
    pub use crate::commands::devtools::crawling_test_commands::*;
        estimated_memory_kb: u64::from(collected_urls * 2 + collected_details * 10), // 추정치
    };

    let result = CrawlingTestResult {
        success,
        tested_pages: test_pages,
        collected_urls,
        collected_details,
    total_duration_ms: u64::try_from(total_duration.as_millis()).unwrap_or(u64::MAX),
        avg_time_per_page_ms: avg_time_per_page,
        error_message,
        site_status,
        performance_metrics,
    };

    info!(
        success = success,
        duration_ms = total_duration.as_millis(),
        collected_urls = collected_urls,
        "🧪 빠른 크롤링 테스트 완료"
    );

    Ok(result)
}

/// 🔍 사이트 상태만 확인 (가장 빠른 테스트)
#[tauri::command]
/// # Errors
/// Returns an error string if site status check fails.
pub async fn check_site_status_only(app: AppHandle) -> Result<SiteStatusInfo, String> {
    info!("🔍 사이트 상태 확인 시작");

    // 설정 로드
    let app_state = app.state::<AppState>();
    let config_guard = app_state.config.read().await;
    let app_config = config_guard.clone();
    drop(config_guard);

    // 크롤링 통합 서비스 생성
    let system_config = Arc::new(SystemConfig::default());
    let integration_service = match CrawlingIntegrationService::new(system_config, app_config).await
    {
        Ok(service) => service,
        Err(e) => {
            error!(error = %e, "Failed to create integration service");
            return Err(format!("서비스 초기화 실패: {}", e));
        }
    };

    // 사이트 상태 확인
    match integration_service.execute_site_analysis().await {
        Ok(status) => {
            info!(
                accessible = status.is_accessible,
                response_time = status.response_time_ms,
                total_pages = status.total_pages,
                estimated_products = status.estimated_products,
                "✅ 사이트 상태 확인 완료"
            );

            Ok(SiteStatusInfo {
                accessible: status.is_accessible,
                response_time_ms: status.response_time_ms,
                total_pages: status.total_pages,
                estimated_products: status.estimated_products,
                health_score: status.health_score,
            })
        }
        Err(e) => {
            error!(error = %e, "사이트 상태 확인 실패");
            Err(format!("사이트 상태 확인 실패: {}", e))
        }
    }
}

/// 📊 크롤링 성능 벤치마크 테스트
#[tauri::command]
/// # Errors
/// Returns an error string if benchmark execution fails.
pub async fn crawling_performance_benchmark(
    app: AppHandle,
) -> Result<Vec<CrawlingTestResult>, String> {
    info!("📊 크롤링 성능 벤치마크 시작");

    let mut results = Vec::new();

    // 다양한 설정으로 테스트
    let test_configs = vec![
        QuickCrawlingTest {
            test_pages: Some(1),
            concurrency: Some(1),
            include_details: Some(false),
        },
        QuickCrawlingTest {
            test_pages: Some(2),
            concurrency: Some(2),
            include_details: Some(false),
        },
        QuickCrawlingTest {
            test_pages: Some(3),
            concurrency: Some(3),
            include_details: Some(false),
        },
        QuickCrawlingTest {
            test_pages: Some(2),
            concurrency: Some(1),
            include_details: Some(true),
        },
    ];

    for (i, config) in test_configs.into_iter().enumerate() {
        info!(test_number = i + 1, config = ?config, "벤치마크 테스트 실행 중");

        match quick_crawling_test(app.clone(), config.clone()).await {
            Ok(result) => {
                info!(
                    test_number = i + 1,
                    success = result.success,
                    duration_ms = result.total_duration_ms,
                    "벤치마크 테스트 완료"
                );
                results.push(result);
            }
            Err(e) => {
                warn!(test_number = i + 1, error = %e, "벤치마크 테스트 실패");
                // 실패한 경우에도 빈 결과 추가
                results.push(CrawlingTestResult {
                    success: false,
                    tested_pages: config.test_pages.unwrap_or(0),
                    collected_urls: 0,
                    collected_details: 0,
                    total_duration_ms: 0,
                    avg_time_per_page_ms: 0,
                    error_message: Some(e),
                    site_status: None,
                    performance_metrics: PerformanceMetrics {
                        avg_urls_per_page: 0.0,
                        network_success_rate: 0.0,
                        parsing_success_rate: 0.0,
                        estimated_memory_kb: 0,
                    },
                });
            }
        }

        // 테스트 간 잠시 대기
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    info!(completed_tests = results.len(), "📊 성능 벤치마크 완료");
    Ok(results)
}
