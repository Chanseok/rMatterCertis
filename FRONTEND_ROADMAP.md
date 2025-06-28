# Matter Certis v2 - 프론트엔드 개발 로드맵

> 현재 상태: 백엔드 API 완료, 기본 SolidJS 구조 존재, 프론트엔드 도메인 가이드 완비
> 
> 목표: prompts와 matter-certis-v2-frontend-domain-knowledge.md를 기반으로 한 체계적인 SolidJS 프론트엔드 구현

## 📅 개발 단계별 계획

### Phase 1: 기반 구조 정비 (1-2일)

#### 1.1 디렉토리 구조 세분화
- [x] 기본 구조 존재 (components, stores, services, types, utils)
- [ ] `components/common/` - 재사용 UI 컴포넌트
- [ ] `components/features/` - 기능별 컴포넌트 
- [ ] `components/layout/` - 레이아웃 컴포넌트
- [ ] `stores/domain/` - 도메인별 스토어
- [ ] `viewmodels/` - 뷰모델 패턴 구현
- [ ] `platform/` - 플랫폼 추상화

#### 1.2 타입 정의 동기화
- [ ] `src/types/domain.ts` - 백엔드 DTO를 TypeScript 인터페이스로 변환
- [ ] `src/types/ui.ts` - UI 전용 타입 정의
- [ ] `src/types/platform.ts` - 플랫폼 API 타입 정의

### Phase 2: 기반 레이어 구현 (2-3일)

#### 2.1 플랫폼 API 추상화 계층
- [ ] `TauriApiAdapter` 클래스 구현
- [ ] `MethodParamsMapping`과 `MethodReturnMapping` 인터페이스
- [ ] 모든 Tauri commands 래핑 및 타입 안전성 확보

#### 2.2 상태 관리 시스템 (Stores)
- [ ] `crawling-store.ts` - 크롤링 세션 및 진행 상태 관리
- [ ] `vendor-store.ts` - 벤더 CRUD 및 목록 관리
- [ ] `product-store.ts` - 제품 데이터 관리
- [ ] `ui-store.ts` - UI 프리퍼런스 및 뷰 상태 관리

#### 2.3 공통 서비스 레이어
- [x] 기본 `api.ts`, `crawlingService.ts` 존재
- [ ] 각 서비스를 추상화된 API 어댑터로 재구성
- [ ] 에러 핸들링 및 로딩 상태 통합 관리

### Phase 3: 공통 컴포넌트 구현 (1주) ✅ COMPLETED

#### 3.1 기본 UI 컴포넌트 ✅ 완료
- [x] `Button.tsx` - 다양한 스타일과 상태를 지원하는 버튼
- [x] `ProgressBar.tsx` - 크롤링 진행률 표시
- [x] `Modal.tsx` - 설정, 삭제 확인 등의 모달
- [x] `Spinner.tsx` - 로딩 인디케이터
- [x] `Toast.tsx` - 알림 메시지

#### 3.2 복합 UI 컴포넌트 ✅ 완료 
- [x] `DataTable.tsx` - 가상 스크롤링 지원 테이블
- [x] `SearchFilter.tsx` - 검색 및 필터링
- [ ] `Pagination.tsx` - 페이지네이션 (DataTable에 통합)
- [ ] `CollapsibleSection.tsx` - 접을 수 있는 섹션

#### 3.3 테마 및 스타일 시스템
- [ ] CSS 변수 기반 테마 시스템 구현
- [ ] 다크/라이트 모드 지원
- [ ] 반응형 디자인 적용

### Phase 4: 핵심 기능 페이지 구현 (1-2주) ✅ Phase 4.1 완료

#### 4.1 Vendor 관리 (첫 번째 E2E 플로우) ✅ 완료
- [x] `VendorManagement.tsx` - 벤더 목록 및 CRUD
- [x] `VendorForm.tsx` - 벤더 생성/수정 폼
- [ ] `VendorCard.tsx` - 벤더 정보 카드
- [x] 첫 번째 완전한 프론트-백엔드 연동 완성

#### 4.2 크롤링 대시보드 
- [x] 기본 `CrawlingDashboard.tsx` 존재
- [ ] 실시간 진행 상태 표시 개선
- [ ] 다중 스토어 데이터 조합 (크롤링, 벤더, 제품)
- [ ] 시각화 컴포넌트 추가

#### 4.3 제품 뷰어
- [x] 기본 `CrawlingResults.tsx` 존재  
- [ ] 제품 상세 정보 표시 개선
- [ ] 필터링 및 검색 기능 강화
- [ ] 대량 데이터 처리 최적화

### Phase 5: 고급 기능 및 최적화 (지속적)

#### 5.1 성능 최적화
- [ ] `createMemo`를 활용한 계산 최적화
- [ ] 가상 스크롤링으로 대량 데이터 처리
- [ ] 지연 로딩 및 코드 분할

#### 5.2 사용자 경험 개선
- [ ] `ErrorBoundary` 구현
- [ ] `Suspense`를 활용한 로딩 상태 처리
- [ ] 오프라인 상태 감지 및 처리

#### 5.3 테스트 및 문서화
- [ ] 컴포넌트 단위 테스트 작성
- [ ] E2E 테스트 시나리오 구현
- [ ] 스토리북 또는 개발 문서 작성

## 🎯 우선순위 및 실행 전략

### 1주차: 기반 다지기
1. **타입 동기화** - 백엔드 DTO → TypeScript 인터페이스 변환
2. **API 추상화** - TauriApiAdapter 구현으로 플랫폼 독립성 확보
3. **스토어 기반** - vendor-store 우선 구현으로 첫 E2E 준비

### 2주차: 첫 E2E 완성
1. **공통 컴포넌트** - Button, Modal, DataTable 우선 구현
2. **Vendor 관리** - 가장 안정적인 백엔드 API를 활용한 완전한 CRUD
3. **검증 및 개선** - 첫 E2E 플로우 통해 아키텍처 검증

### 3주차 이후: 확장 및 최적화
1. **크롤링 대시보드** - 복합 데이터 시각화 구현
2. **제품 뷰어** - 대량 데이터 처리 최적화
3. **UX/성능** - 사용자 경험 및 성능 지속 개선

## 🎯 현재 진행 상황 업데이트

### ✅ 완료된 작업 (2024-12-29)

#### Phase 1: 기반 구조 정비 ✅ COMPLETED
- [x] 디렉토리 구조 세분화
  - [x] `src/types/domain.ts` - 백엔드 DTO와 완전 동기화된 TypeScript 인터페이스
  - [x] `src/types/ui.ts` - UI 전용 타입 정의 (컴포넌트 Props, 테마, 폼 등)
  - [x] `src/platform/tauri.ts` - Tauri API 추상화 계층 (TauriApiAdapter)
  - [x] `src/stores/domain/` - 도메인별 스토어 구조

#### Phase 2: 기반 레이어 구현 ✅ COMPLETED
- [x] **플랫폼 API 추상화 계층** (`src/platform/tauri.ts`)
  - [x] `TauriApiAdapter` 클래스 - 모든 Tauri commands의 타입 안전 래퍼
  - [x] `MethodParamsMapping`과 `MethodReturnMapping` - 완전한 타입 안전성
  - [x] 에러 정규화 및 배치 API 호출 지원
  - [x] 21개 백엔드 명령어 모두 매핑 완료

- [x] **상태 관리 시스템 (Stores)**
  - [x] `vendor-store.ts` - 벤더 CRUD, 검색, 필터링, 실시간 상태 관리
  - [x] `crawling-store.ts` - 크롤링 세션, 진행률, 자동 새로고침 관리
  - [x] `ui-store.ts` - UI 프리퍼런스, 테마, 모달, 검색 필터 상태
  - [x] `index.tsx` - 전역 스토어 컨텍스트 및 프로바이더

- [x] **타입 정의 동기화**
  - [x] 백엔드 DTO → TypeScript 인터페이스 완전 변환
  - [x] UI 컴포넌트용 타입 정의 체계 구축
  - [x] 에러 처리 및 API 응답 타입 정의

### 📋 2024-12-29 개발 세션 완료 요약

### ✅ 달성한 목표

prompts에서 제시된 프론트엔드 개발 전략을 바탕으로 다음 핵심 기반을 완성했습니다:

1. **타입 시스템 구축**: `src/types/domain.ts`와 `src/types/ui.ts`로 백엔드-프론트엔드 완전 동기화
2. **API 추상화**: `src/platform/tauri.ts`로 Tauri 의존성 격리 및 타입 안전 보장  
3. **상태 관리**: SolidJS 기반 반응형 스토어 시스템 (vendor, crawling, ui)
4. **아키텍처**: MVVM 패턴과 클린 아키텍처 원칙 적용

### 🎯 바로 다음 할 일

**Phase 3: 공통 컴포넌트 구현**부터 시작하세요.

```bash
# 우선순위 순서:
1. src/components/ui/Button.tsx 
2. src/components/ui/Modal.tsx  
3. src/components/features/VendorForm.tsx
4. src/components/features/VendorManagement.tsx
```

모든 기반 인프라가 준비되어 있어 UI 컴포넌트 개발에만 집중하면 됩니다.

### 🔧 사용할 패턴

- **스토어 사용**: `useVendorStore()`, `useCrawlingStore()`, `useUIStore()`
- **API 호출**: `apiAdapter.createVendor(dto)` 등 타입 안전 메서드
- **에러 처리**: `safeApiCall()` 래퍼 함수 활용
- **상태 관리**: `createStore()`와 `createSignal()` 조합

### 📚 참고 자료

- `guide/matter-certis-v2-frontend-domain-knowledge.md` - 컴포넌트 구현 패턴
- `src/stores/domain/vendor-store.ts` - 스토어 사용 예시
- `src/platform/tauri.ts` - API 호출 방법

## 🔄 다음 단계

**현재 위치**: Phase 1 완료 준비
**다음 작업**: 타입 동기화 (`src/types/domain.ts` 작성)부터 시작

이 로드맵은 prompts의 분석과 도메인 가이드를 바탕으로 작성되었으며, 실제 구현 과정에서 우선순위는 유연하게 조정될 수 있습니다.

### 🎯 다음 단계: Phase 3 - 공통 컴포넌트 구현 ✅ 기반 준비 완료

**✅ 2024-12-29 추가 달성 사항:**

- [x] **stores.ts 빌드 에러 해결**: JSX 문법 파일을 `.tsx` 확장자로 분리
- [x] **App.tsx 스토어 연동**: 새로운 스토어 시스템을 사용하도록 App 컴포넌트 재구성
- [x] **개발 서버 정상 실행**: 새로운 아키텍처로 http://localhost:1420/ 정상 구동
- [x] **벤더 관리 기본 UI**: Vendor CRUD 기능을 스토어와 연동한 기본 UI 완성

**🚀 현재 상태**: Phase 4.2 진입 - 크롤링 대시보드 고도화 차례
- 모든 스토어가 App.tsx에 연결되어 즉시 사용 가능
- **🆕 Vendor E2E 플로우 완성**: 전체 CRUD 기능을 포함한 완전한 첫 번째 기능 구현
- 타입 안전성과 에러 처리가 모두 구축됨
- **🆕 7개 UI 컴포넌트 완성**: Button, Modal, ProgressBar, Spinner, Toast, DataTable, SearchFilter
- **🆕 2개 기능 컴포넌트 완성**: VendorForm, VendorManagement

**📅 2024-12-29 Phase 4.1 완료 달성:**
- [x] **첫 번째 E2E 플로우**: Vendor 관리 시스템 완전 구현
- [x] **프론트-백엔드 연동**: 실제 vendor-store와 tauri API 완전 연결
- [x] **UI/UX 완성도**: 모달, 폼 유효성 검사, 토스트 알림, 로딩 상태 처리
- [x] **데이터 테이블**: 정렬, 행 클릭, 인라인 액션 버튼, 반응형 디자인
- [x] **타입 안전성**: 모든 컴포넌트와 스토어 간 완전한 타입 동기화
