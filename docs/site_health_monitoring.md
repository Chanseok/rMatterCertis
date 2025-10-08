# 사이트 건강 상태 모니터링

## 개요

사이트의 일시적인 문제를 감지하여 크롤링 시작 전에 사용자에게 경고를 표시하는 기능입니다.

## 구현 내용

### 1. 설정 파일 필드 추가

`matter_certis_config.json`의 `app_managed` 섹션에 두 개의 필드를 추가했습니다:

```json
{
  "app_managed": {
    "last_known_max_page": 596,      // 캐시된 최대 페이지 수 (증가만 가능)
    "last_known_last_page": 589,     // 가장 최근 확인된 페이지 수 (매번 업데이트)
    // ...
  }
}
```

### 2. 업데이트 로직

#### `last_known_last_page`
- **매번 업데이트**: 사이트 상태를 확인할 때마다 현재 페이지 수로 업데이트됩니다.

#### `last_known_max_page`
- **캐싱 전략**: `last_known_last_page`가 이전 최대값보다 클 때만 업데이트됩니다.
- 이를 통해 사이트가 정상 작동할 때의 최대 페이지 수를 기억합니다.

### 3. 사이트 상태 판단

```rust
// 사이트 상태 체크 로직
if last_known_last_page < last_known_max_page {
    // 사이트 일시적 문제로 판단
    // - 페이지 수 감소 감지
    // - 감소율 계산
    // - 경고 플래그 설정
}
```

**판단 기준:**
- 현재 페이지 수(`last_known_last_page`)가 캐시된 최대값(`last_known_max_page`)보다 작으면
- 사이트가 정상적인 응답을 하지 못하는 것으로 판단

### 4. UI 경고 표시

사이트 상태 체크 시 페이지 수 감소가 감지되면 **두 곳**에서 경고를 표시합니다:

#### (1) 상단 상태 카드 (SessionStatusCard)
```
┌─────────────────────────────────────────┐
│ ⚠️ 사이트 일시적 문제 감지              │
│ 페이지 수 감소: 596 → 589 (1.2% 감소)   │
│ 💡 크롤링 대기 권장 - 사이트 정상화 후  │
│    재시도                                │
├─────────────────────────────────────────┤
│ ✅ 크롤링 준비 완료                     │
│ ...                                      │
└─────────────────────────────────────────┘
```

#### (2) 크롤링 컨트롤 섹션
```
┌─────────────────────────────────────────┐
│ 크롤링 컨트롤                           │
├─────────────────────────────────────────┤
│ ⚠️ 크롤링 부적합: 사이트 페이지 수      │
│    감소 감지 (596 → 589)                │
├─────────────────────────────────────────┤
│ [🎭 크롤링] [📊 범위 다시 계산] ...     │
└─────────────────────────────────────────┘
```

**자동 감지 시점:**
- 앱 시작 시 자동으로 사이트 상태 체크
- "범위 다시 계산" 버튼 클릭 시
- 크롤링 플랜 계산 시

## 사용 시나리오

### 정상 케이스
1. 첫 확인: 페이지 수 596 → `last_known_max_page`: 596, `last_known_last_page`: 596
2. 다음 확인: 페이지 수 600 → `last_known_max_page`: 600 (갱신), `last_known_last_page`: 600
3. 정상 크롤링 진행 가능 ✅

### 문제 감지 케이스
1. 이전 확인: 페이지 수 596 → `last_known_max_page`: 596
2. 현재 확인: 페이지 수 589 → `last_known_last_page`: 589 (업데이트)
3. 비교: 589 < 596 → 경고 표시 ⚠️
4. 크롤링 대기 권장

### 복구 케이스
1. 문제 상태: 589 < 596 (경고 중)
2. 재확인: 페이지 수 596 → `last_known_last_page`: 596
3. 비교: 596 = 596 → 경고 해제 ✅
4. 정상 크롤링 진행 가능

## 백엔드 로그

```rust
// 페이지 수 감소 감지 시
tracing::warn!(
    target="site_health",
    prev_max_page=%prev,
    current_page=%total_pages,
    "Page count dropped - possible site issue"
);

// 새로운 최대값 기록 시
tracing::info!(
    target="site_health",
    prev_max_page=%prev,
    new_max_page=%total_pages,
    "New maximum page count recorded"
);
```

## 구현 파일

### 백엔드 (Rust)
- `src-tauri/src/infrastructure/config.rs`: 설정 구조 정의
- `src-tauri/src/infrastructure/crawling_service_impls.rs`: 업데이트 로직

### 프론트엔드 (TypeScript/SolidJS)
- `src/components/tabs/CrawlingEngineTabSimple.tsx`: UI 경고 표시

## 참고

- 페이지 수 감소는 일시적인 사이트 문제일 수 있습니다:
  - 서버 부하
  - 네트워크 문제
  - 사이트 점검
  - 일시적인 데이터 숨김
  
- 감소가 감지되면 크롤링을 대기하고, 잠시 후 재확인하는 것이 좋습니다.
