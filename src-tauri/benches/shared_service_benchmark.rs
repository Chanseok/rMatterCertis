//! Priority 1 구현 완료: 공유 서비스 패턴 성능 벤치마크
//!
//! 목표: 3x 성능 향상 (사이트 상태 확인 중복 제거)
//! - 기존: 각 배치마다 사이트 상태 확인 (3번 × 500ms = 1.5초)
//! - 개선: 세션당 1번만 사이트 상태 확인 (1번 × 500ms = 0.5초)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use matter_certis_v2_lib::infrastructure::config::AppConfig;
use matter_certis_v2_lib::new_architecture::config::SystemConfig;
use matter_certis_v2_lib::new_architecture::services::crawling_integration::CrawlingIntegrationService;
use std::sync::Arc;
use tokio::runtime::Runtime;

async fn create_integration_service() -> Arc<CrawlingIntegrationService> {
    let app_config = AppConfig::default();
    let sys_config = Arc::new(SystemConfig::default());
    Arc::new(
        CrawlingIntegrationService::new(sys_config, app_config)
            .await
            .expect("failed to init integration service"),
    )
}

/// 기존 방식: 각 배치마다 사이트 상태 확인
async fn benchmark_without_shared_service() {
    // Simulate 3 batches; create a new service each time (no sharing)
    for _batch in 0..3 {
        let integration_service = create_integration_service().await;
        let _ = integration_service
            .execute_list_collection_stage(
                Vec::new(),
                1,
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
    }
}

/// 개선된 방식: 공유 서비스 패턴으로 한 번만 사이트 상태 확인
async fn benchmark_with_shared_service() {
    // Reuse a single service across batches
    let integration_service = create_integration_service().await;
    for _batch in 0..3 {
        let _ = integration_service
            .execute_list_collection_stage(
                Vec::new(),
                1,
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
    }
}

fn performance_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("사이트 상태 확인 - 기존 방식 (3번 확인)", |b| {
        b.iter(|| rt.block_on(black_box(benchmark_without_shared_service())))
    });

    c.bench_function("사이트 상태 확인 - 공유 서비스 패턴 (1번 확인)", |b| {
        b.iter(|| rt.block_on(black_box(benchmark_with_shared_service())))
    });
}

criterion_group!(benches, performance_comparison);
criterion_main!(benches);
