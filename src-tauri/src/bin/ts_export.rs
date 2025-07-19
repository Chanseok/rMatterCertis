// 간단한 ts-rs 통합 테스트
use matter_certis_v2_lib::commands::actor_system_monitoring::*;
use matter_certis_v2_lib::commands::crawling_v4::*;

fn main() {
    // 각 타입의 TypeScript 정의를 생성합니다
    use ts_rs::TS;
    
    // 타입 생성 확인을 위한 함수 호출
    println!("ActorSystemStatus TS: {}", ActorSystemStatus::name());
    println!("CrawlingProgress TS: {}", CrawlingProgress::name());
    println!("SystemStatePayload TS: {}", SystemStatePayload::name());
    println!("CrawlingResponse TS: {}", CrawlingResponse::name());
    println!("StartCrawlingRequest TS: {}", StartCrawlingRequest::name());

    // ts-rs 생성 강제
    if let Err(e) = ActorSystemStatus::export() {
        eprintln!("ActorSystemStatus export error: {}", e);
    }
    if let Err(e) = CrawlingProgress::export() {
        eprintln!("CrawlingProgress export error: {}", e);
    }
    if let Err(e) = SystemStatePayload::export() {
        eprintln!("SystemStatePayload export error: {}", e);
    }
    if let Err(e) = CrawlingResponse::export() {
        eprintln!("CrawlingResponse export error: {}", e);
    }
    if let Err(e) = StartCrawlingRequest::export() {
        eprintln!("StartCrawlingRequest export error: {}", e);
    }

    println!("TypeScript 타입 생성 완료");
}
