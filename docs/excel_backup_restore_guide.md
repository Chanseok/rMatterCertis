# Excel 백업/복원 및 데이터 관리 가이드

## 개요

Matter Certis v2에 로컬 데이터베이스의 전체 백업/복원 및 관리 기능이 추가되었습니다.

### 주요 기능

1. **전체 DB Excel 내보내기**: products와 product_details 테이블을 Excel 파일(.xlsx)로 내보내기
2. **Excel 백업 복원**: Excel 파일에서 데이터베이스로 복원
3. **물리 페이지 범위 삭제**: 특정 페이지 범위의 레코드 삭제
4. **전체 레코드 삭제**: 모든 제품 데이터 삭제 (안전장치 포함)

## 1. Excel 내보내기 (Export)

### 백엔드 API

```rust
#[tauri::command]
pub async fn export_full_database_excel(
    state: State<'_, DatabaseConnection>,
) -> Result<ExcelExportResult, String>
```

### 반환값

```rust
pub struct ExcelExportResult {
    pub file_path: String,           // 생성된 Excel 파일 경로
    pub products_count: i64,          // 내보낸 products 레코드 수
    pub product_details_count: i64,   // 내보낸 product_details 레코드 수
}
```

### 프론트엔드 사용법

```typescript
import { localDbDashboardStore } from '@/stores/localDbDashboardStore';

// Excel 내보내기
await localDbDashboardStore.exportFullDatabaseExcel();
```

### 생성되는 파일 구조

Excel 파일은 2개의 시트로 구성됩니다:

#### Sheet 1: "products"
- url (제품 URL)
- manufacturer (제조사)
- model (모델명)
- certificate_id (인증서 ID)
- page_id (페이지 번호)
- index_in_page (페이지 내 위치)
- id (제품 ID)
- created_at (생성일시)
- updated_at (수정일시)

#### Sheet 2: "product_details"
- url (제품 URL)
- page_id, index_in_page, id
- manufacturer, model, device_type, certificate_id
- certification_date (인증 날짜)
- software_version, hardware_version, firmware_version
- specification_version (Matter 스펙 버전)
- vid, pid (벤더/제품 ID)
- family_sku, family_variant_sku, family_id
- tis_trp_tested, transport_interface
- primary_device_type_ids (JSON 배열)
- application_categories (JSON 배열)
- description (설명)
- compliance_document_url (규정 준수 문서 URL)
- program_type (프로그램 유형)
- created_at, updated_at

### 파일 위치

내보낸 파일은 `<database_dir>/exports/` 폴더에 저장됩니다:
- 파일명 형식: `full_database_YYYYMMDDHHMMSS.xlsx`
- 예시: `full_database_20251006143022.xlsx`

## 2. Excel 가져오기 (Import/Restore)

### 백엔드 API

```rust
#[tauri::command]
pub async fn import_full_database_excel(
    state: State<'_, DatabaseConnection>,
    file_path: String,
) -> Result<ExcelImportResult, String>
```

### 반환값

```rust
pub struct ExcelImportResult {
    pub products_imported: u32,       // 새로 추가된 products 수
    pub products_updated: u32,        // 업데이트된 products 수
    pub details_imported: u32,        // 새로 추가된 product_details 수
    pub details_updated: u32,         // 업데이트된 product_details 수
    pub errors: Vec<String>,          // 발생한 오류 목록
    pub backup_file: Option<String>,  // 자동 생성된 백업 파일 경로
}
```

### 프론트엔드 사용법

```typescript
// Excel 파일에서 복원
await localDbDashboardStore.importFullDatabaseExcel('/path/to/backup.xlsx');
```

### 동작 방식

1. **자동 백업**: 가져오기 전에 현재 데이터베이스를 자동으로 백업
2. **트랜잭션 처리**: 모든 작업이 트랜잭션 내에서 실행되어 일관성 보장
3. **UPSERT 로직**: 
   - URL이 존재하면 → 업데이트
   - URL이 없으면 → 새로 삽입
4. **오류 처리**: 개별 행의 오류를 수집하여 반환 (전체 작업은 계속 진행)

### 주의사항

⚠️ **중요**: 
- 가져오기는 기존 데이터를 **덮어쓰거나 병합**합니다
- 자동 백업이 생성되지만, 수동 백업도 권장됩니다
- 큰 파일의 경우 시간이 오래 걸릴 수 있습니다

## 3. 페이지 범위 삭제

### 미리보기 API

```rust
#[tauri::command]
pub async fn preview_delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangePreview, String>
```

### 실행 API

```rust
#[tauri::command]
pub async fn delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangeResult, String>
```

### 프론트엔드 사용법

```typescript
// 삭제 미리보기
await localDbDashboardStore.previewDeleteRange(100, 110);

// 실제 삭제 실행
await localDbDashboardStore.executeDeleteRange(100, 110);
```

### 제한사항

- **최대 범위**: 한 번에 최대 50 페이지까지만 삭제 가능
- **안전장치**: from_page > to_page인 경우 오류 반환

## 4. 전체 레코드 삭제

### 백엔드 API

```rust
#[tauri::command]
pub async fn delete_all_records(
    state: State<'_, DatabaseConnection>,
    confirmation_token: String,
) -> Result<DeleteAllResult, String>
```

### 반환값

```rust
pub struct DeleteAllResult {
    pub deleted_products: u64,
    pub deleted_product_details: u64,
    pub deleted_bridge_rows: u64,
    pub backup_file: Option<String>,
}
```

### 프론트엔드 사용법

```typescript
// 전체 삭제 (확인 대화상자 포함)
await localDbDashboardStore.deleteAllRecordsConfirmed();
```

### 안전장치

1. **확인 대화상자**: 사용자에게 경고 메시지 표시
2. **확인 토큰**: "DELETE_ALL_CONFIRMED" 문자열 필요
3. **자동 백업**: 삭제 전 자동으로 Excel 백업 생성
4. **트랜잭션**: 모든 삭제가 원자적으로 실행
5. **VACUUM**: 삭제 후 데이터베이스 공간 정리

### 삭제 순서

1. `product_primary_device_types` (브리지 테이블)
2. `product_details` (자식 테이블)
3. `products` (부모 테이블)
4. `VACUUM` (공간 회수)

## 사용 시나리오

### 시나리오 1: 정기 백업

```typescript
// 매일 자동 백업
async function dailyBackup() {
  const result = await localDbDashboardStore.exportFullDatabaseExcel();
  console.log(`백업 완료: ${result.file_path}`);
  console.log(`제품 ${result.products_count}개, 상세정보 ${result.product_details_count}개`);
}
```

### 시나리오 2: 데이터 복원

```typescript
// 백업에서 복원
async function restoreFromBackup(backupPath: string) {
  const result = await localDbDashboardStore.importFullDatabaseExcel(backupPath);
  
  console.log(`복원 완료:`);
  console.log(`- 제품: ${result.products_imported}개 추가, ${result.products_updated}개 업데이트`);
  console.log(`- 상세정보: ${result.details_imported}개 추가, ${result.details_updated}개 업데이트`);
  
  if (result.errors.length > 0) {
    console.warn(`오류 ${result.errors.length}개 발생:`, result.errors.slice(0, 5));
  }
}
```

### 시나리오 3: 특정 페이지 범위 정리

```typescript
// 오래된 데이터 삭제 (페이지 1-50)
async function cleanupOldData() {
  // 먼저 미리보기
  const preview = await localDbDashboardStore.previewDeleteRange(1, 50);
  console.log(`삭제 예정: 제품 ${preview.products_count}개, 상세정보 ${preview.product_details_count}개`);
  
  // 사용자 확인 후 삭제
  if (confirm('정말 삭제하시겠습니까?')) {
    const result = await localDbDashboardStore.executeDeleteRange(1, 50);
    console.log(`삭제 완료: ${result.deleted_products}개 제품, ${result.deleted_product_details}개 상세정보`);
  }
}
```

### 시나리오 4: 데이터베이스 초기화

```typescript
// 전체 데이터 삭제 및 재구축
async function resetDatabase() {
  // 전체 삭제 (자동 백업 생성됨)
  const result = await localDbDashboardStore.deleteAllRecordsConfirmed();
  
  if (!result.cancelled) {
    console.log(`데이터베이스 초기화 완료`);
    console.log(`백업 파일: ${result.backup_file}`);
    
    // 이제 새로운 크롤링 시작 가능
  }
}
```

## 기술적 세부사항

### 사용된 크레이트

- **rust_xlsxwriter** (0.82): Excel 파일 쓰기
- **calamine** (0.27): Excel 파일 읽기

### 트랜잭션 보장

모든 데이터 변경 작업은 SQLite 트랜잭션 내에서 실행됩니다:

```rust
let mut tx = pool.begin().await?;
// ... 여러 작업 수행 ...
tx.commit().await?;
```

### 외래 키 제약 처리

삭제 작업 시 외래 키 순서를 준수합니다:
1. 브리지 테이블 (product_primary_device_types)
2. 자식 테이블 (product_details)
3. 부모 테이블 (products)

### 성능 고려사항

- **대용량 데이터**: 수천 개 이상의 레코드는 처리 시간이 오래 걸릴 수 있습니다
- **메모리 사용**: Excel 파일은 메모리에 로드되므로 매우 큰 파일은 주의 필요
- **인덱스**: page_id, index_in_page 인덱스가 삭제 성능에 영향을 줍니다

## 문제 해결

### Q1: Excel 내보내기가 실패합니다

**A**: 
1. exports 폴더 쓰기 권한 확인
2. 디스크 공간 확인
3. 백엔드 로그 확인 (`tracing` 로그)

### Q2: Excel 가져오기 중 일부 행에서 오류가 발생합니다

**A**: 
- `errors` 배열을 확인하여 어떤 행에서 문제가 발생했는지 확인
- Excel 파일의 데이터 형식이 올바른지 확인 (예: vid, pid는 숫자)
- 필수 컬럼(url)이 비어있지 않은지 확인

### Q3: 삭제 후 데이터베이스 크기가 줄어들지 않습니다

**A**: 
- `delete_all_records`는 자동으로 VACUUM을 실행합니다
- 페이지 범위 삭제 후에는 수동으로 VACUUM 실행 필요할 수 있습니다
- SQLite 관리 도구를 사용하여 VACUUM 실행

### Q4: 전체 삭제가 작동하지 않습니다

**A**: 
- 확인 토큰이 정확히 "DELETE_ALL_CONFIRMED"인지 확인
- 프론트엔드에서는 `deleteAllRecordsConfirmed()` 함수 사용 권장
- 이 함수는 자동으로 사용자 확인 대화상자를 표시합니다

## 향후 개선 계획

- [ ] 증분 백업/복원 (변경된 데이터만)
- [ ] 압축된 백업 파일 지원 (.xlsx.gz)
- [ ] 백업 스케줄링 (자동 정기 백업)
- [ ] 백업 파일 관리 UI (목록, 삭제, 복원)
- [ ] 백업 메타데이터 (생성일, 레코드 수 등)
- [ ] 다중 시트 지원 (vendors, device_types 포함)

## 변경 이력

- **2025-10-06**: 초기 구현
  - Excel 전체 내보내기/가져오기 기능 추가
  - 전체 레코드 삭제 기능 추가
  - 페이지 범위 삭제 기능 개선
