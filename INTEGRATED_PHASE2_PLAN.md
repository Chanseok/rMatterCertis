# INTEGRATED_PHASE2_PLAN.md - 통합 Phase 2 실행 계획

## 🎯 통합 접근 전략

**REFACTORING_PLAN.md의 Phase 2**와 **PHASE2_CRAWLING_ENHANCEMENT_PLAN.md**를 통합하여 체계적으로 진행

## 📅 Week-by-Week 통합 실행 계획

### **Week 1: 크롤링 엔진 통합 + 실시간 이벤트 기반**

#### Day 1-2: 크롤링 엔진 아키텍처 통합
- **PHASE2_CRAWLING**: CrawlerManager 구현
- **REFACTORING**: 실시간 이벤트 시스템 기반 구조

#### Day 3-4: 재시도 메커니즘 + 에러 처리 통합
- **PHASE2_CRAWLING**: RetryManager 구현
- **REFACTORING**: 계층별 에러 전파 전략

#### Day 5: 통합 테스트 및 검증
- 두 계획의 결과물 통합 테스트
- 실시간 이벤트 ↔ 크롤링 엔진 연동 확인

### **Week 2: 성능 최적화 + UI 반응성**

#### Day 1-3: 백엔드 성능 최적화
- **PHASE2_CRAWLING**: AdaptivePerformanceManager
- **REFACTORING**: 메모리 사용량 프로파일링

#### Day 4-5: 프론트엔드 최적화
- **REFACTORING**: SolidJS 컴포넌트 렌더링 최적화
- **PHASE2_CRAWLING**: 실시간 이벤트 버퍼링

### **Week 3: 완성 및 안정화**

#### Day 1-3: 최종 통합 및 최적화
- 전체 시스템 통합 테스트
- 성능 벤치마크 및 튜닝

#### Day 4-5: 문서화 및 검증
- 통합 결과 문서화
- Phase 3 준비

## 🔄 진행 순서 결정 기준

### **1순위: PHASE2_CRAWLING_ENHANCEMENT_PLAN 먼저** ✅

**이유:**
1. **의존성 순서**: 크롤링 엔진이 안정화되어야 프론트엔드 최적화 의미있음
2. **영향 범위**: 백엔드 변경사항이 프론트엔드 구조에 영향을 줄 수 있음
3. **테스트 용이성**: 크롤링 기능이 먼저 완성되어야 전체 시스템 테스트 가능

### **2순위: REFACTORING_PLAN Phase 2 연동**

**이유:**
1. **실시간 이벤트**: 크롤링 엔진의 이벤트를 프론트엔드가 받아야 함
2. **에러 처리**: 백엔드 에러를 프론트엔드에서 우아하게 처리
3. **성능 최적화**: 백엔드 최적화 → 프론트엔드 최적화 순서

## 🎯 즉시 시작할 작업

### **Step 1: 크롤링 엔진 통합 (오늘부터)**
```rust
// src-tauri/src/application/crawler_manager.rs (새 파일)
pub struct CrawlerManager {
    batch_processor: Arc<dyn BatchProcessor>,
    session_manager: Arc<SessionManager>,
    retry_manager: Arc<RetryManager>,
}
```

### **Step 2: 실시간 이벤트 연동**
```typescript
// src/stores/crawlerStore.ts 개선
async startRealTimeUpdates(): Promise<void> {
  // 세분화된 이벤트 구독
}
```

### **Step 3: 통합 테스트**
```bash
# 크롤링 엔진 + 실시간 이벤트 연동 테스트
npm run test:integration
```

## 📊 성공 지표

### Week 1 완료 기준
- [ ] CrawlerManager가 3개 엔진 통합 관리
- [ ] 재시도 메커니즘 동작 확인
- [ ] 실시간 이벤트 프론트엔드 연동

### Week 2 완료 기준  
- [ ] 성능 50% 향상 달성
- [ ] 메모리 사용량 30% 절약
- [ ] UI 반응성 100ms 이하

### Week 3 완료 기준
- [ ] 전체 시스템 안정성 95%+
- [ ] 사용자 피드백 긍정적
- [ ] Phase 3 준비 완료

---

**결론: PHASE2_CRAWLING_ENHANCEMENT_PLAN을 먼저 시작하되, REFACTORING_PLAN의 실시간 이벤트 요소를 동시에 진행하는 것이 가장 효율적입니다.**
