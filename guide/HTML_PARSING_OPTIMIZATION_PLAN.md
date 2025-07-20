# HTML 파싱 도메인 최적화 계획 (Phase 4B)

## 🎯 문제 정의
현재 HTML 파싱 관련 코드들이 여러 도메인에 분산되어 있어 실제 Matter Certis 사이트에 최적화되지 않은 상태입니다.

### 문제점 분석
1. **혼재된 테스트 도메인들**:
   - `rra.go.kr` (한국 전파연구원) - 완전히 관련 없는 도메인
   - `csa-iot.org` (실제 Matter Certis 도메인) - 정확한 도메인
   - 혼재로 인한 CSS 셀렉터 최적화 불가

2. **CSS 셀렉터 정합성 부족**:
   - 실제 사이트 구조에 맞지 않는 셀렉터들
   - 하드코딩된 셀렉터로 인한 유지보수 어려움
   - 동적 사이트 구조 변경에 대한 대응 부족

3. **도메인 특화 최적화 부재**:
   - Matter Certis 사이트 구조에 최적화된 파싱 로직 부재
   - 일반적인 파싱 로직으로 인한 정확도 저하

## 🚀 해결 방안: 도메인 특화 HTML 파싱 아키텍처

### Phase 4B-1: 도메인 클린업 및 표준화
- **목표**: 모든 테스트 코드를 Matter Certis 도메인으로 통일
- **범위**: `rra.go.kr` 참조 완전 제거, `csa-iot.org` 최적화

### Phase 4B-2: CSS 셀렉터 중앙화 및 최적화
- **목표**: 설정 기반 셀렉터 관리 시스템 구축
- **범위**: 동적 셀렉터 로딩, A/B 테스트 지원

### Phase 4B-3: Matter Certis 특화 파싱 엔진
- **목표**: 사이트 구조에 최적화된 파싱 로직 구현
- **범위**: 제품 리스트, 상세페이지, 페이지네이션 전문화

## 📋 구현 계획

### 1. 중앙화된 셀렉터 관리 시스템

```rust
/// Matter Certis 특화 CSS 셀렉터 관리
pub struct MatterCertisSelectors {
    // 제품 리스트 페이지 셀렉터들
    pub product_list: ProductListSelectors,
    // 제품 상세 페이지 셀렉터들  
    pub product_detail: ProductDetailSelectors,
    // 페이지네이션 셀렉터들
    pub pagination: PaginationSelectors,
}

pub struct ProductListSelectors {
    pub container: &'static str,           // "div.post-feed"
    pub product_item: &'static str,        // "article.type-product"
    pub product_link: &'static str,        // "a[href*='/csa_product/']"
    pub product_title: &'static str,       // "h2.entry-title"
    pub product_image: &'static str,       // "img.attachment-post-thumbnail"
}

pub struct ProductDetailSelectors {
    pub title: &'static str,               // "h1.entry-title"
    pub description: &'static str,         // "div.entry-content"
    pub specifications: &'static str,      // "table.product-specs"
    pub certification_info: &'static str,  // "div.certification-details"
    pub vendor_info: &'static str,         // "div.vendor-information"
}
```

### 2. 설정 파일 기반 셀렉터 관리

```toml
# config/matter_certis_selectors.toml
[product_list]
container = "div.post-feed"
product_item = "article.type-product"
product_link = "a[href*='/csa_product/']"
fallback_link = "a[href*='/product/']"  # 백업 셀렉터

[product_detail]
title = "h1.entry-title"
description = "div.entry-content"
fallback_description = "div.post-content"  # 백업 셀렉터

[pagination]
next_page = "a.next"
page_numbers = "span.page-numbers"
total_pages = "span.total-pages"
```

### 3. 적응형 파싱 시스템

```rust
/// 사이트 구조 변경에 적응하는 파싱 엔진
pub struct AdaptiveHtmlParser {
    selectors: MatterCertisSelectors,
    fallback_selectors: Vec<SelectorGroup>,
    success_rate_tracker: SuccessRateTracker,
}

impl AdaptiveHtmlParser {
    /// 여러 셀렉터를 순차적으로 시도하여 최적 결과 도출
    pub async fn extract_with_fallback<T>(
        &self,
        html: &str,
        primary_selector: &str,
        fallback_selectors: &[&str],
        extractor: impl Fn(&Html, &Selector) -> Option<T>,
    ) -> Result<T> {
        // 1차: 기본 셀렉터 시도
        // 2차: 백업 셀렉터들 시도
        // 3차: 지능형 셀렉터 추론
    }
}
```

## 🎯 액션 아이템

### 즉시 실행 (Phase 4B-1)
1. ✅ `rra.go.kr` 참조 모두 제거
2. ✅ 모든 테스트 코드를 `csa-iot.org` 기준으로 통일
3. ✅ 현재 CSS 셀렉터들의 실제 사이트 호환성 검증

### 단기 목표 (Phase 4B-2)
1. 중앙화된 셀렉터 관리 시스템 구현
2. 설정 파일 기반 셀렉터 로딩
3. 셀렉터 유효성 자동 검증 시스템

### 중기 목표 (Phase 4B-3)  
1. Matter Certis 특화 파싱 엔진 완성
2. 적응형 셀렉터 시스템 구현
3. 파싱 성공률 모니터링 및 최적화

## 📊 성공 지표
- **파싱 정확도**: 95% 이상
- **사이트 구조 변경 대응**: 자동 감지 및 적응
- **유지보수성**: 셀렉터 변경 시 설정 파일만 수정
- **확장성**: 새로운 도메인 추가 시 플러그인 방식 지원

## 🔗 관련 문서 업데이트
- `re-arch-plan-final.md`: HTML 파싱 아키텍처 섹션 추가
- `matter-certis-v2-html-parsing-guide.md`: 도메인 특화 가이드 업데이트
- Phase 계획서들: HTML 파싱 최적화 단계 반영
