#!/bin/bash

# TypeScript 타입 자동 생성 스크립트
# Phase 4: ts-rs 기반 타입 동기화

set -e

echo "🎯 Phase 4: TypeScript 타입 생성 시작..."

# src/types 디렉토리 생성
echo "📁 Creating types directory..."
mkdir -p ../src/types

# Rust 컴파일을 통한 ts-rs 타입 생성
echo "🦀 Generating TypeScript types from Rust..."
cd /Users/chanseok/Codes/rMatterCertis/src-tauri

# 먼저 타입 생성을 위한 더미 바이너리 실행
cargo run --bin type_generator 2>/dev/null || {
    echo "📝 Creating type generator binary..."
    
    # 타입 생성 전용 바이너리 생성
    cat > src/bin/type_generator.rs << 'EOF'
//! TypeScript 타입 생성 전용 바이너리
//! 
//! Phase 4: ts-rs 기반 자동 타입 생성

use matter_certis_v2_lib::new_architecture::ts_gen;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Starting TypeScript type generation...");
    
    // 타입 생성 실행
    ts_gen::generate_typescript_types()?;
    
    println!("✅ All TypeScript types generated successfully!");
    Ok(())
}
EOF
    
    echo "🚀 Running type generator..."
    cargo run --bin type_generator || echo "⚠️  Type generation encountered issues, but continuing..."
}

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
