# 리팩토링 전 테스트 전략

> 생성일: 2025-10-09
> 목적: 리팩토링 중 regression 방지를 위한 테스트 전략 수립

## 📊 현재 테스트 현황 분석

### 1. 기존 테스트 자산

#### ✅ 통합 테스트 (src-tauri/tests/)
```
✓ stage_vs_batch_parity.rs         - Stage와 Batch 동작 일치성
✓ test_execution_plan_page_slots.rs - 실행 계획 페이지 슬롯
✓ resume_token_tests.rs             - 재시작 토큰 기능
✓ priority1_verification_tests.rs   - 우선순위 1 검증
✓ preplanned_sequential_order.rs    - 사전 계획 순차 처리
✓ test_page_id_calculator.rs        - 페이지 ID 계산
✓ crawl_events_serialization_tests.rs - 크롤링 이벤트 직렬화
✓ system_analysis_happy_path.rs     - 시스템 분석 성공 경로
✓ status_downshift_tests.rs         - 상태 다운시프트
✓ db_analysis_next_start.rs         - DB 분석 다음 시작점
✓ test_http_client_config.rs        - HTTP 클라이언트 설정
✓ metrics_counters.rs               - 메트릭 카운터
✓ db_diagnostics_gating.rs          - DB 진단 게이팅
```

#### ✅ 유닛 테스트 (src-tauri/src/**/*.rs)
```
✓ crawl_engine.rs (#[cfg(test)])
✓ channels/types.rs 
✓ utils.rs
✓ actor_system.rs
✓ real_crawling_commands.rs
✓ ts_gen.rs
✓ real_crawling_integration.rs
✓ stages.rs
✓ planning_service.rs
✓ actor_event_bridge.rs
✓ actors/traits.rs
✓ actors/stage_batcher.rs
✓ config_commands.rs
✓ actors/types.rs
✓ actors/stage_actor.rs
✓ integrated_context.rs
✓ application/state.rs
✓ stages/strategies/default.rs
✓ parsing/product_list_parser.rs
✓ config/system_config.rs
```

#### ⚠️ 비활성화된 테스트 바이너리 (Cargo.toml)
```
❌ test_minimal.rs
❌ test_existing_database.rs
❌ test_with_utils.rs
❌ test_db_light.rs
❌ test_db.rs
❌ test_db_fast.rs
❌ test_session_management.rs
❌ test_crawler.rs
❌ test_core_functionality.rs
```

### 2. 테스트 커버리지 갭 분석

#### 🔴 커버리지 부족 영역

1. **페이지 탐색 알고리즘** (리팩토링 대상!)
   - `find_last_valid_page_with_safety_check` - ✅ 최근 변경됨
   - `discover_total_pages`
   - `verify_last_page`
   - `check_page_has_products`
   
2. **데이터 분석 메서드** (중복 제거 대상!)
   - `analyze_data_changes`
   - `analyze_site_data_changes` 
   - `analyze_current_state` (DatabaseAnalyzer)
   - `analyze_duplicates`

3. **동기화 기능**
   - 빠른 동기화
   - 스마트 동기화
   - 제품 보완 크롤링

4. **UI 통합**
   - DiagnosticsPanel 로직
   - ControlPanel 상태 관리
   - SessionStatusCard 경고 표시

---

## 🎯 리팩토링 전 테스트 전략

### Phase 0: 현재 상태 스냅샷 (1일)

#### 0.1 통합 테스트 실행 및 결과 기록
```bash
# 모든 테스트 실행
cd src-tauri
cargo test --lib --tests 2>&1 | tee ../test-results-baseline.txt

# 성공/실패 통계
cargo test --lib --tests 2>&1 | grep "test result"
```

**산출물**:
- [x] `test-results-baseline.txt` - 베이스라인 테스트 결과
- [ ] `test-coverage-report.md` - 현재 커버리지 리포트
- [ ] `critical-paths.md` - 크리티컬 패스 식별

#### 0.2 E2E 시나리오 실행 및 기록
```bash
# 수동 테스트 시나리오
1. 사이트 상태 체크 (596 페이지 → 589 페이지 감소 감지)
2. 크롤링 범위 재계산
3. 부분 크롤링 실행 (5페이지)
4. DB 레코드 체크 (진단 실행)
5. 스마트 동기화
```

**산출물**:
- [ ] `e2e-scenarios.md` - E2E 시나리오 및 예상 결과
- [ ] `golden-outputs/` - 정상 동작 시 스크린샷/로그

---

### Phase 1: 크리티컬 경로 테스트 추가 (2-3일)

#### 1.1 페이지 탐색 통합 테스트
**파일**: `src-tauri/tests/page_discovery_regression.rs`

```rust
#[tokio::test]
async fn test_page_discovery_with_safety_check() {
    // Given: 5페이지 단위 하향 탐색
    // When: 연속 12회 빈 페이지 발견
    // Then: Fatal error 발생
}

#[tokio::test]
async fn test_page_discovery_cache_consistency() {
    // Given: 캐시된 페이지 분석 결과
    // When: 동일 페이지 재요청
    // Then: 캐시 히트, 네트워크 요청 없음
}

#[tokio::test]
async fn test_page_count_decrease_detection() {
    // Given: 이전 max_page=596, 현재=589
    // When: check_site_status 호출
    // Then: is_page_count_decreased=true, ratio=1.2%
}
```

#### 1.2 데이터 분석 메서드 테스트
**파일**: `src-tauri/tests/data_analysis_regression.rs`

```rust
#[tokio::test]
async fn test_analyze_data_changes_decrease() {
    // Given: 이전 product_count=7128, 현재=7084
    // When: analyze_data_changes 호출
    // Then: SiteDataChangeStatus::Decreased + recommendation
}

#[tokio::test]
async fn test_analyze_site_data_changes_stable() {
    // Given: 0.5% 미만 변화
    // When: analyze_site_data_changes 호출  
    // Then: DataChangeAnalysis::Stable
}
```

#### 1.3 동기화 기능 테스트
**파일**: `src-tauri/tests/sync_operations_regression.rs`

```rust
#[tokio::test]
async fn test_shallow_sync_coordinates() {
    // Given: products with null page_id
    // When: shallow sync 실행
    // Then: 좌표 업데이트, details는 미변경
}

#[tokio::test]
async fn test_smart_sync_full_pipeline() {
    // Given: 부분 데이터
    // When: smart sync (shallow + diagnostics + complement)
    // Then: 전체 파이프라인 성공
}
```

---

### Phase 2: 리팩토링별 테스트 가드 (진행 중)

#### 2.1 중복 메서드 제거 시
**Before**:
```rust
// ✅ 이미 완료: find_last_valid_page_downward 제거
// Before: 두 메서드 존재
// After: find_last_valid_page_with_safety_check만 존재
```

**테스트**:
```rust
#[tokio::test]
async fn test_downward_search_unified() {
    // 통합된 메서드가 두 가지 케이스 모두 처리
    // 1. 5페이지 단위 점프
    // 2. 12회 연속 빈 페이지 시 fatal error
}
```

#### 2.2 analyze_data_changes 통합 시
**계획**:
```rust
// Before: analyze_data_changes + analyze_site_data_changes
// After: analyze_data_changes(with_recommendations: bool)
```

**테스트**:
```rust
#[tokio::test]
async fn test_unified_data_analysis() {
    // with_recommendations=false → DataChangeAnalysis
    // with_recommendations=true → (Status, Option<Recommendation>)
}
```

#### 2.3 DatabaseAnalyzer Mock 분리 시
**계획**:
```rust
// Before: MockDatabaseAnalyzer in crawling_service_impls.rs
// After: tests/mocks/database.rs
```

**테스트**:
```rust
// 프로덕션 코드에서 Mock import 제거 확인
#[test]
fn test_no_mock_in_production() {
    // 컴파일 타임 체크로 충분
}
```

---

### Phase 3: 회귀 테스트 자동화 (1-2일)

#### 3.1 Golden Test 패턴
**파일**: `src-tauri/tests/golden/`

```
golden/
├── site_status_response.json          # 사이트 상태 응답
├── diagnostics_result.json            # 진단 결과
├── crawling_plan.json                 # 크롤링 계획
└── sync_result.json                   # 동기화 결과
```

```rust
#[tokio::test]
async fn test_site_status_golden() {
    let result = check_site_status().await.unwrap();
    let golden = load_golden("site_status_response.json");
    assert_json_eq!(result, golden);
}
```

#### 3.2 Property-Based Testing
**파일**: `src-tauri/tests/property_based.rs`

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_page_calculation_invariants(
        total_pages in 1u32..1000,
        products_per_page in 1u32..20
    ) {
        // Invariant: total_products = (total_pages-1) * ppp + last_page_products
        let result = calculate_products(total_pages, products_per_page);
        prop_assert!(result.total >= total_pages * products_per_page - products_per_page);
    }
}
```

---

### Phase 4: CI/CD 통합 (1일)

#### 4.1 GitHub Actions 워크플로우
**파일**: `.github/workflows/test.yml`

```yaml
name: Regression Tests

on:
  pull_request:
    branches: [main, develop]
  push:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Run unit tests
        run: cd src-tauri && cargo test --lib
        
      - name: Run integration tests
        run: cd src-tauri && cargo test --tests
        
      - name: Check coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --output-dir coverage/
          
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

#### 4.2 Pre-commit Hook
**파일**: `.git/hooks/pre-commit`

```bash
#!/bin/bash
# 리팩토링 전 테스트 실행
cd src-tauri
echo "Running tests before commit..."
cargo test --lib --tests --quiet

if [ $? -ne 0 ]; then
  echo "❌ Tests failed. Commit aborted."
  exit 1
fi

echo "✅ All tests passed."
```

---

## 🚦 리팩토링 실행 가이드

### 1. 리팩토링 전 체크리스트

- [ ] 베이스라인 테스트 모두 통과
- [ ] 크리티컬 경로 테스트 추가 완료
- [ ] E2E 시나리오 수동 검증 완료
- [ ] 현재 동작 스크린샷/로그 저장

### 2. 리팩토링 중 룰

1. **작은 단위로 진행**: 한 번에 하나의 중복 제거
2. **테스트 먼저**: 변경 전 해당 영역 테스트 추가
3. **즉시 검증**: 변경 후 바로 `cargo test` 실행
4. **롤백 준비**: Git stash/branch 활용

### 3. 리팩토링 후 체크리스트

- [ ] 모든 기존 테스트 통과
- [ ] 새로 추가한 테스트 통과
- [ ] E2E 시나리오 재검증
- [ ] UI에서 수동 테스트
- [ ] 성능 저하 없음 확인

---

## 📈 우선순위별 실행 계획

### 🔴 즉시 실행 (리팩토링 전 필수)

**Week 1: 테스트 인프라 구축**
- [ ] Day 1: 베이스라인 테스트 실행 및 결과 기록
- [ ] Day 2: 크리티컬 경로 테스트 추가 (페이지 탐색)
- [ ] Day 3: 데이터 분석 테스트 추가
- [ ] Day 4: 동기화 기능 테스트 추가
- [ ] Day 5: Golden test 패턴 도입

### 🟡 병행 실행 (리팩토링 중)

**Week 2: 리팩토링 + 테스트 가드**
- [ ] 각 리팩토링마다 테스트 먼저 작성
- [ ] 변경 후 즉시 테스트 실행
- [ ] 실패 시 즉시 롤백

### 🟢 장기 개선 (리팩토링 후)

**Week 3+: 지속적 개선**
- [ ] Property-based testing 도입
- [ ] Fuzzing 테스트 추가
- [ ] CI/CD 통합
- [ ] 커버리지 80% 목표

---

## 🎯 성공 기준

### 정량적 지표
- [x] 기존 테스트 모두 통과 (현재: ?)
- [ ] 새로운 테스트 20개 이상 추가
- [ ] 코드 커버리지 70% 이상
- [ ] 리팩토링 후 테스트 실패 0건

### 정성적 지표
- [ ] 리팩토링 중 regression 0건
- [ ] UI 동작 100% 동일
- [ ] 성능 저하 없음
- [ ] 팀원 리뷰 통과

---

## 📚 참고 자료

- [Rust Testing Best Practices](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Property-Based Testing in Rust](https://github.com/proptest-rs/proptest)
- [Golden Testing Pattern](https://ro-che.info/articles/2017-12-04-golden-tests)
- [Test Coverage with Tarpaulin](https://github.com/xd009642/tarpaulin)

---

## 🔧 도구 및 라이브러리

### 현재 사용 중
- `#[tokio::test]` - 비동기 테스트
- `#[cfg(test)]` - 유닛 테스트
- Integration tests in `tests/`

### 추가 권장
```toml
[dev-dependencies]
proptest = "1.5"              # Property-based testing
assert_json_diff = "2.0"      # JSON 비교
insta = "1.40"                # Snapshot testing
mockall = "0.13"              # Mock 생성
criterion = "0.5"             # 벤치마킹
```

---

## 💡 결론

**✅ 현재 시점은 테스트 보완하기에 최적의 타이밍입니다!**

**이유**:
1. **안정된 기능**: 현재 동작하는 기능들이 명확함
2. **리팩토링 예정**: 중복 제거, 아키텍처 개선 계획됨
3. **테스트 자산**: 기존 테스트들이 일부 존재
4. **회귀 위험**: 리팩토링 시 기능 손상 가능성 높음

**권장 접근**:
1. 🔴 **1주차**: 크리티컬 패스 테스트 추가 (필수)
2. 🟡 **2주차**: 리팩토링 시작 + 테스트 가드
3. 🟢 **3주차+**: 지속적 개선

**즉시 시작**:
```bash
# 1. 베이스라인 저장
cd src-tauri && cargo test --lib --tests > ../test-baseline.txt

# 2. 크리티컬 테스트 작성
touch tests/page_discovery_regression.rs
touch tests/data_analysis_regression.rs
touch tests/sync_operations_regression.rs
```
