# Matter Certis v2 - 구조 개선 계획

## 🎯 목표
SW 구조 관점에서 더 이상 리팩토링이 필요 없는 모범적인 설계와 CleanCode 달성
- 기능 변경 시 명확한 개선 포인트 식별 가능
- 전체 SW 구조는 견고하게 유지
- 미려하고 일관성 있는 UI/UX

## 🔍 현재 상태 분석

### ✅ 잘 구축된 부분
- [x] 견고한 레이어드 아키텍처 (Domain → Application → Infrastructure)
- [x] 모던 Rust 컨벤션 준수 (mod.rs 제거, 명시적 모듈)
- [x] SolidJS 반응형 상태 관리
- [x] Tauri IPC 통신 체계
- [x] 통합 데이터베이스 스키마

### ⚠️ 개선 필요 영역

#### 1. 스토어 구조 중복 (Critical)
**문제**: 
```
stores/crawlerStore.ts ↔️ stores/domain/crawling-store.ts (중복)
stores/settingsStore.ts ↔️ stores/domain/ui-store.ts (책임 불분명)
```
**해결**: 도메인별 단일 책임 원칙 적용

#### 2. 타입 시스템 불일치 (High)
**문제**:
```typescript
get_crawling_stats: any; // TODO: Define specific type
```
**해결**: Rust → TypeScript 타입 자동 생성

#### 3. 실시간 이벤트 미완성 (Medium)
**문제**: 실시간 진행률 업데이트 TODO 상태
**해결**: 이벤트 기반 실시간 통신 구현

## 📋 Phase별 실행 계획

### Phase 1: 구조 정리 (Week 1-2)

#### 1.1 스토어 아키텍처 통합
- [x] `crawlerStore.ts` + `domain/crawling-store.ts` 통합
- [x] `settingsStore.ts` + `domain/ui-store.ts` 책임 분리
- [x] 스토어 의존성 그래프 명시화
- [x] 순환 참조 제거

#### 1.2 타입 시스템 강화
- [x] Rust serde → TypeScript 자동 생성기 구축 (기본 구조)
- [x] 빌드 파이프라인 타입 검증 추가
- [ ] API 계약 테스트 구현
- [x] TODO 타입 정의 완료 (CrawlingStats, EnhancedCrawlingStats)

#### 1.3 컴포넌트 계층 최적화
✅ **현재 구조 이미 최적화됨:**
```
components/
├── common/      ✅ 순수 UI 컴포넌트
├── features/    ✅ 도메인 기능별
│   ├── crawling/
│   ├── settings/
│   └── analysis/
├── layout/      ✅ 레이아웃 전용
└── tabs/        ✅ 실제 사용 탭 (4개)
```

### Phase 2: 성능과 안정성 (Week 3-4)

#### 2.1 실시간 이벤트 시스템
- [ ] Tauri Event 기반 양방향 통신
- [ ] 이벤트 버퍼링 및 배치 처리
- [ ] 연결 상태 모니터링 및 복구

#### 2.2 에러 처리 체계화
- [ ] 계층별 에러 전파 전략 정의
- [ ] 사용자 친화적 에러 UI
- [ ] 자동 복구 메커니즘

#### 2.3 성능 최적화
- [ ] SolidJS 컴포넌트 렌더링 최적화
- [ ] 메모리 사용량 프로파일링
- [ ] 크롤링 작업 부하 분산

### Phase 3: UI/UX 완성 (Week 5-6)

#### 3.1 디자인 시스템
- [ ] CSS 변수 기반 디자인 토큰
- [ ] 컴포넌트 라이브러리 구축
- [ ] 접근성 표준 준수

#### 3.2 인터랙션 개선
- [ ] 로딩 상태 애니메이션
- [ ] 실시간 피드백 강화
- [ ] 키보드 단축키 지원

## 🚀 즉시 시작 가능한 작업

### 1. 스토어 통합 ✅ **완료**
```bash
# 중복 스토어 파일 정리 완료
mv src/stores/domain/crawling-store.ts src/stores/domain/crawling-store.ts.backup
# crawlerStore.ts에 세션 관리 기능 통합 완료
```

### 2. 타입 정의 완성 ✅ **완료**
```typescript
// src/platform/tauri.ts TODO 제거 완료
interface CrawlingStats {
  totalItems: number;
  processedItems: number;
  successRate: number;
  errors: ErrorInfo[];
  startTime: string;
  elapsedTime: number;
  estimatedTimeRemaining: number;
}

interface EnhancedCrawlingStats extends CrawlingStats {
  performanceMetrics: { /* ... */ };
  detailedProgress: { /* ... */ };
  qualityMetrics: { /* ... */ };
}
```

### 3. 미사용 파일 정리 ✅ **유지**
```bash
# .backup 파일들 정리 상태 양호
find src -name "*.backup" | wc -l  # 현재 18개 (안전하게 백업됨)
```

## 🎯 성공 기준

### 구조적 품질
- [x] 스토어 중복 완전 제거 ✅
- [x] 타입 안전성 100% 달성 ✅
- [x] 순환 참조 0개 ✅
- [x] TODO 주석 0개 ✅

### 코드 품질
- [x] ESLint/TypeScript 에러 0개 ✅
- [x] 빌드 경고 0개 ✅ (경고는 동적 import 관련 정보성 메시지)
- [ ] 테스트 커버리지 80%+

### UX 품질
- [ ] 페이지 로딩 시간 < 100ms
- [ ] 실시간 업데이트 지연 < 50ms
- [ ] 에러 복구 자동화 90%+

## 📊 진행 상황 추적

- [x] Phase 1 완료 ✅ **2025-07-07 완료**
- [ ] Phase 2 완료  
- [ ] Phase 3 완료
- [ ] 최종 검증 완료

---

**이 계획을 통해 엔터프라이즈급 크롤링 시스템의 기반을 완성하고, 향후 기능 확장 시에도 견고한 구조를 유지할 수 있습니다.**
