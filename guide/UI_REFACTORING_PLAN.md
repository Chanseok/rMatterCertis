# rMatterCertis UI 리팩토링 및 통합 계획

## 🎯 목표
기존 UI 시스템과 새로운 게임 스타일 시각화를 체계적으로 통합하여, 아키텍처 문서(v1.0~v4.0)와 일치하는 일관된 시스템 구축

---

## 🔍 현재 상태 분석

### 문제점
1. **아키텍처 분산**: 기존 탭 시스템과 독립적인 게임 대시보드
2. **상태 관리 중복**: tabStore, uiStore, 게임 시뮬레이션 상태가 각각 존재
3. **백엔드 연동 부재**: 실제 크롤링 엔진과 연결되지 않은 시뮬레이션
4. **문서-구현 간격**: v4.0 아키텍처 문서와 현재 구현 불일치

### 기존 자산 분석
- ✅ 기본 탭 시스템 (Settings, Status, LocalDB, Analysis)
- ✅ 기본 UI 컴포넌트 (Button, Modal, Toast 등)
- ✅ 상태 관리 시스템 (tabStore, uiStore)
- ✅ 게임 스타일 시각화 컴포넌트 (방금 구현)
- ❌ 백엔드 연동 인터페이스
- ❌ 실시간 데이터 연동

---

## 🛠️ 3단계 통합 계획

### Phase 1: 아키텍처 정리 (2-3일)
**목표**: 기존 UI 시스템 정리 및 통합 기반 마련

#### 1.1 상태 관리 통합
- [ ] `CrawlingState` 중앙 상태 스토어 구현
- [ ] 기존 시뮬레이션 로직을 통합 상태로 이동
- [ ] tabStore와 uiStore 역할 명확화

#### 1.2 컴포넌트 구조 정리
```
src/components/
├── layout/              # 레이아웃 (유지)
├── tabs/                # 기존 탭들 (유지)
├── visualization/       # 시각화 컴포넌트 (새로 생성)
│   ├── CrawlingCityDashboard.tsx
│   ├── CrawlingCity3D.tsx
│   ├── CrawlingMetricsChart.tsx
│   └── integrated/      # 통합 뷰 컴포넌트
├── crawling/           # 크롤링 관련 컴포넌트
│   ├── CrawlingControl.tsx
│   ├── CrawlingProgress.tsx
│   └── CrawlingResults.tsx
└── ui/                 # 기본 UI 컴포넌트 (유지)
```

#### 1.3 기존 StatusTab 강화
- [ ] 기존 StatusTab을 게임 대시보드와 통합
- [ ] 뷰 모드 선택 기능 추가 (Classic, City, 3D, Metrics)
- [ ] 기존 사용자 경험 유지

### Phase 2: 백엔드 연동 인터페이스 구현 (3-4일)
**목표**: v4.0 아키텍처 문서의 `SystemStatePayload` 구현

#### 2.1 Tauri 이벤트 시스템 구현
```rust
// src-tauri/src/events/
pub struct SystemStatePayload {
    pub overall_status: OverallStatus,
    pub active_profile: OperatingProfile,
    pub prediction: PredictionData,
    pub progress: ProgressData,
    pub worker_metrics: Vec<WorkerMetrics>,
    pub queue_metrics: Vec<QueueMetrics>,
    pub resource_usage: ResourceUsage,
}
```

#### 2.2 실시간 데이터 브로드캐스팅
- [ ] `SystemStateBroadcaster` 구현
- [ ] 1초마다 상태 업데이트 이벤트 전송
- [ ] 프론트엔드 이벤트 리스너 구현

#### 2.3 시뮬레이션 → 실제 데이터 전환
- [ ] 현재 시뮬레이션 로직을 백엔드로 이동
- [ ] 실제 크롤링 엔진 상태 반영
- [ ] 오류 처리 및 재연결 로직

### Phase 3: 고도화 및 최적화 (2-3일)
**목표**: 사용자 경험 최적화 및 성능 향상

#### 3.1 고급 시각화 기능
- [ ] 예측 분석 시각화
- [ ] 성능 히스토리 차트
- [ ] 작업자 풀 상태 실시간 애니메이션

#### 3.2 제어 시스템 통합
- [ ] 운영 프로파일 선택 UI
- [ ] 실시간 파라미터 조정
- [ ] 즉시 효과 반영

#### 3.3 사용자 경험 개선
- [ ] 뷰 모드 간 부드러운 전환
- [ ] 개인화된 대시보드 레이아웃
- [ ] 접근성 및 키보드 네비게이션

---

## 📊 작업 우선순위 매트릭스

### 즉시 실행 (High Impact, Low Effort)
1. **StatusTab 뷰 모드 통합**: 기존 탭에 게임 대시보드 선택 옵션 추가
2. **상태 관리 통합**: 중복된 상태 로직 정리
3. **컴포넌트 폴더 재구성**: 명확한 구조 정립

### 핵심 개발 (High Impact, High Effort)
1. **SystemStatePayload 구현**: 백엔드-프론트엔드 인터페이스
2. **실시간 데이터 연동**: Tauri 이벤트 시스템
3. **통합 크롤링 제어**: 실제 엔진과 연결

### 장기 개선 (Low Impact, Low Effort)
1. **시각화 효과 개선**: 더 부드러운 애니메이션
2. **테마 시스템 확장**: 다크 모드, 색상 커스터마이징
3. **성능 최적화**: 메모리 사용량 최적화

---

## 🎮 통합 후 최종 모습

```
Status & Control Tab
├── 뷰 모드 선택
│   ├── 📊 Classic View    (기존 UI 유지)
│   ├── 🏙️ City View      (도시 대시보드)
│   ├── 🎮 3D View        (3D 시각화)
│   └── 📈 Metrics View   (차트 중심)
├── 크롤링 제어
│   ├── 운영 프로파일 선택
│   ├── 실시간 파라미터 조정
│   └── 즉시 효과 반영
└── 상태 모니터링
    ├── 실시간 데이터 연동
    ├── 예측 분석 정보
    └── 성능 히스토리
```

---

## 🔧 즉시 실행 가능한 첫 번째 단계

### 1. StatusTab 뷰 모드 통합 (1일)
```tsx
// components/tabs/StatusTab.tsx
export const StatusTab = () => {
  const [viewMode, setViewMode] = createSignal<'classic' | 'city' | '3d' | 'metrics'>('classic');
  
  return (
    <div>
      <ViewModeSelector value={viewMode()} onChange={setViewMode} />
      <Show when={viewMode() === 'classic'}>
        <ClassicStatusView />
      </Show>
      <Show when={viewMode() === 'city'}>
        <CrawlingCityDashboard />
      </Show>
      {/* ... 기타 뷰 모드 */}
    </div>
  );
};
```

### 2. 상태 관리 통합 (1일)
```tsx
// stores/crawlingStore.ts
export const crawlingStore = createStore({
  // 기존 상태 + 게임 대시보드 상태 통합
  status: 'idle',
  progress: { /* ... */ },
  buildings: [ /* ... */ ],
  metrics: { /* ... */ }
});
```

이렇게 하면 **기존 사용자 경험을 해치지 않으면서도** 새로운 게임 스타일 시각화를 점진적으로 통합할 수 있습니다.

**어떤 방향으로 진행하시겠습니까?** 즉시 실행 가능한 첫 번째 단계부터 시작할까요?
