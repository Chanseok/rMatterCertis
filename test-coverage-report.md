# 테스트 커버리지 베이스라인 리포트

> 작성일: 2025-10-09  
> Phase: Day 1 - 베이스라인 확보  
> 목적: 리팩토링 전 현재 테스트 상태 스냅샷

---

## 📊 테스트 실행 결과 요약

### 전체 통계
- **총 테스트 수**: 256개
- **통과**: 255개 (99.6%)
- **실패**: 1개 (0.4%)
- **무시됨**: 0개
- **실행 시간**: 약 3-6초

### 실패한 테스트
```
test infrastructure::crawling_range_calculation_test::tests::test_crawling_range_calculation_prompts6_example
```

**원인**:
- 크롤링 범위 계산 로직의 기존 버그
- Expected end page: 462, Actual: 372
- 리팩토링 작업과 무관 (기존 로직 문제)

**판단**:
- ✅ 베이스라인으로 유효
- 리팩토링 후에도 동일한 실패 유지 시 → regression 아님
- 리팩토링 후 실패 개수 증가 시 → regression 발생

---

## 🧪 테스트 카테고리 분석

### 1. Infrastructure Tests (인프라 계층)

#### 1.1 데이터베이스 (`integrated_product_repository`)
- ✅ `test_bulk_insert_products`
- ✅ `test_bulk_update_product_details`
- ✅ `test_database_migration`
- ✅ `test_migration_007_column_exists`
- ✅ `test_migration_009_legacy_column_absent`

**커버리지**: 기본 CRUD, 마이그레이션, 벌크 연산

#### 1.2 HTTP 클라이언트 (`simple_http_client`)
- ✅ `test_health_check`
- ✅ `test_retry_policy_429_then_ok`
- ✅ `test_cancellation_during_body`
- ✅ `test_rate_limiter_performance`

**커버리지**: 리트라이, 레이트 리밋, 취소, 성능

#### 1.3 크롤링 범위 계산 (`crawling_range_calculation_test`)
- ❌ `test_crawling_range_calculation_prompts6_example` (실패)
- ✅ 기타 크롤링 범위 계산 테스트

**커버리지 갭**: 특정 엣지 케이스 (471→372 범위)

### 2. Crawl Engine Tests (크롤링 엔진)

#### 2.1 Stages (`stages/strategies/default`)
- ✅ `list_page_logic_unsupported_item_errors`
- ✅ `product_detail_logic_rejects_wrong_item`
- ✅ `data_saving_logic_rejects_wrong_item`
- ✅ `product_detail_logic_happy_path_with_empty_urls`
- ✅ `data_validation_logic_happy_path_validates_in_memory_details`
- ✅ `data_saving_logic_happy_path_persists_in_memory_products`

**커버리지**: 에러 케이스, Happy path (인메모리)

#### 2.2 Actor System (`actors/`)
- ✅ `stage_actor` 테스트 (minimal ProductDetails)
- ✅ `types` 테스트 (TypeScript 타입 생성)

**커버리지 갭**: 
- 전체 Actor 통합 시나리오 부족
- SessionActor, BatchActor 간 통신 테스트 부족

### 3. TypeScript 타입 생성
- ✅ `test_typescript_type_generation`

**커버리지**: TS 타입 생성 파이프라인

---

## 🚨 커버리지 갭 (Phase 0에서 추가 필요)

### Critical Paths (리팩토링 대상 영역)

#### 1. 페이지 탐색 알고리즘
**현재 상태**: 유닛 테스트 없음
**필요한 테스트**:
- [x] ❌ `test_downward_search_with_5_page_steps` (신규)
- [x] ❌ `test_consecutive_empty_pages_fatal_error` (신규)
- [x] ❌ `test_page_count_decrease_detection` (신규)
- [x] ❌ `test_page_cache_consistency` (신규)

**위치**: `src-tauri/tests/page_discovery_regression.rs` (생성 필요)

#### 2. 데이터 분석 메서드
**현재 상태**: 통합 테스트만 존재 (인메모리 DB)
**필요한 테스트**:
- [x] ❌ `test_analyze_data_changes_with_decrease` (신규)
- [x] ❌ `test_analyze_data_changes_within_tolerance` (신규)
- [x] ❌ `test_analyze_site_data_changes_increased` (신규)
- [x] ❌ `test_analyze_site_data_changes_stable` (신규)

**위치**: `src-tauri/tests/data_analysis_regression.rs` (생성 필요)

#### 3. 동기화 작업
**현재 상태**: E2E 테스트만 존재 (네트워크 의존)
**필요한 테스트**:
- [x] ❌ `test_shallow_sync_coordinates_only` (신규)
- [x] ❌ `test_smart_sync_full_pipeline` (신규)
- [x] ❌ `test_complement_crawl_missing_fields` (신규)

**위치**: `src-tauri/tests/sync_operations_regression.rs` (생성 필요)

#### 4. 프론트엔드 UI 컴포넌트
**현재 상태**: 테스트 없음
**필요한 테스트**:
- [x] ❌ DiagnosticsPanel 렌더링 테스트
- [x] ❌ CrawlingEngineTabSimple 상태 관리 테스트
- [x] ❌ SettingsTab 폼 유효성 검사 테스트

**도구**: Vitest + Solid Testing Library

---

## 📋 테스트 목록 (알파벳 순)

### Infrastructure Layer
```
test infrastructure::crawling_range_calculation_test::tests::test_crawling_range_calculation_prompts6_example ... FAILED
test infrastructure::crawling_service_impls::tests::test_status_checker_analyze_data_changes
test infrastructure::crawling_service_impls::tests::test_status_checker_site_status_check
test infrastructure::database_connection::tests::test_database_migration
test infrastructure::database_connection::tests::test_migration_007_column_exists
test infrastructure::database_connection::tests::test_migration_009_legacy_column_absent
test infrastructure::integrated_product_repository::tests::test_bulk_insert_products
test infrastructure::integrated_product_repository::tests::test_bulk_update_product_details
test infrastructure::product_schema_adapter::tests::test_matter_product_to_product_conversion
test infrastructure::simple_http_client::tests::test_cancellation_during_body
test infrastructure::simple_http_client::tests::test_health_check
test infrastructure::simple_http_client::tests::test_rate_limiter_performance
test infrastructure::simple_http_client::tests::test_retry_policy_429_then_ok
```

### Crawl Engine Layer
```
test crawl_engine::stages::strategies::default::tests::data_saving_logic_happy_path_persists_in_memory_products
test crawl_engine::stages::strategies::default::tests::data_saving_logic_rejects_wrong_item
test crawl_engine::stages::strategies::default::tests::data_validation_logic_happy_path_validates_in_memory_details
test crawl_engine::stages::strategies::default::tests::list_page_logic_unsupported_item_errors
test crawl_engine::stages::strategies::default::tests::product_detail_logic_happy_path_with_empty_urls
test crawl_engine::stages::strategies::default::tests::product_detail_logic_rejects_wrong_item
test crawl_engine::ts_gen::tests::test_typescript_type_generation
```

### Integration Tests (tests/ 디렉토리)
```
test test_execution_plan_page_slots::tests::page_slots_basic_reverse_order
test test_execution_plan_page_slots::tests::page_slots_forward_order
test test_execution_plan_page_slots::tests::page_slots_mixed_orders
# ... (나머지 통합 테스트)
```

---

## 🎯 Phase 0 목표 (Week 1)

### Day 2-3: 크리티컬 테스트 작성
- [ ] `page_discovery_regression.rs` - 4개 테스트
- [ ] `data_analysis_regression.rs` - 4개 테스트
- [ ] `sync_operations_regression.rs` - 3개 테스트

**기대 결과**: 총 267개 테스트 (현재 256 + 11개)

### Day 4-5: Golden Test 구현
- [ ] `golden_tests.rs` - 4개 테스트
- [ ] Golden outputs 수집 (4개 JSON 파일)

**기대 결과**: 총 271개 테스트

### Week 1 종료 시
- ✅ 베이스라인 확보 (255/256 통과)
- ✅ 크리티컬 경로 테스트 추가 (11개)
- ✅ Golden Test 인프라 구축 (4개)
- ✅ 총 **271개 테스트** 달성
- ✅ 실패율 0.4% 유지 또는 개선

---

## 📝 주요 관찰 사항

### 1. 테스트 실행 속도
- **3-6초**: 매우 빠른 속도 (in-memory DB, 모킹 활용)
- **리팩토링 후 유지 목표**: 10초 이내

### 2. 테스트 격리
- ✅ 각 테스트가 독립적으로 실행 가능
- ✅ 공유 상태 없음 (in-memory DB per test)

### 3. Mock 전략
- ✅ HTTP Client Mock 구현 완료
- ✅ Database In-Memory 지원
- ⚠️ ProductDetailCollector Mock 부재

### 4. 네트워크 의존 테스트
- `#[ignore]` 처리된 E2E 테스트 존재
- 환경 변수 `MC_RUN_*_IT=1` 플래그로 활성화
- CI/CD에서 선택적 실행 가능

---

## 🔄 다음 단계

### 1. E2E 시나리오 수동 실행 (Day 1 오후)
```bash
# 시나리오 1: 사이트 상태 체크
npm run tauri dev
# UI에서 "Check Site Status" 클릭
# 596 → 589 감소 확인

# 시나리오 2: 크롤링 범위 재계산
# "Recalculate Range" 클릭

# 시나리오 3: 부분 크롤링 5페이지
# Start: 590, End: 586 입력 후 실행

# 시나리오 4: DB 진단
# "Run Diagnostics" 클릭

# 시나리오 5: 스마트 동기화
# "Smart Sync" 클릭
```

### 2. Golden Outputs 수집
```bash
mkdir -p golden-outputs
# 각 시나리오 응답 JSON 저장
# DevTools Network 탭에서 API 응답 복사
```

### 3. 크리티컬 테스트 작성 시작 (Day 2)
- Mock HTTP Client 설계
- 페이지 탐색 테스트 구현

---

## 📚 참고 자료

- `test-results-baseline.txt`: 전체 테스트 로그
- `cargo-check-baseline.txt`: 컴파일 경고/에러 (현재 경고 없음)
- `EXECUTION_PLAN.md`: Week 1-4 상세 계획
- `REFACTORING_MASTER_PLAN.md`: 통합 마스터 플랜

---

**다음 작업**: E2E 시나리오 수동 실행 및 Golden Outputs 수집
