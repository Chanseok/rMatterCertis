# 동기화 기능 명세 및 재정의

## 📋 현재 상황 분석

### 구현된 기능

#### 1. 빠른 동기화 (start_shallow_sync)
**현재 이름**: "빠른 동기화"  
**실제 기능**: 전체 페이지 리스트만 크롤링하여 좌표 갱신  
**처리 내용**:
- ListPageCrawling만 실행 (list_only: true)
- 제품 URL 수집 및 좌표(page_id, index_in_page) DB 업데이트
- 제품 상세 정보는 수집하지 않음
- SessionActor 기반 (중지 가능)

**문제점**: "빠른"이라는 이름이 실제 기능을 설명하지 못함

#### 2. 스마트 동기화 (start_smart_sync)
**현재 이름**: "스마트 동기화"  
**의도된 기능**:
- Phase 1: 빠른 동기화 실행
- Phase 2: 누락 분석
- Phase 3: 보완 크롤링

**실제 구현 상태**:
- Phase 1: ✅ 구현됨
- Phase 2: ⚠️ TODO (실제 분석 안 함)
- Phase 3: ⚠️ TODO (실제 크롤링 안 함)

**문제점**: 
- Phase 2, 3이 실제로 동작하지 않음
- 결국 빠른 동기화와 동일하게 작동
- 사용자에게 혼란을 줌

#### 3. 제품 보완 동기화 (start_complement_crawl)
**현재 이름**: "제품 보완 동기화"  
**실제 기능**: 핵심 필드 누락 제품만 재크롤링  
**처리 내용**:
- DB 쿼리로 누락 제품 URL 목록 조회
- ProductDetailCrawling만 실행 (URL 기반)
- ListPageCrawling 건너뛰기
- SessionActor 기반 (중지 가능)

**문제점**: 
- 빈 문자열 session_id로 인한 중지 버튼 오작동 (수정됨)
- "동기화"보다 "보완 크롤링"이 더 명확

---

## 🎯 재정의된 기능명

### 제안 1: 기능 목적 중심

| 기존 이름 | 새 이름 | 설명 | 사용 시기 |
|----------|---------|------|----------|
| 빠른 동기화 | **좌표 갱신** | 제품 URL 및 페이지 좌표만 업데이트 | 제품 위치 정보만 필요할 때 |
| 스마트 동기화 | ~~삭제~~ | 미구현 기능 (TODO 상태) | - |
| 제품 보완 동기화 | **제품 보완 크롤링** | 핵심 필드 누락 제품만 재크롤링 | 데이터 누락 보완 시 |

### 제안 2: 속도 + 범위 중심

| 기존 이름 | 새 이름 | 설명 | 사용 시기 |
|----------|---------|------|----------|
| 빠른 동기화 | **빠른 좌표 갱신** | 리스트만 크롤링 (상세 제외) | 빠른 업데이트 필요 시 |
| 스마트 동기화 | ~~삭제~~ | 미구현 | - |
| 제품 보완 동기화 | **선택적 제품 보완** | 누락 제품만 선별 크롤링 | 특정 제품 보완 시 |

### 제안 3: 기술적 명확성 중심

| 기존 이름 | 새 이름 | 설명 | 사용 시기 |
|----------|---------|------|----------|
| 빠른 동기화 | **리스트 전용 크롤링** | 제품 목록만 수집 | 좌표 정보 갱신 |
| 스마트 동기화 | ~~삭제~~ | 미구현 | - |
| 제품 보완 동기화 | **상세 보완 크롤링** | 누락 상세 정보만 수집 | 데이터 완성도 향상 |

---

## ✅ 최종 권장안

### 선택: **제안 1 (기능 목적 중심)**

이유:
1. 사용자가 "무엇을 하는지" 명확히 이해 가능
2. 기술 용어 최소화 (비개발자도 이해)
3. 사용 목적이 명확

### 적용 결과

```typescript
// Before
const handleShallowSync = async () => { ... }      // 빠른 동기화
const handleSmartSync = async () => { ... }        // 스마트 동기화
const handleComplementCrawl = async () => { ... }  // 제품 보완 동기화

// After
const handleCoordinateUpdate = async () => { ... }    // 좌표 갱신
// handleSmartSync 삭제 (미구현)
const handleComplementCrawl = async () => { ... }     // 제품 보완 크롤링
```

### UI 버튼 텍스트

```tsx
// Before
<Button>빠른 동기화</Button>
<Button>스마트 동기화</Button>
<Button>제품 보완 동기화</Button>

// After
<Button>좌표 갱신</Button>
// 스마트 동기화 버튼 삭제
<Button>제품 보완 크롤링</Button>
```

### 로그 메시지

```typescript
// Before
addLog("🏃 빠른 동기화 시작");
addLog("🧠 스마트 동기화 시작");
addLog("🔧 제품 보완 동기화 시작");

// After
addLog("📍 좌표 갱신 시작 (리스트 크롤링)");
// 스마트 동기화 삭제
addLog("🔧 제품 보완 크롤링 시작 (누락 제품 재수집)");
```

---

## 🔧 구현 계획

### 1단계: 중지 버튼 버그 수정 ✅ (완료)
```typescript
// 빈 문자열 체크 추가
if (sessionId && sessionId.trim() !== "") {
  setCurrentSessionId(sessionId);
}
```

### 2단계: 스마트 동기화 제거
- [ ] 프론트엔드 버튼 제거
- [ ] handleSmartSync 함수 제거
- [ ] 백엔드 start_smart_sync 주석 처리 또는 deprecated 마킹

### 3단계: 기능명 변경
- [ ] handleShallowSync → handleCoordinateUpdate
- [ ] 버튼 텍스트: "빠른 동기화" → "좌표 갱신"
- [ ] 로그 메시지 업데이트
- [ ] 상태 메시지 업데이트

### 4단계: 문서 업데이트
- [ ] README 업데이트
- [ ] 사용 가이드 업데이트
- [ ] API 문서 업데이트

---

## 📊 기능 비교표 (최종)

| 기능 | 목적 | 크롤링 범위 | 속도 | 중지 가능 |
|-----|------|------------|------|----------|
| **좌표 갱신** | 제품 위치 정보 업데이트 | 전체 리스트 페이지 | 빠름 | ✅ |
| **제품 보완 크롤링** | 누락 데이터 보완 | 누락 제품만 선별 | 매우 빠름 | ✅ |

---

## 🎯 사용 시나리오

### 좌표 갱신 사용 케이스
1. **제품 재배치 감지**: 사이트에서 제품 순서가 변경되었을 때
2. **페이지 구조 변경**: 제품 페이지 구성이 바뀌었을 때
3. **정기 좌표 업데이트**: 주기적인 위치 정보 갱신

### 제품 보완 크롤링 사용 케이스
1. **핵심 필드 누락**: certification_date, transport_interface 등이 없을 때
2. **부분 실패 복구**: 이전 크롤링에서 일부 제품이 실패했을 때
3. **데이터 완성도 향상**: DB에 불완전한 제품 정보를 보완할 때

---

## 💡 향후 개선 방향

### 스마트 동기화 재구현 (선택적)
만약 스마트 동기화를 실제로 구현한다면:

```rust
pub async fn start_smart_sync(app: tauri::AppHandle) -> Result<SmartSyncResult, String> {
    // Phase 1: 좌표 갱신
    let shallow_result = start_shallow_sync(app.clone()).await?;
    
    // Phase 2: 실제 누락 분석 (현재 analyze_missing_details는 TODO)
    let missing = query_missing_products(&app).await?;
    
    // Phase 3: 자동 보완 크롤링 (있으면 실행)
    let complement_result = if !missing.is_empty() {
        Some(start_complement_crawl(app).await?)
    } else {
        None
    };
    
    Ok(SmartSyncResult { ... })
}
```

이름: **"전체 동기화"** 또는 **"완전 동기화"**
- 좌표 갱신 + 자동 보완 = 모든 데이터 완성

---

**작성일**: 2025-10-09  
**상태**: 제안서 (검토 대기)
