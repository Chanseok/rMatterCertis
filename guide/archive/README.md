# Archive 문서 색인

이 디렉토리는 개발 과정에서 생성된 참고용 문서들을 보관합니다.

## 📂 디렉토리 구조

### `proposals/` - 프로젝트 제안서들
- `proposal1.md` ~ `proposal9.md` - 시간순으로 작성된 프로젝트 제안서들
- 각 제안서는 당시의 아이디어와 기술적 접근 방법을 담고 있음

### `status_reports/` - 구현 상태 및 분석 보고서들
**아키텍처 분석:**
- `ARCHITECTURE_OVERVIEW.md`, `ARCHITECTURE_OVERVIEW_UPDATED.md` - 과거 아키텍처 분석
- `DESIGN_IMPLEMENTATION_GAP_ANALYSIS_2025_07_10.md` - 설계와 구현 간 차이 분석
- `database-schema-comparison.md` - 데이터베이스 스키마 비교

**현황 보고서:**
- `CURRENT_IMPLEMENTATION_STATUS_2025_07_05.md` - 7월 5일 기준 구현 현황
- `CURRENT_IMPLEMENTATION_STATUS_AND_STRATEGY.md` - 구현 현황과 전략
- `implementation_gap_analysis.md` - 구현 갭 분석
- `integrated-schema-implementation-status.md` - 통합 스키마 구현 현황

**문서화 관련:**
- `DOCUMENTATION_VS_IMPLEMENTATION_GAP_ANALYSIS.md` - 문서와 구현 간 갭 분석
- `DOCUMENTATION_SYNC_CHECKLIST.md` - 문서 동기화 체크리스트

**기타 분석:**
- `GAP_ZERO_EXECUTION_PLAN.md` - 갭 제로 실행 계획
- `back_front_log_analysis.md` - 백엔드/프론트엔드 로그 분석
- `live_production_line_event_analysis.md` - 라이브 프로덕션 라인 이벤트 분석

### `legacy_plans/` - 과거 계획 및 로드맵들
**로드맵 및 계획:**
- `BACKEND_V4_IMPLEMENTATION_PLAN.md` - 백엔드 V4 구현 계획
- `BACKEND_VALIDATION_PLAN.md` - 백엔드 검증 계획
- `FRONTEND_ROADMAP.md` - 프론트엔드 로드맵
- `INTEGRATED_PHASE2_PLAN.md` - 통합 Phase 2 계획
- `LIVE_PRODUCTION_LINE_IMPLEMENTATION_PLAN.md` - 라이브 프로덕션 라인 구현 계획
- `PHASE2_CRAWLING_ENHANCEMENT_PLAN.md` - Phase 2 크롤링 개선 계획
- `REFACTORING_PLAN.md` - 리팩토링 계획

**구현 요약:**
- `FINAL_IMPLEMENTATION_SUMMARY.md` - 최종 구현 요약
- `IMPLEMENTATION_SUMMARY_v7.md` - 구현 요약 v7
- `STATUS_CHECK_IMPLEMENTATION.md` - 상태 체크 구현

**기술적 개선:**
- `crawling_fix_plan.md` - 크롤링 수정 계획
- `crawling_range_fix_analysis.md` - 크롤링 범위 수정 분석
- `fast-compilation-optimization.md` - 빠른 컴파일 최적화
- `logging-optimization-report.md` - 로깅 최적화 보고서
- `matter-certis-v2-configuration-management-analysis.md` - 설정 관리 분석

**가이드 문서들:**
- `matter-certis-v2-phase4-5-guide.md` - Phase 4-5 가이드
- `matter-certis-v2-project-setup-checklist.md` - 프로젝트 설정 체크리스트

### `architecture_iterations/` - 아키텍처 설계 반복들
**Re-architecture 시리즈:**
- `re-arch-claude.md` - Claude 버전의 아키텍처 설계
- `re-arch-red.md` - 초기 아키텍처 재설계
- `re-arch-by-sonnet4.md` - Sonnet4 버전의 복잡한 마이그레이션 접근법 (사용자가 거부함)

**이벤트 기반 아키텍처:**
- `event-driven-crawling-architecture.md` ~ `event-driven-crawling-architecture4.md` - 이벤트 기반 크롤링 아키텍처 설계 반복

**엔진 설계:**
- `crawling-engine-design-analysis.md` - 크롤링 엔진 설계 분석

**기타 아키텍처 제안:**
- `interesting_visual_proposal.md` - 시각적 제안서

## 🔍 문서 찾기 가이드

### 특정 주제별 문서 위치:
- **아키텍처 설계:** `architecture_iterations/`
- **구현 현황 확인:** `status_reports/CURRENT_IMPLEMENTATION_STATUS_*.md`
- **과거 계획 확인:** `legacy_plans/`
- **초기 아이디어 확인:** `proposals/proposal*.md`
- **크롤링 관련:** `legacy_plans/crawling_*.md`, `architecture_iterations/event-driven-*.md`

### 시간순 문서 추적:
1. **제안 단계:** `proposals/proposal1.md` → `proposals/proposal9.md`
2. **초기 아키텍처:** `architecture_iterations/re-arch-red.md`
3. **구현 단계:** `status_reports/CURRENT_IMPLEMENTATION_STATUS_*.md`
4. **최종 계획:** 현재 `guide/re-arch-plan.md` (아카이브 외부)

## ⚠️ 주의사항

- 이 문서들은 **참고용**이며, 현재 개발에는 `guide/` 디렉토리의 문서들을 우선 참조하세요
- 과거 결정 사항이나 변경 이유를 확인할 때만 이 아카이브를 참조하세요
- 새로운 개발은 항상 `guide/re-arch-plan.md`를 기준으로 하세요
