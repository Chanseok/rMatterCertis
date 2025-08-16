// 간단한 ts-rs 통합 테스트
use matter_certis_v2_lib::types::frontend_api::{
    AdvancedCrawlingConfig, CrawlingProgressInfo, ErrorInfo,
};
// Legacy crawling_v4 types removed. Provide minimal stand-ins for TS export continuity.
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct SystemStatePayload {
    pub is_running: bool,
}
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
}
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct V4StartCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
}
use matter_certis_v2_lib::commands::config_commands::{WindowPosition, WindowSize, WindowState};
use std::fs;
use std::path::Path;
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
    export_type::<AdvancedCrawlingConfig>(output_dir, "AdvancedCrawlingConfig");
    export_type::<CrawlingProgressInfo>(output_dir, "CrawlingProgressInfo");
    export_type::<SystemStatePayload>(output_dir, "SystemStatePayload");
    export_type::<CrawlingResponse>(output_dir, "CrawlingResponse");
    export_type::<V4StartCrawlingRequest>(output_dir, "StartCrawlingRequest");

    // Window Management 타입들 추가
    export_type::<WindowState>(output_dir, "WindowState");
    export_type::<WindowPosition>(output_dir, "WindowPosition");
    export_type::<WindowSize>(output_dir, "WindowSize");

    // API 관련 타입들
    export_type::<ErrorInfo>(output_dir, "ErrorInfo");

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
