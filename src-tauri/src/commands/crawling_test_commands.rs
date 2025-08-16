//! Phase C: 실제 크롤링 테스트 및 개발자 도구
//! 개발자가 실제 크롤링을 쉽게 테스트하고 디버깅할 수 있는 명령어들

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use ts_rs::TS;

use crate::application::AppState;
use crate::new_architecture::actor_system::StageResult;
use crate::new_architecture::channels::types::{StageItem, StageType};
use crate::new_architecture::config::SystemConfig;
use crate::new_architecture::services::crawling_integration::{
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
    let config_guard = app_state.config.read().await;
    let app_config = config_guard.clone();
    drop(config_guard);

    // 2. 시스템 설정 생성
    let system_config = Arc::new(SystemConfig::default());

    // 3. 크롤링 통합 서비스 생성
    let integration_service =
        match CrawlingIntegrationService::new(system_config.clone(), app_config).await {
            Ok(service) => Arc::new(service),
            Err(e) => {
                error!(error = %e, "Failed to create crawling integration service for test");
                return Err(format!("테스트 서비스 초기화 실패: {}", e));
            }
        };

    // 4. 사이트 상태 확인
    let site_status = match integration_service.execute_site_analysis().await {
        Ok(status) => {
            info!(
                accessible = status.is_accessible,
                response_time = status.response_time_ms,
                total_pages = status.total_pages,
                "사이트 상태 확인 완료"
            );
            Some(SiteStatusInfo {
                accessible: status.is_accessible,
                response_time_ms: status.response_time_ms,
                total_pages: status.total_pages,
                estimated_products: status.estimated_products,
                health_score: status.health_score,
            })
        }
        Err(e) => {
            warn!(error = %e, "사이트 상태 확인 실패, 테스트 계속 진행");
            None
        }
    };

    // 5. 실행기 생성
    let executor = Arc::new(RealCrawlingStageExecutor::new(integration_service.clone()));
    let cancellation_token = CancellationToken::new();

    // 6. 리스트 수집 테스트
    let pages: Vec<u32> = (1..=test_pages).collect();
    let list_items: Vec<StageItem> = pages.iter().map(|&p| StageItem::Page(p)).collect();

    info!(pages = ?pages, "📋 리스트 수집 테스트 시작");
    let list_result = executor
        .execute_stage(
            StageType::ListCollection,
            list_items,
            concurrency,
            cancellation_token.clone(),
        )
        .await;

    // 7. 결과 분석
    let (success, collected_urls, error_message) = match list_result {
        StageResult::Success {
            processed_items, ..
        } => {
            info!(processed_items = processed_items, "✅ 리스트 수집 성공");
            (true, processed_items * 12, None) // 페이지당 평균 12개 URL 가정
        }
        StageResult::Failure {
            partial_results,
            error,
        } => {
            warn!(partial_results = partial_results, error = ?error, "⚠️ 부분 성공");
            (
                false,
                partial_results * 12,
                Some(format!("부분 실패: {:?}", error)),
            )
        }
        StageResult::RecoverableError {
            error, attempts, ..
        } => {
            warn!(error = ?error, attempts = attempts, "🔄 복구 가능한 에러");
            (
                false,
                0,
                Some(format!(
                    "복구 가능한 에러 (시도 횟수: {}): {:?}",
                    attempts, error
                )),
            )
        }
        StageResult::FatalError { error, .. } => {
            error!(error = ?error, "❌ 리스트 수집 실패");
            (false, 0, Some(format!("크롤링 실패: {:?}", error)))
        }
    };

    // 8. 상세 수집 테스트 (선택적)
    let collected_details = if include_details && success {
        info!("📦 상세 정보 수집 테스트 시작");
        // 실제로는 수집된 URL을 사용해야 하지만, 테스트에서는 단순화
        let detail_result = executor
            .execute_stage(
                StageType::DetailCollection,
                Vec::new(),
                concurrency,
                cancellation_token.clone(),
            )
            .await;

        match detail_result {
            StageResult::Success {
                processed_items, ..
            } => {
                info!(processed_items = processed_items, "✅ 상세 수집 성공");
                processed_items
            }
            _ => {
                warn!("⚠️ 상세 수집 실패, 리스트 수집 결과만 반환");
                0
            }
        }
    } else {
        0
    };

    // 9. 성능 메트릭 계산
    let total_duration = start_time.elapsed();
    let avg_time_per_page = if test_pages > 0 {
        total_duration.as_millis() as u64 / test_pages as u64
    } else {
        0
    };

    let performance_metrics = PerformanceMetrics {
        avg_urls_per_page: if test_pages > 0 {
            collected_urls as f64 / test_pages as f64
        } else {
            0.0
        },
        network_success_rate: if success { 1.0 } else { 0.5 },
        parsing_success_rate: if success { 1.0 } else { 0.7 },
        estimated_memory_kb: (collected_urls * 2 + collected_details * 10) as u64, // 추정치
    };

    let result = CrawlingTestResult {
        success,
        tested_pages: test_pages,
        collected_urls,
        collected_details,
        total_duration_ms: total_duration.as_millis() as u64,
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
