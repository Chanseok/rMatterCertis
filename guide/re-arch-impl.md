# 아키텍처 구현 계획: 현실적 우선순위 기반 로드맵

> **문서 목적:** `re-arch-plan-final2.md`의 설계와 `re-arch-assess2.md`의 Gap 분석을 바탕으로, 현재 구현 상황에서 가장 효과적인 순서로 아키텍처를 완성하기 위한 구체적인 실행 계획을 제시합니다.

## 1. 현재 상황 종합 분석 및 아키텍처 원칙 (2025-07-21 기준)

### 1.1. 핵심 아키텍처 원칙: 설정 파일 기반 완전 분리 🎯

**백엔드 (Rust)**:
- **JSON 설정 파일 기반 자율 운영**: `~/Library/Application Support/matter-certis-v2/matter_certis_config.json` 파일의 모든 설정값을 읽어 크롤링 계획 수립 및 실행
- **완전 자율적 동작**: 프론트엔드로부터 어떤 설정값도 받지 않음
- **크롤링 세션 시작 시 설정 고정**: 한 번 크롤링이 시작되면 해당 세션 동안 설정값 변경 불가

**프론트엔드 (SolidJS)**:
- **설정 편집 전용**: `matter_certis_config.json` 파일의 내용만 편집하고 저장
- **상태 표시 전용**: 백엔드의 크롤링 진행 상황과 결과만 실시간 표시
- **설정 전송 금지**: 백엔드로 설정값을 전송하는 API 호출 완전 제거
- **크롤링 중 설정 편집 차단**: 크롤링 세션이 활성화된 동안 설정 UI 비활성화로 사용자 혼란 방지

### 1.2. 구현 완료된 핵심 요소들 ✅
- **Actor 시스템 뼈대**: `SessionActor`, `BatchActor`, `StageActor`의 기본 구조 구현
- **채널 기반 통신**: `oneshot` 채널을 통한 상위-하위 Actor 간 결과 보고 메커니즘
- **크롤링 범위 계산**: 데이터베이스 상태 기반 동적 범위 계산 로직 (`CrawlingRangeCalculator`)
- **JSON 설정 시스템**: `matter_certis_config.json` 기반 사용자/고급/앱 관리 설정 구조
- **타입 안정성 기반**: `ts-rs` 준비 및 백엔드 타입 정의

### 1.3. 즉시 해결 필요한 문제들 🚨
- **설정 전송 API 제거**: `get_frontend_config` 등 불필요한 설정 API 완전 삭제
- **프론트엔드 의존성 정리**: 설정 로딩 실패로 인한 크롤링 기능 마비 해결
- **타입 불일치**: 프론트엔드가 레거시 타입 사용, 백엔드의 `ts-rs` 미활용

### 1.3. 설계 대비 미구현 요소들 ⏳
- **`CrawlingPlanner`**: 동적 계획 수립 시스템
- **`MetricsAggregator`**: 실시간 진행률 집계
- **통합 이벤트 시스템**: 분산된 이벤트 처리의 통합

## 2. 우선순위별 구현 계획

### **Phase 1: 즉시 안정화 (1-2일)** 🔥
> **목표**: 현재 동작하지 않는 기능들을 빠르게 수정하여 안정적인 기반 확보

#### 1.1. 설정 전송 API 완전 제거

**현재 문제**: 
```typescript
// ❌ 제거 대상: 백엔드에서 설정을 가져오는 불필요한 API
const loadUserConfig = async () => {
  const response = await invoke<any>('get_frontend_config'); // 완전 삭제 필요
}
```

**해결 방향**:
```typescript
// ✅ 목표: 설정 없이 즉시 크롤링 가능
const startCrawling = async () => {
  // 백엔드가 matter_certis_config.json 파일을 자동으로 읽어서 크롤링 시작
  const response = await invoke<CrawlingSession>('start_smart_crawling');
}
```

**구체적 작업**:
1. **백엔드 API 삭제**: `get_frontend_config`, `set_frontend_config` 등 모든 설정 전송 API 제거
2. **설정 자동 로딩**: 백엔드에서 시작 시 `matter_certis_config.json` 파일을 자동으로 읽어 내부 설정으로 사용
3. **프론트엔드 의존성 제거**: `CrawlingEngineTab.tsx`에서 `userConfig` 관련 모든 코드 삭제
4. **크롤링 중 설정 차단**: 크롤링 세션이 활성화된 동안 설정 편집 UI 완전 비활성화

#### 1.2. 설정 파일 편집 UI 구현
**목표**: 프론트엔드는 오직 `matter_certis_config.json` 파일만 편집하되, **크롤링 세션 중에는 편집 차단**

```typescript
// ✅ 프론트엔드의 유일한 설정 관련 기능
const saveConfigFile = async (content: string) => {
  // 크롤링 진행 중이면 저장 차단
  if (isRunning()) {
    throw new Error("크롤링 진행 중에는 설정을 변경할 수 없습니다.");
  }
  
  await invoke('save_config_file', { content });
  // 다음 크롤링 세션에서 새 설정 적용됨
}

// ✅ 크롤링 상태에 따른 UI 제어
const isConfigEditable = () => !isRunning();
```

#### 1.2. 타입 안정성 기본 적용
**구현 작업**:
1. `src-tauri/build.rs`에 `ts-rs` 자동 생성 설정
2. 프론트엔드에서 자동 생성된 타입만 사용하도록 import 경로 수정
3. 기존 `src/types/advanced-engine.ts`를 생성된 타입으로 교체

### **Phase 2: 인터페이스 현대화 (3-5일)** 🎯
> **목표**: 프론트엔드-백엔드 간 완전한 타입 안정성과 효율적인 통신 구조 구축

#### 2.1. 설정 파일 기반 백엔드 명령어

**핵심 원칙**: 모든 명령어는 설정 파일만 사용, 파라미터 전송 금지

```rust
```rust
// ✅ JSON 설정 파일 기반 자율 크롤링
#[tauri::command]
pub async fn start_smart_crawling() -> Result<CrawlingSession, String> {
    // 1. matter_certis_config.json 파일에서 모든 설정 로딩
    let config = ConfigManager::load_from_json().await?;
    
    // 2. 설정 기반 사이트 분석 → 범위 계산 → 크롤링 시작
    let planner = CrawlingPlanner::new(config);
    let plan = planner.create_execution_plan().await?;
    
    // 3. Actor 시스템으로 실행 (설정은 세션 동안 고정)
    let session = SessionActor::start_with_plan(plan).await?;
    Ok(session)
}

// ✅ 설정 파일 편집 전용 (프론트엔드용)
#[tauri::command] 
pub async fn save_config_file(content: String) -> Result<(), String> {
    // 크롤링 진행 중이면 저장 거부
    if is_crawling_active().await {
        return Err("크롤링 진행 중에는 설정을 변경할 수 없습니다.".to_string());
    }
    
    // matter_certis_config.json 파일만 편집 허용
    let config_path = get_config_file_path()?;
    fs::write(config_path, content)?;
    
    // 다음 크롤링 세션에서 적용됨 (현재 세션은 영향 없음)
    Ok(())
}

// ✅ 상태 조회만 (설정 전송 없음)
#[tauri::command]
pub async fn get_crawling_status() -> Result<CrawlingStatus, String> {
    // 현재 진행 상황만 반환, 설정값 포함하지 않음
}
```

#### 2.2. 통합 이벤트 시스템 구현
```rust
// 단일 이벤트 채널로 모든 상태 업데이트 전송
#[derive(Serialize, ts_rs::TS)]
pub enum CrawlingEvent {
    Progress { session_id: String, progress: f64, message: String },
    Completed { session_id: String, stats: CrawlingStats },
    Error { session_id: String, error: String },
}
```

### **Phase 3: 지능형 시스템 구현 (5-7일)** 🧠
> **목표**: 설계의 핵심인 동적 계획 수립과 자동 최적화 기능 구현

#### 3.1. `CrawlingPlanner` 구현
기존 구현된 분석 기능들을 조합하여 지능형 계획 수립:
```rust
pub struct CrawlingPlanner {
    site_analyzer: Arc<SiteAnalyzer>,
    db_analyzer: Arc<DatabaseAnalyzer>,
    config: Arc<Config>,
}

impl CrawlingPlanner {
    pub async fn create_execution_plan(&self) -> Result<ExecutionPlan, PlanningError> {
        // 1. 사이트 상태 분석
        let site_status = self.site_analyzer.analyze().await?;
        
        // 2. 데이터베이스 상태 분석  
        let db_status = self.db_analyzer.analyze_progress().await?;
        
        // 3. 최적 크롤링 범위 및 배치 계획 수립
        let batches = self.calculate_optimal_batches(site_status, db_status)?;
        
        Ok(ExecutionPlan { batches, estimated_duration: /* ... */ })
    }
}
```

#### 3.2. Actor 시스템과 Planner 통합
```rust
impl SessionActor {
    async fn handle_start_crawling(&mut self, _: StartCrawling) -> ActorResult {
        // 1. Planner로 실행 계획 수립
        let plan = self.planner.create_execution_plan().await?;
        
        // 2. 계획에 따라 BatchActor들 순차 실행
        for batch_plan in plan.batches {
            let result = self.execute_batch(batch_plan).await?;
            // 결과에 따라 다음 배치 조정 가능
        }
    }
}
```

### **Phase 4: 모니터링 및 최적화 (3-5일)** 📊
> **목표**: 실시간 성능 모니터링과 적응형 최적화 구현

#### 4.1. `MetricsAggregator` 구현
```rust
pub struct MetricsAggregator {
    event_receiver: broadcast::Receiver<CrawlingEvent>,
    metrics_sender: broadcast::Sender<AggregatedMetrics>,
}

impl MetricsAggregator {
    async fn run(&mut self) {
        while let Ok(event) = self.event_receiver.recv().await {
            let metrics = self.calculate_metrics(event);
            let _ = self.metrics_sender.send(metrics);
        }
    }
}
```

#### 4.2. 적응형 성능 조정
- 실시간 응답 시간 모니터링
- 동시성 레벨 자동 조정
- 오류율 기반 백오프 전략

## 3. 구현 세부 가이드

### 3.1. Phase 1 구체적 작업 항목

**Day 1: 프론트엔드 안정화**
```bash
# 1. 설정 의존성 제거
src/components/tabs/CrawlingEngineTab.tsx
- loadUserConfig() 함수 제거
- userConfig 상태 제거  
- 설정 표시 UI 간소화
- 크롤링 중 설정 편집 차단 UI 추가

# 2. 간단한 크롤링 시작 구현
src-tauri/src/commands/simple_crawling.rs
- start_smart_crawling 명령어 추가
- 기존 calculate_crawling_range + 크롤링 시작 통합
```

**Day 2: 타입 시스템 기본 적용**
```bash
# 1. ts-rs 빌드 설정
src-tauri/build.rs
- 타입 자동 생성 설정 추가

# 2. 프론트엔드 타입 교체  
src/types/generated/ 
- 자동 생성된 타입으로 교체
- import 경로 수정
```

### 3.2. 성공 지표 및 검증 방법

**Phase 1 완료 기준**:
- [ ] UI에서 "크롤링 시작" 버튼 클릭 시 즉시 크롤링 시작
- [ ] 설정 로딩 오류 메시지 완전 제거
- [ ] 크롤링 범위가 UI에 정확히 표시
- [ ] 크롤링 세션 중 설정 편집 UI 완전 비활성화

**Phase 2 완료 기준**:
- [ ] 프론트엔드-백엔드 간 타입 불일치 오류 0건
- [ ] 단일 이벤트 채널로 모든 상태 업데이트 수신
- [ ] 크롤링 중 실시간 진행률 정확한 표시

**Phase 3 완료 기준**:
- [ ] 사이트 상태 변화 시 자동으로 최적 크롤링 계획 수립
- [ ] 데이터베이스 상태에 따른 중복 방지 정확도 99% 이상
- [ ] 배치 크기 자동 조정으로 성능 20% 이상 향상

## 4. 리스크 관리 및 대안 계획

### 4.1. 주요 리스크와 대응책

**리스크 1**: Actor 시스템 복잡도로 인한 디버깅 어려움
- **대응책**: 각 단계별로 충분한 로깅과 단위 테스트 작성
- **대안**: 필요시 Actor 모델 대신 Service 패턴으로 단순화

**리스크 2**: 성능 최적화로 인한 안정성 저하  
- **대응책**: 보수적인 기본값 설정, 점진적 최적화
- **대안**: 수동 설정 옵션 유지

### 4.2. 단계별 롤백 계획

각 Phase는 독립적으로 구현되어, 문제 발생 시 이전 단계로 안전하게 롤백 가능합니다.

## 5. 결론 및 핵심 원칙

### 5.1. 핵심 설계 원칙 (재강조)

1. **설정 파일 기반 완전 분리**: 
   - 백엔드는 `matter_certis_config.json`만 읽고, 프론트엔드는 이 파일만 편집
   - 프론트엔드 → 백엔드 설정 전송 API는 완전히 제거

2. **크롤링 세션 중 설정 불변성**:
   - 크롤링이 시작되면 해당 세션 동안 설정값 고정
   - UI에서 크롤링 중에는 설정 편집 완전 차단
   - 사용자 혼란 방지 및 크롤링 일관성 보장

3. **단순성 우선**:
   - 파일 시스템 감시, 실시간 설정 적용 등 복잡한 기능 제거
   - 명확하고 예측 가능한 동작만 구현

### 5.2. 권장 실행 순서

현재 상황에서는 **Phase 1의 즉시 안정화**가 가장 중요합니다. 설계의 완성도보다는 **현재 동작하지 않는 기능들을 빠르게 수정**하여 안정적인 개발 기반을 마련하는 것이 우선되어야 합니다.

그 다음으로 **Phase 2의 인터페이스 현대화**를 통해 프론트엔드-백엔드 간의 견고한 연결을 구축하고, 마지막으로 **Phase 3, 4의 지능형 기능**을 추가하여 설계의 모든 잠재력을 실현하는 순서가 가장 현실적이고 효과적일 것입니다.

이 계획을 통해 약 2-3주 내에 안정적이고 지능적인 크롤링 시스템을 완성할 수 있을 것으로 예상됩니다.
