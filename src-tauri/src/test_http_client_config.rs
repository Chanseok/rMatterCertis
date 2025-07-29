//! Test HttpClient configuration integration

use crate::infrastructure::{config::AppConfig, simple_http_client::HttpClient};

#[tokio::test]
async fn test_http_client_from_worker_config() {
    // 기본 설정 로드
    let config = AppConfig::default();
    
    // WorkerConfig에서 HttpClient 생성
    let client = HttpClient::from_worker_config(&config.user.crawling.workers);
    assert!(client.is_ok());
    
    let client = client.unwrap();
    
    // 설정 값이 제대로 적용되었는지 확인할 수 있는 방법이 있으면 좋겠지만,
    // 현재는 생성만 확인
    println!("✅ HttpClient successfully created from WorkerConfig");
    
    // 실제 설정 파일에서 로드하는 테스트도 가능
    let config_manager = crate::infrastructure::config::ConfigManager::new();
    if let Ok(config_manager) = config_manager {
        if let Ok(loaded_config) = config_manager.load_config().await {
            let client_from_file = HttpClient::from_worker_config(&loaded_config.user.crawling.workers);
            assert!(client_from_file.is_ok());
            println!("✅ HttpClient successfully created from loaded config file");
            
            // 설정 파일의 값들 출력해보기
            println!("설정 파일에서 읽은 값들:");
            println!("  max_requests_per_second: {}", loaded_config.user.crawling.workers.max_requests_per_second);
            println!("  request_timeout_seconds: {}", loaded_config.user.crawling.workers.request_timeout_seconds);
            println!("  max_retries: {}", loaded_config.user.crawling.workers.max_retries);
            println!("  user_agent: {}", loaded_config.user.crawling.workers.user_agent);
            println!("  follow_redirects: {}", loaded_config.user.crawling.workers.follow_redirects);
        }
    }
}
