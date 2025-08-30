//! TypeScript 타입 생성을 위한 유틸리티
//!
//! Phase 4: ts-rs를 활용한 자동 타입 생성
use ts_rs::TS;

/// TypeScript 바인딩 생성 함수
/// 프론트엔드와 백엔드 간 타입 동기화를 위해 TS 파일을 생성합니다.
pub fn generate_ts_bindings() -> Result<(), Box<dyn std::error::Error>> {
    // 명시적으로 핵심 타입들을 내보내 TS 스키마를 강제 동기화한다.
    // build.rs의 TS_RS_EXPORT_DIR도 동작하지만, 여기서 보완적으로 export_all_to를 호출해 최신 스키마를 보장한다.
    let out_dir = "../src/types/generated";

    // Core actor system types
    crate::crawl_engine::actors::types::AppEvent::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::ActorCommand::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::StageResult::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::StageItemResult::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::CrawlingConfig::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::BatchConfig::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::StageType::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::StageItem::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::CrawlPhase::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::SessionSummary::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::PerformanceMetrics::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::SimpleMetrics::export_all_to(out_dir)?;
    crate::crawl_engine::actors::types::TaskKind::export_all_to(out_dir)?;

    println!("TypeScript bindings generated successfully!");
    Ok(())
}

/// 개발 중 타입 변경 감지를 위한 헬퍼
pub fn watch_type_changes() {
    println!("👀 Watching for type changes in Actor system...");

    // 개발 모드에서 타입 변경 감지 시 자동 재생성
    // 실제 구현은 file system watcher를 사용할 수 있음
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_type_generation() {
        // 기존 함수명으로 수정
        let result = generate_ts_bindings();
        assert!(result.is_ok(), "TS binding generation should succeed");
    }
}
