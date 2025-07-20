#!/bin/bash

# Phase 3 복원 스크립트 - 잘못 변경된 변수들 복원

echo "🔧 Phase 3 복원 중..."

cd /Users/chanseok/Codes/rMatterCertis/src-tauri

# app 변수들 복원 (실제 사용되는 것들)
sed -i '' 's/_app: AppHandle,/app: AppHandle,/g' src/commands/crawling_v4.rs

# db_analysis 복원 (실제 사용됨)
sed -i '' 's/_db_analysis/db_analysis/g' src/commands/crawling_v4.rs

# stats 복원 (실제 사용됨)
sed -i '' 's/_stats/stats/g' src/commands/modern_crawling.rs

# existing_product 복원 (실제 사용됨)
sed -i '' 's/_existing_product/existing_product/g' src/infrastructure/integrated_product_repository.rs

# session_id_clone 복원 (실제 사용됨)
sed -i '' 's/_session_id_clone/session_id_clone/g' src/commands/actor_system_monitoring.rs

# start_time 복원 (실제 사용됨)
sed -i '' 's/_start_time/start_time/g' src/crawling/orchestrator.rs

# duration_ms 복원 (실제 사용됨)
sed -i '' 's/_duration_ms/duration_ms/g' src/crawling/orchestrator.rs

# shared_state 복원 (실제 사용됨)
sed -i '' 's/_shared_state/shared_state/g' src/crawling.rs

# queue_manager 복원 (실제 사용됨)
sed -i '' 's/_queue_manager/queue_manager/g' src/crawling.rs

# batch_size 복원 (actor_system.rs에서 실제 사용됨)
sed -i '' 's/_batch_size: u32,/batch_size: u32,/g' src/new_architecture/actor_system.rs

echo "✅ 복원 완료!"
