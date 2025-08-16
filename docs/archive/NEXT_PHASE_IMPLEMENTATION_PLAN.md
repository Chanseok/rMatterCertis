# 다음 단계 구현 계획: 아키텍처 완성 로드맵

> **기준 문서**: `re-arch-plan-final2.md` (목표 아키텍처), `re-arch-assess3.md` (현재 상태 분석)
> **생성일**: 2025-07-22
> **목표**: 아키텍처 문서의 설계를 완전히 실현하여 프로젝트의 모든 잠재력 발휘

## 🎯 Executive Summary

현재 프로젝트는 **Actor 시스템의 핵심 뼈대와 설정 통합을 성공적으로 완료**했습니다. 하지만 아키텍처의 진정한 가치를 실현하기 위해서는 다음 3가지 핵심 과제를 순차적으로 해결해야 합니다:

1. **🔥 Phase 1**: FE/BE 인터페이스 완전 현대화 (타입 안정성 확보)
2. **🧠 Phase 2**: CrawlingPlanner 지능형 시스템 구현 (자동 모드 활성화)  
3. **🧹 Phase 3**: Actor 모델 구조 개선 (Modern Rust 2024 완전 준수)

---

## 🔥 Phase 1: FE/BE 인터페이스 완전 현대화 (최우선 - 1-2일)

### 목표
**End-to-End 타입 안정성을 확보하여 모든 후속 개발의 견고한 기반을 마련**

### 현재 문제점
- 백엔드: `ts-rs` 준비 완료, 타입 자동 생성 가능
- 프론트엔드: 여전히 `src/types/crawling.ts` 등 수동 관리 레거시 타입에 100% 의존
- 결과: 타입 불일치, 런타임 버그, 불필요한 데이터 변환 로직

### Day 1: 설정 API 정리 및 타입 시스템 기반 구축

#### 1.1 불필요한 설정 API 완전 제거

**작업 대상 파일들**:
- ✅ `src-tauri/capabilities/default.json` - `get_frontend_config` 제거 완료
- `src/services/tauri-api.ts` - `getFrontendConfig()` 메서드 제거
- `src-tauri/src/commands/config_commands.rs` - 관련 command 완전 삭제

**원칙**: 아키텍처 설계 원칙 준수
- 백엔드: JSON 설정 파일만 읽어서 자율 운영
- 프론트엔드: 설정 파일만 편집, 백엔드로 설정 전송 금지

#### 1.2 ts-rs 타입 생성 파이프라인 강화

**현재 상태**: 
- ✅ `src-tauri/build.rs` 구현 완료
- ✅ `src/types/generated/` 디렉토리에 타입 자동 생성됨
- ✅ `tsconfig.json`에 경로 매핑 설정됨

**추가 작업**:
```bash
# 백엔드에서 더 많은 타입 export 활성화
cd src-tauri
cargo run --bin generate_types

# 프론트엔드 빌드 확인
cd ..
npm run build
```

### Day 2: 프론트엔드 타입 시스템 전환

#### 2.1 레거시 타입 파일 제거
```bash
# 수동 관리 타입 파일들 삭제
rm src/types/crawling.ts
rm src/types/advanced-engine.ts  # 필요시 일부만 제거
rm src/types/events.ts           # 자동 생성으로 대체 가능한 부분만
```

#### 2.2 Import 경로 전면 수정

**변경 예시**:
```typescript
// ❌ Before (레거시)
import type { CrawlingProgress } from '../types/crawling';

// ✅ After (자동 생성)
import type { CrawlingProgress } from '@/types';
```

**주요 수정 대상 파일들**:
- `src/stores/settingsStore.ts`
- `src/components/tabs/CrawlingEngineTab.tsx`
- `src/services/tauri-api.ts`

#### 2.3 데이터 변환 로직 제거

**`src/services/tauri-api.ts` 최적화**:
```typescript
// ❌ 제거할 불필요한 변환 로직
const transformApiResponse = (response: any) => {
  return {
    // 복잡한 데이터 변환...
  };
};

// ✅ 직접 사용 (타입이 이미 일치)
const response = await invoke<CrawlingProgress>('get_crawling_progress');
// 변환 없이 바로 사용 가능
```

---

## 🧠 Phase 2: CrawlingPlanner 지능형 시스템 구현 (2-3일)

### 목표
**시스템이 스스로 분석하고 계획을 수립하는 "자동 모드"를 활성화**

### 현재 상태
- ✅ 분석 기능들 (`crawling_integration.rs`) 구현 완료
- ❌ 이를 조합하는 `CrawlingPlanner` 컴포넌트 미구현
- 결과: 현재는 "수동 운전" 모드로만 동작

### 2.1 CrawlingPlanner 핵심 구조 구현

**파일**: `src-tauri/src/new_architecture/services/crawling_planner.rs`

```rust
use crate::new_architecture::services::crawling_integration::CrawlingIntegrationService;

pub struct CrawlingPlanner {
    integration_service: Arc<CrawlingIntegrationService>,
    config: Arc<Config>,
}

impl CrawlingPlanner {
    /// 🧠 지능형 계획 수립: 3가지 정보 종합 분석
    pub async fn create_execution_plan(&self) -> Result<ExecutionPlan, PlanningError> {
        // 1. 사이트 상태 분석
        let site_analysis = self.integration_service
            .execute_site_analysis()
            .await?;
            
        // 2. 데이터베이스 상태 분석  
        let db_analysis = self.integration_service
            .get_database_analysis()
            .await?;
            
        // 3. 크롤링 추천 계산
        let recommendation = self.integration_service
            .calculate_crawling_recommendation(&site_analysis, &db_analysis)
            .await?;
            
        // 4. 최적 배치 계획 생성
        let batches = self.create_optimal_batches(&recommendation).await?;
        
        Ok(ExecutionPlan {
            batches,
            estimated_duration: self.estimate_duration(&batches),
            total_items_expected: self.estimate_items(&batches),
        })
    }
    
    /// 도메인 지식 기반 배치 최적화
    async fn create_optimal_batches(&self, recommendation: &CrawlingRangeRecommendation) -> Result<Vec<BatchPlan>, PlanningError> {
        // 페이지 범위와 시스템 상태를 고려한 최적 배치 크기 결정
        let optimal_batch_size = self.calculate_optimal_batch_size(&recommendation).await?;
        
        // 배치들로 분할
        let mut batches = Vec::new();
        for page_range in recommendation.recommended_ranges.chunks(optimal_batch_size) {
            batches.push(BatchPlan {
                pages: page_range.to_vec(),
                batch_config: self.create_batch_config(&recommendation).await?,
            });
        }
        
        Ok(batches)
    }
}
```

### 2.2 SessionActor와 연동

**파일**: `src-tauri/src/new_architecture/actor_system.rs` 수정

```rust
impl SessionActor {
    async fn handle_start_crawling(&mut self, _cmd: StartCrawling) -> ActorResult {
        // ❌ 기존: 정적 파라미터 사용
        // let batch_plan = BatchPlan::from_static_params(cmd.pages, cmd.batch_size);
        
        // ✅ 새로운: CrawlingPlanner로 동적 계획 수립
        let planner = CrawlingPlanner::new(
            self.integration_service.clone(),
            self.config.clone()
        );
        
        let execution_plan = planner.create_execution_plan().await
            .map_err(|e| ActorError::PlanningFailed(e.to_string()))?;
            
        // 계획에 따라 BatchActor들 실행
        for batch_plan in execution_plan.batches {
            let result = self.execute_batch(batch_plan).await?;
            // 결과 처리...
        }
        
        Ok(())
    }
}
```

### 2.3 지능형 의사결정 알고리즘 구현

**도메인 지식 적용**:
- **사이트 응답 속도** → 배치 크기 조정
- **DB 오류 패턴** → 재시도 정책 조정  
- **데이터 현황** → 증분/복구/전체 크롤링 전략 선택

---

## 🧹 Phase 3: Actor 모델 구조 개선 (1-2일)

### 목표
**Modern Rust 2024 가이드라인 완전 준수 및 코드 품질 향상**

### 3.1 Actor 파일 분리

**현재**: 모든 Actor가 `actor_system.rs` 한 파일에 혼재  
**목표**: 각 Actor를 개별 파일로 분리

```bash
src-tauri/src/new_architecture/actors/
├── session.rs      # SessionActor
├── batch.rs        # BatchActor  
├── stage.rs        # StageActor
└── lib.rs          # 재export
```

### 3.2 AsyncTask 추상화 (선택적)

**현재**: `StageActor` 내부에 작업 처리 로직 혼재  
**목표**: 최소 실행 단위 별도 추상화

```rust
pub trait AsyncTask: Send + Sync {
    type Input;
    type Output;
    
    async fn execute(&self, input: Self::Input) -> Result<Self::Output, TaskError>;
}

impl StageActor {
    async fn process_item(&mut self, item: CrawlingItem) -> StageResult {
        let task = ProductDetailTask::new(self.http_client.clone());
        task.execute(item).await
    }
}
```

---

## 📋 구현 순서 및 우선순위

### Week 1: 인터페이스 현대화 (Phase 1)
- **Day 1**: 설정 API 정리, ts-rs 파이프라인 강화
- **Day 2**: 프론트엔드 타입 전환, 데이터 변환 로직 제거

### Week 2: 지능형 시스템 (Phase 2) 
- **Day 3-4**: CrawlingPlanner 핵심 구현
- **Day 5**: SessionActor 연동 및 테스트

### Week 3: 구조 개선 (Phase 3)
- **Day 6**: Actor 파일 분리
- **Day 7**: AsyncTask 추상화 (필요시)

---

## 🎯 성공 지표

### Phase 1 완료 기준
- [ ] 프론트엔드에서 자동 생성된 타입만 사용 (레거시 타입 0개)
- [ ] `get_frontend_config` 등 불필요한 API 완전 제거
- [ ] 타입 불일치 컴파일 에러 0건

### Phase 2 완료 기준  
- [ ] UI에서 "크롤링 시작" → 시스템이 자동으로 분석 → 최적 계획 수립 → 실행
- [ ] 사이트 상태 변화 시 배치 크기 자동 조정
- [ ] 데이터베이스 현황에 따른 크롤링 전략 자동 선택

### Phase 3 완료 기준
- [ ] 각 Actor가 개별 파일에 위치 (Modern Rust 2024 준수)
- [ ] Clippy pedantic 경고 0건
- [ ] 코드 가독성 및 유지보수성 향상

---

## 🚧 리스크 관리

### 주요 리스크
1. **타입 전환 중 빌드 에러**: 점진적 전환으로 해결
2. **CrawlingPlanner 복잡도**: 기존 검증된 로직 재사용으로 최소화
3. **Actor 분리 중 참조 문제**: 명확한 인터페이스 정의로 예방

### 대응 전략
- 각 Phase별 독립적 구현으로 롤백 가능
- 단위 테스트 우선 작성
- 기존 동작 기능 보존 우선

---

## 🎉 최종 목표

이 계획 완료 후 달성되는 상태:
- **완전한 타입 안정성**: Rust ↔ TypeScript 간 타입 불일치 0%
- **지능형 크롤링**: 시스템이 스스로 분석하고 최적 계획 수립
- **Modern Rust 아키텍처**: 2024 가이드라인 100% 준수
- **유지보수 용이성**: 명확한 책임 분리와 모듈화 구조

이를 통해 `re-arch-plan-final2.md`에서 설계한 아키텍처의 모든 잠재력이 완전히 실현될 것입니다.
