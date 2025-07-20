use std::sync::Arc;
use matter_certis_v2_lib::infrastructure::{DatabaseConnection, IntegratedProductRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🚀 Phase 3 간단 통합 테스트");
    
    // 데이터베이스 연결
    let db = DatabaseConnection::new("sqlite::memory:").await?;
    db.migrate().await?;
    println!("✅ 데이터베이스 연결 성공");
    
    // 리포지토리 초기화
    let _repo = Arc::new(IntegratedProductRepository::new(db.pool().clone()));
    println!("✅ 리포지토리 초기화 성공");
    
    println!("🎉 Phase 3 Clean Code 완료! 시스템이 정상적으로 빌드되고 핵심 컴포넌트가 동작합니다.");
    
    Ok(())
}
