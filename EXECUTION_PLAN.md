# 리팩토링 실행 계획 (간트 차트 스타일)

> 기반 문서: REFACTORING_MASTER_PLAN.md  
> 작성일: 2025-10-09  
> 예상 기간: 3-4주

## 📅 전체 일정 개요

```
Week 1 (Phase 0: 테스트 인프라)
├─ Mon: 베이스라인 확보
├─ Tue-Wed: 크리티컬 테스트 작성
└─ Thu-Fri: Golden Test 구현

Week 2 (Phase 1-3: 코드 정리)
├─ Mon-Tue: 긴급 정리 (아카이브 삭제)
├─ Wed-Fri: 백엔드 중복 제거
└─ Weekend: 프론트엔드 정리 시작

Week 3 (Phase 3-4: 정리 완료)
├─ Mon-Tue: 프론트엔드 타입 안전성
├─ Wed-Thu: 문서 및 스크립트 정리
└─ Fri: CI/CD 통합

Week 4 (Phase 5-6: 검증 및 배포)
├─ Mon-Tue: 아키텍처 개선 (선택)
├─ Wed-Thu: 전체 회귀 테스트
└─ Fri: 배포 준비 및 문서 최종화
```

---

## Week 1: 테스트 인프라 구축

### Day 1 (Monday) - 베이스라인 확보
**목표**: 현재 상태 완전 스냅샷

#### 오전 (2-3시간)
- [ ] **9:00-10:00**: 테스트 실행 및 결과 저장
  ```bash
  cd src-tauri
  cargo test --lib --tests 2>&1 | tee ../test-results-baseline.txt
  cargo check -q 2>&1 | tee ../cargo-check-baseline.txt
  ```

- [ ] **10:00-11:00**: E2E 시나리오 수동 실행
  - 사이트 상태 체크 (596→589 감소 확인)
  - 크롤링 범위 재계산
  - 부분 크롤링 5페이지 실행

- [ ] **11:00-12:00**: Golden outputs 수집
  ```bash
  mkdir -p golden-outputs
  # 각 시나리오 응답 JSON 저장
  # 스크린샷 캡처 (DevTools)
  ```

#### 오후 (2-3시간)
- [ ] **14:00-15:30**: 테스트 커버리지 리포트 작성
  - 현재 테스트 목록 정리
  - 커버리지 갭 식별
  - `test-coverage-report.md` 작성

- [ ] **15:30-17:00**: 크리티컬 패스 문서화
  - E2E 시나리오 상세 기록
  - 예상 결과 및 실제 결과 비교
  - `e2e-scenarios.md` 작성

**산출물**:
- ✅ `test-results-baseline.txt`
- ✅ `cargo-check-baseline.txt`
- ✅ `golden-outputs/` (4개 JSON, 5개 스크린샷)
- ✅ `test-coverage-report.md`
- ✅ `e2e-scenarios.md`

---

### Day 2-3 (Tuesday-Wednesday) - 크리티컬 테스트 작성
**목표**: 리팩토링 대상 영역 테스트 완비

#### Day 2: 페이지 탐색 & 데이터 분석 테스트

##### 오전 (4시간)
- [ ] **9:00-10:30**: `page_discovery_regression.rs` 구조 설계
  - Mock HTTP client 인터페이스 설계
  - 테스트 케이스 4개 시나리오 작성

- [ ] **10:30-13:00**: 페이지 탐색 테스트 구현
  ```rust
  // test_downward_search_with_5_page_steps
  // test_consecutive_empty_pages_fatal_error
  // test_page_count_decrease_detection
  // test_page_cache_consistency
  ```

##### 오후 (4시간)
- [ ] **14:00-16:00**: Mock HTTP client 구현
  - 빈 페이지 응답 생성
  - 페이지네이션 HTML 응답 생성

- [ ] **16:00-18:00**: 테스트 실행 및 디버깅
  ```bash
  cargo test page_discovery_regression
  ```

**산출물**:
- ✅ `src-tauri/tests/page_discovery_regression.rs` (4 tests)
- ✅ Mock HTTP client 유틸리티

#### Day 3: 데이터 분석 & 동기화 테스트

##### 오전 (4시간)
- [ ] **9:00-11:00**: `data_analysis_regression.rs` 작성
  ```rust
  // test_analyze_data_changes_with_decrease
  // test_analyze_data_changes_within_tolerance
  // test_analyze_site_data_changes_increased
  // test_analyze_site_data_changes_stable
  ```

- [ ] **11:00-13:00**: Mock 설정 구현 및 테스트

##### 오후 (4시간)
- [ ] **14:00-17:00**: `sync_operations_regression.rs` 작성
  ```rust
  // test_shallow_sync_coordinates_only
  // test_smart_sync_full_pipeline
  // test_complement_crawl_missing_fields
  ```

- [ ] **17:00-18:00**: 통합 테스트 실행
  ```bash
  cargo test --tests
  ```

**산출물**:
- ✅ `src-tauri/tests/data_analysis_regression.rs` (4 tests)
- ✅ `src-tauri/tests/sync_operations_regression.rs` (3 tests)

---

### Day 4-5 (Thursday-Friday) - Golden Test 구현
**목표**: 정상 동작 응답 비교 테스트

#### Day 4: Golden Test 인프라

##### 오전 (3시간)
- [ ] **9:00-10:00**: Golden 디렉토리 구조 생성
  ```bash
  mkdir -p src-tauri/tests/golden
  touch src-tauri/tests/golden/site_status_response.json
  touch src-tauri/tests/golden/diagnostics_result.json
  touch src-tauri/tests/golden/crawling_plan.json
  touch src-tauri/tests/golden/sync_result.json
  ```

- [ ] **10:00-12:00**: 정상 응답 수집 및 저장
  - 실제 API 호출 → JSON 저장
  - 민감 정보 마스킹

##### 오후 (4시간)
- [ ] **14:00-16:00**: `Cargo.toml` 의존성 추가
  ```toml
  [dev-dependencies]
  assert_json_diff = "2.0"
  insta = "1.40"
  ```

- [ ] **16:00-18:00**: Golden test 헬퍼 함수 작성
  ```rust
  fn load_golden(filename: &str) -> Value { ... }
  fn assert_json_eq!(actual, expected) { ... }
  ```

**산출물**:
- ✅ `src-tauri/tests/golden/*.json` (4 files)
- ✅ Golden test 헬퍼 함수

#### Day 5: Golden Test 구현 및 검증

##### 오전 (3시간)
- [ ] **9:00-12:00**: `golden_tests.rs` 작성
  ```rust
  // test_site_status_golden
  // test_diagnostics_golden
  // test_crawling_plan_golden
  // test_sync_result_golden
  ```

##### 오후 (4시간)
- [ ] **14:00-16:00**: 전체 테스트 실행 및 디버깅
  ```bash
  cargo test --tests
  # 예상: 15개 테스트 (기존 + 신규 11개)
  ```

- [ ] **16:00-18:00**: Week 1 회고 및 정리
  - 테스트 커버리지 확인
  - 누락 영역 식별
  - Week 2 계획 검토

**산출물**:
- ✅ `src-tauri/tests/golden_tests.rs` (4 tests)
- ✅ **전체 테스트 15개 통과**
- ✅ Week 1 회고 문서

---

## Week 2: 코드 정리

### Day 6-7 (Monday-Tuesday) - 긴급 정리

#### Day 6: 아카이브 디렉토리 삭제

##### 오전 (2시간)
- [ ] **9:00-10:00**: Git 백업 태그 생성
  ```bash
  git add .
  git commit -m "chore: before archive cleanup - Week 1 tests complete"
  git tag v-before-archive-cleanup
  git push origin v-before-archive-cleanup
  ```

- [ ] **10:00-11:00**: 아카이브 디렉토리 삭제
  ```bash
  rm -rf archive/
  rm -rf src/_archive/
  git add .
  git commit -m "chore: remove archived components"
  ```

##### 오후 (3시간)
- [ ] **14:00-15:30**: API/서비스 제거 (Gemini 제안)
  ```bash
  rm -f archive/api/dashboard.ts
  rm -f archive/services/dashboardAPI.ts
  # 백엔드 커맨드 확인
  rg "dashboard\|analysis" src-tauri/src/commands/
  ```

- [ ] **15:30-17:00**: 컴파일 확인 및 수정
  ```bash
  npm run type-check
  cd src-tauri && cargo check
  ```

**산출물**:
- ✅ `archive/` 삭제
- ✅ `src/_archive/` 삭제
- ✅ 컴파일 통과

#### Day 7: Unused Exports 정리

##### 오전 (3시간)
- [ ] **9:00-10:00**: 최신 리포트 생성
  ```bash
  npm run analyze:unused-exports > reports/unused_exports_$(date +%Y%m%d).txt
  ```

- [ ] **10:00-12:00**: Unused exports 제거
  ```typescript
  // src/platform/tauri.ts
  // src/stores/tabStore.ts
  ```

##### 오후 (3시간)
- [ ] **14:00-17:00**: Unreachable files 아카이브
  ```bash
  mkdir -p src/_unreachable_backup_20251009
  # VendorForm/Management 확인 후 이동
  mv src/AppTabBased.tsx src/_unreachable_backup_20251009/
  # ... (리스트)
  ```

**산출물**:
- ✅ Unused exports 0건
- ✅ Unreachable files 아카이브

---

### Day 8-10 (Wednesday-Friday) - 백엔드 중복 제거

#### Day 8: analyze_data_changes 통합

##### 전체 (8시간)
- [ ] **9:00-11:00**: 사용처 파악
  ```bash
  rg "analyze_data_changes\(" src-tauri/
  rg "analyze_site_data_changes\(" src-tauri/
  ```

- [ ] **11:00-13:00**: 새 API 설계
  ```rust
  async fn analyze_data_changes(
      &self, 
      current: u32,
      with_recommendations: bool
  ) -> Result<DataAnalysisResult>
  ```

- [ ] **14:00-16:00**: 구현 및 마이그레이션
- [ ] **16:00-18:00**: 테스트 실행 및 디버깅

**산출물**:
- ✅ 통합 API 구현
- ✅ 중복 메서드 제거

#### Day 9: DatabaseAnalyzer Mock 분리

##### 전체 (8시간)
- [ ] **9:00-10:00**: Mock 디렉토리 생성
  ```bash
  mkdir -p src-tauri/tests/mocks
  touch src-tauri/tests/mocks/mod.rs
  touch src-tauri/tests/mocks/database.rs
  ```

- [ ] **10:00-13:00**: Mock 코드 이동
- [ ] **14:00-17:00**: Import 경로 수정 및 테스트
- [ ] **17:00-18:00**: 프로덕션 코드 정리

**산출물**:
- ✅ Mock 분리 완료
- ✅ 프로덕션 코드 정리

#### Day 10: CrawlingPlanner 통합 & 미사용 기능 제거

##### 오전 (4시간)
- [ ] **9:00-13:00**: CrawlingPlanner 통합
  ```rust
  pub async fn analyze_system_state(
      &self, 
      app_state: &AppState,
      force_refresh: bool
  ) -> Result<SystemAnalysis>
  ```

##### 오후 (4시간)
- [ ] **14:00-18:00**: 미사용 백엔드 기능 제거
  ```bash
  rg "analytics_query" src-tauri/
  rg "DbDiagnosticsReport" src-tauri/
  rg "check_page_index_consistency" src-tauri/
  ```

**산출물**:
- ✅ CrawlingPlanner 통합
- ✅ 미사용 기능 제거

---

## Week 3: 프론트엔드 정리 & 문서화

### Day 11-12 (Monday-Tuesday) - 프론트엔드 타입 안전성

#### Day 11: DiagnosticsPanel 타입 정의

##### 전체 (8시간)
- [ ] **9:00-11:00**: `src/types/diagnostics.ts` 작성
  ```typescript
  export interface DiagnosticsResult { ... }
  export interface GroupSummary { ... }
  export interface PageGap { ... }
  export interface DuplicatePosition { ... }
  ```

- [ ] **11:00-13:00**: DiagnosticsPanel 적용
- [ ] **14:00-16:00**: CrawlingEngineTabSimple 적용
- [ ] **16:00-18:00**: `any` 제거 및 테스트

**산출물**:
- ✅ `src/types/diagnostics.ts`
- ✅ `any` 사용 50% 감소

#### Day 12: 백업 API 타입 & Deprecated 제거

##### 오전 (4시간)
- [ ] **9:00-13:00**: `src/types/backup.ts` 작성 및 적용
  ```typescript
  export interface BackupResult { ... }
  export interface RestoreResult { ... }
  ```

##### 오후 (4시간)
- [ ] **14:00-18:00**: Deprecated API 제거
  ```bash
  rg "getCrawlingProgress" src/
  # 마이그레이션 or 제거
  ```

**산출물**:
- ✅ `src/types/backup.ts`
- ✅ Deprecated API 0건

---

### Day 13-14 (Wednesday-Thursday) - 문서 정리

#### Day 13: UI 가이드 & 레거시 키워드

##### 오전 (4시간)
- [ ] **9:00-13:00**: `SolidJS-UI-Implementation-Guide.md` 업데이트
  - 분석 탭 섹션 "Deprecated" 표시
  - 현재 3개 탭 위주 재구성

##### 오후 (4시간)
- [ ] **14:00-18:00**: 레거시 키워드 검색 및 정리
  ```bash
  rg -i "dashboard|analysis tab|actor system" docs/ guide/
  # 발견된 문서 업데이트
  ```

**산출물**:
- ✅ UI 가이드 업데이트
- ✅ 문서 레거시 키워드 정리

#### Day 14: 스크립트 & 상태 관리 정리

##### 오전 (4시간)
- [ ] **9:00-13:00**: 레거시 스크립트 정리
  ```bash
  diff dev.sh src-tauri/dev.sh
  ls -la scripts/*.{sh,mjs,ts}
  # 미사용 스크립트 아카이브
  ```

##### 오후 (4시간)
- [ ] **14:00-18:00**: 레거시 상태 관리 제거
  ```bash
  rg -i "dashboard|analysis" src/stores/
  rg -i "DashboardData|AnalyticsReport" src/types/
  ```

**산출물**:
- ✅ 스크립트 정리
- ✅ 레거시 타입 제거

---

### Day 15 (Friday) - CI/CD 통합

##### 오전 (4시간)
- [ ] **9:00-11:00**: `.github/workflows/test.yml` 작성
  - Backend test job
  - Frontend test job
  - Coverage upload

- [ ] **11:00-13:00**: GitHub Actions 테스트
  - PR 생성 테스트
  - 워크플로우 실행 확인

##### 오후 (4시간)
- [ ] **14:00-16:00**: Pre-commit hook 설정
  ```bash
  npm install --save-dev husky
  npx husky install
  npx husky add .husky/pre-commit "npm run test:pre-commit"
  ```

- [ ] **16:00-18:00**: Week 3 회고
  - 완료 항목 확인
  - 남은 작업 정리
  - Week 4 계획 최종 검토

**산출물**:
- ✅ GitHub Actions 워크플로우
- ✅ Pre-commit hook

---

## Week 4: 검증 및 배포 준비

### Day 16-17 (Monday-Tuesday) - 아키텍처 개선 (선택적)

#### Day 16: 캐시 최적화
- [ ] **전체**: 페이지 탐색 캐시 LRU 도입 (선택)

#### Day 17: DB 분석 성능 개선
- [ ] **전체**: 배치 처리, 인덱스 최적화 (선택)

---

### Day 18-19 (Wednesday-Thursday) - 전체 회귀 테스트

#### Day 18: 자동화 테스트

##### 전체 (8시간)
- [ ] **9:00-11:00**: 모든 테스트 실행
  ```bash
  cargo test --lib --tests
  npm run type-check
  ```

- [ ] **11:00-13:00**: 커버리지 측정
  ```bash
  cargo tarpaulin --out Html
  ```

- [ ] **14:00-18:00**: 테스트 실패 디버깅 및 수정

**산출물**:
- ✅ 모든 테스트 통과
- ✅ 커버리지 리포트 70%+

#### Day 19: E2E 시나리오 재검증

##### 전체 (8시간)
- [ ] **9:00-18:00**: E2E 시나리오 수동 재실행
  - 사이트 상태 체크
  - 크롤링 범위 재계산
  - 부분 크롤링
  - DB 진단
  - 스마트 동기화

**산출물**:
- ✅ E2E 시나리오 100% 통과

---

### Day 20 (Friday) - 배포 준비 & 문서 최종화

##### 오전 (4시간)
- [ ] **9:00-11:00**: 메트릭 확인
  - 코드 감소량 계산
  - 빌드 시간 비교
  - 성능 벤치마크

- [ ] **11:00-13:00**: 배포 노트 작성
  - 변경 사항 요약
  - 마이그레이션 가이드
  - 알려진 이슈

##### 오후 (4시간)
- [ ] **14:00-16:00**: 아키텍처 다이어그램 업데이트
- [ ] **16:00-17:00**: 리팩토링 히스토리 문서화
- [ ] **17:00-18:00**: 최종 리뷰 및 배포

**산출물**:
- ✅ 배포 노트
- ✅ 업데이트된 아키텍처 문서
- ✅ 리팩토링 완료! 🎉

---

## 일일 체크리스트 템플릿

```markdown
## Day X: <작업명>

### 시작 전 체크
- [ ] 최신 코드 pull
- [ ] 테스트 실행 (베이스라인)
- [ ] 작업 브랜치 생성

### 작업 중
- [ ] Task 1
- [ ] Task 2
- [ ] ...

### 종료 전 체크
- [ ] 테스트 실행 (변경 후)
- [ ] 컴파일 확인
- [ ] 커밋 및 푸시
- [ ] 진행 상황 기록

### 이슈 및 블로커
- 없음 / <이슈 설명>

### 다음 날 할 일
- [ ] ...
```

---

## 긴급 상황 대응

### 🚨 테스트 실패 시
1. **즉시 중단**: 추가 변경 금지
2. **로그 확인**: 실패 원인 파악
3. **롤백 결정**: 
   - 간단한 수정 → 즉시 수정
   - 복잡한 문제 → 커밋 revert
4. **재시도**: 수정 후 테스트 재실행

### 🚨 컴파일 실패 시
1. **에러 메시지 확인**
2. **최근 변경 검토**
3. **단계별 롤백**: 
   ```bash
   git stash
   cargo check # 통과 확인
   git stash pop # 재적용
   ```

### 🚨 성능 저하 발견 시
1. **벤치마크 측정**
2. **프로파일링 실행**
3. **원인 특정**
4. **최적화 or 롤백 결정**

---

## 성공 축하! 🎉

리팩토링 완료 후:
1. **배포 노트 공유**
2. **팀 리뷰 미팅**
3. **회고 세션**
4. **다음 개선 계획**

**축하합니다!** 깨끗하고 안전한 코드베이스를 만드셨습니다! 🚀
