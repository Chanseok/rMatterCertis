# 리팩토링 마스터 플랜 (통합본)

> 생성일: 2025-10-09  
> 최종 업데이트: 2025-10-09  
> 통합 문서: refactoring-todo.md + refactoring-todo-by-gemini.md + testing-strategy-before-refactoring.md

## 📋 목차

1. [실행 전략 개요](#1-실행-전략-개요)
2. [Phase 0: 테스트 인프라 구축 (1주)](#phase-0-테스트-인프라-구축-1주)
3. [Phase 1: 긴급 정리 (1-2일)](#phase-1-긴급-정리-1-2일)
4. [Phase 2: 백엔드 중복 제거 (3-5일)](#phase-2-백엔드-중복-제거-3-5일)
5. [Phase 3: 프론트엔드 정리 (2-3일)](#phase-3-프론트엔드-정리-2-3일)
6. [Phase 4: 문서 및 스크립트 정리 (1-2일)](#phase-4-문서-및-스크립트-정리-1-2일)
7. [Phase 5: 아키텍처 개선 (필요시)](#phase-5-아키텍처-개선-필요시)
8. [Phase 6: CI/CD 통합 (1일)](#phase-6-cicd-통합-1일)
9. [작업 체크리스트](#작업-체크리스트)
10. [성공 기준 및 메트릭](#성공-기준-및-메트릭)

---

## 1. 실행 전략 개요

### 🎯 핵심 원칙

1. **테스트 우선 (Test First)**: 리팩토링 전 반드시 테스트 추가
2. **작은 단위 (Small Steps)**: 한 번에 하나의 변경만 수행
3. **즉시 검증 (Immediate Validation)**: 변경 후 바로 테스트 실행
4. **안전한 롤백 (Safe Rollback)**: Git branch/stash 적극 활용

### 📊 현황 요약

#### ✅ 완료된 작업
- [x] `find_last_valid_page_downward` 제거 (5페이지 단위 탐색으로 통합)
- [x] UI 3개 탭 구조 확정 (크롤링, 설정, 로컬DB)
- [x] AnalysisTab, DatabaseDiagnostics 아카이브

#### ⚠️ 테스트 커버리지 갭
- 페이지 탐색 알고리즘 (최근 변경됨!)
- 데이터 분석 메서드 (중복 존재)
- 동기화 기능 (빠른/스마트/보완)
- DiagnosticsPanel UI 로직

#### 🗑️ 제거 예정 코드량
- **백엔드**: ~500-800 라인 (중복 메서드, Mock 등)
- **프론트엔드**: ~2000-3000 라인 (아카이브 탭, 미사용 파일)
- **문서**: ~200 라인 (오래된 가이드)
- **스크립트**: ~300-500 라인 (레거시 스크립트)

---

## Phase 0: 테스트 인프라 구축 (1주)

> **목적**: 리팩토링 중 regression 방지  
> **우선순위**: 🔴 최우선 (리팩토링 전 필수!)

### Day 1: 베이스라인 확보

#### Task 0.1: 현재 상태 스냅샷
```bash
# 1. 테스트 실행 및 결과 저장
cd src-tauri
cargo test --lib --tests 2>&1 | tee ../test-results-baseline.txt

# 2. 성공/실패 통계
cargo test --lib --tests 2>&1 | grep "test result" >> ../test-results-baseline.txt

# 3. 컴파일 확인
cargo check -q 2>&1 | tee ../cargo-check-baseline.txt
```

**산출물**:
- [ ] `test-results-baseline.txt`
- [ ] `cargo-check-baseline.txt`
- [ ] `test-coverage-report.md` (수동 작성)

#### Task 0.2: E2E 시나리오 수동 실행
```markdown
# E2E 시나리오
1. 사이트 상태 체크
   - 예상: 596 → 589 페이지 감소 감지
   - 확인: is_page_count_decreased=true, ratio=1.2%

2. 크롤링 범위 재계산
   - 예상: Partial(5) 권장
   
3. 부분 크롤링 실행
   - 범위: 589~585 (5페이지)
   
4. DB 레코드 체크
   - 진단 실행 → 이상 그룹 확인
   
5. 스마트 동기화
   - shallow + diagnostics + complement
```

**산출물**:
- [ ] `e2e-scenarios.md` - 시나리오 및 예상 결과
- [ ] `golden-outputs/` - 스크린샷, 로그 파일
  - `site-status-response.json`
  - `diagnostics-result.json`
  - `crawling-plan.json`
  - `sync-result.json`

### Day 2-3: 크리티컬 경로 테스트 추가

#### Task 0.3: 페이지 탐색 회귀 테스트
**파일**: `src-tauri/tests/page_discovery_regression.rs`

```rust
//! 페이지 탐색 알고리즘 회귀 테스트
//! 최근 변경: find_last_valid_page_downward 제거

use matter_certis_v2_lib::infrastructure::crawling_service_impls::StatusCheckerImpl;

#[tokio::test]
async fn test_downward_search_with_5_page_steps() {
    // Given: Mock HTTP client returning empty pages
    // When: find_last_valid_page_with_safety_check(600)
    // Then: 5페이지 단위로 점프 (600 → 595 → 590...)
}

#[tokio::test]
async fn test_consecutive_empty_pages_fatal_error() {
    // Given: 12회 연속 빈 페이지 (5페이지 단위)
    // When: find_last_valid_page_with_safety_check
    // Then: Fatal error 발생, "60개 페이지 연속 비어있음" 메시지
}

#[tokio::test]
async fn test_page_count_decrease_detection() {
    // Given: 설정에 last_known_max_page=596 저장됨
    // When: check_site_status() → 현재 589 페이지 발견
    // Then: 
    //   - is_page_count_decreased=true
    //   - previous_max_pages=Some(596)
    //   - page_decrease_ratio=Some(0.0117) // 1.17%
}

#[tokio::test]
async fn test_page_cache_consistency() {
    // Given: 페이지 500 분석 후 캐시 저장
    // When: 동일 페이지 500 재요청
    // Then: 캐시 히트, HTTP 요청 0건
}
```

**체크리스트**:
- [ ] 테스트 파일 생성
- [ ] Mock HTTP client 구현
- [ ] 4개 테스트 케이스 작성
- [ ] `cargo test page_discovery_regression` 통과 확인

#### Task 0.4: 데이터 분석 회귀 테스트
**파일**: `src-tauri/tests/data_analysis_regression.rs`

```rust
//! 데이터 분석 메서드 회귀 테스트
//! 중복 메서드: analyze_data_changes vs analyze_site_data_changes

#[tokio::test]
async fn test_analyze_data_changes_with_decrease() {
    // Given: 이전 product_count=7128, 현재=7084
    // When: analyze_data_changes(7084)
    // Then: 
    //   - SiteDataChangeStatus::Decreased
    //   - decrease_amount=44
    //   - DataDecreaseRecommendation::severity=Low (0.6% 감소)
}

#[tokio::test]
async fn test_analyze_data_changes_within_tolerance() {
    // Given: 0.5% 미만 변화 (7128 → 7125)
    // When: analyze_data_changes(7125)
    // Then: SiteDataChangeStatus::Stable (tolerance 내)
}

#[tokio::test]
async fn test_analyze_site_data_changes_increased() {
    // Given: 이전 count=7000, 현재=7200
    // When: analyze_site_data_changes(7200)
    // Then: DataChangeAnalysis::Increased { new_products: 200 }
}

#[tokio::test]
async fn test_analyze_site_data_changes_stable() {
    // Given: 동일 count (7128 → 7128)
    // When: analyze_site_data_changes(7128)
    // Then: DataChangeAnalysis::Stable
}
```

**체크리스트**:
- [ ] 테스트 파일 생성
- [ ] Mock 설정 구현
- [ ] 4개 테스트 케이스 작성
- [ ] 두 메서드 차이점 문서화

#### Task 0.5: 동기화 기능 회귀 테스트
**파일**: `src-tauri/tests/sync_operations_regression.rs`

```rust
//! 동기화 기능 회귀 테스트
//! 빠른 동기화, 스마트 동기화, 제품 보완 크롤링

#[tokio::test]
async fn test_shallow_sync_coordinates_only() {
    // Given: products with null (page_id, index_in_page)
    // When: shallow_sync() 실행
    // Then:
    //   - page_id, index_in_page 업데이트됨
    //   - certification_date, transport_interface는 NULL 유지
}

#[tokio::test]
async fn test_smart_sync_full_pipeline() {
    // Given: 부분 데이터
    // When: smart_sync()
    // Then:
    //   1. shallow_sync 완료
    //   2. diagnostics 실행
    //   3. complement_crawl 실행
    //   4. 모든 필드 채워짐
}

#[tokio::test]
async fn test_complement_crawl_missing_fields() {
    // Given: 
    //   - Product A: certification_date=NULL
    //   - Product B: transport_interface=NULL
    // When: complement_crawl()
    // Then:
    //   - A, B만 재크롤링
    //   - 기존 데이터 유지
}
```

**체크리스트**:
- [ ] 테스트 파일 생성
- [ ] 테스트 DB 셋업
- [ ] 3개 테스트 케이스 작성
- [ ] 각 동기화 모드 동작 확인

### Day 4-5: Golden Test 패턴 도입

#### Task 0.6: Golden Test 구조 생성
```bash
# Golden test 디렉토리 생성
mkdir -p src-tauri/tests/golden

# 템플릿 파일 생성
touch src-tauri/tests/golden/site_status_response.json
touch src-tauri/tests/golden/diagnostics_result.json
touch src-tauri/tests/golden/crawling_plan.json
touch src-tauri/tests/golden/sync_result.json
```

#### Task 0.7: Golden Test 구현
**파일**: `src-tauri/tests/golden_tests.rs`

```rust
//! Golden Test: 정상 동작 시 응답과 비교

use std::fs;
use serde_json::Value;

fn load_golden(filename: &str) -> Value {
    let path = format!("tests/golden/{}", filename);
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[tokio::test]
async fn test_site_status_golden() {
    // When: 정상 사이트 상태 체크
    let result = check_site_status().await.unwrap();
    
    // Then: Golden output과 일치
    let golden = load_golden("site_status_response.json");
    assert_json_eq!(result, golden);
}

#[tokio::test]
async fn test_diagnostics_golden() {
    // When: DB 진단 실행
    let result = run_diagnostics().await.unwrap();
    
    // Then: Golden output과 일치
    let golden = load_golden("diagnostics_result.json");
    assert_json_eq!(result, golden);
}
```

**체크리스트**:
- [ ] `assert_json_diff` crate 추가
- [ ] Golden 파일 생성 (현재 정상 응답 저장)
- [ ] 2개 Golden test 작성
- [ ] `cargo test golden_tests` 통과 확인

---

## Phase 1: 긴급 정리 (1-2일)

> **목적**: 명백히 미사용인 코드 즉시 제거  
> **우선순위**: 🔴 High (테스트 커버 후 즉시 실행)

### Task 1.1: 아카이브 디렉토리 완전 삭제

```bash
# 1. Git에 커밋 전 백업 태그 생성
git tag v-before-archive-cleanup
git push origin v-before-archive-cleanup

# 2. 아카이브 디렉토리 삭제
rm -rf archive/
rm -rf src/_archive/

# 3. 변경사항 커밋
git add .
git commit -m "chore: remove archived components and tabs

- Deleted archive/ directory (7 components)
- Deleted src/_archive/ directory (7 tabs)
- Components: AnalysisTab, DatabaseDiagnostics, LiveCrawlingTab, etc.
- All preserved in Git history at tag v-before-archive-cleanup"
```

**삭제 대상**:
```
✅ archive/components/DatabaseDiagnostics.tsx
✅ archive/components/tabs/AnalysisTab.tsx
✅ archive/components/tabs/LiveCrawlingTab.tsx
✅ archive/components/tabs/RealtimeDashboardTab.tsx
✅ archive/components/tabs/ActorSystemTab.tsx
✅ archive/components/tabs/LiveProductionTab.tsx
✅ archive/components/tabs/DomainDashboardTab.tsx

✅ src/_archive/tabs/RealtimeDashboardTab.tsx
✅ src/_archive/tabs/LiveCrawlingTab.tsx
✅ src/_archive/tabs/ActorSystemTab.tsx
✅ src/_archive/tabs/GameDashboardTab.tsx
✅ src/_archive/tabs/StatusTab.tsx
✅ src/_archive/tabs/LiveProductionTab.tsx
✅ src/_archive/tabs/NewArchTestTab.tsx
```

**체크리스트**:
- [ ] 백업 태그 생성
- [ ] `archive/` 삭제
- [ ] `src/_archive/` 삭제
- [ ] 컴파일 확인: `npm run type-check`
- [ ] 커밋 및 푸시

### Task 1.2: 아카이브 관련 API/서비스 제거

**파일 삭제**:
```bash
# API 서비스 (Gemini 제안)
rm -f archive/api/dashboard.ts
rm -f archive/services/dashboardAPI.ts
rm -f archive/hooks/useDashboardData.ts
```

**백엔드 커맨드 확인 및 제거**:
```rust
// src-tauri/src/commands/ 에서 검색
grep -r "dashboard\|analysis" src-tauri/src/commands/

// 발견 시 제거:
// - get_dashboard_stats
// - get_analysis_data
// - etc.
```

**체크리스트**:
- [ ] API 파일 삭제
- [ ] 백엔드 커맨드 검색
- [ ] 미사용 커맨드 제거
- [ ] `cargo check` 통과 확인

### Task 1.3: Unused Exports 정리

**파일**: `reports/unused_exports_20250907T223324.txt` 기반

```bash
# 1. 재검증 (최신 리포트 생성)
npm run analyze:unused-exports

# 2. 확인된 미사용 export 제거
```

**제거 대상**:
```typescript
// src/platform/tauri.ts
❌ batchApiCalls (355)
❌ isApiError (373)
❌ hasApiError (382)
❌ hasApiData (386)

// src/stores/tabStore.ts
❌ toggleExpandedSection (124)
❌ setExpandedSection (128)
```

**체크리스트**:
- [ ] 최신 unused exports 리포트 생성
- [ ] 각 export 실제 미사용 확인
- [ ] 제거 및 커밋
- [ ] `npm run type-check` 통과

### Task 1.4: Unreachable Files 아카이브

**파일**: `reports/unused_frontend_20250907T220407.txt` 기반

```bash
# 아카이브 디렉토리 생성
mkdir -p src/_unreachable_backup_20251009

# 파일 이동
mv src/AppTabBased.tsx src/_unreachable_backup_20251009/
mv src/components/ActiveBatchView.tsx src/_unreachable_backup_20251009/
# ... (리스트 전체)

# Git 커밋
git add .
git commit -m "chore: archive unreachable files identified by ts-prune"
```

**이동 대상** (일부):
```
src/AppTabBased.tsx
src/components/ActiveBatchView.tsx
src/components/BatchAnchors.tsx
src/components/CrawlingDashboard.tsx
src/components/CrawlingForm.tsx
src/components/MissionBriefingPanel.tsx
src/components/common/ExpandableSection.tsx
src/components/crawling/CrawlingProgressDisplay.tsx
src/components/features/VendorForm.tsx
src/components/features/VendorManagement.tsx
```

**단, 확인 필요**:
- `VendorForm.tsx` - 설정 탭에서 사용 가능성
- `VendorManagement.tsx` - 설정 탭에서 사용 가능성

**체크리스트**:
- [ ] VendorForm/Management 사용 여부 확인
- [ ] 미사용 파일 백업 디렉토리로 이동
- [ ] 컴파일 확인
- [ ] 커밋

---

## Phase 2: 백엔드 코드 품질 개선 (5-7일)

> **목적**: Modern Rust 2024 원칙 준수, 중복 제거, 에러 처리 개선  
> **우선순위**: � High (코드 품질 핵심)  
> **참고**: 부록 A - 아키텍처 원칙 빠른 참조

### Task 2.1: unwrap()/expect() 제거 (프로덕션 코드)

**현재 문제**: 50+ 곳에서 `unwrap()` 또는 `expect()` 사용 (테스트 코드 제외)

**우선순위 대상**:
```bash
# 프로덕션 코드에서 unwrap 검색 (테스트 제외)
rg "\.unwrap\(\)|\.expect\(" src-tauri/src --type rust | grep -v "test"
```

**주요 수정 대상**:
1. **Stage 실행 로직** (`stage_actor.rs`, `strategies/default.rs`):
   ```rust
   // ❌ Before
   let http = Arc::new(cfg.create_http_client().expect("http client"));
   
   // ✅ After
   let http = Arc::new(cfg.create_http_client()
       .context("Failed to create HTTP client")?);
   ```

2. **데이터베이스 경로 관리** (`database_paths.rs`):
   ```rust
   // ❌ Before
   .expect("DatabasePathManager가 초기화되지 않았습니다")
   
   // ✅ After
   .ok_or_else(|| anyhow!("DatabasePathManager not initialized. Call initialize() first"))?
   ```

3. **파일 경로 처리** (`database_connection.rs`):
   ```rust
   // ❌ Before
   let label = path.file_name().unwrap().to_string_lossy().to_string();
   
   // ✅ After
   let label = path.file_name()
       .ok_or_else(|| anyhow!("Invalid file path: no filename"))?
       .to_string_lossy()
       .to_string();
   ```

4. **동시성 제어** (`simple_http_client.rs`):
   ```rust
   // ❌ Before
   let _permit = self.semaphore.acquire().await.unwrap();
   
   // ✅ After
   let _permit = self.semaphore.acquire().await
       .map_err(|e| anyhow!("Semaphore acquire failed: {}", e))?;
   ```

**실행 계획**:
1. [ ] `anyhow` crate 의존성 확인 (이미 추가됨)
2. [ ] 파일별로 순차 수정 (한 번에 하나씩)
   - Stage 실행 로직 → 데이터베이스 경로 → HTTP 클라이언트
3. [ ] 각 파일 수정 후 `cargo test` 실행
4. [ ] Clippy 경고 확인: `cargo clippy --all-targets`

**체크리스트**:
- [ ] `stage_actor.rs`: 10개 수정
- [ ] `strategies/default.rs`: 8개 수정
- [ ] `database_paths.rs`: 4개 수정
- [ ] `database_connection.rs`: 2개 수정
- [ ] `simple_http_client.rs`: 3개 수정
- [ ] `integrated_product_repository.rs`: 10개 수정

### Task 2.2: mod.rs 사용 검토 및 제거

**현재 상태**: `src-tauri/src/infrastructure/mod.rs` 사용 중

**Modern Rust 2024 권장 구조**:
```
src-tauri/src/
├── infrastructure.rs      // ✅ 권장 (디렉토리 게이트 파일)
│   or
├── infrastructure/        // 현재 구조
│   ├── lib.rs            // ✅ 대안 (디렉토리용 lib.rs)
│   ├── config.rs
│   ├── crawler.rs
│   └── ...
```

**조사 필요**:
- [ ] `infrastructure/mod.rs` → `infrastructure.rs` 이동 가능 여부
- [ ] 다른 `mod.rs` 파일 존재 여부 확인
  ```bash
  find src-tauri/src -name "mod.rs" -type f
  ```

**참고**: 현재 `infrastructure/mod.rs`는 주석에 "Directory-style module"이라고 명시되어 있으므로, 이미 Modern 스타일로 구현됨 (유지)

**결정**: ✅ 현재 구조 유지 (directory-style은 허용됨, `mod.rs` 남용만 금지)

### Task 2.3: 불필요한 clone() 제거

**현재 문제**: `Arc::new(value.clone())` 패턴 8곳 발견

**주요 수정 대상**:
```rust
// ❌ Before
let executor = Arc::new(RealCrawlingStageExecutor::new(integration_service.clone()));

// ✅ After
let executor = Arc::new(RealCrawlingStageExecutor::new(Arc::clone(&integration_service)));
// 또는 integration_service가 이미 Arc라면
let executor = integration_service.clone(); // Arc::clone은 cheap
```

**조사 항목**:
1. [ ] 각 `clone()` 사용처의 타입 확인
2. [ ] 불필요한 intermediate clone 제거
3. [ ] `Arc::clone(&x)` 명시적 사용 (가독성)

**체크리스트**:
- [ ] `real_crawling_commands.rs`: 1개
- [ ] `shallow_sync_commands.rs`: 1개
- [ ] `crawling_integration.rs`: 1개
- [ ] `session_actor.rs`: 1개 (plan.clone → Arc::clone)
- [ ] `lib.rs`: 1개 (app_state.clone())

### Task 2.4: events.log 개발 전용으로 전환

**현재 상황**: `events.log`가 프로덕션 경로에 생성됨 (`~/Library/Application Support/matter-certis-v2/logs/events.log`)

**목표**: 개발/디버깅 전용으로 전환 (release 빌드에서 완전히 제외)

**핵심 원칙**:
- ✅ Debug 빌드 전용: `#[cfg(debug_assertions)]`
- ✅ 개발 경로 사용: `src-tauri/target/debug/logs/events.log`
- ✅ Release 빌드: events.log 레이어 완전히 제외
- ✅ 설정 기반 활성화/비활성화 (선택사항)

**구현 계획**:

#### 2.4.1 개발 빌드 전용 경로 설정
```rust
// src-tauri/src/infrastructure/logging.rs

/// Get events log directory (debug builds only)
#[cfg(debug_assertions)]
fn get_events_log_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join("logs")
}

/// Events log disabled in release builds
#[cfg(not(debug_assertions))]
fn get_events_log_dir() -> PathBuf {
    PathBuf::new() // 빈 경로 반환
}
```

#### 2.4.2 조건부 events.log 레이어 추가
```rust
// src-tauri/src/infrastructure/logging.rs (기존 코드 수정)

// 기존: let events_appender = rolling::never(&log_dir, "events.log");
// 변경:
#[cfg(debug_assertions)]
let events_appender = rolling::never(&get_events_log_dir(), "events.log");

// events.log 레이어를 #[cfg(debug_assertions)]로 감싸기
#[cfg(debug_assertions)]
let events_layer = fmt::Layer::new()
    .with_writer(events_writer)
    .with_timer(KstTimeFormatter)
    .with_target(true)
    .with_thread_ids(false)
    .with_file(false)
    .with_line_number(false)
    .with_ansi(false)
    .with_filter(make_events_filter());

// registry 초기화 시 조건부 추가
#[cfg(debug_assertions)]
registry
    .with(file_layer)
    .with(console_layer)
    .with(events_layer)  // Debug 빌드만
    .init();

#[cfg(not(debug_assertions))]
registry
    .with(file_layer)
    .with(console_layer)
    // events_layer 제외
    .init();
```

#### 2.4.3 설정 구조 추가 (선택사항)
```rust
// src-tauri/src/infrastructure/config.rs
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoggingConfig {
    // ... 기존 필드들
    
    /// Enable events.log for development/debugging (debug builds only)
    #[cfg(debug_assertions)]
    #[serde(default)]
    pub enable_events_log: bool,
}
```

#### 2.4.4 UI 설정 탭 업데이트 (선택사항)
```typescript
// src/components/settings/LoggingSettings.tsx
{import.meta.env.DEV && (
  <Switch
    checked={config.logging.enable_events_log}
    onChange={(enabled) => updateConfig('logging.enable_events_log', enabled)}
    label="Enable Events Log (Debug Only)"
    description="Creates events.log in src-tauri/target/debug/logs/ (actor-event, kpi.* targets)"
  />
)}

{/* Release 빌드에서는 비활성화 표시 */}
{import.meta.env.MODE === 'production' && (
  <Notice type="info">
    Events logging is disabled in production builds
  </Notice>
)}
```

**체크리스트**:
- [ ] `get_events_log_dir()` 함수 구현 (#[cfg(debug_assertions)])
- [ ] logging.rs에서 events.log 레이어를 #[cfg(debug_assertions)]로 감싸기
- [ ] 기존 4곳의 events_appender 경로 수정 (line 332, 339, 429, 435)
- [ ] registry 초기화를 조건부 컴파일로 분기 (debug: events_layer 포함, release: 제외)
- [ ] (선택) LoggingConfig에 enable_events_log 필드 추가
- [ ] (선택) UI 설정 탭에 events.log 토글 추가 (import.meta.env.DEV만)
- [ ] 테스트: `cargo build` (debug) → `src-tauri/target/debug/logs/events.log` 생성 확인
- [ ] 테스트: `cargo build --release` → events.log 미생성 확인
- [ ] 문서: 개발자 가이드에 events.log 사용법 추가 (actor-event, kpi.* 타겟 설명)

**참고**:
- Debug 빌드 경로: `src-tauri/target/debug/logs/events.log`
- Release 빌드: events.log 레이어 완전히 제외 (#[cfg(not(debug_assertions))])
- 프로덕션 경로 사용 금지: `~/Library/Application Support/` 아님
- 기록 대상: actor-event, kpi.plan, kpi.batch, kpi.session, kpi.execution_plan 등

### Task 2.5: ts-rs 타입 동기화 검증

**현재 상황**: Rust → TypeScript 타입 자동 생성 (ts-rs)

**검증 항목**:

#### 2.5.1 생성된 타입 파일 확인
```bash
# 생성된 TypeScript 타입 파일 목록
find src/bindings -name "*.ts" | head -20

# 최근 수정된 파일 확인 (타입 변경 후 재생성 여부)
ls -lt src/bindings/*.ts | head -10
```

#### 2.5.2 타입 동기화 체크
```bash
# Rust 구조체에 #[derive(TS)] 있는지 확인
rg "#\[derive.*TS.*\]" src-tauri/src --type rust

# ts-rs export 설정 확인
rg "#\[ts\(export\)\]" src-tauri/src --type rust

# 프론트엔드에서 import하는 타입 확인
rg "from.*bindings" src --type typescript
```

#### 2.5.3 타입 불일치 탐지
```bash
# 수동으로 정의된 타입과 ts-rs 생성 타입 비교
rg "interface.*Result|type.*Result" src/types --type typescript
rg "export interface" src/bindings --type typescript

# 중복 타입 정의 확인
```

**수정 전략**:
1. **누락된 #[derive(TS)]**: 백엔드 구조체에 추가
2. **불일치**: `npm run generate-types` 실행 (있다면)
3. **수동 타입 제거**: ts-rs 생성 타입 사용

**예시**:
```rust
// ❌ Before (ts-rs 미사용)
pub struct MyResult {
    pub success: bool,
}

// ✅ After (ts-rs 사용)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MyResult {
    pub success: bool,
}
```

**체크리스트**:
- [ ] 생성된 타입 파일 목록 확인
- [ ] #[derive(TS)] 누락된 구조체 검색
- [ ] 프론트엔드에서 수동 정의된 타입과 비교
- [ ] 중복 타입 정의 제거
- [ ] `npm run type-check` 통과
- [ ] 타입 생성 스크립트 문서화 (scripts/generate_types.sh 등)

**참고**:
- ts-rs 설정: `src-tauri/Cargo.toml`의 ts-rs dependency
- 생성 경로: `src/bindings/` (기본값)
- 빌드 후크: `build.rs` 또는 별도 스크립트

---

## Phase 3: 프론트엔드 정리 (2-3일)

> **목적**: 타입 안전성 개선, deprecated API 제거  
> **우선순위**: 🟡 Medium

### Task 3.1: DiagnosticsPanel 타입 정의

**현재 문제**:
```typescript
// CrawlingEngineTabSimple.tsx
const parts = Object.fromEntries(...) as Record<string, number>; // ❌ any 남용
const gap: any // ❌
const g: any // ❌
```

**새 타입 정의**:
```typescript
// src/types/diagnostics.ts
export interface DiagnosticsResult {
  total_products: number;
  total_products_without_coords: number;
  max_page_id_db: number | null;
  group_summaries: GroupSummary[];
  missing_pages: PageGap[];
  duplicate_positions: DuplicatePosition[];
  total_missing_pages: number;
}

export interface GroupSummary {
  page_id: number;
  current_page_number: number | null;
  status: 'ok' | 'warning' | 'error';
  count: number;
  distinct_indices: number;
  duplicate_indices?: number[];
  missing_indices?: number[];
  out_of_range_count?: number;
}

export interface PageGap {
  gap_type: 'single' | 'range';
  start_page: number;
  end_page?: number;
  start_physical_page?: number | null;
  end_physical_page?: number | null;
  missing_count: number;
}

export interface DuplicatePosition {
  page_id: number;
  index_in_page: number;
  count: number;
  product_ids: number[];
}
```

**실행 순서**:
1. [ ] `src/types/diagnostics.ts` 생성
2. [ ] 타입 정의 추가
3. [ ] DiagnosticsPanel에서 타입 import
4. [ ] `any` 제거
5. [ ] `npm run type-check` 통과

**체크리스트**:
- [ ] 타입 파일 생성
- [ ] 모든 인터페이스 정의
- [ ] DiagnosticsPanel 적용
- [ ] CrawlingEngineTabSimple 적용
- [ ] `any` 제거 확인

### Task 3.2: 백업/복원 API 타입 통합

**현재 문제**:
```typescript
// tauri-api.ts
backup_file?: string | null; // 여러 곳 중복 정의
```

**새 타입 정의**:
```typescript
// src/types/backup.ts
export interface BackupResult {
  backup_file: string | null;
  timestamp: string;
  size_bytes: number;
}

export interface RestoreResult {
  dataset: string;
  processed: number;
  inserted: number;
  updated: number;
  errors: string[];
  backup_source?: string | null;
}

export interface ImportOptions {
  reseed: boolean;
  dry_run?: boolean;
}
```

**실행 순서**:
1. [ ] `src/types/backup.ts` 생성
2. [ ] 타입 정의
3. [ ] `tauri-api.ts` 적용
4. [ ] 관련 컴포넌트 타입 수정

**체크리스트**:
- [ ] 타입 파일 생성
- [ ] API 서비스 적용
- [ ] 타입 체크 통과

### Task 3.3: Deprecated API 제거

**대상**:
```typescript
// tauri-api.ts
// 라인 280
/**
 * Get the current crawling progress and status (deprecated: use actor-system API directly)
 */
async getCrawlingProgress(): Promise<...> { ... } // ❌ 제거

// 라인 586
// Deprecated legacy subscriptions removed: progress, task-status, ...
// 관련 코드 정리

// 라인 703
// legacy detailed-crawling-event removed
// 관련 코드 정리
```

**실행 순서**:
1. [ ] deprecated 메서드 사용처 확인
   ```bash
   rg "getCrawlingProgress" src/
   ```
2. [ ] 새 API로 전환 (또는 제거)
3. [ ] deprecated 메서드 삭제
4. [ ] 주석 정리

**체크리스트**:
- [ ] 사용처 파악
- [ ] 마이그레이션 or 제거
- [ ] deprecated 코드 삭제
- [ ] 주석 정리

---

## Phase 4: 문서 및 스크립트 정리 (1-2일)

> **목적**: 현재 아키텍처 반영, 레거시 스크립트 제거  
> **우선순위**: 🟢 Low (기능 영향 없음)

### Task 4.1: UI 가이드 문서 업데이트

**파일**: `guide/SolidJS-UI-Implementation-Guide.md`

**변경사항**:
```markdown
# Before (라인 655-823)
### 4. 분석 탭 (Analysis Tab)
...

# After
### 4. 분석 탭 (Deprecated)
> ⚠️ 이 섹션은 더 이상 사용되지 않습니다.
> 현재 UI는 3개 탭 (크롤링, 설정, 로컬DB)만 사용합니다.
...

## 현재 활성 탭 구조

### 1. 크롤링 엔진 탭
- 사이트 상태 체크
- 크롤링 범위 계산
- ...

### 2. 설정 탭
- ...

### 3. 로컬DB 탭
- ...
```

**체크리스트**:
- [ ] 분석 탭 섹션 "Deprecated" 표시
- [ ] 현재 3개 탭 위주로 재구성
- [ ] 탭 추가 방법 간략 가이드 추가

### Task 4.2: 레거시 키워드 검색 및 정리

```bash
# 1. 문서 디렉토리 검색
rg -i "dashboard|analysis tab|actor system" docs/ guide/

# 2. 발견된 문서들 리스트업
# - docs/shallow_crawl_ui_guide.md
# - docs/site_health_monitoring.md
# - ...

# 3. 각 문서 업데이트
```

**업데이트 가이드**:
- "Dashboard" → "3개 탭 UI" 또는 제거
- "Analysis Tab" → "LocalDB 탭의 분석 기능" 또는 제거
- "Actor System Tab" → "크롤링 엔진 (내부 아키텍처)" 또는 제거

**체크리스트**:
- [ ] 키워드 검색 실행
- [ ] 발견된 문서 목록 작성
- [ ] 각 문서 업데이트
- [ ] 크로스 체크

### Task 4.3: 레거시 스크립트 정리

**조사 대상**:
```bash
# 1. dev.sh 중복 확인
diff dev.sh src-tauri/dev.sh

# 2. scripts/ 폴더 스크립트 분석
ls -la scripts/*.{sh,mjs,ts}
```

**중복 제거**:
```bash
# dev.sh 통합 예시
# 결정: 루트 dev.sh = 프론트엔드, src-tauri/dev.sh = 백엔드
echo "# Frontend dev server" > dev.sh
echo "npm run dev" >> dev.sh

echo "# Backend dev build" > src-tauri/dev.sh
echo "cargo build" >> src-tauri/dev.sh
```

**미사용 스크립트 아카이브**:
```bash
mkdir -p scripts/_archived_20251009

# 조건부 이동
mv scripts/diagnose_canonical_page.mjs scripts/_archived_20251009/
mv scripts/test_site_structure.sh scripts/_archived_20251009/
mv scripts/check_csa_list_pages.mjs scripts/_archived_20251009/
```

**체크리스트**:
- [ ] dev.sh 중복 분석 및 통합
- [ ] scripts/ 스크립트 용도 파악
- [ ] 미사용 스크립트 아카이브
- [ ] README 업데이트 (스크립트 용도 문서화)

### Task 4.4: 레거시 상태 관리 및 타입 정리

**조사 대상**:
```bash
# 1. 스토어 파일에서 레거시 상태 검색
rg -i "dashboard|analysis" src/stores/

# 2. 타입 파일에서 레거시 타입 검색
rg -i "DashboardData|AnalyticsReport" src/types/
```

**제거 대상 예시**:
```typescript
// src/stores/uiStore.ts
interface UIState {
  // ❌ 제거
  dashboardExpanded: boolean;
  analysisTabFilter: string;
}

// src/types/global.d.ts
interface DashboardData { ... } // ❌ 제거
interface AnalyticsReport { ... } // ❌ 제거
```

**체크리스트**:
- [ ] 스토어 레거시 상태 검색
- [ ] 타입 레거시 정의 검색
- [ ] 미사용 항목 제거
- [ ] `ts-prune` 재실행 확인

---

## Phase 5: 아키텍처 원칙 준수 및 선택적 개선

> **목적**: 리팩토링 시 Modern Rust 2024 원칙 준수, 구현 완료된 아키텍처 검증  
> **우선순위**: 🟡 Medium (코드 품질 핵심)  
> **참고 문서**: `guide/re-arch-plan-final2.md`, `guide/re-arch-plan-final3.md`

### 🦀 Modern Rust 2024 필수 준수 원칙

> **⚠️ 리팩토링 시 모든 코드는 아래 원칙을 준수해야 함**

#### 코드 품질 기준
- **`mod.rs` 사용 금지**: 모듈은 `lib.rs` 또는 `directory/file.rs` 사용
- **Clippy 100% 준수**: `cargo clippy --all-targets --all-features`
  - `#![warn(clippy::all, clippy::pedantic, clippy::nursery)]` 적용
- **`unwrap()` 금지**: 모든 에러는 `Result<T, E>`로 적절히 처리
- **불필요한 `clone()` 최소화**: 참조 전달 우선, 소유권 이동 최적화
- **ts-rs 8.0 타입 안전성**: 백엔드-프론트엔드 타입 자동 동기화

#### Clean Code 원칙
- **명확한 네이밍**: 변수/함수명은 의도를 명확히 표현
- **단일 책임 원칙 (SRP)**: 하나의 함수/모듈은 하나의 책임만
- **최소 의존성**: 순환 참조 제거, 의존성 그래프 단순화

#### 함수형 프로그래밍 원칙
- **Stateless 메서드 우선**: 가급적 순수 함수로 작성
- **불변성 추구**: 가변 상태 최소화, `Arc` 대신 메시지 전달
- **명시적 의존성**: 내부 캐시/상태 대신 파라미터로 명시적 전달
  ```rust
  // ❌ 암시적 상태 의존
  fn analyze(&self) -> Result<Report> {
      let data = self.cache.get(); // 숨겨진 의존성
  }
  
  // ✅ 명시적 파라미터
  fn analyze(data: &Data) -> Result<Report> {
      // 순수 함수
  }
  ```

### 📋 구현 완료된 아키텍처 검증 체크리스트

> **현재 상태 확인**: 아래 항목들은 이미 구현 완료. 리팩토링 시 손상 여부만 확인

#### ✅ Backend-Only CRUD 패턴 (완료)
- [x] AppState 공유 연결 풀 (`src-tauri/src/application/state.rs`)
- [x] 중앙화된 데이터베이스 경로 (`~/Library/Application Support/matter-certis-v2/database/`)
- [x] 모든 CRUD는 Tauri Commands를 통해서만 수행
- [ ] **검증**: 프론트엔드에 직접 DB 접근 코드 없는지 확인
  ```bash
  # 프론트엔드에서 직접 DB 접근 검색
  rg "Database\\.load|SqlitePool|sqlx::" src/ --type ts
  ```

#### ✅ Actor 시스템 핵심 인프라 (완료)
- [x] AppContext 공유 컨텍스트 (`src-tauri/src/new_architecture/context.rs`)
- [x] 삼중 채널 시스템 (Control/Data/Event)
- [x] ExecutionPlan Contract v1 (page_slots, plan_hash)
- [x] SessionActor → BatchActor 실행 경로
- [ ] **검증**: Phase trait 구현 확인
  ```bash
  # Phase trait 구현체 검색
  rg "impl Phase for" src-tauri/src/
  ```

#### ✅ Session 제어 시스템 (완료)
- [x] SessionRegistry (pause/resume/shutdown)
- [x] watch 채널 기반 제어 신호
- [x] 진행률 추적 (pages, batches)
- [x] 에러 추적 (last_error, error_count)
- [x] Resume 토큰 (generated_at, remaining_pages)
- [ ] **검증**: get_session_status API 응답 확인
  ```bash
  # 세션 상태 API 테스트
  cargo test session_status
  ```

#### ✅ Graceful Shutdown (완료)
- [x] Phase loop 종료 감지
- [x] PhaseAborted 이벤트 발행
- [x] SessionRegistry 상태 전환 (ShuttingDown → Completed)
- [ ] **검증**: Shutdown 시나리오 테스트
  ```bash
  # Graceful shutdown 테스트
  cargo test graceful_shutdown
  ```

#### ✅ 설정 파일 기반 자율 운영 (완료)
- [x] `matter_certis_config.json` 기반 설정 로드
- [x] ConfigManager 파일 감시 (notify crate)
- [x] 백엔드 자율 동작 (프론트엔드 독립)
- [ ] **검증**: UI에서 설정값 전송하는 API 제거 확인
  ```bash
  # 설정값 전송 API 검색
  rg "invoke.*config|invoke.*batch_size|invoke.*concurrency" src/ --type ts
  ```

### 🔧 선택적 개선 사항 (필요시만 수행)

#### Task 5.1: 페이지 탐색 캐시 최적화
**현재 구조**:
```rust
page_cache: Arc<tokio::sync::Mutex<HashMap<u32, PageAnalysisCache>>>
```

**개선 아이디어** (선택적):
- [ ] LRU 캐시 도입 (`lru` crate)
- [ ] TTL 설정 및 캐시 히트율 모니터링
- [ ] 성능 벤치마크

**실행 조건**: 메모리 사용량이 문제가 되거나 캐시 히트율이 낮을 때만

#### Task 5.2: 데이터베이스 분석 성능 개선
**문제점**: 대규모 데이터셋에서 `analyze_current_state` 느림

**개선 방안** (선택적):
- [ ] 인덱스 최적화 (page_id, index_in_page)
- [ ] 배치 처리 (청크 단위 분석)
- [ ] Stream API 도입

**실행 조건**: 7000+ 제품에서 분석 시간이 5초 이상 걸릴 때만

#### Task 5.3: 에러 처리 일관성 개선
**현재 문제**: `Result<T, String>` vs `anyhow::Error` 혼재

**개선 방안** (선택적):
```rust
// anyhow::Error 통일
use anyhow::{Result, Context};

pub async fn some_operation() -> Result<T> {
    load_config().context("Failed to load configuration")?;
    // ...
}
```

**실행 조건**: 에러 추적/디버깅이 어려울 때만

### 📚 Phase 5 참고 문서

**필수 읽기**:
- `guide/re-arch-plan-final2.md` - Actor 모델 및 Phase 로드맵
- `guide/re-arch-plan-final3.md` - 핵심 인프라 설계

**권장 읽기**:
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clean Architecture in Rust](https://www.youtube.com/watch?v=wU8hQvU8aKM)
- [Actor Model in Tokio](https://ryhl.io/blog/actors-with-tokio/)
- [DDD with Rust](https://github.com/rust-unofficial/patterns/blob/master/patterns/behavioural/strategy.md)

---

## Phase 6: CI/CD 통합 (1일)

> **목적**: 자동화된 테스트 및 품질 관리  
> **우선순위**: 🟡 Medium

### Task 6.1: GitHub Actions 워크플로우

**파일**: `.github/workflows/test.yml`

```yaml
name: Regression Tests

on:
  pull_request:
    branches: [main, develop, more-data]
  push:
    branches: [main, more-data]

jobs:
  test-backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
          
      - name: Run unit tests
        run: cd src-tauri && cargo test --lib
        
      - name: Run integration tests
        run: cd src-tauri && cargo test --tests
        
      - name: Check coverage
        run: |
          cargo install cargo-tarpaulin
          cd src-tauri
          cargo tarpaulin --out Xml --output-dir ../coverage/
          
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage/cobertura.xml

  test-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          
      - name: Install dependencies
        run: npm ci
        
      - name: Type check
        run: npm run type-check
        
      - name: Lint
        run: npm run lint
```

**체크리스트**:
- [ ] `.github/workflows/` 디렉토리 생성
- [ ] `test.yml` 작성
- [ ] PR에서 테스트 실행 확인
- [ ] 커버리지 리포트 연동

### Task 6.2: Pre-commit Hook

**파일**: `.git/hooks/pre-commit` (또는 `husky` 사용)

```bash
#!/bin/bash
echo "🧪 Running tests before commit..."

# Backend tests
cd src-tauri
cargo test --lib --tests --quiet
if [ $? -ne 0 ]; then
  echo "❌ Backend tests failed. Commit aborted."
  exit 1
fi
cd ..

# Frontend type check
npm run type-check --silent
if [ $? -ne 0 ]; then
  echo "❌ Type check failed. Commit aborted."
  exit 1
fi

echo "✅ All checks passed. Proceeding with commit."
```

**또는 Husky 사용**:
```bash
# 1. Husky 설치
npm install --save-dev husky

# 2. husky 초기화
npx husky install

# 3. pre-commit hook 추가
npx husky add .husky/pre-commit "npm run test:pre-commit"
```

**package.json**:
```json
{
  "scripts": {
    "test:pre-commit": "cd src-tauri && cargo test --lib --quiet && cd .. && npm run type-check"
  }
}
```

**체크리스트**:
- [ ] Pre-commit hook 설정 (shell 또는 husky)
- [ ] 테스트 스크립트 추가
- [ ] 로컬에서 동작 확인

---

## 작업 체크리스트

### Week 1: 테스트 인프라 (Phase 0)
- [ ] **Day 1**: 베이스라인 확보
  - [ ] 테스트 결과 저장
  - [ ] E2E 시나리오 수동 실행
  - [ ] Golden outputs 수집
  
- [ ] **Day 2-3**: 크리티컬 테스트 추가
  - [ ] page_discovery_regression.rs
  - [ ] data_analysis_regression.rs
  - [ ] sync_operations_regression.rs
  
- [ ] **Day 4-5**: Golden Test
  - [ ] Golden 디렉토리 생성
  - [ ] Golden 파일 수집
  - [ ] golden_tests.rs 작성

### Week 2: 코드 정리 (Phase 1-3)
- [ ] **Day 1-2**: 긴급 정리 (Phase 1)
  - [ ] 아카이브 디렉토리 삭제
  - [ ] API/서비스 제거
  - [ ] Unused exports 정리
  - [ ] Unreachable files 아카이브
  
- [ ] **Day 3-5**: 백엔드 중복 제거 (Phase 2)
  - [ ] analyze_data_changes 통합
  - [ ] DatabaseAnalyzer Mock 분리
  - [ ] CrawlingPlanner 통합
  - [ ] 미사용 백엔드 기능 제거
  
- [ ] **Day 6-7**: 프론트엔드 정리 (Phase 3)
  - [ ] DiagnosticsPanel 타입 정의
  - [ ] 백업/복원 API 타입 통합
  - [ ] Deprecated API 제거

### Week 3: 문서 및 통합 (Phase 4-6)
- [ ] **Day 1-2**: 문서 정리 (Phase 4)
  - [ ] UI 가이드 업데이트
  - [ ] 레거시 키워드 정리
  - [ ] 스크립트 정리
  - [ ] 상태 관리 타입 정리
  
- [ ] **Day 3**: CI/CD (Phase 6)
  - [ ] GitHub Actions 설정
  - [ ] Pre-commit hook 설정
  
- [ ] **Day 4-5**: 아키텍처 개선 (Phase 5, 선택적)
  - [ ] 캐시 최적화
  - [ ] DB 분석 성능 개선
  - [ ] 에러 처리 개선

### Week 4+: 검증 및 모니터링
- [ ] **전체 회귀 테스트**
  - [ ] 모든 테스트 통과 확인
  - [ ] E2E 시나리오 재검증
  - [ ] 성능 저하 없음 확인
  
- [ ] **문서 최종 업데이트**
  - [ ] 아키텍처 다이어그램
  - [ ] 리팩토링 히스토리
  - [ ] 개발 가이드

---

## 성공 기준 및 메트릭

### 정량적 지표

#### 테스트 커버리지
- [x] 기존 테스트 모두 통과 (베이스라인)
- [ ] 새 테스트 20개 이상 추가
- [ ] 코드 커버리지 70% 이상
- [ ] 리팩토링 후 테스트 실패 0건

#### 코드 품질
- [ ] `cargo clippy` 경고 0건
- [ ] `npm run type-check` 오류 0건
- [ ] Unused exports 0건
- [ ] Unreachable files 0건

#### 코드 감소
- [ ] 백엔드: 500-800 라인 감소
- [ ] 프론트엔드: 2000-3000 라인 감소
- [ ] 문서: 200 라인 정리
- [ ] 스크립트: 300-500 라인 정리
- [ ] **총 20-25% 코드베이스 감소**

### 정성적 지표

#### 기능 보존
- [ ] 리팩토링 중 regression 0건
- [ ] UI 동작 100% 동일
- [ ] 모든 E2E 시나리오 통과

#### 성능
- [ ] 사이트 상태 체크 시간 유지 또는 개선
- [ ] 크롤링 속도 저하 없음
- [ ] DB 진단 시간 유지 또는 개선

#### 개발 경험
- [ ] 빌드 시간 단축 (목표: 10% 이상)
- [ ] 타입 안전성 개선 (`any` 사용 50% 감소)
- [ ] 에러 메시지 명확성 향상

---

## 도구 및 라이브러리

### 현재 사용 중
```toml
# Cargo.toml [dev-dependencies]
# (테스트 관련은 현재 최소)
```

### 추가 권장
```toml
[dev-dependencies]
# Property-based testing
proptest = "1.5"

# JSON 비교
assert_json_diff = "2.0"

# Snapshot testing
insta = "1.40"

# Mock 생성
mockall = "0.13"

# 벤치마킹
criterion = "0.5"

# 커버리지 (CLI 도구)
# cargo install cargo-tarpaulin
```

### 프론트엔드
```json
{
  "devDependencies": {
    "ts-prune": "^0.10.3",
    "depcheck": "^1.4.7",
    "husky": "^8.0.0"
  }
}
```

---

## 리스크 관리

### 🔴 High Risk

#### 페이지 탐색 알고리즘 변경
- **리스크**: 최근 변경된 로직에 숨겨진 버그
- **완화책**: 
  - ✅ Task 0.3 테스트 추가 완료 필수
  - [ ] 다양한 시나리오 테스트 (빈 페이지, 네트워크 오류 등)
  - [ ] 프로덕션 환경에서 단계적 롤아웃

#### 데이터 분석 메서드 통합
- **리스크**: 통합 시 기존 로직 손상
- **완화책**:
  - [ ] Task 0.4 테스트 먼저 작성
  - [ ] 사용처 완전 파악 후 진행
  - [ ] Feature flag로 점진적 전환

### 🟡 Medium Risk

#### 아카이브 파일 삭제
- **리스크**: 실수로 필요한 파일 삭제
- **완화책**:
  - [ ] Git 태그로 백업 (`v-before-archive-cleanup`)
  - [ ] 삭제 전 마지막 컴파일 확인
  - [ ] 1주일 후 최종 삭제 결정

#### Deprecated API 제거
- **리스크**: 프론트엔드에서 아직 사용 중
- **완화책**:
  - [ ] `rg` 로 전체 사용처 확인
  - [ ] 단계적 마이그레이션
  - [ ] 마이그레이션 가이드 문서화

### 🟢 Low Risk

#### 문서 및 스크립트 정리
- **리스크**: 낮음 (기능 영향 없음)
- **완화책**: 백업 디렉토리 보관

---

## 롤백 계획

### 빠른 롤백 (긴급 상황)
```bash
# 1. 현재 작업 중단
git stash

# 2. 마지막 안정 커밋으로 복귀
git reset --hard <last-stable-commit>

# 3. 강제 푸시 (주의!)
git push origin more-data --force
```

### 단계별 롤백 (계획적)
```bash
# 1. 문제 있는 커밋 식별
git log --oneline

# 2. 해당 커밋만 revert
git revert <problematic-commit>

# 3. 테스트 후 푸시
cargo test && git push
```

### 완전 롤백 (프로젝트 재시작)
```bash
# 백업 태그로 복귀
git checkout v-before-refactoring

# 새 브랜치 생성
git checkout -b refactoring-attempt-2
```

---

## 커뮤니케이션 플랜

### 일일 체크인
- 매일 작업 종료 시 진행 상황 기록
- 문제 발생 시 즉시 공유
- 리스크 식별 시 완화책 논의

### 주간 리뷰
- 매주 금요일 진행 상황 리뷰
- 다음 주 계획 수립
- 메트릭 확인 (코드 감소량, 테스트 커버리지 등)

### 마일스톤 리뷰
- Phase 0 완료: 테스트 인프라 검증
- Phase 1-3 완료: 코드 정리 검증
- Phase 4-6 완료: 최종 검증 및 배포 준비

---

## 참고 자료

### 내부 문서
- [refactoring-todo.md](./refactoring-todo.md) - 최초 리팩토링 계획
- [refactoring-todo-by-gemini.md](./refactoring-todo-by-gemini.md) - Gemini 제안 사항
- [docs/testing-strategy-before-refactoring.md](./docs/testing-strategy-before-refactoring.md) - 테스트 전략

### 외부 자료
- [Rust Testing Best Practices](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Property-Based Testing in Rust](https://github.com/proptest-rs/proptest)
- [Golden Testing Pattern](https://ro-che.info/articles/2017-12-04-golden-tests)
- [Test Coverage with Tarpaulin](https://github.com/xd009642/tarpaulin)
- [SolidJS Testing Guide](https://www.solidjs.com/guides/testing)

---

## 부록 B: 참고 링크 모음

### Modern Rust & Clean Code
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) - 공식 API 설계 가이드
- [Clippy Lint List](https://rust-lang.github.io/rust-clippy/master/) - Clippy 경고 목록
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/) - Rust 디자인 패턴
- [Clean Code in Rust](https://www.youtube.com/watch?v=wU8hQvU8aKM) - Clean Architecture 적용

### Actor Model & Concurrency
- [Actor Model with Tokio](https://ryhl.io/blog/actors-with-tokio/) - Tokio 기반 Actor 구현
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) - 공식 Tokio 튜토리얼
- [Async Rust Book](https://rust-lang.github.io/async-book/) - 비동기 프로그래밍

### Testing & Quality
- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html) - 공식 테스트 가이드
- [Property-Based Testing](https://github.com/proptest-rs/proptest) - proptest 사용법
- [Golden Testing Pattern](https://ro-che.info/articles/2017-12-04-golden-tests) - Golden 테스트 설명
- [Tarpaulin Coverage](https://github.com/xd009642/tarpaulin) - 코드 커버리지 도구

### Frontend & TypeScript
- [SolidJS Testing Guide](https://www.solidjs.com/guides/testing) - SolidJS 테스트
- [ts-rs Documentation](https://docs.rs/ts-rs/latest/ts_rs/) - TypeScript 타입 생성
- [Tauri Best Practices](https://tauri.app/v1/guides/features/command/) - Tauri 명령 패턴

### Database & SQLite
- [SQLx Documentation](https://docs.rs/sqlx/latest/sqlx/) - SQLx 공식 문서
- [SQLite Best Practices](https://www.sqlite.org/bestpractice.html) - SQLite 모범 사례

### Architecture References
- **프로젝트 내부 문서**:
  - `guide/re-arch-plan-final2.md` - Actor 모델 및 Phase 로드맵
  - `guide/re-arch-plan-final3.md` - 핵심 인프라 설계
  - `guide/matter-certis-v2-development-guide.md` - 개발 가이드
  - `guide/DATABASE_SCHEMA.md` - 데이터베이스 스키마

---

## 부록 C: 빠른 참조 명령어

```bash
# 테스트 실행
cd src-tauri && cargo test --lib --tests

# 타입 체크
npm run type-check

# Unused exports 검사
npm run analyze:unused-exports

# 커버리지 측정
cd src-tauri && cargo tarpaulin --out Html

# 전체 검증 (pre-commit)
cd src-tauri && cargo test --lib --quiet && cd .. && npm run type-check
```

## 부록 D: 주요 파일 경로

**테스트 파일**:
- `src-tauri/tests/page_discovery_regression.rs`
- `src-tauri/tests/data_analysis_regression.rs`
- `src-tauri/tests/sync_operations_regression.rs`
- `src-tauri/tests/golden_tests.rs`

**타입 정의**:
- `src/types/diagnostics.ts`
- `src/types/backup.ts`

**문서**:
- `guide/SolidJS-UI-Implementation-Guide.md`
- `docs/testing-strategy-before-refactoring.md`

## 부록 E: 연락처 및 지원

**문제 발생 시**:
1. GitHub Issue 생성
2. Slack 채널 공유
3. 긴급: 롤백 후 논의

---

## 변경 이력

- **2025-10-09 v1.0**: 
  - 초기 마스터 플랜 생성
  - refactoring-todo.md + refactoring-todo-by-gemini.md + testing-strategy-before-refactoring.md 통합
  - 6개 Phase로 구조화
  - 상세 작업 체크리스트 작성
  - 리스크 관리 및 롤백 계획 수립

- **2025-10-09 v1.1**:
  - Phase 5 (아키텍처 개선) 대폭 확장
  - `guide/re-arch-plan-final2.md`, `guide/re-arch-plan-final3.md` 통합
  - Modern Rust 2024 원칙 추가
  - Actor 시스템 핵심 인프라 설계 반영
  - Backend-Only CRUD 패턴, Session 제어, Graceful Shutdown 등 상세화
  - 아키텍처 원칙 빠른 참조 섹션 추가

---

## 부록 A: 아키텍처 원칙 빠른 참조 🦀

> **리팩토링 시 모든 코드는 이 원칙을 준수해야 합니다**

### 1️⃣ Modern Rust 2024 체크리스트

**코드 작성 시 확인사항**:
```
□ mod.rs 사용하지 않음 (lib.rs 또는 file.rs 사용)
□ cargo clippy --all-targets 경고 없음
□ unwrap() 사용하지 않음 (모든 에러 Result<T, E>로 처리)
□ clone() 최소화 (참조 전달 또는 Arc 사용)
□ #[derive(TS)] 추가하여 TypeScript 타입 자동 생성
□ 명확한 함수/변수 네이밍 (의도 표현)
□ 단일 책임 원칙 준수 (하나의 함수는 하나의 일만)
```

### 2️⃣ 함수형 프로그래밍 패턴

**✅ 권장**:
```rust
// 순수 함수 (상태 없음, 부작용 없음)
fn calculate_severity(current: u32, previous: u32) -> SeverityLevel {
    let change_pct = ((previous - current) as f64 / previous as f64) * 100.0;
    match change_pct {
        pct if pct < 10.0 => SeverityLevel::Low,
        pct if pct < 30.0 => SeverityLevel::Medium,
        pct if pct < 50.0 => SeverityLevel::High,
        _ => SeverityLevel::Critical,
    }
}

// 명시적 의존성 (파라미터로 전달)
async fn analyze_data(
    pool: &SqlitePool,
    config: &AnalysisConfig,
    current_count: u32,
) -> Result<AnalysisReport> {
    // ...
}
```

**❌ 피해야 할 패턴**:
```rust
// 암시적 상태 의존
struct Analyzer {
    cache: HashMap<String, Data>, // 숨겨진 상태
}

impl Analyzer {
    fn analyze(&mut self) -> Report {
        let data = self.cache.get("key").unwrap(); // ❌ unwrap!
        // ❌ 내부 상태에 의존
    }
}
```

### 3️⃣ Backend-Only CRUD 원칙

**데이터베이스 접근 흐름**:
```
Frontend (SolidJS)
    ↓ (invoke Tauri Command)
Backend (Rust)
    ↓ (use AppState.get_database_pool())
Repository Layer
    ↓ (execute SQL)
SQLite Database
```

**금지 사항**:
```typescript
// ❌ 프론트엔드에서 직접 DB 연결
const db = await Database.load("sqlite:...");
const result = await db.execute("SELECT ...");

// ✅ 백엔드 API 사용
const result = await invoke('get_products_page', { page: 1, pageSize: 50 });
```

### 4️⃣ Actor 시스템 핵심 개념 (현재 구현)

**실제 구현된 Actor 구조**:
```
CrawlingPlanner (분석 & 계획 수립)
    ↓ generates
ExecutionPlan (실행 계획 계약)
    ↓ handed to
SessionActor (세션 전체 관리)
    ↓ (순차적 배치 실행)
    └── Batch 1, 2, 3... (순차 처리)
        ↓ (Stage 파이프라인)
        StageActor (Stage 2-3-4 실행)
            ├── Stage 2: ListPages (페이지 목록 크롤링)
            ├── Stage 3: ProductDetails (상세 정보 수집)
            └── Stage 4: DatabaseStorage (DB 저장)
                ↓ (AppEvent via broadcast)
                UI (SolidJS)
```

**주요 구성 요소**:

1. **CrawlingPlanner** (분석 & 계획 수립):
   - 사이트 상태 분석 (총 페이지 수, 마지막 페이지 제품 수)
   - DB 상태 분석 (총 제품 수, 중복 제품, 마지막 업데이트)
   - 최적화 전략 결정 (Full/Partial/Incremental/Smart)
   - ExecutionPlan 생성 (plan_hash로 무결성 보장)

2. **ExecutionPlan** (실행 계획 계약 - Contract v1):
   ```rust
   pub struct ExecutionPlan {
       pub plan_id: String,              // 계획 고유 ID
       pub session_id: String,            // 세션 ID
       pub crawling_ranges: Vec<PageRange>, // 크롤링 범위 목록
       pub batch_size: u32,               // 배치 크기 (예: 10)
       pub concurrency_limit: u32,        // 동시성 제한 (예: 3)
       pub estimated_duration_secs: u64,  // 예상 소요 시간
       pub plan_hash: String,             // 무결성 검증 해시 (SHA256)
       pub page_slots: Vec<PageSlot>,     // 사전 계산된 page_id 매핑
       pub list_only: bool,               // true: List만, false: List+Detail
       pub contract_version: u32,         // API 계약 버전 (현재: 1)
   }
   
   pub struct PageSlot {
       pub physical_page: u32,  // 물리 페이지 번호 (예: 589)
       pub page_id: i32,        // 논리 page_id (total_pages - physical)
       pub capacity: u8,        // 페이지 용량 (마지막: products_on_last_page, 나머지: 12)
   }
   ```

   **핵심 규칙**:
   - `page_id` 계산: `total_pages - physical_page` (역순 매핑)
   - `index_in_page`: 내림차순 (capacity-1 .. 0)
   - 마지막 페이지 capacity: `products_on_last_page`
   - 나머지 페이지: `PRODUCTS_PER_PAGE` (12)
   - `plan_hash`: SHA256(page_slots sorted + input_snapshot)

3. **SessionActor** (세션 관리):
   - ExecutionPlan 수신 및 검증
   - 배치 순차 실행 (한 번에 하나씩)
   - 진행률 추적 (processed_pages, completed_batches)
   - SessionRegistry 상태 업데이트
   - Graceful Shutdown 감지

4. **StageActor** (Stage 실행):
   - Stage 2-4 파이프라인 실행
   - 각 Stage 로직 실행 (ListPages, ProductDetails, DatabaseStorage)
   - AppEvent 발행 (StageStarted, StageCompleted, StageFailed)

5. **BatchActor**: Retired (현재 미사용, SessionActor가 배치 관리)

6. **IntegratedContext**: AppContext + EventEmitter 통합

**제어 신호 전파 (Graceful Shutdown)**:
```
UI → request_graceful_shutdown
    ↓
PHASE_SHUTDOWN_TX (watch channel)
    ↓
SessionRegistry (status = ShuttingDown)
    ↓
SessionActor (detect shutdown signal in batch loop)
    ↓
PhaseAborted event
    ↓
SessionRegistry (status = Completed)
```

**이벤트 흐름**:
```
StageActor (Stage 2-4 실행)
    ↓ emit
AppEvent (SessionStarted, BatchStarted, StageCompleted...)
    ↓ broadcast
ActorEventBridge (프론트엔드 브릿지)
    ↓
UI (SolidJS - listen & update)
```

**ExecutionPlan 생명주기**:
```
1. CrawlingPlanner.analyze_system_state()
   → 사이트/DB 상태 분석
   
2. CrawlingPlanner.create_execution_plan()
   → ExecutionPlan 생성 (plan_hash 계산)
   
3. SessionActor.execute(plan)
   → 계획 검증 후 실행
   
4. SessionCompleted event
   → integrity 검증 (completed_batches == expected_batches)
   
5. Resume Token 생성 (실패 시)
   → remaining_pages + plan_hash 저장
```

### 5️⃣ 설정 파일 기반 자율 운영

**역할 분리**:
- **백엔드**: `matter_certis_config.json` 읽기 → 자율 동작
- **프론트엔드**: 설정 파일 편집 + 상태 표시만

**금지**:
```typescript
// ❌ 프론트엔드에서 설정값 전송
await invoke('start_crawling', { 
  batchSize: 10,  // ❌ 설정값을 직접 전달
  concurrency: 5  // ❌
});

// ✅ 설정 파일에 저장 → 백엔드가 자동 감지
await invoke('save_config', { config });
await invoke('start_crawling'); // 파라미터 없음
```

### 6️⃣ ExecutionPlan Contract v1

**핵심 규칙**:
```rust
// 1. page_id 계산: total_pages - physical_page
// 2. index_in_page: 내림차순 (capacity-1 .. 0)
// 3. 마지막 페이지 capacity: products_on_last_page
// 4. 나머지 페이지: PRODUCTS_PER_PAGE (12)
// 5. plan_hash: SHA256(page_slots sorted)
```

**Integrity 검증**:
```rust
// Session 완료 시
if completed_batches != expected_batches {
    log::warn!("Integrity mismatch detected");
    final_state = "CompletedWithDiscrepancy";
}
```

### 7️⃣ 테스트 작성 원칙

**Regression 테스트 패턴**:
```rust
#[test]
fn test_critical_behavior() {
    // Given: 초기 상태 설정
    let input = create_test_data();
    
    // When: 동작 실행
    let result = function_under_test(input);
    
    // Then: 예상 결과 검증
    assert_eq!(result.status, ExpectedStatus);
    assert!(result.value > threshold);
}
```

**테스트 커버리지 목표**:
- 크리티컬 경로: 100%
- 비즈니스 로직: 80% 이상
- UI 컴포넌트: 50% 이상 (주요 상호작용)

### 8️⃣ 에러 처리 패턴

**백엔드 (Rust)**:
```rust
use anyhow::{Result, Context};

pub async fn risky_operation() -> Result<Data> {
    let config = load_config()
        .context("Failed to load configuration")?;
    
    let data = fetch_data(&config)
        .await
        .context("Failed to fetch data from remote")?;
    
    Ok(data)
}
```

**프론트엔드 (TypeScript)**:
```typescript
try {
  const result = await invoke<Result>('risky_command', params);
  if (!result.success) {
    throw new Error(result.error);
  }
} catch (error) {
  console.error('[ErrorCode: CMD_001]', error);
  showNotification('Operation failed', 'error');
}
```

### 9️⃣ Git Workflow

**브랜치 전략**:
```
main (안정)
  ↓
refactoring/phase-N (작업)
  ↓
refactoring/phase-N-task-M (개별 작업)
```

**커밋 메시지 규칙**:
```
type(scope): subject

- feat: 새 기능 추가
- fix: 버그 수정
- refactor: 리팩토링 (기능 변경 없음)
- test: 테스트 추가/수정
- docs: 문서 변경
- chore: 빌드/설정 변경

예: refactor(backend): unify analyze_data_changes methods
```

### 🔟 Code Review Checklist

**리뷰어 확인사항**:
```
□ Modern Rust 2024 원칙 준수
□ 테스트 추가됨 (또는 기존 테스트 통과)
□ 타입 안전성 (ts-rs 타입 동기화)
□ 에러 처리 적절함 (unwrap 없음)
□ 명확한 네이밍
□ 문서 주석 추가 (복잡한 로직)
□ 성능 고려 (불필요한 clone 없음)
□ 보안 고려 (SQL injection 방지 등)
```

---

## 부록 B: 참고 링크 모음
