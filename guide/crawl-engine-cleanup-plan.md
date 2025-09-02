# `src-tauri/src` 전체 구조 리팩토링 계획 (v2, 2025-09-03)

**문서 목적**: `src-tauri/src`의 전체 소스 트리를 Modern Rust 2024, Clean Code 원칙에 맞춰 체계적으로 리팩토링하기 위한 실행 계획과 체크리스트를 제공합니다. 이 문서는 살아있는 문서로, 진행 상황에 따라 계속 업데이트됩니다.

**핵심 원칙**:
- **계층형 아키텍처 (Layered Architecture)**: `domain`, `application`, `infrastructure`, `api` 계층을 명확히 분리하고 의존성 규칙을 준수합니다.
- **단일 책임 원칙 (SRP)**: 모든 모듈과 함수는 하나의 명확한 책임을 가집니다.
- **Rust 2018+ Idioms**: `mod.rs`를 사용하지 않는 현대적인 모듈 시스템을 전면 적용합니다.
- **명확성 > 간결성**: 이름과 구조는 모호함 없이 역할을 드러내야 합니다.

---

## Phase 0: 기반 구조 리팩토링 (Foundational Refactoring)

**목표**: `src` 디렉토리 전체에 명확한 계층형 아키텍처를 적용하고, 레거시 코드를 제거하여 Phase 1, 2를 위한 안정적인 기반을 마련합니다.

### 체크리스트 (Phase 0)

- [ ] **`_archive` 디렉토리 완전 삭제**
  - 설명: 레거시 코드는 Git 히스토리로 충분합니다. 소스 트리의 노이즈를 제거하여 코드베이스의 명확성을 확보합니다.
  - 기존 계획의 "아카이브 폴더 유지" 항목을 "완전 삭제"로 변경합니다.

- [ ] **최상위 모듈 계층 재정의**
  - [ ] `types` 모듈을 `api`로 이름 변경: 프론트엔드와의 데이터 계약(DTO) 책임 명시.
  - [ ] `services` 모듈을 `application/services`로 이동: UI 로직 및 Use Case를 담당하는 애플리케이션 계층으로 통합.
  - [ ] `utils.rs` 기능 분산 및 파일 삭제: 관련된 모듈(주로 `infrastructure`)로 유틸리티 함수를 이전하고 최종적으로 파일을 삭제.

- [ ] **`commands` 모듈 구조화**
  - [ ] 기능별 하위 디렉토리(`crawling`, `database`, `analysis` 등)를 생성하여 20개 이상의 커맨드 파일을 그룹화.

- [ ] **프로젝트 전반의 모듈 시스템 현대화**
  - [ ] 남아있는 모든 `mod.rs` 파일을 제거하고, `module_name.rs`와 `module_name/` 디렉토리 구조로 통일.

### 제안 구조 (최종 목표)

```
src/
├── main.rs              # [인프라] 바이너리 실행 (최소화)
├── lib.rs               # 라이브러리 루트 (최상위 모듈 선언)
│
├── application.rs       # [애플리케이션] 모듈 선언
│   └── application/
│       ├── services.rs  # UI 로직, Use Case 등
│       └── state.rs     # Tauri 공유 상태
│
├── domain.rs            # [도메인] 모듈 선언
│   └── domain/
│       ├── product.rs   # 엔티티
│       └── repository.rs# 리포지토리 Trait (인터페이스)
│
├── infrastructure.rs    # [인프라] 모듈 선언
│   └── infrastructure/
│       ├── database.rs  # DB 커넥션, 리포지토리 구현체
│       ├── http_client.rs
│       └── parsing.rs   # HTML 파서 구현체
│
├── commands.rs          # [API] Tauri 커맨드 모듈 선언
│   └── commands/
│       ├── crawling.rs  # 기능별로 명확히 그룹화
│       └── database.rs
│
└── api.rs               # [API] 프론트엔드와 통신하는 타입(DTO) 모듈 선언
    └── api/
        └── crawling_types.rs
```

---

## Phase 1: `crawl_engine` 모듈 정제

**목표**: Phase 0에서 마련된 기반 위에서, `crawl_engine` 내부의 복잡도를 낮추고 Clean Code 원칙을 적용합니다.

### 체크리스트 (Phase 1)

  - [x] 작업 브랜치 생성: `crawl-engine-cleanup`
  - [x] 최신 main에 변경 반영 및 푸시
  - [x] 안전망 확인: 기본 빌드/테스트 그린 상태 확인(cargo test 238/238)
  - [ ] **(Phase 0에서 처리)** `mod.rs` 제거 및 모듈 시스템 통일
  - [ ] `ts_gen.rs` 로직을 빌드 스크립트(`build.rs`) 또는 별도 스크립트로 이전.
  - [ ] `test_utils.rs`를 `#[cfg(test)]`로 격리하거나 `tests/common`으로 이동.
  - [ ] 명백한 죽은 파일/폴더 제거(주석만, 실험/백업 잔재 등)
  - [ ] 중복 구현/이름만 다른 파일 통합 계획 수립 (`crawling_integration.rs` vs `real_crawling_integration.rs`)
  - [ ] `crawl_engine` 루트(`crawl_engine.rs`)에서 하위 모듈 선언 및 `pub use`를 통한 API 표면 정리.
  - [ ] 파일/모듈 이름을 역할 기반으로 정리(예: `actor_system.rs` → `system.rs`)
  - [ ] dead_code/unused_imports 제거(도구: rust-analyzer, clippy)
  - [x] 불필요한 `clone()` 제거(1차)
  - [ ] 함수 시그니처 정리: 입력→출력(가능하면 stateless), 명확한 에러 타입
  - [ ] 거대 파일 분리 또는 공통부 통합
  - [x] Strategy 패턴 경로 고정: `stages/strategies/default/*` 경량화 및 일관화
  - [ ] CrawlingPolicy 명시화: 재시도/중복/성능 옵션을 구조체로 관리하고 ExecutionPlan에 포함
  - [x] ts-rs 기반 타입 자동 생성 재검증(`scripts/generate_types.sh`)
  - [x] 이벤트 발행 경로 정리
- 품질 게이트
  - [x] cargo check, cargo test --all-features
  - [ ] cargo clippy --all-targets -- -D warnings

---

## Phase 2: `commands` 모듈 리팩토링

**목표**: Phase 0에서 구조화된 `commands` 모듈 내부의 레거시 코드를 정리하고, feature-gate를 활용하여 빌드를 최적화합니다.

### 현재 상태 분석
- **사용 중**: `unified_crawling`, `actor_system_commands`, `real_crawling_commands`, `system_analysis`, `data_queries`, `config_commands` 등
- **사용 중단/레거시**: `dashboard_commands`, `actor_system_monitoring`, `db_cleanup`, `db_diagnostics`, `debug_commands` 등

### 실행 계획 (Phase 2)
1.  **(Phase 0에서 처리)** 기능별 하위 디렉토리 생성 및 파일 이동.
2.  **feature-gate 적용**:
    - `commands/archive/` 대신, 사용 중단된 커맨드들에 `#[cfg(feature = "legacy-ui")]` 또는 `#[cfg(feature = "dev-tools")]`를 적용하여 코드베이스에 유지하되 기본 빌드에서는 제외.
3.  **API 표면 축소**:
    - `config_commands.rs` 등에서 `#[tauri::command]` 어노테이션이 있지만 실제 사용되지 않는 함수는 어노테이션을 제거하여 내부 유틸리티 함수로 전환.
4.  **네이밍/역할 기반 모듈 정리**:
    - `actor_system_commands.rs` → `actor_system.rs`와 같이 역할 중심 이름으로 실제 파일명 변경.

### 체크리스트 (Phase 2)
- [x] 레거시 커맨드 파일에 feature-gate 적용 완료
- [x] `config_commands` 미사용 커맨드 비노출화 완료
- [x] `lib.rs` 등록 커맨드 목록과 문서 동기화
- [ ] **(진행 중)** 네이밍 조정 및 모듈 경로 정리
- [ ] clippy/테스트 그린 확인 및 문서 갱신

---

## 공통 관리 항목

- **작업 방식**: 1) Phase 0 (기반) → 2) Phase 1 (Crawl Engine) → 3) Phase 2 (Commands) 순으로 진행. 각 단계는 작은 커밋으로 분리하여 리뷰와 롤백 용이성 확보.
- **품질 게이트**: 각 주요 변경 후 `cargo clippy --all-targets -- -D warnings`와 `cargo test --all-features`를 통과해야 함.
- **문서**: 이 문서는 모든 리팩토링 작업의 중심. 작업 시작 전 계획을 업데이트하고, 완료 후 체크리스트를 갱신.

## 변경 로그(요약)
- **2025-09-03**: Gemini 제안에 따라 `src` 전체 리팩토링 계획으로 확장. Phase 0, 1, 2로 구조화. `_archive` 삭제 및 계층형 아키텍처 적용을 최우선 과제로 설정.
- **2025-09-01**: Stage/Batch/Session Actor의 `emit` 헬퍼 도입 및 이벤트 경로 통일. clone 최소화 적용.