# Gemini 제안 리팩토링 TODO 목록

> 생성일: 2025-10-09
> 원본 `refactoring-todo.md` 분석 기반

기존에 작성하신 `refactoring-todo.md`는 매우 훌륭하며, 프로젝트의 주요 리팩토링 포인트를 정확히 짚고 있습니다. 해당 문서를 기반으로 추가적인 관점에서 검토하여, 중복되거나 놓칠 수 있는 부분을 중심으로 새로운 제안 목록을 작성했습니다.

---

## 1. 아카이브 대상에 포함된 API 서비스 및 관련 로직

**문제점**:
`refactoring-todo.md`의 2.1 항목은 주로 UI 컴포넌트 아카이브에 초점을 맞추고 있습니다. 이와 직접적으로 연결된 API 서비스 및 데이터 관련 파일들도 명시적으로 함께 제거해야 합니다.

**대상 파일**:
- `archive/api/dashboard.ts`: 대시보드 탭에서 사용되던 API 라우팅 및 요청 핸들러.
- `archive/services/dashboardAPI.ts`: 대시보드 API를 호출하던 프론트엔드 서비스 레이어.
- `archive/hooks/useDashboardData.ts` (가정): 만약 대시보드 전용 커스텀 훅이 있었다면 이 역시 삭제 대상입니다.

**제안**:
- [ ] `archive` 디렉토리 삭제 시, 내부에 포함된 UI 컴포넌트 외 API 서비스, 훅, 타입 정의 등 모든 파일이 삭제 대상임을 명확히 인지하고 진행합니다.
- [ ] 관련 `Tauri` 커맨드가 백엔드에 아직 남아있다면 (예: `get_dashboard_stats`), `refactoring-todo.md`의 3.1 항목과 연계하여 함께 제거합니다.

---

## 2. 문서 및 가이드 전체의 일관성 검토

**문제점**:
`SolidJS-UI-Implementation-Guide.md` 외 다른 가이드 문서에도 삭제된 탭(Dashboard, Analysis, Actor System 등)에 대한 설명이 남아있을 가능성이 있습니다.

**대상 문서**:
- `docs/shallow_crawl_ui_guide.md`: Shallow Crawl 기능이 현재 UI와 어떻게 통합되었는지 명시하고, 오래된 UI 가이드는 제거해야 합니다.
- `docs/site_health_monitoring.md`: 사이트 상태 모니터링 방식이 변경되었다면 (예: 별도 탭 -> 크롤링 탭의 일부 기능), 해당 문서도 업데이트가 필요합니다.
- `guide/` 및 `docs/` 폴더 내의 모든 `.md` 파일에서 "Dashboard", "Analysis Tab", "Actor System" 등의 키워드로 검색하여 오래된 내용을 식별합니다.

**제안**:
- [ ] `grep` 이나 IDE의 전역 검색 기능을 사용하여 `docs/`, `guide/` 폴더에서 레거시 기능 관련 키워드를 검색합니다.
- [ ] 발견된 모든 문서의 내용을 현재 아키텍처와 3개 탭 UI에 맞게 수정하거나 "Deprecated" 섹션으로 이동시킵니다.

---

## 3. 중복 및 잠재적 미사용 스크립트

**문제점**:
프로젝트 루트와 `src-tauri`에 각각 `dev.sh` 파일이 존재하며, `scripts/` 폴더에 더 이상 사용되지 않을 수 있는 진단/테스트 스크립트가 있을 수 있습니다.

**대상 파일**:
- `dev.sh`
- `src-tauri/dev.sh`
- `scripts/diagnose_canonical_page.mjs`: `DiagnosticsPanel`의 기능과 중복될 수 있습니다.
- `scripts/test_site_structure.sh`: 오래된 사이트 구조를 가정하고 작성되었을 수 있습니다.
- `scripts/check_csa_list_pages.mjs`: 현재 크롤링 엔진 로직에 포함된 검증과 중복될 수 있습니다.

**제안**:
- [ ] 루트 `dev.sh`와 `src-tauri/dev.sh`의 내용을 비교 분석하여, 하나로 통합하거나 역할을 명확히 분리하고 문서화합니다. (예: 하나는 프론트엔드 전용, 다른 하나는 백엔드 전용)
- [ ] `scripts/` 폴더의 각 스크립트의 용도를 현재 개발 및 CI/CD 워크플로우와 비교하여, 더 이상 사용되지 않는 스크립트를 `scripts/_backups/`로 이동하거나 삭제합니다.

---

## 4. 레거시 상태 관리 로직 및 타입

**문제점**:
`tabStore.ts` 외 다른 스토어 파일이나 타입 정의 파일에도 삭제된 탭과 관련된 코드가 남아있을 수 있습니다.

**대상 파일**:
- `src/stores/`: `tabStore.ts` 외 다른 스토어 파일(예: `uiStore.ts`, `settingsStore.ts`)에 `Analysis`, `Dashboard` 관련 상태나 액션이 있는지 확인합니다.
- `src/types/`: 전역 타입 정의 파일에 `DashboardData`, `AnalyticsReport` 등 더 이상 사용되지 않는 타입이 있는지 확인합니다.
- `src/events/event-schemas.ts` (가정): 만약 이벤트 스키마가 정의되어 있다면, 레거시 UI에서만 사용되던 이벤트가 있는지 확인합니다.

**제안**:
- [ ] `ts-prune` 이나 `depcheck` 같은 도구를 사용하여 `src/stores`와 `src/types` 디렉토리 내에서 사용되지 않는 `export`를 다시 한번 확인합니다.
- [ ] 특히 `refactoring-todo.md`의 4.2 항목(`Unreachable Files`)에 언급된 컴포넌트들이 사용하던 스토어 로직이나 타입을 역추적하여 제거합니다.

---
