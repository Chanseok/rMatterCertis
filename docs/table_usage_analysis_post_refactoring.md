# 테이블 사용 분석 (리팩토링 이후)

**분석 날짜**: 2025년 10월 9일  
**분석 대상**: BatchActor 제거 등 대규모 리팩토링 이후 테이블 사용 현황  
**이전 분석**: sync_sessions, sync_observed, page_fetch_attempts, crawling_results 사용 중으로 분석

---

## 📊 분석 결과 요약

| 테이블명 | 이전 분석 | 현재 상태 | 변경 사항 | 권장 조치 |
|---------|----------|----------|----------|----------|
| `sync_sessions` | ✅ 사용 중 | ✅ **사용 중** | 변화 없음 | **유지** |
| `sync_observed` | ✅ 사용 중 | ✅ **사용 중** | 변화 없음 | **유지** |
| `page_fetch_attempts` | ✅ 사용 중 | ✅ **사용 중** | 변화 없음 | **유지** |
| `crawling_results` | ✅ 사용 중 | ❌ **사용 안 함** | 🔴 **기능 제거됨** | **삭제 가능** |

---

## 1️⃣ sync_sessions & sync_observed (사용 중) ✅

### 위치
- `src-tauri/src/commands/sync_commands.rs`

### 사용 현황

#### sync_sessions 테이블
```rust
// 라인 886: 세션 완료 표시
sqlx::query(
    "UPDATE sync_sessions SET status='completed', finished_at=CURRENT_TIMESTAMP WHERE session_id = ?"
)
.bind(&session_id)
.execute(&pool)
.await
```

#### sync_observed 테이블
```rust
// 라인 403: 동기화 중 관찰된 URL 기록
sqlx::query(
    "INSERT INTO sync_observed(session_id, url, page_id, index_in_page) VALUES(?, ?, ?, ?) \
     ON CONFLICT(session_id, url) DO UPDATE SET page_id=excluded.page_id, index_in_page=excluded.index_in_page"
)
.bind(&session_id)
.bind(url)
.bind(calc.page_id)
.bind(calc.index_in_page)
.execute(&mut *tx)
.await;

// 라인 837-844: 삭제 대상 감지 (페이지당 12개 + 현재 세션에 없는 것)
sqlx::query(
    "DELETE FROM products p
     WHERE p.page_id BETWEEN ? AND ?
       AND p.page_id IN (
           SELECT o.page_id FROM (
               SELECT page_id, COUNT(*) AS cnt
               FROM sync_observed
               WHERE session_id = ?
               GROUP BY page_id
           ) o
           WHERE o.cnt = 12
       )
       AND NOT EXISTS (
           SELECT 1 FROM sync_observed o2
           WHERE o2.session_id = ? AND o2.url = p.url
       )"
)
```

### 용도
1. **sync_sessions**: 벤더 동기화 세션 추적 (시작/종료 시간, 상태 등)
2. **sync_observed**: 동기화 중 발견된 URL과 위치 정보 기록
   - 삭제 대상 감지: 이전에 12개였다가 사라진 상품 찾기
   - 중복 방지: 같은 세션에서 이미 처리한 URL인지 확인

### UI 연동
```tsx
// src/components/tabs/LocalDBTab.tsx:563
const syncVendors = async () => {
  const res = await tauriApi.dashboardVendorSync({ dry_run: false });
  // ...
}
```

### 결론
**유지 필요** ✅
- LocalDBTab의 "벤더 동기화" 기능에서 적극 사용 중
- 동기화 세션 추적 및 삭제 대상 감지에 필수적

---

## 2️⃣ page_fetch_attempts (사용 중) ✅

### 위치
- `src-tauri/src/infrastructure/crawling_service_impls.rs` (주 사용)
- `src-tauri/src/commands/devtools/page_completeness.rs` (조회)

### 사용 현황

#### 데이터 기록 (crawling_service_impls.rs)
```rust
// 라인 2376, 2389, 2426, 2438, 2558, 2592, 2606, 2629
// 크롤링 시도마다 상세 정보 기록
sqlx::query(
    "INSERT OR IGNORE INTO page_fetch_attempts 
     (logical_page_id, attempt_no, product_count, distinct_indices, 
      contiguous_ok, count_mismatch, index_mismatch, is_terminal_guess, 
      success_final, error_code, error_detail, duration_ms, retry_scheduled) 
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
)
// ... 13개 파라미터 바인딩
.execute(&mut *tx)
.await;

// 라인 2624: 중복 방지 체크
let safeguard_exists: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
    "SELECT 1 FROM page_fetch_attempts WHERE logical_page_id=?1 AND attempt_no=?2 LIMIT 1"
)
.bind(logical_page_id)
.bind(attempt_no)
.fetch_one(&mut *tx)
.await
.ok()
.flatten();
```

#### 데이터 조회 (page_completeness.rs)
```rust
// 라인 56: 추적된 페이지 총 개수
let total_pages_tracked: i64 = sqlx::query_scalar(
    "SELECT COUNT(DISTINCT logical_page_id) FROM page_fetch_attempts"
)
.fetch_one(&pool)
.await
.unwrap_or(0);
```

### 용도
1. **크롤링 시도 추적**: 각 페이지의 크롤링 시도 이력 저장
   - 시도 번호 (attempt_no)
   - 상품 개수, 인덱스 일치 여부
   - 성공/실패 상태, 에러 정보
   - 소요 시간

2. **페이지 완전성 분석**: devtools 기능
   - 문제가 있는 페이지 찾기
   - 전체 추적된 페이지 수 집계

### 결론
**유지 필요** ✅
- 크롤링 엔진의 핵심 디버깅/모니터링 기능
- 페이지 완전성 분석(devtools)에 사용
- 재시도 로직 및 실패 추적에 필수

---

## 3️⃣ crawling_results (사용 안 함) ❌

### 이전 분석 (틀림)
- ~~AnalysisTab.tsx의 exportCrawlingResults 기능에서 사용~~
- ~~실제로는 백엔드 코드에서만 정의되고 UI에서는 사용 안 함~~

### 현재 상태

#### 백엔드 코드 존재
```rust
// src-tauri/src/infrastructure/integrated_product_repository.rs:2183-2220
pub async fn save_crawling_result(&self, result: &CrawlingResult) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO crawling_results 
         (session_id, started_at, ended_at, success, total_pages, 
          processed_pages, failed_pages, total_duration_ms, error_message) 
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    // ...
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn get_crawling_results(&self, page: i32, limit: i32) -> Result<Vec<CrawlingResult>> {
    let offset = (page - 1) * limit;
    let rows = sqlx::query("SELECT * FROM crawling_results ORDER BY started_at DESC LIMIT ? OFFSET ?")
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
    // ...
}
```

#### 호출 경로 추적

1. **Repository**: `IntegratedProductRepository::save_crawling_result()` ✅ 정의됨
2. **Use Case**: `IntegratedCrawlingUseCases::save_crawling_result()` ✅ 정의됨
3. **Command**: ❌ **Tauri command로 노출 안 됨!**
4. **Frontend API**: `tauri-api.ts::exportCrawlingResults()` ✅ 정의됨
5. **UI 사용**: ❌ **실제 호출 없음!**

#### 검증 결과

```bash
# Tauri command 검색
grep -r "export_crawling_results" src-tauri/src/**/*.rs
# → 결과 없음! Tauri command로 등록되지 않음

# save_crawling_result 호출 검색
grep -r "save_crawling_result(" src-tauri/src/**/*.rs
# → repository와 use_case에만 정의됨, 실제 호출 없음!

# UI에서 exportCrawlingResults 호출 검색
grep -r "exportCrawlingResults" src/**/*.{ts,tsx}
# → tauri-api.ts에만 정의됨, 실제 호출 없음!
```

### 문제점

1. **백엔드**: 
   - Repository/UseCase에 메서드 정의만 존재
   - Tauri command로 노출되지 않음
   - 실제 크롤링 흐름에서 호출되지 않음

2. **프론트엔드**:
   - `tauri-api.ts`에 함수 정의만 존재
   - UI 어디에서도 호출하지 않음
   - `exportCrawlingResults()` → 백엔드 command 없어서 실행 불가

### 테이블 데이터 확인

```bash
sqlite3 certis_cache.db "SELECT COUNT(*) FROM crawling_results;"
# → 0 (데이터 없음)

sqlite3 certis_cache.db "SELECT * FROM crawling_results LIMIT 1;"
# → 비어있음
```

### 결론
**삭제 가능** ❌

`crawling_results` 테이블은:
- ✅ 마이그레이션에 정의되어 있음
- ✅ Repository/UseCase에 메서드 정의됨
- ❌ **Tauri command 없음** (백엔드-프론트엔드 연결 끊김)
- ❌ **실제 호출 없음** (크롤링 흐름에서 사용 안 함)
- ❌ **데이터 없음** (테이블 비어있음)

**Legacy 코드**로 판단됨. 과거 설계 당시 계획했으나 실제 구현되지 않은 기능.

---

## 🎯 최종 권장 조치

### 즉시 삭제 가능 (1개)
```sql
-- crawling_results 테이블 및 관련 코드 제거
DROP TABLE IF EXISTS crawling_results;
```

**제거할 코드**:
1. ✅ `src-tauri/migrations/` - crawling_results 테이블 생성 부분
2. ✅ `src-tauri/src/infrastructure/integrated_product_repository.rs` - save/get 메서드 (2개)
3. ✅ `src-tauri/src/application/integrated_use_cases.rs` - save/get 메서드 (2개)
4. ✅ `src-tauri/src/domain/session_manager.rs` - CrawlingResult 구조체 정의
5. ✅ `src/services/tauri-api.ts` - exportCrawlingResults() 메서드
6. ✅ `src/types/crawling.ts` - CrawlingResult 타입 정의
7. ✅ `src/stores/crawlerStore.ts` - lastResult 필드 (사용 안 함)

### 유지 필요 (3개)
```sql
-- 아래 테이블들은 현재 적극 사용 중
-- sync_sessions: 벤더 동기화 세션 추적
-- sync_observed: 벤더 동기화 URL 관찰
-- page_fetch_attempts: 크롤링 시도 추적 (디버깅/분석)
```

---

## 📝 변경 이력

### 이전 분석 (틀린 부분)
- ❌ "crawling_results - AnalysisTab.tsx의 exportCrawlingResults 기능에서 사용"
  - 실제로는 UI에서 호출되지 않음
  - Tauri command 자체가 없음

### 현재 분석 (정확)
- ✅ crawling_results는 완전히 사용되지 않는 legacy 테이블
- ✅ sync_sessions, sync_observed, page_fetch_attempts는 실제 사용 중

---

## 🔍 검증 방법

### 1. crawling_results 데이터 확인
```bash
sqlite3 certis_cache.db "SELECT COUNT(*) FROM crawling_results;"
# 결과: 0 (비어있음)
```

### 2. 코드 검색
```bash
# Tauri command 검색
rg "export_crawling_results" src-tauri/
# 결과: 없음

# 호출 검색
rg "save_crawling_result\(" src-tauri/
# 결과: 정의만 있고 호출 없음

# UI 호출 검색
rg "exportCrawlingResults\(" src/
# 결과: tauri-api.ts 정의만, 실제 호출 없음
```

### 3. 리팩토링 후 재검증
BatchActor 제거 등 대규모 리팩토링 이후에도:
- ✅ sync 관련 테이블: 여전히 사용 중
- ✅ page_fetch_attempts: 여전히 사용 중
- ❌ crawling_results: 이미 사용 안 했고 지금도 안 함

---

## ⚡ 다음 단계

1. **즉시 실행** (crawling_results 제거):
   ```bash
   # 1. 마이그레이션 수정
   # 2. 관련 코드 7개 파일 수정
   # 3. 타입 정의 정리
   # 4. 테스트
   ```

2. **검증**:
   ```bash
   # 앱 실행 후 모든 기능 정상 동작 확인
   npm run tauri dev
   
   # 특히 벤더 동기화 및 크롤링 기능 테스트
   ```

3. **문서화**:
   - 제거 사유 기록
   - 향후 비슷한 케이스 방지 가이드라인 작성
