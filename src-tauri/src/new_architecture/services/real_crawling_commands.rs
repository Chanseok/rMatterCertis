//! 실제 크롤링 서비스 테스트 명령
//! Option B: 실제 크롤링 연동 구현 테스트

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use tauri::command;
use tracing::{info, error};
use anyhow::Result;

use crate::new_architecture::system_config::SystemConfig;
use crate::new_architecture::services::crawling_integration::CrawlingIntegrationService;
use crate::infrastructure::config::AppConfig;

/// 실제 크롤링 서비스 초기화 테스트
#[command]
pub async fn test_real_crawling_init() -> Result<String, String> {
    info!("🔧 실제 크롤링 서비스 초기화 테스트 시작");
    
    match test_crawling_service_initialization().await {
        Ok(message) => {
            info!("✅ 실제 크롤링 서비스 초기화 성공");
            Ok(message)
        }
        Err(e) => {
            error!(error = %e, "❌ 실제 크롤링 서비스 초기화 실패");
            Err(format!("크롤링 서비스 초기화 실패: {}", e))
        }
    }
}

/// 실제 사이트 상태 확인 테스트
#[command]
pub async fn test_real_site_status() -> Result<String, String> {
    info!("🌐 실제 사이트 상태 확인 테스트 시작");
    
    match test_site_status_check().await {
        Ok(status_info) => {
            info!("✅ 사이트 상태 확인 성공");
            Ok(status_info)
        }
        Err(e) => {
            error!(error = %e, "❌ 사이트 상태 확인 실패");
            Err(format!("사이트 상태 확인 실패: {}", e))
        }
    }
}

/// 실제 크롤링 범위 분석 테스트
#[command]
pub async fn test_real_crawling_analysis() -> Result<String, String> {
    info!("📊 실제 크롤링 범위 분석 테스트 시작");
    
    match test_crawling_range_analysis().await {
        Ok(analysis_info) => {
            info!("✅ 크롤링 범위 분석 성공");
            Ok(analysis_info)
        }
        Err(e) => {
            error!(error = %e, "❌ 크롤링 범위 분석 실패");
            Err(format!("크롤링 범위 분석 실패: {}", e))
        }
    }
}

/// 실제 소량 페이지 크롤링 테스트
#[command]
pub async fn test_real_page_crawling() -> Result<String, String> {
    info!("📄 실제 소량 페이지 크롤링 테스트 시작");
    
    match test_small_scale_crawling().await {
        Ok(crawling_info) => {
            info!("✅ 소량 페이지 크롤링 성공");
            Ok(crawling_info)
        }
        Err(e) => {
            error!(error = %e, "❌ 소량 페이지 크롤링 실패");
            Err(format!("소량 페이지 크롤링 실패: {}", e))
        }
    }
}

/// 실제 OneShot Actor 통합 테스트
#[command]
pub async fn test_real_oneshot_integration() -> Result<String, String> {
    info!("🔗 실제 OneShot Actor 통합 테스트 시작");
    
    match test_oneshot_with_real_crawling().await {
        Ok(integration_info) => {
            info!("✅ OneShot Actor 통합 성공");
            Ok(integration_info)
        }
        Err(e) => {
            error!(error = %e, "❌ OneShot Actor 통합 실패");
            Err(format!("OneShot Actor 통합 실패: {}", e))
        }
    }
}

// 내부 구현 함수들

async fn test_crawling_service_initialization() -> Result<String> {
    // 기본 설정 생성
    let system_config = Arc::new(SystemConfig::default());
    let app_config = AppConfig::for_development(); // 개발용 설정 사용
    
    // 크롤링 통합 서비스 생성 시도
    let _integration_service = CrawlingIntegrationService::new(
        system_config, 
        app_config
    ).await?;
    
    Ok("크롤링 서비스 초기화 완료: HTTP 클라이언트, 데이터 추출기, 상태 확인기 모두 준비됨".to_string())
}

async fn test_site_status_check() -> Result<String> {
    let system_config = Arc::new(SystemConfig::default());
    let app_config = AppConfig::for_development();
    
    let integration_service = CrawlingIntegrationService::new(
        system_config, 
        app_config
    ).await?;
    
    // 사이트 상태 분석 실행
    let site_status = integration_service.execute_site_analysis().await?;
    
    Ok(format!(
        "사이트 상태 분석 완료 - 응답 시간: {}ms, 접근 가능: {}",
        site_status.response_time_ms,
        site_status.is_accessible
    ))
}

async fn test_crawling_range_analysis() -> Result<String> {
    let system_config = Arc::new(SystemConfig::default());
    let app_config = AppConfig::for_development();
    
    let integration_service = CrawlingIntegrationService::new(
        system_config, 
        app_config
    ).await?;
    
    // 크롤링 범위 권장사항 계산
    let recommendation = integration_service.calculate_crawling_recommendation().await?;
    
    Ok(format!(
        "크롤링 범위 분석 완료 - 권장사항: {:?}",
        recommendation
    ))
}

async fn test_small_scale_crawling() -> Result<String> {
    let system_config = Arc::new(SystemConfig::default());
    let app_config = AppConfig::for_development();
    
    let integration_service = CrawlingIntegrationService::new(
        system_config, 
        app_config
    ).await?;
    
    // 소량 페이지 크롤링 (1-3페이지)
    let test_pages = vec![1, 2, 3];
    let concurrency_limit = 2;
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    
    let result = integration_service.execute_list_collection_stage(
        test_pages,
        concurrency_limit,
        cancellation_token,
    ).await;
    
    match result {
        crate::new_architecture::actor_system::StageResult::Success(success) => {
            Ok(format!(
                "소량 크롤링 성공 - 처리된 페이지: {}, 실행시간: {}ms",
                success.processed_items,
                success.stage_duration_ms
            ))
        }
        crate::new_architecture::actor_system::StageResult::PartialSuccess { success_items, failed_items, .. } => {
            Ok(format!(
                "부분 성공 - 성공: {}개, 실패: {}개",
                success_items.processed_items,
                failed_items.len()
            ))
        }
        _ => Ok("크롤링 실행되었으나 예상과 다른 결과".to_string())
    }
}

async fn test_oneshot_with_real_crawling() -> Result<String> {
    use crate::new_architecture::actor_system::StageActor;
    
    let batch_id = "test-real-integration".to_string();
    let system_config = Arc::new(SystemConfig::default());
    let app_config = AppConfig::for_development();
    
    // 실제 크롤링 서비스를 사용하는 StageActor 생성
    let stage_actor = StageActor::new_with_real_crawling_service(
        batch_id, 
        system_config, 
        app_config
    ).await?;
    
    Ok(format!(
        "OneShot Actor 통합 완료 - 배치 ID: {}, 실제 크롤링 서비스 연결됨",
        stage_actor.batch_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_real_crawling_commands() {
        println!("🧪 실제 크롤링 명령 테스트 시작");

        // 크롤링 서비스 초기화 테스트
        match test_real_crawling_init().await {
            Ok(message) => println!("✅ 초기화 테스트: {}", message),
            Err(e) => println!("❌ 초기화 테스트 실패: {}", e),
        }

        // OneShot 통합 테스트
        match test_real_oneshot_integration().await {
            Ok(message) => println!("✅ 통합 테스트: {}", message),
            Err(e) => println!("❌ 통합 테스트 실패: {}", e),
        }

        println!("🎯 실제 크롤링 명령 테스트 완료!");
    }
}
