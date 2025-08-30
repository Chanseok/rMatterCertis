//! Test `HttpClient` configuration integration (integration test)

use matter_certis_v2_lib::infrastructure::{config::AppConfig, simple_http_client::HttpClient};

#[tokio::test]
async fn test_http_client_from_worker_config() {
    // 기본 설정 로드
    let config = AppConfig::default();

    // WorkerConfig에서 HttpClient 생성
    let client = HttpClient::from_worker_config(&config.user.crawling.workers);
    assert!(client.is_ok());

    let _client = client.unwrap();

    println!("✅ HttpClient successfully created from WorkerConfig");

    // 실제 설정 파일에서 로드하는 테스트도 가능
    if let Ok(config_manager) = matter_certis_v2_lib::infrastructure::config::ConfigManager::new() {
        if let Ok(loaded_config) = config_manager.load_config().await {
            let client_from_file =
                HttpClient::from_worker_config(&loaded_config.user.crawling.workers);
            assert!(client_from_file.is_ok());
            println!("✅ HttpClient successfully created from loaded config file");
        }
    }
}
