#!/bin/bash

# Phase 3: Clean Code Implementation
# Modern Rust 2024 코드 품질 향상

echo "🧹 Phase 3: Clean Code 시작..."

cd /Users/chanseok/Codes/rMatterCertis/src-tauri

echo "📋 Step 1: Unused imports 자동 제거"

# 1. Unused imports 제거 (자주 발생하는 패턴들)
echo "  - chrono unused imports 정리..."
sed -i '' 's/use chrono::{DateTime, Utc};$/use chrono::DateTime;/g' src/infrastructure/html_parser.rs

echo "  - tracing unused imports 정리..."
sed -i '' 's/use tracing::{info, warn, error};$/use tracing::info;/g' src/commands/system_analysis.rs

echo "  - actor_system unused imports 정리..."
sed -i '' '/use crate::new_architecture::actor_system::\*;/d' src/commands/actor_system_monitoring.rs
sed -i '' '/use crate::new_architecture::services::crawling_integration::CrawlingIntegrationService;/d' src/commands/actor_system_monitoring.rs
sed -i '' '/use std::collections::HashMap;/d' src/commands/actor_system_monitoring.rs
sed -i '' '/use std::sync::Arc;/d' src/commands/actor_system_monitoring.rs
sed -i '' '/use tokio::sync::RwLock;/d' src/commands/actor_system_monitoring.rs
sed -i '' '/use tauri::State;/d' src/commands/actor_system_monitoring.rs

echo "  - async_trait unused import 정리..."
sed -i '' '/use async_trait::async_trait;/d' src/crawling.rs

echo "  - domain unused imports 정리..."
sed -i '' '/use crate::domain::CrawlingStatus;/d' src/crawling/state.rs

echo "  - collections unused imports 정리..."
sed -i '' '/use std::collections::VecDeque;/d' src/crawling/queues.rs

echo "📋 Step 2: Unused variables prefix 추가"

# 2. Unused variables에 underscore prefix 추가
echo "  - target_url 변수 처리..."
sed -i '' 's/let target_url =/let _target_url =/g' src/new_architecture/actor_system.rs

echo "  - batch_size 변수 처리..."
sed -i '' 's/batch_size: u32,/_batch_size: u32,/g' src/new_architecture/actor_system.rs

echo "  - timeout_secs 변수 처리..."
sed -i '' 's/timeout_secs$/timeout_secs: _/g' src/new_architecture/services/real_crawling_integration.rs

echo "  - existing_product 변수 처리..."
sed -i '' 's/let existing_product =/let _existing_product =/g' src/infrastructure/integrated_product_repository.rs

echo "📋 Step 3: Dead code 필드/메소드 제거 준비"

echo "  - 사용되지 않는 imports 추가 정리..."
sed -i '' '/use crate::domain::ProductData;/d' src/crawling/workers/list_page_fetcher.rs
sed -i '' 's/use chrono::{DateTime, Utc, NaiveDateTime};$/use chrono::{DateTime, Utc};/g' src/crawling/workers/product_detail_parser.rs
sed -i '' '/use crate::domain::value_objects::ProductData;/d' src/crawling/workers/product_detail_parser.rs

echo "📋 Step 4: Cargo fix 실행"
cargo fix --lib --allow-dirty --allow-staged 2>/dev/null || true

echo "✅ Phase 3 Clean Code 1단계 완료!"
echo "🔍 남은 warnings 확인 중..."

cargo check 2>&1 | grep -c "warning" | head -1 || echo "0"
