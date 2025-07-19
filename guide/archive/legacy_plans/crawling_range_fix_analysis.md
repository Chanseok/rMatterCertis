# 크롤링 범위 계산 문제 분석 및 해결방안

## 📊 현재 상황 분석

### 🔍 발견된 문제점

**1. 하드코딩된 값들**
```rust
// service_based_crawling_engine.rs:235
let optimal_range = self.range_calculator.calculate_next_crawling_range(
    site_status.total_pages,
    10, // ⚠️ 하드코딩: products_on_last_page (default assumption)
).await?;

// crawling_service_impls.rs:1851
let products_per_page = defaults::DEFAULT_PRODUCTS_PER_PAGE; // ⚠️ 하드코딩: 12
```

**2. 잘못된 범위 계산 로직**
```log
2025-07-13 13:38:43.797 +09:00  INFO 🆕 No existing data, starting from page 482 to 383
2025-07-13 13:38:43.797 +09:00  INFO 🎯 Next crawling range: pages 482 to 383 (limit: 100)
```

**3. 설정 파일의 실제 값 무시**
- 설정: `last_known_max_page: 482`, `avg_products_per_page: 12.0`
- DB 상태: `116 products`
- 하지만 코드에서 `products_on_last_page=10` 하드코딩 사용

### 🎯 로그 분석 결과

**사이트 상태:**
- 총 페이지: 482
- 마지막 페이지 제품 수: 4 (로그에서 확인)
- 평균 페이지당 제품 수: 12

**DB 상태:**
- 현재 저장된 제품: 116개
- 예상 pageId: 9 (116 ÷ 12 = 9.67, floor = 9)
- 예상 indexInPage: 7 (116 % 12 - 1 = 7, 0-based)

**올바른 계산:**
- 마지막 저장 인덱스: (9 × 12) + 7 = 115
- 다음 크롤링 인덱스: 116
- 총 제품 수: ((482 - 1) × 12) + 4 = 5776
- Forward 인덱스: 5776 - 1 - 116 = 5659
- 목표 페이지: (5659 ÷ 12) + 1 = 472
- 크롤링 범위: 472 to 373 (100페이지 제한)

**❌ 현재 잘못된 계산:** 482 to 383 (100페이지)
**✅ 올바른 계산:** 472 to 373 (100페이지)

## 🛠️ 해결방안

### 1. 하드코딩 제거

**A. site_status에서 실제 값 사용**
```rust
// 현재 (service_based_crawling_engine.rs:233)
let optimal_range = self.range_calculator.calculate_next_crawling_range(
    site_status.total_pages,
    10, // ❌ 하드코딩
).await?;

// 수정 후
let optimal_range = self.range_calculator.calculate_next_crawling_range(
    site_status.total_pages,
    site_status.products_on_last_page.unwrap_or(12), // ✅ 실제 값 사용
).await?;
```

**B. 설정에서 products_per_page 사용**
```rust
// 현재 (crawling_service_impls.rs:1851)
let products_per_page = defaults::DEFAULT_PRODUCTS_PER_PAGE; // ❌ 하드코딩

// 수정 후  
let products_per_page = self.config.app_managed.avg_products_per_page as u32; // ✅ 설정값 사용
```

### 2. SiteStatus 구조체 확장

**site_status에 products_on_last_page 필드 추가:**
```rust
pub struct SiteStatus {
    pub accessible: bool,
    pub total_pages: u32,
    pub estimated_products: u32,
    pub products_on_last_page: Option<u32>, // ✅ 추가
    pub response_time_ms: u64,
    pub health_score: f64,
}
```

### 3. 범위 계산 검증 로직 강화

**계산 과정 세부 로깅:**
```rust
tracing::info!("📊 Detailed calculation breakdown:");
tracing::info!("  Site: total_pages={}, products_on_last_page={}", total_pages_on_site, products_on_last_page);
tracing::info!("  DB: max_page_id={:?}, max_index_in_page={:?}", max_page_id, max_index_in_page);
tracing::info!("  Config: products_per_page={}, crawl_limit={}", products_per_page, crawl_page_limit);
```

### 4. 설정 파일 활용 개선

**app_managed 설정 적극 활용:**
```json
{
  "app_managed": {
    "last_known_max_page": 482,
    "avg_products_per_page": 12.0, // ✅ 이 값을 사용
    "last_crawl_product_count": 5784
  }
}
```

## 🔧 구현 우선순위

### Phase 1: 즉시 수정 (Critical)
1. **하드코딩된 `products_on_last_page=10` 제거**
2. **`site_status.products_on_last_page` 실제 값 전달**
3. **설정의 `avg_products_per_page` 사용**

### Phase 2: 구조 개선 (High)
1. **SiteStatus 구조체에 `products_on_last_page` 필드 추가**
2. **범위 계산 검증 로직 강화**
3. **상세 로깅 추가**

### Phase 3: 장기 개선 (Medium)
1. **동적 products_per_page 계산 로직**
2. **범위 계산 알고리즘 최적화**
3. **에러 처리 개선**

## 📝 즉시 적용 가능한 수정사항

### 1. service_based_crawling_engine.rs 수정
```rust
// Line 233-237 수정
let products_on_last_page = match &site_status.response_data {
    Some(data) if data.contains("4 products") => 4, // 실제 파싱된 값
    _ => self.config.app_managed.avg_products_per_page as u32
};

let optimal_range = self.range_calculator.calculate_next_crawling_range(
    site_status.total_pages,
    products_on_last_page, // ✅ 실제 값 사용
).await?;
```

### 2. crawling_service_impls.rs 수정
```rust
// Line 1851 수정
let products_per_page = self.config.app_managed.avg_products_per_page as u32;
```

이 수정으로 **482 to 383**이 아닌 **472 to 373**의 올바른 범위가 계산될 것입니다.
