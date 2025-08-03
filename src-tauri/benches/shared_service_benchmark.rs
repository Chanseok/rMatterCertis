//! Priority 1 구현 완료: 공유 서비스 패턴 성능 벤치마크
//! 
//! 목표: 3x 성능 향상 (사이트 상태 확인 중복 제거)
//! - 기존: 각 배치마다 사이트 상태 확인 (3번 × 500ms = 1.5초)
//! - 개선: 세션당 1번만 사이트 상태 확인 (1번 × 500ms = 0.5초)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;
use matter_certis_v2::new_architecture::{
    actor_system::{SessionActor, BatchActor},
    system_config::SystemConfig,
    services::crawling_integration::CrawlingIntegrationService,
};
use matter_certis_v2::infrastructure::crawling_service_impls::{StatusCheckerImpl, CollectorConfig};
use matter_certis_v2::infrastructure::config::AppConfig;

async fn create_mock_integration_service() -> Arc<CrawlingIntegrationService> {
    let app_config = AppConfig::default();
    let collector_config = CollectorConfig::default();
    
    Arc::new(CrawlingIntegrationService::new(
        Arc::new(StatusCheckerImpl::new(collector_config.clone())),
        app_config,
    ).unwrap())
}

/// 기존 방식: 각 배치마다 사이트 상태 확인
async fn benchmark_without_shared_service() {
    let integration_service = create_mock_integration_service().await;
    
    // 3개 배치 시뮬레이션 (각각 사이트 상태 확인)
    for _batch in 0..3 {
        let _ = integration_service.execute_list_collection_stage_with_site_check(
            vec![1, 2, 3],
            tokio_util::sync::CancellationToken::new(),
        ).await;
    }
}

/// 개선된 방식: 공유 서비스 패턴으로 한 번만 사이트 상태 확인
async fn benchmark_with_shared_service() {
    let config = Arc::new(SystemConfig::default());
    let session_actor = SessionActor::new(
        "bench_session".to_string(),
        config,
        3, // 3개 배치
    );
    
    // 첫 번째 배치에서만 사이트 상태 확인, 나머지는 재사용
    let _ = session_actor.execute_real_crawling_stage(
        vec![1, 2, 3],
        vec![4, 5, 6], 
        vec![7, 8, 9],
        10,
        tokio_util::sync::CancellationToken::new(),
    ).await;
}

fn performance_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("사이트 상태 확인 - 기존 방식 (3번 확인)", |b| {
        b.to_async(&rt).iter(|| {
            black_box(benchmark_without_shared_service())
        })
    });
    
    c.bench_function("사이트 상태 확인 - 공유 서비스 패턴 (1번 확인)", |b| {
        b.to_async(&rt).iter(|| {
            black_box(benchmark_with_shared_service())
        })
    });
}

criterion_group!(benches, performance_comparison);
criterion_main!(benches);
