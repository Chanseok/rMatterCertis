# 크롤링 전략 개선 가이드
## 사이트 순서 변동 대응 방안

**작성일**: 2025-10-03  
**상황**: 수동 크롤링 후 오히려 중복/누락 문제가 악화되는 현상 발견

---

## 🔍 문제 분석

### 발견된 문제
- **증상**: 진단 → 수동크롤링 → 재진단 시 문제 제품 수가 증가
- **원인**: CSA-IOT 사이트가 제품을 재정렬하여 **좌표(page_id, index_in_page)가 유동적**

### 핵심 가정의 한계
```
기존 가정: page_id, index_in_page는 영구적
실제 현상: 신규 제품 추가/삭제 시 전체 순서 재배치
```

### 구체적 시나리오
```
초기 상태 (물리 페이지 522):
  index 8: 제품A (url_A) → DB에 page_id=46, index=8로 저장

사이트 변경 후:
  페이지 521: index 5: 제품A (url_A)  ← 이동
  페이지 522: index 8: 제품B (url_B)  ← 새로운 제품

수동 크롤링:
  페이지 522 스캔 → 제품B 발견
  BUT: 좌표(46, 8)는 이미 제품A가 차지
  결과: 제품B 저장 실패 또는 제품A 좌표 잘못 갱신
```

---

## ✅ 적용된 즉시 해결책

### 1. **UNIQUE 제약 조건 제거**
**파일**: `migrations/1003_drop_product_details_slot_unique.sql`

**변경 사항**:
```sql
-- 제거된 제약
DROP INDEX IF EXISTS ux_product_details_slot;
```

**이유**:
- ✅ **URL이 실제 식별자** (PRIMARY KEY)
- ✅ **좌표는 참고 정보**일 뿐 ("마지막 발견 위치")
- ✅ 동일 좌표에 여러 제품이 순차적으로 나타날 수 있음

**효과**:
- 제품B를 새 좌표로 저장 가능
- 제품A는 이전 좌표를 유지 (또는 재크롤링 시 갱신)
- 중복 충돌 해소

---

## 🎯 장기 전략 권장사항

### **전략 A: URL 중심 전체 재크롤링** (가장 안전) ⭐⭐⭐

#### 방식
```
1. 주기적으로 전체 페이지 범위 크롤링 (예: 주 1회)
2. URL을 기준으로 UPSERT (INSERT OR REPLACE)
3. 좌표 정보는 크롤링 시점 기준으로 자동 갱신
```

#### 장점
- ✅ 사이트 순서 변동에 완전히 대응
- ✅ 누락/중복 문제 근본적 해결
- ✅ 진단 도구 없이도 데이터 정확성 보장

#### 단점
- ⚠️ 전체 크롤링 시간 소요 (현재 568페이지 = 약 10-15분)

#### 구현 예시
```rust
// DuplicatePersistencePolicy::FullUpdate 사용
pub async fn full_recrawl(&self) -> Result<()> {
    let config = CrawlingConfig {
        start_page: 1,
        end_page: total_pages,
        duplicate_policy: DuplicatePersistencePolicy::FullUpdate,
        // ...
    };
    self.execute_crawl(config).await
}
```

---

### **전략 B: 하이브리드 접근** (효율적) ⭐⭐

#### 방식
```
평상시: 부분 크롤링 (최신 10-20 페이지만)
진단 시: 문제 감지되면 전체 재크롤링 트리거
```

#### 로직
```rust
let diagnostic = run_diagnostics().await?;

if diagnostic.has_critical_issues() {
    tracing::warn!("🚨 Critical issues detected, triggering full recrawl");
    full_recrawl().await?;
} else {
    partial_crawl(recommended_range).await?;
}
```

#### 장점
- ✅ 평상시 빠른 업데이트
- ✅ 문제 발생 시 자동 복구
- ✅ 사용자 개입 최소화

#### 단점
- ⚠️ 진단 로직 정확도에 의존
- ⚠️ 일시적으로 불일치 상태 허용

---

### **전략 C: 증분 크롤링 + 변경 감지** (최적화) ⭐

#### 방식
```
1. 전체 페이지 빠른 스캔 (URL 리스트만 수집)
2. 로컬 DB와 비교하여 신규/변경 URL 식별
3. 상세 페이지는 필요한 것만 크롤링
```

#### 2단계 프로세스
```
Stage 1: URL Collection (빠름)
  - 568페이지 × 12제품 = 6,816개 URL (5-10분)
  - 네트워크만 사용, 파싱 최소화

Stage 2: Detail Crawling (선택적)
  - 신규/변경된 URL만 크롤링 (1-3분)
  - DB 쓰기 최소화
```

#### 장점
- ✅ 가장 빠른 동기화
- ✅ 네트워크 부하 최소화
- ✅ 완전한 정확성 보장

#### 단점
- ⚠️ 구현 복잡도 증가
- ⚠️ 2단계 로직 필요

---

## 📊 전략 비교표

| 전략 | 정확도 | 속도 | 구현 난이도 | 권장 용도 |
|------|--------|------|-------------|-----------|
| **A: 전체 재크롤링** | ★★★ | ★☆☆ | ★☆☆ | 주간 배치 작업 |
| **B: 하이브리드** | ★★☆ | ★★☆ | ★★☆ | 일일 자동 업데이트 |
| **C: 증분 크롤링** | ★★★ | ★★★ | ★★★ | 실시간 모니터링 |

---

## 🛠️ 즉시 적용 가능한 조치

### 1. **환경 변수 활용** (현재 시스템)
```bash
# 수동 크롤링 시 좌표 강제 업데이트
MC_DUP_POLICY=update_id_index_only npm run crawl:manual -- --from 568 --to 1

# 전체 재크롤링 (모든 필드 갱신)
MC_DUP_POLICY=full_update npm run crawl:auto
```

### 2. **진단 기반 자동 복구**
```typescript
// frontend에서 호출
async function smartCrawl() {
  const diagnostic = await invoke('run_diagnostics');
  
  if (diagnostic.coord_mismatch > 100) {
    // 좌표 불일치가 심각하면 전체 재크롤링
    await invoke('full_recrawl');
  } else if (diagnostic.missing_products > 50) {
    // 누락 제품만 있으면 부분 크롤링
    await invoke('partial_crawl', { 
      range: diagnostic.recommended_range 
    });
  }
}
```

### 3. **주간 전체 동기화 스케줄링**
```rust
// 매주 일요일 새벽 2시 전체 재크롤링
pub async fn schedule_weekly_sync(&self) {
    use tokio::time::{interval, Duration};
    
    let mut ticker = interval(Duration::from_secs(7 * 24 * 3600));
    loop {
        ticker.tick().await;
        tracing::info!("🔄 Starting weekly full sync");
        self.full_recrawl().await?;
    }
}
```

---

## ⚠️ 주의사항

### 좌표의 의미 재정의
```
❌ 잘못된 해석: page_id, index_in_page는 제품의 영구적 식별자
✅ 올바른 해석: 마지막으로 발견된 위치 (스냅샷)
```

### URL의 역할
```
✅ URL = 제품의 진짜 식별자 (PRIMARY KEY)
✅ 사이트가 제품을 재정렬해도 URL은 불변
✅ 모든 로직은 URL 기준으로 작동
```

### 진단 도구의 한계
```
⚠️ coord_mismatch는 "문제"가 아닐 수 있음
   → 단지 사이트가 재정렬되었다는 신호
⚠️ duplicates는 실제 중복이 아닐 수 있음
   → 좌표 충돌일 뿐, URL은 고유함
```

---

## 🎯 최종 권장사항

### **단기 (즉시 적용)**
1. ✅ UNIQUE 제약 제거 (완료)
2. ✅ 수동 크롤링 시 `MC_DUP_POLICY=update_id_index_only` 사용
3. 📝 진단 결과 해석 시 "좌표 불일치"는 정상으로 인식

### **중기 (1-2주 내)**
1. 하이브리드 전략 구현
2. 진단 기반 자동 복구 로직 추가
3. UI에 "전체 재동기화" 버튼 추가

### **장기 (1개월 내)**
1. 증분 크롤링 시스템 구현
2. 주간 자동 전체 동기화 스케줄러 추가
3. 좌표 이력 추적 기능 (선택사항)

---

## 📚 참고 자료

- **마이그레이션**: `migrations/1003_drop_product_details_slot_unique.sql`
- **중복 정책**: `src-tauri/src/crawl_engine/actors/types.rs:1220`
- **크롤링 엔진**: `src-tauri/src/crawl_engine/`
- **진단 도구**: `src-tauri/src/crawl_engine/services/data_consistency_checker.rs`

---

## 📝 변경 이력

| 날짜 | 버전 | 내용 |
|------|------|------|
| 2025-10-03 | 1.0 | 초안 작성 |
| 2025-10-03 | 1.1 | UNIQUE 제약 제거 적용 |
