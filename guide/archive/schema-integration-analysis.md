# Database Schema Integration Analysis

## Three Schema Comparison

### 1. **Production Database Schema** (검증된 실제 데이터)
```sql
-- 5,582개 실제 제품으로 검증됨
CREATE TABLE products (
    url TEXT PRIMARY KEY,
    manufacturer TEXT,
    model TEXT,
    certificateId TEXT,  -- camelCase
    pageId INTEGER,      -- camelCase  
    indexInPage INTEGER  -- camelCase
);

CREATE TABLE product_details (
    url TEXT PRIMARY KEY,
    pageId INTEGER,
    indexInPage INTEGER,
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    deviceType TEXT,
    certificationId TEXT,
    certificationDate TEXT,
    softwareVersion TEXT,
    hardwareVersion TEXT,
    vid INTEGER,  -- 최적화: INTEGER 타입
    pid INTEGER,  -- 최적화: INTEGER 타입
    familySku TEXT,
    familyVariantSku TEXT,
    firmwareVersion TEXT,
    familyId TEXT,
    tisTrpTested TEXT,
    specificationVersion TEXT,
    transportInterface TEXT,
    primaryDeviceTypeId TEXT,
    applicationCategories TEXT  -- JSON array
);

CREATE TABLE vendors (
    vendorId INTEGER PRIMARY KEY,
    vendorName TEXT,
    companyLegalName TEXT
);
```

### 2. **001_initial.sql Schema** (새 설계 v1)
```sql
CREATE TABLE matter_products (
    url TEXT PRIMARY KEY,
    page_id INTEGER,           -- snake_case
    index_in_page INTEGER,     -- snake_case
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    device_type TEXT,
    certificate_id TEXT,       -- snake_case
    certification_date TEXT,
    software_version TEXT,
    hardware_version TEXT,
    vid TEXT,                  -- TEXT 타입 (덜 최적화)
    pid TEXT,                  -- TEXT 타입 (덜 최적화)
    family_sku TEXT,
    family_variant_sku TEXT,
    firmware_version TEXT,
    family_id TEXT,
    tis_trp_tested TEXT,
    specification_version TEXT,
    transport_interface TEXT,
    primary_device_type_id TEXT,
    application_categories TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### 3. **002_matter_products.sql Schema** (새 설계 v2)
```sql
CREATE TABLE matter_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,  -- 추가 PK
    certificate_id TEXT UNIQUE NOT NULL,   -- UNIQUE 제약
    company_name TEXT NOT NULL,            -- NOT NULL 제약
    product_name TEXT NOT NULL,            -- NOT NULL 제약
    description TEXT,                      -- 새 필드
    firmware_version TEXT,
    hardware_version TEXT,
    specification_version TEXT,
    product_id TEXT,
    vendor_id TEXT,
    primary_device_type_id TEXT,
    transport_interface TEXT,
    certified_date DATE,                   -- DATE 타입
    tis_trp_tested BOOLEAN DEFAULT FALSE,  -- BOOLEAN 타입
    compliance_document_url TEXT,          -- 새 필드
    program_type TEXT DEFAULT 'Matter',    -- 새 필드
    device_type TEXT,
    detail_url TEXT UNIQUE,
    listing_url TEXT,                      -- 새 필드
    page_number INTEGER,
    position_in_page INTEGER,
    crawled_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

## Integration Strategy: Best of All Worlds

### 📊 **INTEGRATED OPTIMAL SCHEMA**

#### 장점 분석 및 통합 결정:

1. **Production Schema 장점** ✅
   - 5,582개 실제 데이터로 검증
   - 성능 최적화된 타입 (vid/pid INTEGER)
   - 완전한 필드 커버리지
   - 2테이블 분리 설계 (성능 최적화)

2. **001_initial 장점** ✅  
   - 표준 snake_case 네이밍
   - 완전한 audit 필드 (created_at, updated_at)
   - 종합적인 인덱싱 전략

3. **002_matter_products 장점** ✅
   - 강화된 데이터 제약 (NOT NULL, UNIQUE)
   - 추가 유용한 필드 (description, compliance_document_url)
   - 더 명확한 타입 정의 (BOOLEAN, DATE)

### 🎯 **FINAL INTEGRATED SCHEMA**

#### 결정: Production Schema + 개선사항 적용
