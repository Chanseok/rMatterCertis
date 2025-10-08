# 리팩토링 TODO 목록

> 생성일: 2025-10-09
> 최종 업데이트: 2025-10-09

## 📋 목차

1. [중복 메서드 및 유사 기능 통합](#1-중복-메서드-및-유사-기능-통합)
2. [삭제된 탭 관련 기능 제거](#2-삭제된-탭-관련-기능-제거)
3. [사용되지 않는 백엔드 기능](#3-사용되지-않는-백엔드-기능)
4. [프론트엔드 미사용 코드](#4-프론트엔드-미사용-코드)
5. [아키텍처 개선 사항](#5-아키텍처-개선-사항)
6. [우선순위 및 실행 계획](#6-우선순위-및-실행-계획)

---

## 1. 중복 메서드 및 유사 기능 통합

### 🔴 High Priority

#### 1.1 페이지 탐색 관련 중복 메서드
**파일**: `src-tauri/src/infrastructure/crawling_service_impls.rs`

**문제점**: 
- ✅ ~~`find_last_valid_page_downward` - 1페이지 단위 하향 탐색 (삭제됨)~~
- ✅ `find_last_valid_page_with_safety_check` - 5페이지 단위 하향 탐색 + 안전성 체크 (활성)

**현재 상태**: 
- 2025-10-09: `find_last_valid_page_downward` 메서드 제거 완료
- 모든 호출이 `find_last_valid_page_with_safety_check`로 통합됨

**결과**:
- ✅ 1페이지 단위의 비효율적인 검색 제거
- ✅ 5페이지 단위 검색 + 12회 연속 빈 페이지 감지 시 fatal error 발생
- ✅ 더 빠르고 안정적인 페이지 탐색

---

#### 1.2 데이터 분석 중복 메서드
**파일**: `src-tauri/src/infrastructure/crawling_service_impls.rs`

**문제점**:
```rust
// 라인 1031
async fn analyze_data_changes(&self, current_estimated_products: u32) 
    -> (SiteDataChangeStatus, Option<DataDecreaseRecommendation>)

// 라인 1459  
async fn analyze_site_data_changes(&self, current_estimated_products: u32) 
    -> DataChangeAnalysis
```

**차이점**:
- `analyze_data_changes`: 사이트 전체 상태 + 권장사항 생성
- `analyze_site_data_changes`: 단순 증감 분석만 수행 (Initial/Increased/Decreased/Stable)

**제안**:
- [ ] 두 메서드의 사용처 파악
- [ ] 가능하면 `analyze_site_data_changes`를 `analyze_data_changes`로 통합
- [ ] 또는 역할을 명확히 분리하고 네이밍 개선 (`analyze_data_changes` → `analyze_data_changes_with_recommendations`)

---

#### 1.3 DatabaseAnalyzer 중복 구현
**파일**: `src-tauri/src/infrastructure/crawling_service_impls.rs`

**문제점**:
```rust
// DatabaseAnalyzerImpl (라인 2802)
impl DatabaseAnalyzer for DatabaseAnalyzerImpl {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis>
    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis>
}

// MockDatabaseAnalyzer (라인 3754)
impl DatabaseAnalyzer for MockDatabaseAnalyzer {
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis>
    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis>
}
```

**제안**:
- [ ] MockDatabaseAnalyzer를 테스트 전용 모듈로 분리 (`src-tauri/src/test_utils/mock_database.rs`)
- [ ] 프로덕션 코드에서 Mock 제거

---

#### 1.4 진행 상황 분석 메서드 중복
**파일**: `src-tauri/src/infrastructure/crawling_service_impls.rs`

**문제점**:
```rust
// 라인 3828
pub async fn analyze_simple_progress(repo: &IntegratedProductRepository) 
    -> Result<SimpleProgressInfo>

// 라인 3913
pub async fn analyze_crawling_progress(repo: &IntegratedProductRepository) 
    -> Result<CrawlingProgressInfo>
```

**제안**:
- [ ] 두 메서드의 반환 타입과 용도 분석
- [ ] 가능하면 통합하거나 명확한 역할 분리
- [ ] `SimpleProgressInfo`와 `CrawlingProgressInfo`의 차이점 문서화

---

### 🟡 Medium Priority

#### 1.5 CrawlingPlanner 분석 메서드 중복
**파일**: `src-tauri/src/crawl_engine/services/crawling_planner.rs`

**문제점**:
```rust
// 라인 195
pub async fn analyze_system_state(&self, app_state: &AppState) 
    -> Result<SystemAnalysis>

// 라인 223
pub async fn analyze_system_state_with_cache(&self, app_state: &AppState) 
    -> Result<SystemAnalysis>
```

**차이점**:
- `analyze_system_state`: 매번 새로운 분석
- `analyze_system_state_with_cache`: 캐시된 사이트 상태 활용

**제안**:
- [ ] 기본 동작을 캐시 사용으로 변경
- [ ] `force_refresh` 파라미터 추가로 통합
- [ ] 예: `analyze_system_state(app_state, force_refresh: bool)`

---

## 2. 삭제된 탭 관련 기능 제거

### 🔴 High Priority

#### 2.1 아카이브된 탭 컴포넌트 완전 삭제
**상태**: 이미 아카이브됨, 완전 삭제 필요

**대상 파일**:
```
archive/components/DatabaseDiagnostics.tsx
archive/components/tabs/AnalysisTab.tsx
archive/components/tabs/LiveCrawlingTab.tsx
archive/components/tabs/RealtimeDashboardTab.tsx
archive/components/tabs/ActorSystemTab.tsx
archive/components/tabs/LiveProductionTab.tsx
archive/components/tabs/DomainDashboardTab.tsx
```

**추가 아카이브 대상**:
```
src/_archive/tabs/RealtimeDashboardTab.tsx
src/_archive/tabs/LiveCrawlingTab.tsx
src/_archive/tabs/ActorSystemTab.tsx
src/_archive/tabs/GameDashboardTab.tsx
src/_archive/tabs/StatusTab.tsx
src/_archive/tabs/LiveProductionTab.tsx
src/_archive/tabs/NewArchTestTab.tsx
```

**제안**:
- [ ] `archive/` 디렉토리 전체 삭제 (Git 히스토리에 보존)
- [ ] `src/_archive/` 디렉토리 전체 삭제
- [ ] 필요시 Git tag 생성: `v-before-archive-cleanup`

---

#### 2.2 탭 관련 UI 가이드 문서 업데이트
**파일**: `guide/SolidJS-UI-Implementation-Guide.md`

**문제점**:
- 라인 655-823: "분석 탭 (Analysis Tab)" 구현 가이드가 여전히 존재
- 실제로는 3개 탭만 사용 (크롤링, 설정, 로컬DB)

**제안**:
- [ ] 분석 탭 섹션 제거 또는 "Deprecated" 표시
- [ ] 현재 활성 3개 탭 위주로 문서 재구성
- [ ] 탭 추가 방법에 대한 간단한 가이드만 남기기

---

#### 2.3 사용되지 않는 탭스토어 헬퍼 함수
**파일**: `src/stores/tabStore.ts`

**문제점**:
```typescript
// 라인 108: Removed unused expanded section helpers during cleanup
// 라인 124-128
export const toggleExpandedSection = ...  // unused
export const setExpandedSection = ...     // unused
```

**제안**:
- [ ] `reports/unused_exports_20250907T223324.txt` 확인
- [ ] 실제 미사용 함수들 제거
- [ ] TabConfig, TabState 타입도 현재 3개 탭에 맞게 단순화

---

### 🟡 Medium Priority

#### 2.4 DiagnosticsPanel 내 미사용 기능
**파일**: `src/components/tabs/parts/DiagnosticsPanel.tsx`

**현재 상태**:
- Stage X: "DB 레코드 체크 및 동기화"로 이름 변경됨
- 빠른 동기화, 스마트 동기화, 제품 보완 동기화 버튼 추가됨

**문제점**:
```tsx
// 진단 실행 버튼이 여전히 존재하지만 사용 빈도가 낮음
<button onClick={p.runDiagnostics}>
  {p.diagLoading() ? '진단 중…' : '진단 실행'}
</button>

// URL 중복 제거, products→details 동기화 등은 제거됨 (2025-10-08)
```

**제안**:
- [ ] 진단 실행 버튼의 실제 사용 패턴 분석
- [ ] 자동 진단으로 전환 가능한지 검토
- [ ] 또는 "고급 옵션"으로 숨기기

---

## 3. 사용되지 않는 백엔드 기능

### 🟡 Medium Priority

#### 3.1 진단 관련 Command 정리
**파일**: `src-tauri/src/commands/database/data_queries.rs`

**문제점**:
```rust
// 라인 435
pub async fn analytics_query(...)  // AnalysisTab에서 사용하던 기능

// 라인 956
pub struct DbDiagnosticsReport { ... }  // DatabaseDiagnostics 탭 전용

// 라인 ???
pub async fn diagnostics_analytics_mapping(...)  // 미사용?
```

**제안**:
- [ ] `analytics_query` 함수 사용처 확인
- [ ] AnalysisTab 삭제 후 미사용이면 제거
- [ ] `DbDiagnosticsReport` 타입 사용처 확인
- [ ] LocalDBTab에서 사용 중이면 유지, 아니면 제거

---

#### 3.2 중복 페이지 체크 기능
**파일**: `src-tauri/src/commands/crawling/actor_system.rs`

**문제점**:
```rust
// 라인 1299
pub async fn check_page_index_consistency() -> Result<String, String>
```

**제안**:
- [ ] 이 함수가 실제로 호출되는지 확인
- [ ] DiagnosticsPanel의 진단 기능과 중복되는지 검토
- [ ] 미사용이면 제거 또는 아카이브

---

#### 3.3 Shallow Sync 관련 분석 함수
**파일**: `src-tauri/src/commands/crawling/shallow_sync_commands.rs`

**문제점**:
```rust
// 라인 174
pub async fn analyze_missing_details(...)
```

**제안**:
- [ ] 이 함수의 사용처와 목적 확인
- [ ] 스마트 동기화와 중복되는 기능인지 검토
- [ ] 필요하면 유지, 아니면 통합 고려

---

### 🟢 Low Priority

#### 3.4 Repository의 find 계열 메서드들
**파일**: `src-tauri/src/domain/repositories.rs`

**문제점**:
```rust
async fn find_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProduct>>;
async fn find_by_device_type(&self, device_type: &str) -> Result<Vec<MatterProduct>>;
async fn find_by_vid(&self, vid: &str) -> Result<Vec<MatterProduct>>;
async fn find_by_certification_date_range(...) -> Result<Vec<MatterProduct>>;
```

**제안**:
- [ ] LocalDBTab 필터 기능에서 실제 사용 중인지 확인
- [ ] 미사용 메서드 제거
- [ ] 성능 최적화 (인덱스, 쿼리 최적화)

---

## 4. 프론트엔드 미사용 코드

### 🔴 High Priority

#### 4.1 Unused Exports 정리
**파일**: `reports/unused_exports_20250907T223324.txt`

**대상**:
```
src/platform/tauri.ts:355 - batchApiCalls
src/platform/tauri.ts:373 - isApiError
src/platform/tauri.ts:382 - hasApiError
src/platform/tauri.ts:386 - hasApiData
src/stores/tabStore.ts:124 - toggleExpandedSection
src/stores/tabStore.ts:128 - setExpandedSection
```

**제안**:
- [ ] 각 함수별 실제 사용처 최종 확인
- [ ] 완전 미사용이면 제거
- [ ] 내부에서만 사용하면 export 제거

---

#### 4.2 Unreachable Files 정리
**파일**: `reports/unused_frontend_20250907T220407.txt`

**대상**:
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

**제안**:
- [ ] 파일 아카이브 또는 삭제
- [ ] VendorForm, VendorManagement는 설정 탭에서 사용 가능성 있으니 확인 필요

---

#### 4.3 Deprecated API 호출 제거
**파일**: `src/services/tauri-api.ts`

**문제점**:
```typescript
// 라인 280
/**
 * Get the current crawling progress and status (deprecated: use actor-system API directly)
 */

// 라인 586
// Deprecated legacy subscriptions removed: progress, task-status, stage-change, 
// database-update, completion, crawling-stopped

// 라인 703
// legacy detailed-crawling-event removed
```

**제안**:
- [ ] deprecated 표시된 메서드들의 실제 사용처 확인
- [ ] 새 API로 완전 전환 후 삭제
- [ ] 마이그레이션 가이드 문서화

---

### 🟡 Medium Priority

#### 4.4 백업/복원 관련 API 정리
**파일**: `src/services/tauri-api.ts`

**문제점**:
```typescript
// 라인 389-395
async backupDatabase(): Promise<string>

// 라인 429, 489, 501, 514
backup_file?: string | null;  // 여러 곳에서 중복 타입 정의
```

**제안**:
- [ ] 백업 관련 타입을 별도 인터페이스로 통합
- [ ] `BackupResult`, `RestoreResult` 타입 정의
- [ ] 일관된 에러 핸들링 패턴 적용

---

## 5. 아키텍처 개선 사항

### 🔴 High Priority

#### 5.1 페이지 탐색 알고리즘 최적화
**현재 상태**:
- ✅ 5페이지 단위 하향 탐색으로 통합 완료
- ✅ 12회 연속 빈 페이지 감지 시 fatal error

**추가 개선 사항**:
- [ ] 캐시 전략 개선 (PageAnalysisCache 활용도 높이기)
- [ ] 페이지네이션 분석 신뢰도 향상
- [ ] 네트워크 오류 재시도 정책 개선

---

#### 5.2 데이터베이스 분석 성능 개선
**문제점**:
- 대규모 데이터셋에서 `analyze_current_state` 느림
- 진단 실행 시 UI 블로킹 발생

**제안**:
- [ ] 배치 처리 도입
- [ ] 인덱스 최적화 (page_id, index_in_page)
- [ ] 점진적 결과 반환 (Stream API)

---

### 🟡 Medium Priority

#### 5.3 타입 안전성 개선
**문제점**:
```typescript
// CrawlingEngineTabSimple.tsx에서 any 타입 남용
const parts = Object.fromEntries(...) as Record<string, number>;
const gap: any
const g: any
```

**제안**:
- [ ] 명확한 인터페이스 정의
- [ ] `DiagnosticsResult`, `GapInfo`, `GroupSummary` 등 타입 생성
- [ ] `any` 사용 최소화

---

#### 5.4 에러 처리 일관성 개선
**문제점**:
- 일부는 `Result<T, String>` 반환
- 일부는 `try-catch`로 처리
- 프론트엔드 에러 메시지 일관성 부족

**제안**:
- [ ] 백엔드: `Result<T>` 패턴 통일 (anyhow::Error 사용)
- [ ] 프론트엔드: ErrorBoundary 도입
- [ ] 에러 코드 정의 및 i18n 준비

---

## 6. 우선순위 및 실행 계획

### Phase 1: 긴급 정리 (1-2일)
- [x] **완료**: `find_last_valid_page_downward` 제거
- [ ] `archive/` 및 `src/_archive/` 디렉토리 완전 삭제
- [ ] Unused exports 정리 (tabStore, platform/tauri.ts)
- [ ] Unreachable files 아카이브

### Phase 2: 백엔드 중복 제거 (3-5일)
- [ ] `analyze_data_changes` vs `analyze_site_data_changes` 통합
- [ ] `analyze_simple_progress` vs `analyze_crawling_progress` 통합
- [ ] `analyze_system_state` 캐시 전략 통합
- [ ] MockDatabaseAnalyzer 테스트 모듈 분리

### Phase 3: 프론트엔드 타입 안전성 (2-3일)
- [ ] DiagnosticsPanel 관련 타입 정의
- [ ] `any` 타입 제거
- [ ] 백업/복원 API 타입 통합

### Phase 4: 문서 및 가이드 업데이트 (1-2일)
- [ ] SolidJS-UI-Implementation-Guide.md 업데이트 (3개 탭 위주)
- [ ] 아키텍처 다이어그램 업데이트
- [ ] 리팩토링 히스토리 문서화

### Phase 5: 성능 최적화 (필요시)
- [ ] 데이터베이스 분석 배치 처리
- [ ] 페이지 탐색 캐시 최적화
- [ ] 네트워크 재시도 정책 개선

---

## 📊 메트릭

### 제거 예상 코드량
- **백엔드**: ~500-800 라인 (중복 메서드, Mock 등)
- **프론트엔드**: ~2000-3000 라인 (아카이브 탭, 미사용 파일)
- **문서**: ~200 라인 (오래된 가이드)

### 기대 효과
- ✅ 코드베이스 20-25% 감소
- ✅ 타입 안전성 개선으로 버그 감소
- ✅ 빌드 시간 단축
- ✅ 신규 개발자 온보딩 시간 단축

---

## 🔗 관련 문서

- [아키텍처 가이드](./guide/matter-certis-v2-core-domain-knowledge.md)
- [개발 가이드](./guide/matter-certis-v2-development-guide.md)
- [UI 구현 가이드](./guide/SolidJS-UI-Implementation-Guide.md)
- [프론트엔드 도메인 지식](./guide/matter-certis-v2-frontend-domain-knowledge.md)

---

## 📝 변경 이력

- **2025-10-09**: 초기 문서 생성
  - `find_last_valid_page_downward` 제거 완료 기록
  - 중복 메서드 및 미사용 기능 식별
  - 3개 탭 구조에 맞춘 정리 계획 수립
