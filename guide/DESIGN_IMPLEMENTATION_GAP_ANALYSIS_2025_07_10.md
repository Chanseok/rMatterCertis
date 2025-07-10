# 설계와 구현 간 GAP 분석 보고서
*작성일: 2025년 7월 10일*

## 📋 개요

본 문서는 rMatterCertis 프로젝트의 설계 의도와 실제 구현 사이의 차이점을 분석하고, 발견된 문제점들과 해결 방안을 정리한 종합 보고서입니다.

## 🔍 주요 발견사항

### 1. 페이지네이션 시스템 GAP

#### 설계 의도
- 원본 사이트의 페이지 구조를 그대로 반영
- 단순한 페이지별 인덱싱

#### 실제 구현 문제점
- 원본 사이트의 불규칙한 페이지 구조 (마지막 페이지에 7개만 존재)
- 빈 페이지에서 fallback selector가 잘못된 요소 매칭
- 페이지별 제품 수 불일치로 인한 데이터 정합성 문제

#### 해결된 구현
```rust
// 정교한 페이지네이션 계산식 적용
pub fn calculate_page_index(&self, current_page: u32, index_on_page: u32) -> (i32, i32) {
    let total_products = (self.total_pages - 1) * self.items_per_page + self.items_on_last_page;
    let index_from_newest = (current_page - 1) * self.items_per_page + index_on_page;
    let total_index = total_products - 1 - index_from_newest;
    
    let page_id = total_index / self.target_page_size;
    let index_in_page = total_index % self.target_page_size;
    
    (page_id as i32, index_in_page as i32)
}
```

### 2. 데이터베이스 스키마 GAP

#### 설계 의도
- Rust의 snake_case 컨벤션 사용
- 단순한 필드 매핑

#### 실제 구현 문제점
- 데이터베이스는 camelCase, Rust 코드는 snake_case 사용
- "no such column: manufacturer" 등의 컬럼명 불일치 에러
- TypeScript와 Rust 간 필드명 불일치

#### 해결된 구현
```rust
// Product 구조체에 serde 어노테이션 추가
pub struct Product {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "certificateId")]
    pub certificate_id: Option<String>,
    #[serde(rename = "pageId")]
    pub page_id: Option<i32>,
    #[serde(rename = "indexInPage")]
    pub index_in_page: Option<i32>,
    // ...
}
```

### 3. 제품 데이터 구조 GAP

#### 설계 의도
- Product 구조체에 모든 필드 포함
- 단일 구조체로 모든 정보 관리

#### 실제 구현 문제점
- Product와 ProductDetail 간 필드 중복
- `device_type`, `certification_date` 필드가 잘못된 구조체에 위치
- 기본 정보와 상세 정보의 명확한 분리 부족

#### 해결된 구현
```rust
// 명확한 역할 분리
pub struct Product {
    // 제품 목록에서 추출 가능한 기본 정보만
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    // 페이지네이션 관련
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    // 타임스탬프
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ProductDetail {
    // 제품 상세 페이지에서만 얻을 수 있는 정보
    pub device_type: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    // ... 기타 상세 정보
}
```

### 4. 타임스탬프 관리 GAP

#### 설계 의도
- 간단한 현재 시간 설정

#### 실제 구현 문제점
- 업데이트 시 created_at도 덮어써짐
- INSERT OR REPLACE로 인한 타임스탬프 정보 손실

#### 해결된 구현
```rust
pub async fn create_or_update_product(&self, product: &Product) -> Result<()> {
    let existing = self.get_product_by_url(&product.url).await?;
    
    let (created_at, updated_at) = if let Some(existing_product) = existing {
        // 기존 제품: created_at 보존, updated_at만 업데이트
        (existing_product.created_at, chrono::Utc::now())
    } else {
        // 새 제품: 둘 다 현재 시간
        let now = chrono::Utc::now();
        (now, now)
    };
    
    // UPSERT 쿼리 실행...
}
```

## ✅ 해결된 주요 문제들

### 1. 제품 개수 오탐지 해결
- **문제**: 빈 페이지에서 "Final verified last page: 480 with 1 products" 오보고
- **원인**: fallback selector가 관련 없는 `<article>` 요소 매칭
- **해결**: 정확한 selector만 사용 (`div.post-feed article.type-product`)

### 2. 데이터베이스 컬럼 에러 해결
- **문제**: "no such column: manufacturer" 등의 런타임 에러
- **원인**: snake_case vs camelCase 불일치
- **해결**: 모든 SQL 쿼리를 camelCase로 통일

### 3. 페이지네이션 정확성 확보
- **문제**: 원본 사이트의 불규칙한 페이지 구조
- **해결**: prompts6 기반의 정교한 계산식 적용

## 📊 GAP 분석 매트릭스

| 영역 | 설계 복잡도 | 구현 복잡도 | GAP 크기 | 해결 상태 |
|------|-------------|-------------|----------|-----------|
| 페이지네이션 | 낮음 | 높음 | 큰 GAP | ✅ 해결 |
| 데이터베이스 스키마 | 중간 | 높음 | 중간 GAP | ✅ 해결 |
| 제품 데이터 구조 | 낮음 | 중간 | 작은 GAP | ✅ 해결 |
| 타임스탬프 관리 | 낮음 | 중간 | 작은 GAP | ✅ 해결 |
| 에러 처리 | 중간 | 중간 | 작은 GAP | ✅ 해결 |

## 🔧 적용된 해결 방안

### 1. Architecture Pattern 개선
- **Repository Pattern**: 데이터베이스 접근 로직 캡슐화
- **Strategy Pattern**: 페이지네이션 계산 로직 분리
- **Builder Pattern**: 복잡한 설정 객체 생성

### 2. 코드 품질 향상
- **Type Safety**: Rust의 타입 시스템 활용
- **Error Handling**: Result 타입으로 명시적 에러 처리
- **Concurrency**: Arc<RwLock>으로 스레드 안전성 확보

### 3. 데이터 일관성 보장
- **Schema Validation**: 컴파일 타임 스키마 검증
- **Transaction Management**: 원자적 데이터베이스 작업
- **Cache Strategy**: 페이지 분석 결과 캐싱

## 📈 성능 개선 결과

### Before (문제 상황)
```
❌ 빈 페이지에서 제품 1개 오탐지
❌ 데이터베이스 컬럼 에러로 크롤링 중단
❌ 페이지네이션 불일치로 데이터 중복
❌ 타임스탬프 정보 손실
```

### After (해결 후)
```
✅ 정확한 제품 개수 탐지 (빈 페이지 = 0개)
✅ 모든 데이터베이스 작업 정상 동작
✅ 일관된 페이지네이션 (prompts6 기준)
✅ 완전한 타임스탬프 추적
```

## 🎯 핵심 학습 사항

### 1. 설계 시 고려해야 할 요소들
- **외부 시스템의 불규칙성**: 원본 사이트 구조 변화 대응
- **데이터 일관성**: 다중 컴포넌트 간 명명 규칙 통일
- **확장성**: 미래 요구사항 변화에 대한 유연성

### 2. 구현 시 주의사항
- **가정의 검증**: 설계 단계의 가정이 실제와 일치하는지 확인
- **점진적 개발**: 작은 단위로 검증하며 진행
- **통합 테스트**: 컴포넌트 간 상호작용 검증

### 3. 유지보수성 향상
- **문서화**: 복잡한 로직의 명확한 문서화 (prompts6 등)
- **테스트 커버리지**: 핵심 로직의 철저한 테스트
- **모니터링**: 런타임 동작 상태 추적

## 🚀 향후 개선 방향

### 1. 단기 목표 (1-2주)
- [ ] 페이지네이션 로직 단위 테스트 추가
- [ ] 데이터베이스 마이그레이션 스크립트 정리
- [ ] 에러 로깅 시스템 개선

### 2. 중기 목표 (1-2개월)
- [ ] 실시간 모니터링 대시보드 구축
- [ ] 자동 백업/복구 시스템 구현
- [ ] 성능 최적화 (배치 처리, 캐싱 전략)

### 3. 장기 목표 (3-6개월)
- [ ] 마이크로서비스 아키텍처 검토
- [ ] AI 기반 데이터 품질 검증
- [ ] 다중 사이트 지원 확장

## 📝 결론

이번 GAP 분석을 통해 설계와 구현 사이의 주요 차이점들을 식별하고 체계적으로 해결했습니다. 특히 페이지네이션 시스템의 정교한 개선과 데이터베이스 스키마 일관성 확보는 시스템의 안정성과 확장성을 크게 향상시켰습니다.

**핵심 성과**:
- ✅ 모든 크리티컬 GAP 해결 완료
- ✅ 시스템 안정성 대폭 개선
- ✅ 코드 품질 및 유지보수성 향상
- ✅ 확장 가능한 아키텍처 구축

이러한 개선을 통해 rMatterCertis는 더욱 견고하고 신뢰할 수 있는 CSA-IoT 인증 데이터 크롤링 시스템으로 발전했습니다.

---
*이 문서는 지속적으로 업데이트되며, 새로운 GAP이 발견되거나 해결될 때마다 내용이 추가됩니다.*