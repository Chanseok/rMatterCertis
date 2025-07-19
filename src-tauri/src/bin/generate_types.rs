// TypeScript 타입 생성을 위한 바이너리
use matter_certis_v2_lib::commands::actor_system_monitoring::*;
use matter_certis_v2_lib::commands::crawling_v4::*;

fn main() {
    // ts-rs는 타입이 실제로 사용될 때만 TypeScript 파일을 생성합니다.
    // 이 바이너리는 모든 ts(export) 타입들을 참조하여 생성을 강제합니다.
    
    // 각 타입의 type_id를 참조하여 ts-rs 생성을 강제합니다
    println!("ActorSystemStatus: {:?}", std::any::type_name::<ActorSystemStatus>());
    println!("CrawlingProgress: {:?}", std::any::type_name::<CrawlingProgress>());
    println!("SystemStatePayload: {:?}", std::any::type_name::<SystemStatePayload>());
    println!("CrawlingResponse: {:?}", std::any::type_name::<CrawlingResponse>());
    println!("StartCrawlingRequest: {:?}", std::any::type_name::<StartCrawlingRequest>());

    // Default 생성자로 타입 인스턴스 생성 시도
    let _result = std::panic::catch_unwind(|| {
        // 이 코드가 실행되지 않아도 타입이 참조되면 ts-rs가 작동합니다
        let _: Option<ActorSystemStatus> = None;
        let _: Option<CrawlingProgress> = None;
        let _: Option<SystemStatePayload> = None;
        let _: Option<CrawlingResponse> = None;
        let _: Option<StartCrawlingRequest> = None;
    });

    println!("TypeScript 타입 생성을 위한 모든 참조 완료");
}
