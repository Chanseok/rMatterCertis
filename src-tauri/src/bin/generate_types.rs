// TypeScript 타입 생성을 위한 바이너리
use matter_certis_v2_lib::commands::actor_system_monitoring::*;
use matter_certis_v2_lib::commands::crawling_v4::*;
use matter_certis_v2_lib::domain::product::*;
use matter_certis_v2_lib::domain::product_url::*;
use matter_certis_v2_lib::domain::services::crawling_services::*;

fn main() {
    // ts-rs는 타입이 실제로 사용될 때만 TypeScript 파일을 생성합니다.
    // 이 바이너리는 모든 ts(export) 타입들을 참조하여 생성을 강제합니다.
    
    // 명령어 타입들
    println!("ActorSystemStatus: {:?}", std::any::type_name::<ActorSystemStatus>());
    println!("CrawlingProgress: {:?}", std::any::type_name::<CrawlingProgress>());
    println!("SystemStatePayload: {:?}", std::any::type_name::<SystemStatePayload>());
    println!("CrawlingResponse: {:?}", std::any::type_name::<CrawlingResponse>());
    println!("StartCrawlingRequest: {:?}", std::any::type_name::<StartCrawlingRequest>());
    
    // 도메인 타입들
    println!("Product: {:?}", std::any::type_name::<Product>());
    println!("ProductDetail: {:?}", std::any::type_name::<ProductDetail>());
    println!("ProductUrl: {:?}", std::any::type_name::<ProductUrl>());
    println!("SiteStatus: {:?}", std::any::type_name::<SiteStatus>());
    println!("DatabaseAnalysis: {:?}", std::any::type_name::<DatabaseAnalysis>());
    println!("FieldAnalysis: {:?}", std::any::type_name::<FieldAnalysis>());
    println!("DuplicateAnalysis: {:?}", std::any::type_name::<DuplicateAnalysis>());
    println!("ProcessingStrategy: {:?}", std::any::type_name::<ProcessingStrategy>());
    println!("CrawlingRangeRecommendation: {:?}", std::any::type_name::<CrawlingRangeRecommendation>());
    println!("SiteDataChangeStatus: {:?}", std::any::type_name::<SiteDataChangeStatus>());
    println!("DataDecreaseRecommendation: {:?}", std::any::type_name::<DataDecreaseRecommendation>());
    println!("RecommendedAction: {:?}", std::any::type_name::<RecommendedAction>());
    println!("SeverityLevel: {:?}", std::any::type_name::<SeverityLevel>());
    println!("DuplicateGroup: {:?}", std::any::type_name::<DuplicateGroup>());
    println!("DuplicateType: {:?}", std::any::type_name::<DuplicateType>());

    // Default 생성자로 타입 인스턴스 생성 시도
    let _result = std::panic::catch_unwind(|| {
        // 명령어 타입들
        let _: Option<ActorSystemStatus> = None;
        let _: Option<CrawlingProgress> = None;
        let _: Option<SystemStatePayload> = None;
        let _: Option<CrawlingResponse> = None;
        let _: Option<StartCrawlingRequest> = None;
        
        // 도메인 타입들
        let _: Option<Product> = None;
        let _: Option<ProductDetail> = None;
        let _: Option<ProductUrl> = None;
        let _: Option<SiteStatus> = None;
        let _: Option<DatabaseAnalysis> = None;
        let _: Option<FieldAnalysis> = None;
        let _: Option<DuplicateAnalysis> = None;
        let _: Option<ProcessingStrategy> = None;
        let _: Option<CrawlingRangeRecommendation> = None;
    });

    // ts-rs export 강제 실행
    use ts_rs::TS;
    let _ = SiteStatus::export();
    let _ = DatabaseAnalysis::export();
    let _ = Product::export();
    let _ = ProductDetail::export();
    let _ = ProductUrl::export();
    let _ = FieldAnalysis::export();
    let _ = DuplicateAnalysis::export();
    let _ = ProcessingStrategy::export();
    let _ = CrawlingRangeRecommendation::export();

    println!("TypeScript 타입 생성을 위한 모든 참조 완료");
}
