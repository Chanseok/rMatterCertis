# 제안 6: 설정 기반 동작 및 구조적 개선 방안

**문서 목적:** 로그 분석 결과 발견된 핵심 문제들 - 하드코딩된 값들, 설정 무시, 중복 작업 등을 해결하기 위한 구조적 개선 방안을 제시합니다.

---

## 🔍 발견된 핵심 문제들 (로그 분석 기반)

### 1. 하드코딩된 값들이 설정을 무시
```log
INFO 🚀 Creating 100 concurrent tasks with semaphore control (max: 3)
```
- **문제**: 페이지 범위 100개, 동시성 3개가 하드코딩
- **설정**: `page_range_limit: 100`, `max_concurrent_requests: 3` in config
- **실제**: 사용자가 설정을 24로 변경해도 하드코딩된 3 사용

### 2. 중복된 사이트 분석 수행
```log
19:01:00.509  INFO Site status check completed: 482 pages found, 1931ms total time
19:01:03.509  INFO Site status check completed: 482 pages found, 2997ms total time
```
- **문제**: 동일 세션에서 사이트 분석을 2번 수행 (총 4.9초 소요)
- **원인**: SharedStateCache TTL 미활용

### 3. DB 상태 불일치
```log
WARN ⚠️  Product repository not available - assuming empty DB
INFO Stage 1 completed: 116 total products, quality score: 1
```
- **문제**: 같은 세션에서 DB가 "비어있음"과 "116개 제품"으로 모순
- **원인**: 다른 접근 방식 간 불일치

### 4. 설정값 기반 범위 계산 실패
```log
INFO 🆕 No existing data, starting from page 482 to 383
```
- **문제**: 로컬 DB 116개 제품 무시하고 "빈 DB" 가정
- **기대**: pageId=9, indexInPage=7 기준으로 정확한 범위 계산

---

## 🎯 구조적 개선 방안

### 1. 설정 중심 아키텍처 (Configuration-Driven Architecture)

```rust
// 현재 문제: 하드코딩
let semaphore = Arc::new(Semaphore::new(3)); // 하드코딩!

// 개선 방안: 설정 기반
let config = app_config.user.crawling.workers;
let semaphore = Arc::new(Semaphore::new(config.list_page_max_concurrent as usize));
let page_limit = config.page_range_limit;
```

#### 설정 우선순위 체계
1. **사용자 설정**: `matter_certis_config.json`의 user 섹션
2. **지능형 계산**: 사이트/DB 분석 기반 추천값
3. **기본값**: defaults 모듈의 fallback 값

### 2. SharedStateCache 활용 강화

```rust
#[derive(Debug, Clone)]
pub struct SharedStateCache {
    // TTL 기반 캐싱으로 중복 작업 방지
    site_analysis: Option<CachedSiteAnalysis>,
    db_analysis: Option<CachedDbAnalysis>,
    calculated_range: Option<CachedRange>,
    
    // 설정 기반 동작 보장
    effective_config: EffectiveConfig,
}

#[derive(Debug, Clone)]
pub struct CachedSiteAnalysis {
    pub result: SiteStatus,
    pub cached_at: Instant,
    pub ttl: Duration, // 5-10분
}

#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub max_concurrent: u32,        // 설정에서 가져온 실제 값
    pub page_range_limit: u32,      // 설정에서 가져온 실제 값
    pub batch_size: u32,            // 설정에서 가져온 실제 값
    pub request_delay_ms: u64,      // 설정에서 가져온 실제 값
}
```

### 3. 정확한 pageId/indexInPage 기반 범위 계산

```rust
#[derive(Debug, Clone)]
pub struct DbCursorPosition {
    pub max_page_id: i32,      // 0-based
    pub max_index_in_page: i32, // 0-based
    pub total_products: u32,
}

impl DbCursorPosition {
    /// 로컬 DB 상태 기반 다음 크롤링 시작점 계산
    pub fn calculate_next_range(&self, site_total_pages: u32, config: &CrawlingConfig) -> CrawlingRange {
        let products_per_page = 12; // 사이트 기본 값
        
        // 현재 저장된 마지막 제품의 절대 인덱스 (0-based)
        let last_saved_absolute_index = (self.max_page_id * products_per_page) + self.max_index_in_page;
        
        // 다음 크롤링할 제품의 절대 인덱스
        let next_absolute_index = last_saved_absolute_index + 1;
        
        // 웹사이트 페이지 번호로 변환 (1-based)
        let next_page = (next_absolute_index / products_per_page) + 1;
        
        // 설정된 페이지 범위 제한 적용
        let end_page = next_page + config.page_range_limit.min(config.max_pages);
        let end_page = end_page.min(site_total_pages);
        
        CrawlingRange {
            start_page: next_page as u32,
            end_page: end_page as u32,
            total_pages: (end_page - next_page + 1) as u32,
            is_complete_crawl: false,
        }
    }
}
```

### 4. 설정 값 검증 및 적용 시스템

```rust
#[derive(Debug, Clone)]
pub struct ValidatedCrawlingConfig {
    pub max_concurrent: u32,        // 검증된 설정값
    pub page_range_limit: u32,      // 검증된 설정값  
    pub batch_size: u32,            // 검증된 설정값
    pub semaphore_permits: usize,   // max_concurrent와 동일
}

impl ValidatedCrawlingConfig {
    pub fn from_user_config(config: &AppConfig) -> Self {
        let user_config = &config.user.crawling;
        
        // 설정값 검증 및 범위 제한
        let max_concurrent = user_config.workers.list_page_max_concurrent
            .max(1)  // 최소 1
            .min(50); // 최대 50
            
        let page_range_limit = user_config.page_range_limit
            .max(1)   // 최소 1페이지
            .min(500); // 최대 500페이지
            
        Self {
            max_concurrent,
            page_range_limit,
            batch_size: user_config.workers.db_batch_size,
            semaphore_permits: max_concurrent as usize,
        }
    }
}
```

---

## 🔧 구현 우선순위

### Phase 1: 하드코딩 제거 (최우선)
1. **설정 기반 동시성 제어**
   ```rust
   // 현재: Semaphore::new(3)
   // 개선: Semaphore::new(validated_config.semaphore_permits)
   ```

2. **설정 기반 페이지 범위**
   ```rust
   // 현재: range 482 to 383 (하드코딩된 100페이지)
   // 개선: 사용자 설정 page_range_limit 사용
   ```

### Phase 2: SharedStateCache TTL 활용
1. **사이트 분석 결과 재사용**
   - 5-10분 TTL로 중복 분석 방지
   - 세션 내 일관성 보장

2. **DB 분석 결과 캐싱**
   - 정확한 pageId/indexInPage 유지
   - Repository 불일치 해결

### Phase 3: 정확한 범위 계산
1. **pageId/indexInPage 기반 로직**
   - 0-based 인덱스 정확히 계산
   - 사이트 페이지 번호 (1-based)로 정확한 변환

2. **설정값 우선순위 적용**
   - 사용자 설정 → 지능형 계산 → 기본값 순서

---

## 🎮 사용자 경험 개선

### 설정 유효성 즉시 반영
```rust
// 사용자가 동시성을 24로 변경하면
// 즉시 새로운 크롤링에서 24개 동시 작업 수행
let concurrent_tasks = validated_config.max_concurrent; // 24

// 사용자가 페이지 범위를 20으로 설정하면
// 다음 크롤링에서 정확히 20페이지만 처리
let page_limit = validated_config.page_range_limit; // 20
```

### 일관된 상태 표시
```rust
// 동일 세션에서 일관된 정보 제공
if let Some(cached_site) = shared_cache.get_site_analysis() {
    // 캐시된 결과 사용 (중복 분석 방지)
    use_cached_result(cached_site);
} else {
    // 새로운 분석 수행 후 캐시에 저장
    let new_analysis = perform_site_analysis().await;
    shared_cache.store_site_analysis(new_analysis, Duration::from_secs(600));
}
```

---

## 📋 검증 방법

### 1. 설정값 적용 검증
- 설정 변경 후 로그에서 실제 적용된 값 확인
- `max_concurrent_requests: 24` → `Creating 24 concurrent tasks`

### 2. 중복 작업 제거 검증  
- 동일 세션에서 사이트 분석 1회만 수행
- 캐시 적중률 로그 모니터링

### 3. 정확한 범위 계산 검증
- pageId=9, indexInPage=7 → 정확한 시작 페이지 계산
- 로컬 DB 상태와 크롤링 범위 일치 확인

이 구조적 개선을 통해 설정 기반으로 동작하는 안정적이고 예측 가능한 크롤링 시스템을 구축할 수 있습니다.

## 3. 워크플로우 및 역할 재정의

### 3.1. `StatusTab`의 역할: 분석 및 캐시 업데이트

-   **사용자 액션:** 사용자가 `StatusTab`에서 "사이트 종합 분석" 버튼을 클릭합니다.
-   **프론트엔드:** `invoke('analyze_system_status')` 커맨드를 호출합니다.
-   **백엔드 (`analyze_system_status` 커맨드):**
    1.  사이트 분석과 DB 분석을 수행합니다.
    2.  결과를 `SiteAnalysisResult`와 `DbAnalysisResult` 구조체에 담습니다.
    3.  **`AppStateCache`를 잠그고(lock), 분석 결과를 캐시에 업데이트합니다.**
    4.  분석 결과를 UI에 전송하여 화면에 표시합니다.
-   **결과:** 백엔드는 이제 `total_pages`가 몇 개인지, DB 커서가 어디에 있는지 **기억하게 됩니다.**

### 3.2. `크롤링 시작`의 역할: 캐시 활용 및 실행

-   **사용자 액션:** 사용자가 `크롤링 시작` 버튼을 클릭합니다.
-   **프론트엔드:** `invoke('start_crawling')` 커맨드를 호출합니다. **더 이상 `start_page`, `end_page` 같은 구체적인 실행 계획을 전달하지 않습니다.**
-   **백엔드 (`start_crawling` 커맨드):**
    1.  **`AppStateCache`를 잠그고(lock), 캐시된 분석 결과를 읽어옵니다.**
    2.  만약 캐시가 비어있다면(사용자가 분석을 건너뛰었다면), 내부적으로 `analyze_system_status` 로직을 호출하여 분석을 수행하고 결과를 캐싱합니다.
    3.  **캐시된 `SiteAnalysisResult`와 `DbAnalysisResult`를 바탕으로, 최적의 크롤링 범위(`start_page`, `end_page`)를 계산합니다.** (`proposal3.md`의 알고리즘 사용)
    4.  계산된 범위로 크롤링 엔진(`Orchestrator`)을 시작합니다.
-   **결과:** 백엔드가 스스로의 기억(캐시)을 바탕으로 판단하고 실행하는, 진정한 의미의 역할 분리가 완성됩니다.

## 4. 구체적인 구현 제안

### 4.1. `start_crawling` 커맨드 시그니처 변경

-   **기존:** `start_crawling(start_page: u32, end_page: u32)`
-   **변경:** `start_crawling(operating_profile: OperatingProfile)`
    -   UI는 이제 어떻게(How)가 아닌, 무엇(What)과 정책(Policy)만 전달합니다. "균형 모드로 크롤링 시작해줘" 라고만 요청하면, 백엔드가 알아서 범위를 계산합니다.

### 4.2. `Orchestrator` 및 `Worker` 수정

-   **동시성 문제 해결:** `proposal6.md`에서 제안된 "Spawn All, Control with Semaphore" 패턴을 `ProductListCollector` 또는 관련 작업자에 적용하여, 작업들이 진정한 병렬로 실행되도록 수정합니다.
-   **이벤트 시스템 구현:** `proposal5.md`에서 제안된 "듀얼 채널 이벤트 시스템"을 구현합니다.
    -   `Orchestrator`가 개별 작업 완료 시 `atomic-task-update` 이벤트를 즉시 `emit` 하도록 수정합니다.
    -   주기적으로 `system-state-update`를 `emit`하는 로직을 추가합니다.

## 5. 기대 효과

-   **명확한 역할 분리:** 프론트엔드는 사용자 경험에, 백엔드는 비즈니스 로직과 데이터 처리에만 집중할 수 있게 됩니다.
-   **성능 및 효율성 향상:** 불필요한 중복 분석 작업을 제거하여 크롤링 시작 시간을 단축하고 서버 부하를 줄입니다.
-   **아키텍처의 완성:** 시스템이 상태를 기억하고, 그 상태를 기반으로 스스로 판단하고, 모든 과정을 UI에 실시간으로 보고하는, 우리가 원래 설계했던 지능적이고 반응적인 아키텍처가 완성됩니다.

이 제안을 적용하여 백엔드를 "기억력을 가진" 상태 저장 시스템으로 전환하는 것이 현재의 모든 문제를 해결하고 다음 단계로 나아가기 위한 가장 핵심적인 과제입니다.