# rMatterCertis 가이드 문서 구조

이 디렉토리는 향후 개발과 유지보수에 필요한 핵심 문서들만을 포함합니다.

## 🎯 핵심 참조 문서 (항상 유지)

### 1. 프로젝트 목표 및 요구사항
- **`matter-certis-v2-requirements.md`** - 프로젝트의 핵심 요구사항과 달성 목표
- **`re-arch-plan.md`** - 새 아키텍처 구축을 위한 최종 실행 계획

### 2. 도메인 지식 및 개발 가이드
- **`matter-certis-v2-core-domain-knowledge.md`** - 크롤링 도메인의 핵심 비즈니스 로직
- **`matter-certis-v2-development-guide.md`** - Rust/Tauri 개발을 위한 종합 가이드
- **`matter-certis-v2-frontend-domain-knowledge.md`** - 프론트엔드 도메인 지식
- **`matter-certis-v2-html-parsing-guide.md`** - HTML 파싱 로직 가이드

### 3. 기술 설계 문서
- **`DATABASE_SCHEMA.md`** - 데이터베이스 스키마 설계
- **`SolidJS-UI-Implementation-Guide.md`** - SolidJS UI 구현 가이드
- **`re-arch-by-gem.md`** - Trait 기반 아키텍처 설계 (참고용)

### 4. 개발 가이드라인
- **`DOCUMENTATION_GUIDELINES.md`** - 문서 작성 및 관리 가이드라인

### 5. 특화 도메인 폴더
- **`crawling/`** - 크롤링 관련 세부 구현 가이드
- **`work_perf/`** - 성능 최적화 관련 문서

## 📚 Archive된 문서들

### `archive/proposals/` - 과거 제안서들
- proposal1-9.md 등 - 개발 과정에서 작성된 다양한 제안서들

### `archive/status_reports/` - 구현 상태 보고서들  
- 과거 구현 현황과 분석 보고서들
- 아키텍처 개요 문서들
- 갭 분석 보고서들

### `archive/legacy_plans/` - 과거 계획 문서들
- 이전 로드맵과 리팩토링 계획들
- 기술적 개선 방안들
- 마이그레이션 가이드들

### `archive/architecture_iterations/` - 아키텍처 설계 반복들
- re-arch 시리즈 문서들 (v1-v4)
- 이벤트 기반 아키텍처 설계들

## 🎯 향후 개발 시 참조 우선순위

### 1순위 (필수 참조)
1. `re-arch-plan.md` - 새 아키텍처 구현 계획
2. `matter-certis-v2-core-domain-knowledge.md` - 도메인 지식
3. `matter-certis-v2-development-guide.md` - 개발 가이드

### 2순위 (기능 구현 시 참조)
1. `DATABASE_SCHEMA.md` - DB 작업 시
2. `matter-certis-v2-html-parsing-guide.md` - 파싱 로직 구현 시
3. `SolidJS-UI-Implementation-Guide.md` - UI 개발 시

### 3순위 (필요 시 참조)
1. `re-arch-by-gem.md` - 아키텍처 설계 참고
2. Archive 문서들 - 과거 결정 사항 확인 시

## 📝 문서 관리 원칙

1. **핵심 문서는 guide/ 루트에 유지**
2. **참고용/과거 문서는 archive/에 보관**
3. **새로운 문서 추가 시 README.md 업데이트**
4. **더 이상 참조하지 않는 문서는 적극적으로 archive 처리**

이렇게 정리하여 개발자가 필요한 정보를 빠르게 찾을 수 있도록 구조화했습니다.
