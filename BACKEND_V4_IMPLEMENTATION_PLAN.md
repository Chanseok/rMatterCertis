# 이벤트 기반 크롤링 엔진 v4.0 구현 계획

## 🎯 목표
기존 단계별(Stage) 크롤링 시스템을 완전히 교체하여, 작업 큐 기반의 유연하고 확장 가능한 이벤트 기반 시스템으로 전환

## 🏗️ 설계 지향점

### Modern Rust 2024 + Tauri v2.0 + Clean Code 철학
- **Rust 2024 Edition**: 최신 Rust 에디션의 모든 기능과 모범 사례 적용
- **Clippy Compliance**: 모든 코드가 `clippy::pedantic` 수준의 린터 규칙을 준수
- **Tauri v2.0 Modern APIs**: 최신 Tauri 아키텍처 가이드와 API 패턴 적용
- **Clean Code 원칙**: 
  - 명확하고 의도를 표현하는 네이밍
  - 단일 책임 원칙 (SRP) 적용
  - 의존성 역전 원칙 (DIP) 구현
  - 테스트 주도 개발 (TDD) 접근
- **Type Safety**: 컴파일 타임에 최대한 많은 오류 방지
- **Zero-Cost Abstractions**: 런타임 성능에 영향 없는 추상화 구현

---

## 📊 구현 현황 (2025년 7월 12일 기준)

### Phase 1: 핵심 구조 설계 및 구현 - 🚧 **80% 완료**
- ✅ **CrawlingTask v4.0 enum 정의** - Modern Rust 2024 스타일로 구현
- ✅ **SharedState 구조 구현** - Thread-safe 상태 관리 시스템
- ✅ **작업 큐 시스템 설계** - Type-safe 백프레셔 지원 큐 시스템
- 🚧 **작업자 풀 인터페이스 정의** - 진행 중

### Phase 2: 작업자 풀 구현 - ⏳ **준비 중**
- [ ] ListPageFetcher 구현 (Modern async/await 패턴)
- [ ] ListPageParser 구현 (HTML 파싱 최적화)
- [ ] ProductDetailFetcher 구현 (HTTP 클라이언트 재사용)
- [ ] ProductDetailParser 구현 (구조화된 데이터 추출)
- [ ] DbSaver 구현 (배치 저장 최적화)

### Phase 3: 오케스트레이터 구현 - ⏳ **대기 중**
- [ ] 오케스트레이터 생명주기 관리
- [ ] 작업자 생성 및 스케줄링
- [ ] 취소 토큰 시스템
- [ ] 상태 브로드캐스터 통합

### Phase 4: 통합 및 테스트 - ⏳ **대기 중**
- [ ] 기존 명령어 인터페이스 교체
- [ ] 데이터베이스 통합
- [ ] 성능 테스트 및 최적화
- [ ] 프론트엔드 연동 테스트

### ✅ **완료된 핵심 컴포넌트**
1. **`/src/crawling/tasks.rs`** - 타입 안전한 작업 정의 시스템
   - TaskId with UUID 기반 고유성 보장
   - Clean Code 원칙 적용된 ProductData 빌더 패턴
   - 포괄적인 단위 테스트 포함

2. **`/src/crawling/state.rs`** - 현대적인 상태 관리
   - Arc/RwLock/Mutex를 활용한 thread-safe 설계
   - 실시간 통계 수집 및 성능 메트릭
   - CancellationToken 기반 우아한 종료 시스템

3. **`/src/crawling/queues.rs`** - 고성능 작업 큐 시스템
   - 백프레셔 지원으로 메모리 안정성 보장
   - 타입 안전한 라우팅 시스템
   - 실시간 메트릭 및 텔레메트리 지원

---

## 🗂️ Modern Rust 2024 + Tauri v2.0 + Clean Code 프로젝트 구조

```
src-tauri/
├── Cargo.toml                       # Rust 2024 edition, modern dependencies
├── src/
│   ├── lib.rs                       # 라이브러리 진입점 (모든 모듈 명시적 선언)
│   ├── main.rs                      # Tauri v2.0 애플리케이션 진입점
│   │
│   ├── domain/                      # 🏗️ 도메인 레이어 (비즈니스 로직)
│   │   ├── entities.rs              # 도메인 엔티티 (Product, CrawlingSession, Task)
│   │   ├── value_objects.rs         # 값 객체 (TaskId, Url, ProductData)
│   │   ├── services.rs              # 도메인 서비스 (CrawlingOrchestrator, DataValidator)
│   │   └── events.rs                # 도메인 이벤트 (TaskEvents, CrawlingEvents)
│   │
│   ├── application/                 # 🎯 애플리케이션 레이어 (Use Cases)
│   │   ├── use_cases.rs             # 사용 사례 (StartCrawling, StopCrawling, GetStatus)
│   │   ├── ports.rs                 # 포트 인터페이스 (Repository, HttpClient, EventPublisher)
│   │   └── dto.rs                   # 데이터 전송 객체 (CrawlingRequest, CrawlingResponse)
│   │
│   ├── infrastructure/              # 🔧 인프라스트럭처 레이어 (외부 시스템 연동)
│   │   ├── adapters.rs              # 어댑터 (DatabaseRepository, HttpClient, EventPublisher)
│   │   ├── workers.rs               # 작업자 (ListFetcher, Parser, DetailFetcher, DbSaver)
│   │   ├── queues.rs                # 큐 시스템 (TaskQueue, QueueManager)
│   │   └── config.rs                # 설정 관리 (AppConfig)
│   │
│   ├── presentation/                # 🎨 프레젠테이션 레이어 (UI/API)
│   │   ├── tauri_handlers.rs        # Tauri v2.0 명령어 핸들러
│   │   ├── dto.rs                   # 프레젠테이션 DTO (SystemStatePayload, UiEvents)
│   │   └── validators.rs            # 입력 검증 (RequestValidator)
│   │
│   ├── shared/                      # 🔧 공유 유틸리티 (Cross-cutting Concerns)
│   │   ├── telemetry.rs             # 관찰 가능성 (Metrics, Tracing, Logging)
│   │   ├── error.rs                 # 에러 처리 (ErrorTypes, ErrorHandler)
│   │   ├── utils.rs                 # 유틸리티 함수 (TimeUtils, StringUtils)
│   │   └── constants.rs             # 상수 정의 (AppConstants)
│   │
│   └── tests/                       # 🧪 통합 테스트
│       ├── integration.rs           # 통합 테스트 (CrawlingWorkflow, TauriCommands)
│       └── fixtures.rs              # 테스트 픽스처 (TestData)
│
└── tests/                           # 📋 E2E 테스트 (프로젝트 루트)
    ├── crawling_e2e_test.rs
    └── performance_test.rs
```

### 🚀 Modern Rust 2024 "No mod.rs" 철학 구현

#### 1. **완전한 mod.rs 제거**
- ✅ 모든 모듈을 파일명으로 직접 정의
- ✅ lib.rs에서 명시적 모듈 선언: `pub mod domain;`
- ✅ 단일 파일로 관련 기능 그룹화 (entities.rs, services.rs 등)

#### 2. **Clean Architecture 레이어 분리**
```rust
// lib.rs 예시
pub mod domain;        // 비즈니스 로직 (외부 의존성 없음)
pub mod application;   // 사용 사례 (domain만 의존)
pub mod infrastructure; // 외부 시스템 (domain 인터페이스 구현)
pub mod presentation;  // UI/API (application 사용)
pub mod shared;        // 공유 유틸리티
```

#### 3. **파일별 책임 분리**
- **`entities.rs`**: 모든 도메인 엔티티 한 곳에
- **`value_objects.rs`**: 모든 값 객체 한 곳에
- **`services.rs`**: 모든 도메인 서비스 한 곳에
- **`ports.rs`**: 모든 포트 인터페이스 한 곳에
- **`adapters.rs`**: 모든 어댑터 구현 한 곳에

---

## 🔧 즉시 착수 가능한 첫 번째 단계

### 1. 모듈 구조 생성
기본 디렉토리 구조와 모듈 파일들을 생성합니다.

### 2. CrawlingTask v4.0 정의
SystemStatePayload와 호환되는 작업 정의를 구현합니다.

### 3. SharedState 구조
백엔드 상태 관리를 위한 핵심 구조를 구현합니다.

**바로 시작하시겠습니까?**
