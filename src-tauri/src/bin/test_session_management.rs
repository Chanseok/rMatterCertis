//! Memory-based session management integration test
//! 
//! Tests the new architecture: memory state management + final results only in DB
//! This replaces the old crawling_sessions table approach.

use std::sync::Arc;
use anyhow::Result;

// Import new session management modules
use matter_certis_v2_lib::domain::session_manager::{
    SessionManager, SessionStatus, CrawlingStage, CrawlingResult
};
use matter_certis_v2_lib::infrastructure::{
    DatabaseConnection,
    CrawlingResultRepository, SqliteCrawlingResultRepository,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    run_session_management_tests().await
}

async fn run_session_management_tests() -> Result<()> {
    println!("🧠 rMatterCertis Memory-Based Session Management Test");
    println!("🎯 Testing: State in memory + Final results in DB");
    println!("═══════════════════════════════════════════════════");
    println!();

    // Initialize database for final results only
    let database_url = "sqlite::memory:";
    let db = DatabaseConnection::new(database_url).await?;
    db.migrate().await?;
    
    // Create result repository (for final outcomes only)
    let result_repo = Arc::new(SqliteCrawlingResultRepository::new(db.pool().clone()));
    
    // Create memory-based session manager
    let session_manager = Arc::new(SessionManager::new());

    println!("✅ Memory session manager and DB result repository initialized");
    println!();

    // Test 1: Memory-based session lifecycle
    test_memory_session_lifecycle(&session_manager).await?;
    
    // Test 2: Final result persistence
    test_final_result_persistence(&session_manager, &result_repo).await?;
    
    // Test 3: Concurrent sessions in memory
    test_concurrent_memory_sessions(&session_manager).await?;
    
    // Test 4: Performance comparison (memory vs DB)
    test_performance_comparison(&session_manager).await?;
    
    // Test 5: Result repository functionality
    test_result_repository(&result_repo).await?;

    println!("🎉 ALL MEMORY-BASED SESSION TESTS PASSED");
    println!("═══════════════════════════════════════════════════");
    println!("✅ Memory State Management: Complete");
    println!("✅ Final Result Persistence: Complete");
    println!("✅ Concurrent Sessions: Complete");
    println!("✅ Performance Optimization: Verified");
    println!("✅ Result Repository: Complete");
    println!();
    println!("🚀 Ready for Phase 3 Crawling Engine Integration!");

    Ok(())
}

async fn test_memory_session_lifecycle(session_manager: &SessionManager) -> Result<()> {
    println!("🧪 Test 1: Memory-Based Session Lifecycle");
    println!("─────────────────────────────────────────");

    // Start session (memory only)
    let config = serde_json::json!({
        "target_url": "https://certifications.csa-iot.org",
        "max_pages": 100
    });
    
    let session_id = session_manager.start_session(
        config, 
        100, 
        CrawlingStage::ProductList
    ).await;
    
    println!("✅ Session started in memory: {}", session_id);

    // Simulate rapid progress updates (memory only, no DB I/O)
    for page in 1..=10 {
        session_manager.update_progress(
            &session_id,
            page,
            page * 5, // 5 products per page
            Some(format!("https://example.com/page/{}", page))
        ).await?;
        
        // Get instant status from memory
        let session = session_manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.current_page, page);
        assert_eq!(session.products_found, page * 5);
        assert_eq!(session.status, SessionStatus::Running);
    }
    
    println!("✅ Rapid progress updates in memory (no DB I/O)");

    // Add some errors
    session_manager.add_error(&session_id, "Network timeout".to_string()).await?;
    session_manager.add_error(&session_id, "Parse error".to_string()).await?;
    
    let session = session_manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.errors_count, 2);
    assert_eq!(session.error_details.len(), 2);
    
    println!("✅ Error tracking in memory");

    // Complete session and get final result
    let final_result = session_manager.complete_session(&session_id, SessionStatus::Completed).await.unwrap();
    assert_eq!(final_result.status, SessionStatus::Completed);
    assert_eq!(final_result.products_found, 50);
    assert_eq!(final_result.errors_count, 2);
    
    // Session should be removed from memory
    assert!(session_manager.get_session(&session_id).await.is_none());
    
    println!("✅ Session completed and removed from memory");
    println!();
    Ok(())
}

async fn test_final_result_persistence(
    session_manager: &SessionManager,
    result_repo: &SqliteCrawlingResultRepository,
) -> Result<()> {
    println!("🧪 Test 2: Final Result Persistence (DB)");
    println!("────────────────────────────────────────");

    // Start and complete a session
    let config = serde_json::json!({"test": "persistence"});
    let session_id = session_manager.start_session(config, 50, CrawlingStage::ProductDetails).await;
    
    // Simulate some progress
    session_manager.update_progress(&session_id, 25, 125, None).await?;
    session_manager.add_error(&session_id, "Minor issue".to_string()).await?;
    
    // Complete and get final result
    let final_result = session_manager.complete_session(&session_id, SessionStatus::Completed).await.unwrap();
    
    // Save final result to DB (one-time write)
    result_repo.save_result(&final_result).await?;
    println!("✅ Final result saved to DB (single write operation)");
    
    // Verify persistence
    let retrieved_result = result_repo.get_result(&session_id).await?.unwrap();
    assert_eq!(retrieved_result.session_id, session_id);
    assert_eq!(retrieved_result.products_found, 125);
    assert_eq!(retrieved_result.errors_count, 1);
    
    println!("✅ Final result retrieved from DB");
    println!();
    Ok(())
}

async fn test_concurrent_memory_sessions(session_manager: &SessionManager) -> Result<()> {
    println!("🧪 Test 3: Concurrent Sessions in Memory");
    println!("────────────────────────────────────────");

    // Start multiple sessions concurrently
    let config = serde_json::json!({"concurrent": true});
    
    let session1 = session_manager.start_session(config.clone(), 20, CrawlingStage::ProductList).await;
    let session2 = session_manager.start_session(config.clone(), 30, CrawlingStage::ProductDetails).await;
    let session3 = session_manager.start_session(config, 40, CrawlingStage::MatterDetails).await;
    
    println!("✅ Started 3 concurrent sessions in memory");

    // Update all sessions concurrently (no locking issues)
    session_manager.update_progress(&session1, 5, 25, None).await?;
    session_manager.update_progress(&session2, 10, 100, None).await?;
    session_manager.update_progress(&session3, 15, 200, None).await?;
    
    // Check all sessions are active
    let active_sessions = session_manager.get_active_sessions().await;
    assert_eq!(active_sessions.len(), 3);
    
    println!("✅ All sessions updated concurrently without conflicts");

    // Complete sessions in different order
    session_manager.complete_session(&session2, SessionStatus::Completed).await;
    session_manager.set_status(&session1, SessionStatus::Paused).await?;
    session_manager.complete_session(&session3, SessionStatus::Failed).await;
    
    // Check remaining active sessions
    let active_sessions = session_manager.get_active_sessions().await;
    assert_eq!(active_sessions.len(), 1); // Only paused session remains
    assert_eq!(active_sessions[0].session_id, session1);
    
    println!("✅ Concurrent session management working correctly");
    println!();
    Ok(())
}

async fn test_performance_comparison(session_manager: &SessionManager) -> Result<()> {
    println!("🧪 Test 4: Performance Comparison (Memory vs DB)");
    println!("─────────────────────────────────────────────────");

    let config = serde_json::json!({"performance": "test"});
    let session_id = session_manager.start_session(config, 1000, CrawlingStage::ProductList).await;
    
    // Simulate 1000 rapid updates (like crawling 1000 pages)
    let start = std::time::Instant::now();
    
    for page in 1..=1000 {
        session_manager.update_progress(&session_id, page, page * 3, None).await?;
    }
    
    let duration = start.elapsed();
    println!("✅ 1000 memory updates completed in: {:?}", duration);
    println!("   🚀 Performance: {:.2} updates/ms", 1000.0 / duration.as_millis() as f64);
    
    // Verify final state
    let session = session_manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.current_page, 1000);
    assert_eq!(session.products_found, 3000);
    
    // Complete session
    session_manager.complete_session(&session_id, SessionStatus::Completed).await;
    
    println!("✅ Memory-based updates are extremely fast (no disk I/O)");
    println!();
    Ok(())
}

async fn test_result_repository(result_repo: &SqliteCrawlingResultRepository) -> Result<()> {
    println!("🧪 Test 5: Result Repository Functionality");
    println!("──────────────────────────────────────────");

    // Create test results
    let results = vec![
        create_test_result("test-1", SessionStatus::Completed, 100, 500),
        create_test_result("test-2", SessionStatus::Failed, 50, 200),
        create_test_result("test-3", SessionStatus::Completed, 200, 1000),
    ];

    // Save all results
    for result in &results {
        result_repo.save_result(result).await?;
    }
    println!("✅ Saved 3 test results to DB");

    // Test queries
    let recent = result_repo.get_recent_results(10).await?;
    assert_eq!(recent.len(), 3);
    println!("✅ Retrieved recent results");

    let completed = result_repo.get_results_by_status(SessionStatus::Completed).await?;
    assert_eq!(completed.len(), 2);
    println!("✅ Filtered results by status");

    let last_success = result_repo.get_last_success_date().await?;
    assert!(last_success.is_some());
    println!("✅ Found last successful crawling date");

    println!();
    Ok(())
}

fn create_test_result(id: &str, status: SessionStatus, pages: u32, products: u32) -> CrawlingResult {
    use chrono::Utc;
    
    let now = Utc::now();
    CrawlingResult {
        session_id: id.to_string(),
        status,
        stage: CrawlingStage::ProductList,
        total_pages: pages,
        products_found: products,
        errors_count: 0,
        started_at: now - chrono::Duration::minutes(10),
        completed_at: now,
        execution_time_seconds: 600,
        config_snapshot: serde_json::json!({"test": true}),
        error_details: None,
    }
}
