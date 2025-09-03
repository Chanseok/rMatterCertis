#!/usr/bin/env bash
# Modern Rust 2024 Clippy Fix Script
# 
# 이 스크립트는 1767개의 clippy 에러를 단계적으로 해결합니다.
# re-arch-plan-final2.md 기준으로 Modern Rust 2024 준수를 목표로 합니다.

if [[ -n "${CI:-}" ]]; then
    set -euo pipefail
else
    set -e
fi

echo "🦀 Modern Rust 2024 Clippy Fix Script v2.0"
echo "================================================"

# 프로젝트 루트로 이동
cd "$(dirname "$0")/../src-tauri"

echo "📍 현재 위치: $(pwd)"

# Phase 1: Unknown lint 수정
echo ""
echo "🔧 Phase 1: Unknown lint 수정 중..."
echo "   - clippy::unnecessary_qualification → clippy::unnecessary_operation"

find src -name "*.rs" -exec sed -i '' 's/clippy::unnecessary_qualification/clippy::unnecessary_operation/g' {} \;

echo "✅ Unknown lint 수정 완료"

# Phase 2: Unused import 제거 (주요 파일들 대상)
echo ""
echo "🔧 Phase 2: Unused import 제거 중..."

# actor_system.rs
echo "   - actor_system.rs 정리 중..."
sed -i '' '/use serde::{Deserialize, Serialize};/d' src/new_architecture/actor_system.rs
sed -i '' '/use crate::infrastructure::config::AppConfig;/d' src/new_architecture/actor_system.rs

# task_actor.rs  
echo "   - task_actor.rs 정리 중..."
sed -i '' 's/use tokio::sync::{Mutex, mpsc};/use tokio::sync::Mutex;/' src/new_architecture/task_actor.rs
sed -i '' 's/ActorCommand, AppEvent, StageType, StageItem/AppEvent, StageType/' src/new_architecture/task_actor.rs

# crawling_integration.rs
echo "   - crawling_integration.rs 정리 중..."
sed -i '' 's/use tokio::sync::{mpsc, oneshot};//' src/new_architecture/services/crawling_integration.rs
sed -i '' 's/FailedItem, ProductInfo/FailedItem/' src/new_architecture/services/crawling_integration.rs

# real_crawling_commands.rs
echo "   - real_crawling_commands.rs 정리 중..."
sed -i '' '/use crate::new_architecture::services::real_crawling_integration::\*;/d' src/new_architecture/services/real_crawling_commands.rs

# domain 모듈들
echo "   - domain 모듈들 정리 중..."
sed -i '' '/use std::collections::HashMap;/d' src/domain/entities.rs
sed -i '' '/use std::time::Duration;/d' src/domain/constants.rs  
sed -i '' 's/use crate::domain::product::{Product, ProductDetail};/use crate::domain::product::ProductDetail;/' src/domain/services/crawling_services.rs

# application 모듈들
echo "   - application 모듈들 정리 중..."
sed -i '' 's/use crate::domain::atomic_events::{AtomicTaskEvent, AtomicEventStats};/use crate::domain::atomic_events::AtomicTaskEvent;/' src/application/events.rs

echo "✅ Unused import 제거 완료"

# Phase 3: Redundant 표현 정리
echo ""
echo "🔧 Phase 3: Redundant 표현 정리 중..."

# Redundant field names
echo "   - Redundant field names 수정 중..."
sed -i '' 's/failed_items: failed_items,/failed_items,/' src/new_architecture/actor_system.rs

# Needless continue 제거
echo "   - Needless continue 제거 중..."
sed -i '' '/continue;$/d' src/new_architecture/actor_system.rs

# Redundant else 블록 제거
echo "   - Redundant else 블록 정리 중..."
# shared_state.rs 수정 (라인 362-364, 386-388)
sed -i '' '362,364c\
            }\
            tracing::warn!("⏰ Site analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());' src/application/shared_state.rs

sed -i '' '386,388c\
            }\
            tracing::warn!("⏰ DB analysis cache expired or invalid (age: {:?})", analysis.cached_at.elapsed());' src/application/shared_state.rs

echo "✅ Redundant 표현 정리 완료"

# Phase 4: Raw string literal 최적화
echo ""
echo "🔧 Phase 4: Raw string literal 최적화 중..."

# integrated_product_repository.rs 수정
echo "   - SQL 쿼리 문자열 최적화 중..."
sed -i '' 's/r#"/r"/' src/infrastructure/integrated_product_repository.rs
sed -i '' 's/"#/"/' src/infrastructure/integrated_product_repository.rs

echo "✅ Raw string literal 최적화 완료"

# Phase 5: 빌드 테스트
echo ""
echo "🔧 Phase 5: 빌드 테스트 중..."

if cargo check --quiet; then
    echo "✅ 기본 빌드 성공"
else
    echo "❌ 기본 빌드 실패 - 수동 확인 필요"
fi

# Phase 6: Clippy 검사 (warning 레벨)
echo ""
echo "🔧 Phase 6: Clippy 검사 중..."

echo "   현재 Clippy 상태 확인 중..."
if cargo clippy --all-targets --all-features 2>&1 | head -20; then
    echo ""
    echo "📊 Clippy 에러 개수 확인 중..."
    ERROR_COUNT=$(cargo clippy --all-targets --all-features 2>&1 | grep -c "error:" || echo "0")
    echo "   남은 Clippy 에러: $ERROR_COUNT 개"
    
    if [ "$ERROR_COUNT" -lt 100 ]; then
        echo "✅ 상당한 개선 달성! (1767개 → $ERROR_COUNT 개)"
    elif [ "$ERROR_COUNT" -lt 500 ]; then
        echo "🟡 일부 개선 달성 (1767개 → $ERROR_COUNT 개)"
    else
        echo "🔄 추가 수동 작업 필요 (1767개 → $ERROR_COUNT 개)"
    fi
else
    echo "🔄 Clippy 검사 진행 중..."
fi

echo ""
echo "🎯 수정 완료 요약:"
echo "   ✅ Unknown lint 수정"
echo "   ✅ 주요 unused import 제거"
echo "   ✅ Redundant 표현 정리"
echo "   ✅ Raw string literal 최적화"
echo ""
echo "📋 다음 단계 (수동 작업 필요):"
echo "   1. 남은 unused async 함수들 동기화"
echo "   2. needless_pass_by_ref_mut 최적화"
echo "   3. large_stack_frames 개선"
echo "   4. ambiguous_glob_reexports 해결"
echo ""
echo "🔗 참고: IMPLEMENTATION_STATUS_ASSESSMENT_v2.md 확인"
echo "================================================"
