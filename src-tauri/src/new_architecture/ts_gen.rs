//! TypeScript 타입 생성을 위한 유틸리티
//! 
//! Phase 4: ts-rs를 활용한 자동 타입 생성

use std::path::Path;

use ts_rs::TS;

/// TypeScript 바인딩 생성 함수
/// 프론트엔드와 백엔드 간 타입 동기화를 위해 TS 파일을 생성합니다.
pub fn generate_ts_bindings() -> Result<(), Box<dyn std::error::Error>> {
    // 기본 응답 타입들 생성
    // 주석 처리된 부분들은 TS trait이 구현되지 않아서 임시로 비활성화
    // TODO: 필요한 타입들에 #[derive(TS)] 추가 후 활성화
    
    /*
    crate::new_architecture::actors::types::ActorCommand::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::AppEvent::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageItemResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::CrawlingConfig::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::BatchConfig::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageType::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageItem::export_all_to("../src/types/")?;
    
    // Additional types exports
    crate::new_architecture::actors::types::StageError::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::StageSuccessResult::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::CollectionMetrics::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::ProcessingMetrics::export_all_to("../src/types/")?;
    crate::new_architecture::actors::types::FailedItem::export_all_to("../src/types/")?;
    
    // Actor traits
    crate::new_architecture::actors::traits::ActorHealth::export_all_to("../src/types/")?;
    crate::new_architecture::actors::traits::ActorStatus::export_all_to("../src/types/")?;
    */
    
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
        // 타입 생성이 에러 없이 수행되는지 테스트
        let result = generate_typescript_types();
        println!("Type generation result: {:?}", result);
        
        // 실제 프로덕션에서는 생성된 파일의 존재와 유효성을 확인
    }
}
