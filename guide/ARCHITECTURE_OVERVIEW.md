# Matter Certis v2 - Architecture Overview

## 목적
이 문서는 Matter Certis v2의 현재 아키텍처를 정확히 반영합니다. 실제 구현된 코드와 100% 일치하는 내용을 담고 있습니다.

## 전체 아키텍처

### 기술 스택
- **백엔드**: Rust + Tauri v2
- **프론트엔드**: SolidJS + TypeScript
- **데이터베이스**: SQLite (통합 스키마)
- **상태 관리**: SolidJS createStore
- **HTTP 클라이언트**: reqwest
- **HTML 파싱**: scraper
- **로깅**: tracing (기본 설정만 적용됨)

### 핵심 모듈 구조

```
src-tauri/src/
├── application/           # 비즈니스 로직 레이어
│   ├── crawling_use_cases.rs      # 크롤링 세션 관리 (실제 크롤링 로직 없음)
│   └── integrated_use_cases.rs    # 통합 DB 조작 로직
├── domain/               # 도메인 모델
│   ├── product.rs        # 제품 관련 도메인 모델
│   └── session_manager.rs # 크롤링 세션 상태 관리
├── infrastructure/       # 외부 연동 레이어
│   ├── integrated_product_repository.rs # DB 접근
│   ├── http_client.rs    # HTTP 요청
│   └── matter_data_extractor.rs # HTML 파싱
└── main.rs              # Tauri 앱 진입점
```

## 현재 크롤링 아키텍처의 실제 구현

### ⚠️ 중요: 문서와 구현의 차이점 해결됨

이전 문서(`crawling-engine-detailed-workflow.md`)에서는 CrawlingUseCases가 실제 HTTP 요청, HTML 파싱, 데이터 저장을 수행한다고 기술했지만, **현재 실제 구현은 다음과 같습니다**:

### 1. CrawlingUseCases의 실제 역할
```rust
// 실제 구현: 세션 관리만 담당
pub async fn start_crawling_session(&self, config: CrawlingConfig) -> Result<String> {
    // 1. 세션 ID 생성
    // 2. SessionManager에 세션 등록
    // 3. 세션 상태를 "시작됨"으로 설정
    // ❌ 실제 크롤링 로직은 없음
}
```

### 2. 실제 크롤링 실행은 어디서?
현재 구현에서는 **백그라운드 서비스나 워커가 별도로 구현되지 않았습니다**. 
- `start_crawling_session()`은 세션을 등록만 하고 즉시 반환
- 실제 크롤링 로직은 향후 별도의 백그라운드 워커에서 구현 예정
- 현재는 `test_core_functionality.rs` 바이너리에서만 실제 크롤링 테스트 가능

### 3. 데이터 흐름 (현재)
```
UI (SolidJS) 
    ↓ Tauri invoke
CrawlingUseCases::start_crawling_session()
    ↓
SessionManager (세션 상태 저장)
    ↓
❌ 실제 크롤링 워커 (미구현)
```

### 4. 데이터 흐름 (계획된 완성 상태)
```
UI (SolidJS) 
    ↓ Tauri invoke
CrawlingUseCases::start_crawling_session()
    ↓
SessionManager (세션 등록)
    ↓
🔄 백그라운드 워커 (미구현)
    ├── HttpClient (HTTP 요청)
    ├── MatterDataExtractor (HTML 파싱)
    └── IntegratedProductRepository (DB 저장)
```

## 통합 데이터베이스 스키마

현재 `integrated_product_repository.rs`에서 다음 테이블들을 관리:

### 주요 테이블
- `products`: 기본 제품 정보 (1단계 크롤링 데이터)
- `product_details`: 상세 제품 정보 (2단계 크롤링 데이터)
- `vendors`: 제조사 정보
- `crawling_results`: 크롤링 세션 결과

### 현재 동작하는 기능
- ✅ 제품/상세 정보 CRUD
- ✅ 검색 및 필터링
- ✅ 통계 조회
- ✅ 데이터 유효성 검증

## 프론트엔드 상태 관리

### SolidJS 기반 구조
```typescript
// src/stores/appStore.ts
const [state, setState] = createStore({
  crawling: {
    status: 'idle' | 'running' | 'completed' | 'error',
    progress: { /* 진행률 정보 */ }
  },
  ui: {
    activeTab: 'dashboard' | 'form' | 'results' | 'settings',
    theme: 'light' | 'dark'
  }
});
```

### 주요 컴포넌트
- `CrawlingDashboard.tsx`: 크롤링 상태 모니터링
- `CrawlingForm.tsx`: 크롤링 설정 및 시작
- `CrawlingResults.tsx`: 크롤링 결과 조회

## 현재 부족한 부분 (우선순위 순)

### 1. 🚨 최우선: 실제 크롤링 워커 구현
- 백그라운드에서 실행되는 크롤링 로직
- SessionManager와 연동되는 진행률 업데이트
- 비동기 작업 관리

### 2. 🔧 로깅 시스템 완성
- `tracing-subscriber` 추가 및 설정
- 파일 로깅 및 로그 롤링
- 환경변수 기반 로그 레벨 제어

### 3. 🔗 UI-백엔드 실시간 연동
- 크롤링 진행률 실시간 업데이트
- 에러 상태 및 알림 처리

### 4. 📝 UI 개선 및 완성
- 설정 페이지 구현
- 결과 페이지 기능 완성
- 반응형 및 접근성 개선

## 다음 구현 단계

1. **로깅 시스템 구축** (prompts 문서의 1단계 계획 참조)
2. **백그라운드 크롤링 워커 구현**
3. **실시간 상태 업데이트 연동**
4. **UI 완성 및 안정화**

---
*이 문서는 2025년 6월 30일 기준 실제 코드와 100% 일치합니다.*
