# 크롤링 시스템 수정 계획 (2025-07-13)

## 🚨 우선순위별 수정 계획

### Phase 1: 긴급 수정 (크리티컬 버그)

#### 1.1 크롤링 범위 계산 로직 수정 🔥
**문제**: Repository 인스턴스 누락으로 DB 상태 오진단

**원인 파일**: 
- `src-tauri/src/services/site_status_checker.rs`
- `src-tauri/src/services/range_calculator.rs`

**수정 방법**:
```rust
// 현재 문제 코드
WARN ⚠️  Product repository not available - assuming empty DB

// 수정 후
1. Repository 인스턴스를 range calculation 함수에 정상 전달
2. DB 상태 조회 로직 안정화
3. pageId, indexInPage 기반 정확한 다음 페이지 계산
```

**예상 결과**:
```
116개 제품 → pageId: 9, indexInPage: 7
다음 시작점: 9페이지 8번째 제품 → 10페이지부터 크롤링
범위: 10페이지 ~ 설정된 최대 범위 (482페이지가 아님)
```

#### 1.2 설정 기반 동시성 제어 구현 🔥
**문제**: 하드코딩된 세마포어 `max: 3`

**수정 파일**: 
- `src-tauri/src/services/batch_crawling_service.rs`

**수정 방법**:
```rust
// 현재
let semaphore = Arc::new(Semaphore::new(3)); // 하드코딩

// 수정 후
let concurrent_limit = config.crawling.concurrent_requests.unwrap_or(3);
let semaphore = Arc::new(Semaphore::new(concurrent_limit));
```

### Phase 2: 중요 개선 (아키텍처 정합성)

#### 2.1 캐시 활용 최적화 ⚡
**문제**: 사이트 분석 3회 중복 수행

**수정 전략**:
1. **첫 번째 사이트 분석**: StatusTab에서 수행 → 캐시 저장
2. **크롤링 시작**: 캐시된 결과 활용, 재분석 스킵
3. **Stage 0**: 캐시 유효성 확인 후 재사용

**수정 파일**:
- `src-tauri/src/commands/crawling_v4.rs`
- `src-tauri/src/services/shared_state_cache.rs`

#### 2.2 이벤트 기반 UI 상태 동기화 📡
**문제**: 크롤링 취소 후 UI가 "크롤링 중" 상태 유지

**수정 방법**:
```rust
// 취소 완료 후 이벤트 발송
app_handle.emit_all("crawling_stopped", CrawlingStoppedEvent {
    session_id,
    reason: "user_cancelled",
    timestamp: Utc::now(),
})?;
```

**프론트엔드 리스너 추가**:
```typescript
// StatusTab.tsx에서
useEffect(() => {
  const unlisten = listen('crawling_stopped', (event) => {
    setCrawlerState('status', 'Idle');
    setCrawlerState('isRunning', false);
  });
  return () => unlisten.then(f => f());
}, []);
```

### Phase 3: 최적화 및 안정성 (사용자 경험)

#### 3.1 지능형 범위 계산 알고리즘 개선 🧠
**목표**: 설정과 DB 상태를 모두 고려한 최적 범위 계산

**로직**:
```rust
fn calculate_optimal_range(
    db_cursor: (u32, u32), // (pageId, indexInPage)
    site_info: SiteInfo,   // (total_pages, last_page_products)
    config: &CrawlingConfig
) -> CrawlingRange {
    let next_page = db_cursor.0 + 1; // 다음 크롤링 시작 페이지
    let max_pages = config.max_pages_per_session.unwrap_or(10);
    let end_page = (next_page + max_pages).min(site_info.total_pages);
    
    CrawlingRange {
        start_page: next_page,
        end_page,
        estimated_products: calculate_products_in_range(next_page, end_page, &site_info)
    }
}
```

#### 3.2 실시간 진행 상황 이벤트 🔄
**구현 항목**:
- 페이지별 크롤링 완료 이벤트
- 전체 진행률 계산 및 전송
- 예상 완료 시간 계산
- 메모리 사용량 모니터링

## 🛠 구체적 수정 단계

### Step 1: Repository 의존성 해결 (30분)
1. `range_calculator.rs`에서 Repository 매개변수 추가
2. DB 연결 상태 검증 로직 강화
3. 오류 처리 개선

### Step 2: 설정 기반 동시성 (15분)
1. `BatchCrawlingConfig`에서 concurrent_requests 읽기
2. 세마포어 초기화 시 설정값 사용
3. 로그에 사용된 동시성 값 출력

### Step 3: 캐시 활용 로직 (45분)
1. SharedStateCache TTL 검증 로직 강화
2. 캐시 적중/실패 로그 추가
3. 불필요한 사이트 분석 스킵 구현

### Step 4: UI 이벤트 동기화 (30분)
1. 크롤링 상태 변경 이벤트 정의
2. 백엔드에서 상태 변경 시 이벤트 발송
3. 프론트엔드 리스너 구현

## 📊 예상 효과

### 성능 개선
- **크롤링 시간**: 100페이지 → 실제 필요 페이지 (10-20페이지)로 80-90% 단축
- **네트워크 요청**: 사이트 분석 3회 → 1회로 66% 감소
- **동시성**: 3 → 24로 800% 향상

### 정확성 개선
- **범위 계산**: 하드코딩 → DB 상태 기반 지능형 계산
- **UI 동기화**: 취소 후 상태 불일치 해결
- **설정 준수**: 모든 동작이 설정 파일 기반

### 사용자 경험
- **즉시 피드백**: 실시간 진행 상황 표시
- **정확한 예측**: 실제 필요 작업량 기반 시간 예상
- **안정적 취소**: UI와 백엔드 상태 완벽 동기화

## 🎯 검증 방법

### 테스트 시나리오
1. **정상 크롤링**: 10페이지부터 시작하는지 확인
2. **설정 적용**: concurrent_requests=24가 실제 적용되는지 확인
3. **캐시 활용**: 두 번째 분석이 스킵되는지 확인
4. **취소 기능**: UI가 즉시 "대기" 상태로 변경되는지 확인

### 로그 확인 포인트
```log
✅ 예상 로그
📊 DB cursor at 9:7, next crawling from page 10
🚀 Using concurrent_requests=24 from config
🎯 Using cached site analysis (age: 150s, valid)
🛑 Crawling stopped, UI state updated
```

이 계획을 단계별로 실행하면 현재 문제들이 체계적으로 해결될 것입니다.
