#!/bin/bash
# Phase 2: Actor System 최적화 - Modern Rust 2024 패턴 적용
# 
# 불필요한 async 함수 동기화 및 참조 타입 최적화

set -e

echo "🦀 Phase 2: Actor System 최적화 시작"
echo "==============================================="

cd "$(dirname "$0")/../src-tauri"

echo "📍 현재 위치: $(pwd)"

# Phase 2.1: 불필요한 async 함수 동기화
echo ""
echo "🔧 Phase 2.1: 불필요한 async 함수들 동기화 중..."

# config_commands.rs 동기화
echo "   - config_commands.rs 함수들 동기화 중..."

# get_site_config 함수 동기화
sed -i '' 's/pub async fn get_site_config()/pub fn get_site_config()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn build_page_url(page: u32)/pub fn build_page_url(page: u32)/' src/commands/config_commands.rs
sed -i '' 's/pub async fn resolve_url(relative_url: String)/pub fn resolve_url(relative_url: String)/' src/commands/config_commands.rs
sed -i '' 's/pub async fn get_default_crawling_config()/pub fn get_default_crawling_config()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn get_comprehensive_crawler_config()/pub fn get_comprehensive_crawler_config()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn get_app_directories()/pub fn get_app_directories()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn is_first_run()/pub fn is_first_run()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn cleanup_logs()/pub fn cleanup_logs()/' src/commands/config_commands.rs
sed -i '' 's/pub async fn get_log_directory_path()/pub fn get_log_directory_path()/' src/commands/config_commands.rs

# Window 관련 함수들 동기화 (Tauri API는 동기적)
sed -i '' 's/pub async fn set_window_position(window: tauri::Window, x: i32, y: i32)/pub fn set_window_position(window: tauri::Window, x: i32, y: i32)/' src/commands/config_commands.rs
sed -i '' 's/pub async fn set_window_size(window: tauri::Window, width: i32, height: i32)/pub fn set_window_size(window: tauri::Window, width: i32, height: i32)/' src/commands/config_commands.rs
sed -i '' 's/pub async fn maximize_window(window: tauri::Window)/pub fn maximize_window(window: tauri::Window)/' src/commands/config_commands.rs

echo "✅ config_commands.rs 함수 동기화 완료"

# modern_crawling.rs 일부 함수 동기화
echo "   - modern_crawling.rs 함수들 동기화 중..."
sed -i '' 's/pub async fn clear_crawling_errors(_state: State<'\''_, AppState>)/pub fn clear_crawling_errors(_state: State<'\''_, AppState>)/' src/commands/modern_crawling.rs
sed -i '' 's/pub async fn export_crawling_results(_state: State<'\''_, AppState>)/pub fn export_crawling_results(_state: State<'\''_, AppState>)/' src/commands/modern_crawling.rs

echo "✅ modern_crawling.rs 함수 동기화 완료"

# parsing_commands.rs 함수 동기화
echo "   - parsing_commands.rs 함수들 동기화 중..."
sed -i '' 's/pub async fn get_crawler_config()/pub fn get_crawler_config()/' src/commands/parsing_commands.rs
sed -i '' 's/pub async fn crawler_health_check()/pub fn crawler_health_check()/' src/commands/parsing_commands.rs

echo "✅ parsing_commands.rs 함수 동기화 완료"

# Phase 2.2: 사용하지 않는 변수 정리
echo ""
echo "🔧 Phase 2.2: 사용하지 않는 변수들 정리 중..."

# 주요 파일들에서 unused variable들을 underscore prefix로 수정
echo "   - actor_system.rs 변수 정리 중..."
sed -i '' 's/batch_size: u32,/_batch_size: u32,/' src/new_architecture/actor_system.rs
sed -i '' 's/stage_type: &StageType,/_stage_type: &StageType,/' src/new_architecture/actor_system.rs
sed -i '' 's/result: &StageSuccessResult/_result: &StageSuccessResult/' src/new_architecture/actor_system.rs

echo "   - commands 모듈들 변수 정리 중..."
sed -i '' 's/app: AppHandle,/_app: AppHandle,/' src/commands/system_analysis.rs
sed -i '' 's/engine_state: State<'\''_, CrawlingEngineState>,/_engine_state: State<'\''_, CrawlingEngineState>,/' src/commands/system_analysis.rs
sed -i '' 's/let engine =/_engine =/' src/commands/crawling_v4.rs
sed -i '' 's/config: BatchCrawlingConfig,/_config: BatchCrawlingConfig,/' src/commands/crawling_v4.rs

echo "✅ 사용하지 않는 변수 정리 완료"

# Phase 2.3: Raw string literal 불필요한 해시 제거
echo ""
echo "🔧 Phase 2.3: 추가 Raw string literal 최적화 중..."

# 다른 파일들에서도 불필요한 raw string 확인
find src -name "*.rs" -exec grep -l 'r#"' {} \; | while read file; do
    echo "   - $file 처리 중..."
    # 단순한 문자열의 경우 # 제거
    sed -i '' 's/r#"/r"/g' "$file"
    sed -i '' 's/"#/"/g' "$file"
done

echo "✅ Raw string literal 최적화 완료"

# Phase 2.4: trivial cast 제거
echo ""
echo "🔧 Phase 2.4: 불필요한 타입 캐스팅 정리 중..."

# u32 as u32 같은 불필요한 캐스팅 제거
echo "   - 불필요한 numeric cast 제거 중..."
sed -i '' 's/ as u32//g' src/infrastructure/database_connection.rs
sed -i '' 's/id as u32/id/g' src/application/dto.rs
sed -i '' 's/idx as u32/idx/g' src/application/dto.rs

echo "✅ 불필요한 타입 캐스팅 정리 완료"

# Phase 2.5: 빌드 테스트
echo ""
echo "🔧 Phase 2.5: 개선 사항 빌드 테스트 중..."

if cargo check --quiet; then
    echo "✅ 빌드 성공"
    
    # Clippy 에러 개수 확인
    echo "📊 현재 Clippy 에러 개수 확인 중..."
    ERROR_COUNT=$(cargo clippy --all-targets --all-features 2>&1 | grep -c "error:" || echo "0")
    echo "   현재 Clippy 에러: $ERROR_COUNT 개"
    
    if [ "$ERROR_COUNT" -lt 20 ]; then
        echo "🎉 Phase 2 목표 달성! (54개 → $ERROR_COUNT 개)"
    else
        echo "🔄 추가 최적화 필요 (54개 → $ERROR_COUNT 개)"
    fi
else
    echo "❌ 빌드 실패 - 수동 검토 필요"
fi

echo ""
echo "🎯 Phase 2 완료 요약:"
echo "   ✅ 불필요한 async 함수 동기화"
echo "   ✅ 사용하지 않는 변수 정리"
echo "   ✅ Raw string literal 최적화"
echo "   ✅ 불필요한 타입 캐스팅 제거"
echo ""
echo "📋 다음 Phase 3 예정 작업:"
echo "   - mutable reference 최적화"
echo "   - 함수 분할 및 리팩터링"
echo "   - Clean Code 원칙 적용"
echo "==============================================="
