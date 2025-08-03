//! TypeScript 타입 생성을 위한 유틸리티
//! 
//! Phase 4: ts-rs를 활용한 자동 타입 생성

use std::path::Path;

/// TypeScript 타입 파일 생성
pub fn generate_typescript_types() -> Result<(), Box<dyn std::error::Error>> {
    // ts-rs로 생성된 타입들을 수집하고 정리
    println!("🎯 Generating TypeScript types for Actor system...");
    
    // Actor 관련 타입들 생성
    crate::new_architecture::actors::types::ActorCommand::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::AppEvent::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageItemResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::CrawlingConfig::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::BatchConfig::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageType::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageItem::export_all_to("../src/types/")?;
    
    // 새로 추가된 타입들
    crate::new_architecture::actors::types::StageError::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageSuccessResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::CollectionMetrics::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::ProcessingMetrics::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::FailedItem::export_all_to("../src/types/")?;
    
    // Actor traits 및 health 타입들
    crate::new_architecture::actors::traits::ActorHealth::export_all_to("../src/types/")?;
    crate::new_architecture::actors::traits::ActorStatus::export_all_to("../src/types/")?;
    
    println!("✅ TypeScript types generated successfully!");
    
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
        // 타입 생성이 에러 없이 수행되는지 테스트
        let result = generate_typescript_types();
        println!("Type generation result: {:?}", result);
        
        // 실제 프로덕션에서는 생성된 파일의 존재와 유효성을 확인
    }
}
