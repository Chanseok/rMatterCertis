# 크롤링 시간 추정 기능 가이드

## 개요

크롤링 진행 중 예상 소요 시간과 예상 종료 시간을 실시간으로 보여주는 기능입니다.

## 구현 내용

### 1. 데이터 구조 (`src/types/crawling.ts`)

`CrawlingProgress` 인터페이스에 `time_estimates` 필드가 추가되었습니다:

```typescript
interface CrawlingProgress {
  // ... 기존 필드들 ...
  
  time_estimates?: {
    // ListPage 크롤링 통계
    list_page_stats?: {
      completed_pages: number;
      remaining_pages: number;
      avg_time_per_page_ms: number;
      estimated_remaining_ms: number;
    };
    // Detail 크롤링 통계
    detail_stats?: {
      completed_products: number;
      remaining_products: number;
      avg_time_per_10_products_ms: number;
      estimated_remaining_ms: number;
    };
    // 전체 추정
    total_estimated_remaining_ms: number;
    estimated_completion_time: string; // ISO string format
  };
}
```

### 2. 시간 통계 수집 (`src/stores/crawlerStore.ts`)

#### 2.1 통계 저장 구조

`CrawlerState`에 `timeStats` 필드 추가:
- `listPageDurations`: ListPage 크롤링 소요 시간 배열 (최근 50개)
- `detailDurations`: Detail 크롤링 소요 시간 배열 (최근 100개)
- `sessionStartTime`: 세션 시작 시간

#### 2.2 통계 수집 로직

`handleStageItemCompleted` 메서드에서:
1. 완료된 아이템의 `duration_ms` 수집
2. Stage type에 따라 적절한 배열에 저장
3. `calculateTimeEstimates` 메서드 호출하여 추정치 계산

#### 2.3 시간 추정 계산

`calculateTimeEstimates` 메서드:
```typescript
private calculateTimeEstimates(
  stageType: string,
  currentItems: number,
  totalItems: number,
  currentBatch?: number,
  totalBatches?: number
)
```

- **ListPage 통계**: 페이지당 평균 시간 × 남은 페이지 수
- **Detail 통계**: 제품당 평균 시간 × 남은 제품 수
- **전체 예상**: 두 통계의 합산

### 3. UI 컴포넌트 (`src/components/TimeEstimatesPanel.tsx`)

시간 추정 정보를 표시하는 독립적인 컴포넌트:

#### 표시 내용:
1. **목록 페이지 수집** (파란색 패널)
   - 완료/남은 페이지 수
   - 페이지당 평균 소요 시간
   - 예상 남은 시간

2. **상세 정보 수집** (녹색 패널)
   - 완료/남은 제품 수
   - 10개당 평균 소요 시간
   - 예상 남은 시간

3. **전체 예상** (보라색 패널)
   - 총 예상 남은 시간
   - 예상 완료 시각

### 4. 유틸리티 함수 (`src/utils/timeUtils.ts`)

시간 포맷팅 및 계산 함수들:

- `formatDuration(ms)`: "1시간 23분" 형태로 변환
- `formatTime(isoString)`: "오후 3:45" 형태로 변환
- `formatDateTime(isoString)`: "1월 15일 오후 3:45" 형태로 변환
- `getElapsedTime(start, end)`: 경과 시간 계산
- `estimateRemainingTime(completed, total, elapsed)`: 남은 시간 추정

## 사용 방법

### CrawlingTab에 추가하기

```tsx
import TimeEstimatesPanel from '../components/TimeEstimatesPanel';
import { crawlerStore } from '../stores/crawlerStore';

export default function CrawlingTab() {
  const progress = crawlerStore.progress();
  
  return (
    <div>
      {/* 기존 진행 상황 UI */}
      
      {/* 시간 추정 패널 추가 */}
      <TimeEstimatesPanel progress={progress} />
      
      {/* 나머지 UI */}
    </div>
  );
}
```

### crawlerStore에서 직접 사용하기

```tsx
import { crawlerStore } from '../stores/crawlerStore';
import { formatDuration, formatTime } from '../utils/timeUtils';

function MyComponent() {
  const progress = crawlerStore.progress();
  
  return (
    <Show when={progress?.time_estimates}>
      {(est) => (
        <div>
          <p>예상 남은 시간: {formatDuration(est().total_estimated_remaining_ms)}</p>
          <p>예상 완료: {formatTime(est().estimated_completion_time)}</p>
        </div>
      )}
    </Show>
  );
}
```

## 특징

### 1. 적응형 예측
- 최근 N개의 소요 시간만 사용하여 평균 계산
- 크롤링 속도 변화에 빠르게 적응

### 2. 단계별 통계
- ListPage와 Detail 크롤링을 별도로 추적
- 각 단계의 특성에 맞는 통계 제공

### 3. 자동 초기화
- 세션 시작 시 통계 자동 초기화
- 이전 세션 데이터 영향 없음

### 4. 실시간 업데이트
- 각 아이템 완료 시마다 통계 갱신
- progress 상태 업데이트와 동기화

## 예시

### 크롤링 진행 중 표시 예:

```
┌────────────────────────────────────┐
│ 🕐 예상 소요 시간                  │
├────────────────────────────────────┤
│                                     │
│ 📄 목록 페이지 수집                │
│   완료/남은 페이지: 25 / 75        │
│   페이지당 평균: 2초               │
│   예상 남은 시간: 2분 30초         │
│                                     │
│ 📋 상세 정보 수집                  │
│   완료/남은 제품: 150 / 850        │
│   10개당 평균: 5초                 │
│   예상 남은 시간: 7분 5초          │
│                                     │
│ ⚡ 전체 예상                        │
│   총 예상 남은 시간: 9분 35초      │
│   예상 완료 시각: 오후 3:45        │
└────────────────────────────────────┘
```

## 주의사항

1. **초기 단계**: 통계가 충분히 쌓이기 전에는 예측이 부정확할 수 있습니다.
2. **네트워크 변동**: 네트워크 속도 변화는 반영되지만, 지연이 있을 수 있습니다.
3. **서버 부하**: 사이트 서버 상태 변화는 예측에 포함되지 않습니다.

## 향후 개선 사항

1. 시간대별 크롤링 속도 분석
2. 예측 정확도 표시
3. 과거 세션 데이터를 활용한 개선된 예측
4. 에러 발생 시 재시도 시간 반영
