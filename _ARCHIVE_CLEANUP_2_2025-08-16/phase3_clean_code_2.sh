#!/bin/bash

# Phase 3-2: 더 구체적인 unused variables 정리

echo "🧹 Phase 3-2: Unused Variables 정리..."

cd /Users/chanseok/Codes/rMatterCertis/src-tauri

echo "📋 Step 1: 자주 발생하는 unused variables 처리"

# app variables
sed -i '' 's/app: &AppHandle,/_app: &AppHandle,/g' src/commands/crawling_v4.rs
sed -i '' 's/app: AppHandle,/_app: AppHandle,/g' src/commands/crawling_v4.rs

# stats variables  
sed -i '' 's/let stats =/let _stats =/g' src/commands/modern_crawling.rs

# state variables
sed -i '' 's/state: &State/_state: &State/g' src/commands/modern_crawling.rs

# dependencies variables
sed -i '' 's/dependencies: CrawlingEngineDependencies,/_dependencies: CrawlingEngineDependencies,/g' src/crawling.rs

# shared_state variables
sed -i '' 's/let shared_state =/let _shared_state =/g' src/crawling.rs

# queue_manager variables
sed -i '' 's/let queue_manager =/let _queue_manager =/g' src/crawling.rs

# start_time variables
sed -i '' 's/let start_time =/let _start_time =/g' src/crawling/orchestrator.rs

# duration_ms variables
sed -i '' 's/let duration_ms =/let _duration_ms =/g' src/crawling/orchestrator.rs

# Various unused variables in specific contexts
sed -i '' 's/product_id,/product_id: _,/g' src/crawling/orchestrator.rs

echo "📋 Step 2: 복잡한 unused variables 처리"

# db_analysis variables
sed -i '' 's/let db_analysis =/let _db_analysis =/g' src/infrastructure/system_broadcaster.rs
sed -i '' 's/let db_analysis =/let _db_analysis =/g' src/commands/crawling_v4.rs

# session_id_clone
sed -i '' 's/let session_id_clone =/let _session_id_clone =/g' src/commands/actor_system_monitoring.rs

# RwLock import cleanup
sed -i '' 's/use tokio::sync::{RwLock, Semaphore};$/use tokio::sync::Semaphore;/g' src/crawling/orchestrator.rs

echo "📋 Step 3: 더 많은 unused imports 정리"

# Remove more unused imports
sed -i '' '/use crate::domain::value_objects::ProductData;/d' src/crawling/workers/db_saver_sqlx.rs
sed -i '' '/use crate::crawling::tasks::TaskProductData;/d' src/crawling/workers/db_saver_sqlx.rs

echo "✅ Phase 3-2 완료!"

# Check remaining warnings
cargo check 2>&1 | grep -c "warning" | head -1 || echo "0"
