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

## Phase 2: 백엔드 중복 제거 (3-5일)

> **목적**: 중복 메서드 통합, Mock 분리  
> **우선순위**: 🟡 Medium (테스트 가드 필수)

### Task 2.1: analyze_data_changes 통합

**현재 상태**:
```rust
// crawling_service_impls.rs
async fn analyze_data_changes(&self, current: u32) 
    -> (SiteDataChangeStatus, Option<DataDecreaseRecommendation>)

async fn analyze_site_data_changes(&self, current: u32) 
    -> DataChangeAnalysis
```

**통합 계획**:
```rust
// 새로운 API
async fn analyze_data_changes(
    &self, 
    current: u32,
    with_recommendations: bool
) -> Result<DataAnalysisResult>

enum DataAnalysisResult {
    Simple(DataChangeAnalysis),
    Detailed {
        status: SiteDataChangeStatus,
        recommendation: Option<DataDecreaseRecommendation>,
    }
}
```

**실행 순서**:
1. [ ] 테스트 먼저 작성 (Task 0.4 완료 필요)
2. [ ] 사용처 파악
   ```bash
   rg "analyze_data_changes\(" src-tauri/
   rg "analyze_site_data_changes\(" src-tauri/
   ```
3. [ ] 새 API 구현
4. [ ] 기존 호출부 변경
5. [ ] 중복 메서드 제거
6. [ ] `cargo test` 통과 확인

**체크리스트**:
- [ ] 사용처 목록 작성
- [ ] 새 API 설계 및 구현
- [ ] 마이그레이션
- [ ] 테스트 통과

### Task 2.2: DatabaseAnalyzer Mock 분리

**현재 상태**:
```rust
// crawling_service_impls.rs (프로덕션 코드)
impl DatabaseAnalyzer for MockDatabaseAnalyzer { ... }
```

**목표 구조**:
```rust
// 프로덕션: src-tauri/src/infrastructure/crawling_service_impls.rs
impl DatabaseAnalyzer for DatabaseAnalyzerImpl { ... }

// 테스트: src-tauri/tests/mocks/database.rs
pub struct MockDatabaseAnalyzer { ... }
impl DatabaseAnalyzer for MockDatabaseAnalyzer { ... }
```

**실행 순서**:
1. [ ] 테스트 Mock 디렉토리 생성
   ```bash
   mkdir -p src-tauri/tests/mocks
   touch src-tauri/tests/mocks/mod.rs
   touch src-tauri/tests/mocks/database.rs
   ```
2. [ ] Mock 코드 이동
3. [ ] 프로덕션 코드에서 Mock 제거
4. [ ] 테스트에서 Mock import 변경
5. [ ] `cargo test` 통과 확인

**체크리스트**:
- [ ] Mock 디렉토리 생성
- [ ] Mock 코드 이동
- [ ] Import 경로 수정
- [ ] 프로덕션 코드 정리

### Task 2.3: CrawlingPlanner 분석 메서드 통합

**현재 상태**:
```rust
// crawling_planner.rs
pub async fn analyze_system_state(&self, app_state: &AppState) 
    -> Result<SystemAnalysis>

pub async fn analyze_system_state_with_cache(&self, app_state: &AppState) 
    -> Result<SystemAnalysis>
```

**통합 계획**:
```rust
pub async fn analyze_system_state(
    &self, 
    app_state: &AppState,
    force_refresh: bool
) -> Result<SystemAnalysis> {
    if force_refresh {
        // 캐시 무시하고 새로 분석
    } else {
        // 캐시된 사이트 상태 활용
    }
}
```

**실행 순서**:
1. [ ] 사용처 파악
2. [ ] 테스트 작성
3. [ ] 통합 API 구현
4. [ ] 호출부 변경 (`force_refresh` 파라미터 추가)
5. [ ] 중복 메서드 제거

**체크리스트**:
- [ ] 사용처 분석
- [ ] 통합 구현
- [ ] 마이그레이션
- [ ] 테스트 통과

### Task 2.4: 진행 상황 분석 메서드 정리

**현재 상태**:
```rust
// crawling_service_impls.rs
pub async fn analyze_simple_progress(...) -> Result<SimpleProgressInfo>
pub async fn analyze_crawling_progress(...) -> Result<CrawlingProgressInfo>
```

**조사 항목**:
1. [ ] 두 메서드의 반환 타입 차이 분석
2. [ ] 각각의 사용처 파악
3. [ ] 통합 가능 여부 결정

**결정 후 실행**:
- 통합 가능: Task 2.1과 유사하게 진행
- 분리 필요: 네이밍 개선 및 문서화

**체크리스트**:
- [ ] 차이점 문서화
- [ ] 통합 or 분리 결정
- [ ] 실행 및 테스트

### Task 2.5: 미사용 백엔드 기능 제거

#### 2.5.1 진단 관련 Command
```bash
# analytics_query 사용처 확인
rg "analytics_query" src-tauri/ src/

# DbDiagnosticsReport 사용처 확인
rg "DbDiagnosticsReport" src-tauri/ src/
```

**조건부 제거**:
- AnalysisTab 삭제 후 미사용 → 제거
- LocalDBTab에서 사용 중 → 유지

#### 2.5.2 페이지 일관성 체크
```bash
rg "check_page_index_consistency" src-tauri/ src/
```

**조건부 제거**:
- DiagnosticsPanel과 중복 → 제거
- 독립적 기능 → 유지

**체크리스트**:
- [ ] analytics_query 사용처 확인
- [ ] DbDiagnosticsReport 사용처 확인
- [ ] check_page_index_consistency 사용처 확인
- [ ] 미사용 함수 제거

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

## Phase 5: 아키텍처 개선 (필요시)

> **목적**: 성능 최적화, 코드 품질 향상  
> **우선순위**: 🟢 Low (선택적)

### Task 5.1: 페이지 탐색 캐시 최적화

**현재 구조**:
```rust
// PageAnalysisCache 활용도 분석
page_cache: Arc<tokio::sync::Mutex<HashMap<u32, PageAnalysisCache>>>
```

**개선 아이디어**:
1. LRU 캐시 도입 (용량 제한)
2. TTL(Time To Live) 설정
3. 캐시 히트율 모니터링

**실행 (선택적)**:
- [ ] 현재 캐시 히트율 측정
- [ ] LRU 캐시 구현 (`lru` crate)
- [ ] 성능 벤치마크

### Task 5.2: 데이터베이스 분석 성능 개선

**문제점**:
- 대규모 데이터셋에서 `analyze_current_state` 느림
- UI 블로킹 발생

**개선 방안**:
1. 배치 처리 (청크 단위 분석)
2. 인덱스 최적화
3. Stream API 도입 (점진적 결과 반환)

**실행 (선택적)**:
- [ ] 현재 성능 측정 (벤치마크)
- [ ] 인덱스 추가 (page_id, index_in_page)
- [ ] 배치 처리 구현
- [ ] 성능 비교

### Task 5.3: 에러 처리 일관성 개선

**현재 문제**:
- 일부는 `Result<T, String>` 반환
- 일부는 `try-catch`로 처리
- 에러 메시지 일관성 부족

**개선 계획**:
```rust
// 백엔드: anyhow::Error 통일
use anyhow::{Result, Context};

pub async fn some_operation() -> Result<T> {
    // ...
    .context("Failed to perform operation")?
}
```

```typescript
// 프론트엔드: ErrorBoundary + 에러 코드
enum ErrorCode {
  SITE_INACCESSIBLE = 'SITE_001',
  DB_CONNECTION_FAILED = 'DB_001',
  // ...
}
```

**실행 (선택적)**:
- [ ] 백엔드 에러 패턴 통일
- [ ] 프론트엔드 ErrorBoundary 도입
- [ ] 에러 코드 정의

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

## 부록

### A. 빠른 참조 명령어

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

### B. 주요 파일 경로

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

### C. 연락처 및 지원

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
