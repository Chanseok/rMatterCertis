# Matter Certis v2 데이터베이스 스키마

## 현재 데이터베이스 스키마 (2025년 7월 2일)

현재 Matter Certis v2 애플리케이션은 **통합 스키마**를 사용합니다. 이 스키마는 실제 5,582개의 Matter 제품 데이터를 기반으로 최적화되었으며, 성능 및 확장성을 고려해 설계되었습니다.

### 핵심 테이블

#### 1. Products (기본 제품 정보)
```sql
CREATE TABLE products (
    url TEXT PRIMARY KEY,                    -- 제품 상세 페이지 URL
    manufacturer TEXT,                       -- 제조사 이름
    model TEXT,                              -- 제품 모델명
    certificate_id TEXT,                     -- 인증서 ID
    page_id INTEGER,                         -- 발견된 페이지 번호
    index_in_page INTEGER,                   -- 페이지 내 위치
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

#### 2. Product Details (상세 제품 정보)
```sql
CREATE TABLE product_details (
    url TEXT PRIMARY KEY,                   -- products.url과 연결
    page_id INTEGER,                        -- 페이지 번호
    index_in_page INTEGER,                  -- 페이지 내 위치
    
    -- 핵심 식별 정보
    id TEXT,                               -- Matter 내부 제품 ID
    manufacturer TEXT,                     -- 제조사 이름
    model TEXT,                            -- 제품 모델명
    device_type TEXT,                      -- 디바이스 유형 분류
    certificate_id TEXT,                   -- 인증서 ID
    
    -- 인증 상세 정보
    certification_date TEXT,               -- 인증 날짜
    software_version TEXT,                 -- 소프트웨어 버전
    hardware_version TEXT,                 -- 하드웨어 버전
    firmware_version TEXT,                 -- 펌웨어 버전
    specification_version TEXT,            -- Matter 스펙 버전
    
    -- 기술적 식별자 (성능 최적화: 정수형)
    vid INTEGER,                           -- 벤더 ID (16진수를 정수로 변환)
    pid INTEGER,                           -- 제품 ID (16진수를 정수로 변환)
    
    -- 제품 패밀리 정보
    family_sku TEXT,                       -- 패밀리 SKU
    family_variant_sku TEXT,               -- 패밀리 변형 SKU
    family_id TEXT,                        -- 패밀리 ID
    
    -- 테스트 및 준수
    tis_trp_tested TEXT,                   -- TIS/TRP 테스트 상태
    transport_interface TEXT,              -- 전송 인터페이스
    primary_device_type_id TEXT,           -- 주 디바이스 유형 ID
    application_categories TEXT,           -- JSON 배열 형태의 카테고리
    
    -- 새 설계의 추가 기능
    description TEXT,                      -- 제품 설명
    compliance_document_url TEXT,          -- 규정 준수 문서 다운로드 URL
    program_type TEXT DEFAULT 'Matter',    -- 인증 프로그램 유형
    
    -- 감사 필드
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- 외래 키 관계
    FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);
```

#### 3. Vendors (벤더 정보)
```sql
CREATE TABLE vendors (
    vendor_id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor_number INTEGER UNIQUE,
    vendor_name TEXT NOT NULL,
    company_legal_name TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

#### 4. Crawling Results (크롤링 결과)
```sql
CREATE TABLE crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    stage TEXT NOT NULL,
    total_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    details_fetched INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    execution_time_seconds INTEGER,
    config_snapshot TEXT,
    error_details TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### 레거시 호환성 뷰

레거시 코드와의 호환성을 위해 다음 뷰가 제공됩니다:

```sql
CREATE VIEW matter_products_legacy AS
SELECT 
    pd.url, pd.page_id, pd.index_in_page, pd.id, pd.manufacturer,
    pd.model, pd.device_type, pd.certificate_id, pd.certification_date,
    pd.software_version, pd.hardware_version, pd.vid, pd.pid,
    pd.family_sku, pd.family_variant_sku, pd.firmware_version,
    pd.family_id, pd.tis_trp_tested, pd.specification_version,
    pd.transport_interface, pd.primary_device_type_id,
    pd.application_categories, pd.created_at, pd.updated_at
FROM product_details pd
INNER JOIN products p ON pd.url = p.url
WHERE pd.program_type = 'Matter' OR pd.program_type IS NULL;
```

## 마이그레이션 파일

프로젝트는 다음 마이그레이션 파일을 사용합니다:

1. `003_integrated_schema.sql` - 통합 스키마 정의
2. `004_migrate_legacy_data.sql` - 레거시 데이터 마이그레이션

> **참고**: 이전 마이그레이션 파일(`001_initial.sql`, `002_matter_products.sql`)은 아카이브되었으며 더 이상 사용되지 않습니다.

## 스키마 개선 사항

이 통합 스키마는 다음과 같은 이점을 제공합니다:

- **프로덕션 검증**: 5,582개의 실제 Matter 제품 데이터 기반
- **최적화된 설계**: products + product_details 2테이블 구조
- **성능 최적화**: vid/pid에 정수형 사용으로 성능 최적화
- **확장성**: CSA-IoT의 모든 필드를 완전히 포함

- **현대적 개선**:
  - 일관된 snake_case 네이밍
  - 강력한 데이터 제약 (NOT NULL, UNIQUE, FK)
  - 포괄적인 감사 추적 (created_at, updated_at)
  - 향상된 필드 (description, compliance_document_url)
  - 자동 타임스탬프 트리거

- **하위 호환성**:
  - 기존 프론트엔드를 위한 레거시 뷰
  - 프로덕션 데이터베이스와 동일한 핵심 구조
  - 기존 5,582개 제품에서 원활한 마이그레이션 경로

자세한 구현 상태는 [integrated-schema-implementation-status.md](./integrated-schema-implementation-status.md)를 참조하세요.
