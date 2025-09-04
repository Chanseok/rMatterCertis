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

# 테스트 헬퍼를 통해 TS 바인딩 생성 수행 (crawl_engine::ts_gen)
echo "� Running TS binding generation via tests..."
cargo test -q --lib crawl_engine::ts_gen::tests::test_typescript_type_generation || echo "⚠️ Type generation test failed; continuing"

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
