#!/usr/bin/env bash

# TypeScript 타입 자동 생성 스크립트
# Phase 4: ts-rs 기반 타입 동기화

# Fail fast in CI; keep interactive runs resilient
if [[ -n "${CI:-}" ]]; then
    set -euo pipefail
else
    set -e
fi

echo "🎯 Phase 4: TypeScript 타입 생성 시작..."

# src/types 디렉토리 생성
echo "📁 Creating types directory..."
mkdir -p ../src/types

# Rust 컴파일을 통한 ts-rs 타입 생성
echo "🦀 Generating TypeScript types from Rust..."
cd /Users/chanseok/Codes/rMatterCertis/src-tauri

# 테스트 실행 대신 전용 바이너리를 통해 TS 바인딩 생성 수행
echo "🧰 Running TS binding generation bin..."
cargo run --quiet --bin gen_ts_types

# TODO(codegen:CrawlEvent v1 -> vNext)
# 1. 현재 CrawlEvent TS 정의(`src/types/crawlEvents.ts`)는 수동 유지.
# 2. 향후 절차:
#    - (a) `crawl_events.rs` 에서 각 variant 에 `#[ts(export)]` 혹은 별도 derive 대상 래퍼 struct 도입
#    - (b) 내부 tag("event_type") + 필드 rename 정책을 매크로/attribute 로 명시
#    - (c) build script (build.rs) 또는 전용 `gen_ts_crawl_events` 바이너리에서 serde metadata reflect 후 TS union 생성
#    - (d) 이 스크립트에서 gen_ts_types 실행 후 자동 생성된 `crawlEvents.generated.ts` 와 수동 파일 diff → 실패 시 경고
# 3. 마이그레이션 전략:
#    - 초기: generated 파일을 `crawlEvents.generated.ts` 로 두고 기존 수동 파일 유지
#    - 안정 후: 수동 파일 제거 + barrel export 수정
# 4. 스키마 버전 증가 시: codegen 출력 헤더에 `// schema_version: <N>` 라인 포함하여 FE 빌드 단계에서 mismatch 체크 가능
# 5. 실패 허용 정책: codegen 실패 시 (CI에서만) 빌드 실패; 로컬은 경고만.


# 생성된 타입 파일들 확인
echo "📋 Checking generated TypeScript files..."
if [ -d "../src/types" ]; then
    echo "✅ Types directory exists"
    ls -la ../src/types/ || echo "📁 Types directory is empty or doesn't exist yet"
else
    echo "⚠️  Types directory not found, creating manually..."
    mkdir -p ../src/types
fi

# Phase 4 완료 메시지
echo ""
echo "🎉 Phase 4: TypeScript 타입 생성 완료!"
echo "📁 타입 파일들은 src/types/ 디렉토리에 생성됩니다."
echo ""
echo "다음 단계:"
echo "1. 프론트엔드에서 생성된 타입들을 import하여 사용"
echo "2. 상태 관리 스토어를 새로운 타입에 맞게 업데이트"
echo "3. Actor 시스템과 프론트엔드 간 타입 안전한 통신 구현"
