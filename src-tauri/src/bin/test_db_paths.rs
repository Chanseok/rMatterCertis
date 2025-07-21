use matter_certis_v2_lib::infrastructure::database_paths;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 데이터베이스 경로 초기화 테스트 시작...");
    
    // 중앙집중식 경로 관리자 초기화
    match database_paths::initialize_database_paths().await {
        Ok(()) => {
            println!("✅ 데이터베이스 경로 초기화 성공");
            
            let main_url = database_paths::get_main_database_url();
            println!("📁 메인 데이터베이스 URL: {}", main_url);
            
            // 실제 파일 경로 추출
            let file_path = main_url.strip_prefix("sqlite:").unwrap_or(&main_url);
            println!("📂 파일 경로: {}", file_path);
            
            // 파일 존재 여부 확인
            if std::path::Path::new(file_path).exists() {
                println!("✅ 데이터베이스 파일 존재함");
                
                // 파일 권한 확인
                match std::fs::OpenOptions::new().write(true).open(file_path) {
                    Ok(_) => println!("✅ 데이터베이스 파일 쓰기 가능"),
                    Err(e) => println!("❌ 데이터베이스 파일 쓰기 불가: {}", e),
                }
            } else {
                println!("❌ 데이터베이스 파일 존재하지 않음");
            }
            
            // 디렉토리 권한 확인
            if let Some(parent_dir) = std::path::Path::new(file_path).parent() {
                if parent_dir.exists() {
                    println!("✅ 데이터베이스 디렉토리 존재함: {}", parent_dir.display());
                } else {
                    println!("❌ 데이터베이스 디렉토리 존재하지 않음: {}", parent_dir.display());
                }
            }
        },
        Err(e) => {
            println!("❌ 데이터베이스 경로 초기화 실패: {}", e);
            return Err(e.into());
        }
    }
    
    println!("🎯 테스트 완료");
    Ok(())
}
