// 간단한 ts-rs 통합 테스트
use matter_certis_v2_lib::commands::actor_system_monitoring::*;
use matter_certis_v2_lib::commands::crawling_v4::*;
use matter_certis_v2_lib::commands::config_commands::*;
use std::path::Path;
use std::fs;
use ts_rs::TS;

fn main() {
    // 출력 디렉토리 설정
    let output_dir = Path::new("../src/types/generated");
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir).expect("Failed to create output directory");
    }
    
    println!("🔄 TypeScript 파일 생성 시작...");
    println!("📁 출력 폴더: {:?}", output_dir);
    
    // 각 타입별로 TypeScript 파일 생성
    export_type::<ActorSystemStatus>(output_dir, "ActorSystemStatus");
    export_type::<CrawlingProgress>(output_dir, "CrawlingProgress");
    export_type::<SystemStatePayload>(output_dir, "SystemStatePayload");
    export_type::<CrawlingResponse>(output_dir, "CrawlingResponse");
    export_type::<StartCrawlingRequest>(output_dir, "StartCrawlingRequest");
    
    // Window Management 타입들 추가
    export_type::<WindowState>(output_dir, "WindowState");
    export_type::<WindowPosition>(output_dir, "WindowPosition");
    export_type::<WindowSize>(output_dir, "WindowSize");

    println!("🎯 TypeScript 타입 생성 완료 - Modern Rust 2024 ts-rs 정책 적용됨");
}

fn export_type<T: TS>(output_dir: &Path, type_name: &str) {
    let type_def = T::decl();
    let export_statement = format!("export {};", type_def);
    let file_path = output_dir.join(format!("{}.ts", type_name));
    
    match fs::write(&file_path, export_statement) {
        Ok(_) => println!("✅ {} exported to {:?}", type_name, file_path),
        Err(e) => eprintln!("❌ {} export error: {}", type_name, e),
    }
}
