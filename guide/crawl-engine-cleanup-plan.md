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

- [x] **`_archive` 디렉토리 완전 삭제**
  - 설명: 레거시 코드는 Git 히스토리로 충분합니다. 소스 트리의 노이즈를 제거하여 코드베이스의 명확성을 확보합니다.
  - 기존 계획의 "아카이브 폴더 유지" 항목을 "완전 삭제"로 변경합니다.

- [ ] **최상위 모듈 계층 재정의**
  - [ ] `types` 모듈을 `api`로 이름 변경: 프론트엔드와의 데이터 계약(DTO) 책임 명시.
  - [ ] `services` 모듈을 `application/services`로 이동: UI 로직 및 Use Case를 담당하는 애플리케이션 계층으로 통합.
  - [ ] `utils.rs` 기능 분산 및 파일 삭제: 관련된 모듈(주로 `infrastructure`)로 유틸리티 함수를 이전하고 최종적으로 파일을 삭제.

- [x] **`commands` 모듈 구조화**
  - [x] 기능별 하위 디렉토리(`crawling`, `database`, `analysis`, `devtools`, `legacy`) 생성 및 물리 이동 완료
  - [x] `lib.rs`의 invoke_handler 경로와 re-export를 nested 구조에 맞게 정리
  - [x] 상위(legacy) `commands/*.rs` 중복 파일 제거(혼동 방지)

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

### 상세 실행 가이드 (Phase 0)

- Step-by-step 작업 순서 (권장 커밋 쪼개기)
   1) `_archive` 폴더 전량 삭제 (완료: 2025-09-04, 사전 grep 및 스크립트 경로 전환 후 삭제)
  2) `types` → `api` 이름 변경 및 경로 변경
     - `lib.rs` 내 `pub mod types` → `pub mod api`로 변경
     - TypeScript 생성 스크립트(`scripts/generate_types.sh`)가 참조하는 경로 업데이트
     - Frontend import 경로에 영향 여부 점검 (생성 산출물 경로 유지 시 영향 없음)
  3) `services` 이동: `src-tauri/src/services/*` → `src-tauri/src/application/services/*`
     - `lib.rs` 재배치된 경로로 re-export 정리
  4) `utils.rs` 기능 분산: 파싱, 로깅, 경로 등 관련 모듈로 이전 후 파일 제거
  5) `mod.rs` 일괄 제거: 남아있는 `mod.rs`를 파일/디렉토리 기반 모듈로 전환 (Rust 2024)

- Definition of Done (Phase 0)
  - cargo check, npm run type-check 통과
  - `_archive` 디렉토리 미존재, 미참조
  - `lib.rs`의 최상위 모듈 선언이 제안 구조와 합치
  - Frontend 타입 생성 및 사용 정상 동작 (scripts/generate_types.sh → PASS)

---

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

### 상세 실행 가이드 (Phase 1)

- 정합성 있는 공개 API 표면 정리
  - `crawl_engine.rs`에서 하위 모듈을 명시적으로 선언하고, 외부에서 필요한 타입/함수만 `pub use`로 노출
  - Session/Stage/Worker 단위의 기본 계약(Contract) 주석 업데이트 및 안정화

- 파일 분할·통합 기준
  - 500~800라인 초과 파일은 역할별 하위 파일로 분할 (예: planning, session_registry, event_bridge)
  - 중복 기능은 단일 서비스로 통합 (예: 통일된 Pagination 유틸)

- 함수 시그니처 정리
  - 입력은 명시적 구조체(옵션 세트) 사용, 출력은 Result<T, E> 통일
  - 에러 타입은 도메인 에러 enum으로 수렴, anyhow는 경계(최상위 API)에서만 사용

- Definition of Done (Phase 1)
  - 공개 API 목록(문서/주석)과 실제 `pub use`가 일치
  - 거대 파일 분할/통합 커밋 포함, 불필요한 clone 제거, dead_code/unused_imports 제거
  - `cargo clippy --all-targets -- -D warnings` 그린

---

---

## Phase 2: `commands` 모듈 리팩토링

**목표**: Phase 0에서 구조화된 `commands` 모듈 내부의 레거시 코드를 정리하고, feature-gate를 활용하여 빌드를 최적화합니다.

### 현재 상태 분석
- 사용 중: `unified_crawling`, `actor_system`(이전: `actor_system_commands`), `real_crawling_commands`, `system_analysis`, `data_queries`, `config_commands` 등
- 사용 중단/레거시: `dashboard_commands`, `actor_system_monitoring`, `db_cleanup`, `debug_commands` 등
- 개발·진단: `db_diagnostics`는 dev 전용이나, 현재 debug 빌드(`debug_assertions`)에서도 활성화되어 개발 편의 보장
- 이행 현황: `actor_system_commands.rs` → `actor_system.rs`로 구현 본체 이전 완료, 레거시 파일은 얇은 re-export shim으로 유지 (호환성)

### 실행 계획 (Phase 2)
1.  **(Phase 0에서 처리)** 기능별 하위 디렉토리 생성 및 파일 이동.
2.  **feature-gate 적용**:
    - `commands/archive/` 대신, 사용 중단된 커맨드들에 `#[cfg(feature = "legacy-ui")]` 또는 `#[cfg(feature = "dev-tools")]`를 적용하여 코드베이스에 유지하되 기본 빌드에서는 제외.
3.  **API 표면 축소**:
    - `config_commands.rs` 등에서 `#[tauri::command]` 어노테이션이 있지만 실제 사용되지 않는 함수는 어노테이션을 제거하여 내부 유틸리티 함수로 전환.
4.  **네이밍/역할 기반 모듈 정리**:
  - `actor_system_commands.rs` → `actor_system.rs` (이미 완료). 다음 PR에서 shim 제거 및 참조 전환 진행.

#### 디렉토리 구조안 (commands)

```
src-tauri/src/commands/
├── crawling/
│   ├── actor_system.rs        # 메인 Actor 시스템 명령
│   ├── unified_crawling.rs
│   └── real_crawling_commands.rs
├── database/
│   ├── data_queries.rs
│   ├── db_repair.rs           # [dev-tools]
│   └── db_cleanup.rs          # [dev-tools]
├── analysis/
│   ├── system_analysis.rs
│   └── performance_commands.rs
├── devtools/
│   ├── db_diagnostics.rs      # [dev-tools or debug_assertions]
│   └── debug_commands.rs      # [dev-tools]
├── legacy/
│   └── dashboard_commands.rs  # [legacy-ui]
└── config_commands.rs
```

#### 마이그레이션/정리 작업 항목

- [x] `lib.rs`의 generate_handler 등록을 nested 경로로 교체(예: `commands::crawling::*`, `commands::analysis::*`, `commands::database::*`, `commands::devtools::*`)
- [x] 워크스페이스 전역 경로 정리: 레거시 `commands::<top-level>` 참조를 nested로 수렴(필요 경로는 re-export 유지)
- [x] 레거시 shim 및 상위 중복 파일 제거
- [x] 디렉토리 구조에 맞춰 파일 이동 및 `lib.rs`/re-export 정리
- [x] dev 전용 커맨드에 feature-gate 적용(`dev-tools` 또는 `any(dev-tools, debug_assertions)`)
- [ ] 사용 빈도가 낮고 미노출 가능 함수의 `#[tauri::command]` 제거 → 내부 util로 전환

#### Definition of Done (Phase 2)

- `commands` 하위가 기능 폴더로 재구성되고, `lib.rs` 등록과 일치
- 레거시 shim 제거, 모든 참조가 `commands::actor_system::`로 수렴
- 불필요한 `#[tauri::command]` 제거 및 API 표면 축소 완료
- cargo check / npm type-check / (선택) 간단 E2E 버튼 동작 스모크 테스트 통과

### 체크리스트 (Phase 2)
- [x] 레거시 커맨드 파일에 feature-gate 적용 완료
- [x] `config_commands` 미사용 커맨드 비노출화 완료
- [x] `lib.rs` 등록 커맨드 목록과 문서 동기화
- [x] 네이밍 조정 및 모듈 경로 정리(Commands 그룹화 및 상위 중복 제거)
- [x] simple_actor_test 전체 게이팅 적용(모듈 선언, re-export, manage, invoke 등록)
- [x] 초기 clippy 정리(중복 match arm 제거, 문서 주석 markdown 수정, 락 스코프 축소, Option 처리 간소화)
- [ ] clippy/테스트 그린 확인 및 문서 갱신(남은 경고 처리: parsing/*, retry_manager 추가 개선, unnecessary_wraps 등)

---

## 실행·검증 가이드 (공통)

- 자주 사용하는 로컬 체크
  - Rust
    - 빌드 확인: `cargo check`
    - 테스트: `cargo test --all-features`
    - 린트: `cargo clippy --all-targets -- -D warnings`
  - TypeScript
    - 타입 재생성: 워크스페이스 task "regen types" 또는 `bash scripts/generate_types.sh`
    - 타입 체크: 워크스페이스 task "typecheck"

- 리그레션 방지 스모크
  - FE 진단 버튼(“진단 실행”) → `scan_db_pagination_mismatches` 호출 성공 (dev 또는 debug 빌드)
  - 기본 Actor 시작/정지/목록 명령이 에러 없이 처리되는지 확인

## 리스크/완화

- 대규모 경로 이동에 따른 import 붕괴 → grep/검색 기반 일괄 수정 + 컴파일러 에러 드리븐 수정
- dev-tools 게이트 누락으로 기능 비노출 → `#[cfg(any(feature = "dev-tools", debug_assertions))]` 패턴 재사용
- Frontend 타입 경로 변동 → 타입 산출물 경로는 유지, Rust 내부 모듈명만 변경하도록 설계

---

## 공통 관리 항목

- **작업 방식**: 1) Phase 0 (기반) → 2) Phase 1 (Crawl Engine) → 3) Phase 2 (Commands) 순으로 진행. 각 단계는 작은 커밋으로 분리하여 리뷰와 롤백 용이성 확보.
- **품질 게이트**: 각 주요 변경 후 `cargo clippy --all-targets -- -D warnings`와 `cargo test --all-features`를 통과해야 함.
- **문서**: 이 문서는 모든 리팩토링 작업의 중심. 작업 시작 전 계획을 업데이트하고, 완료 후 체크리스트를 갱신.

## 변경 로그(요약)
- **2025-09-03**: Gemini 제안에 따라 `src` 전체 리팩토링 계획으로 확장. Phase 0, 1, 2로 구조화. `_archive` 삭제 및 계층형 아키텍처 적용을 최우선 과제로 설정.
- **2025-09-03(2)**: Commands 도메인 그룹화(\`crawling\`, \`database\`, \`analysis\`, \`devtools\`, \`legacy\`) 완료. `lib.rs` invoke_handler 및 re-export 정리. 상위 중복 파일 제거. `analysis::system_analysis`/`performance_commands` 이관 및 보완. `database::{data_queries, db_cleanup, db_repair}`와 `devtools::{db_diagnostics, debug_commands, product_details_analytics}` 정리. 빌드/타입체크 그린.
- **2025-09-01**: Stage/Batch/Session Actor의 `emit` 헬퍼 도입 및 이벤트 경로 통일. clone 최소화 적용.

### 진행 스냅샷 (2025-09-03)
- Diagnostics 가시성 개선: dev 빌드에서 진단 명령(feature-gate) 활성화, `db_diagnostics`는 `any(dev-tools, debug_assertions)`로 개발 편의 보장
- Commands 그룹화: `commands/{crawling,database,analysis,devtools,legacy}` 구조 확립 및 이관 완료
- Analysis 이관: `analysis::system_analysis` 누락 커맨드 보강, 실제 상태/리포지토리 기반 분석으로 교체, 로그 ASCII 정리
- Database/Devtools 정리: `data_queries`, `db_cleanup`, `db_repair`, `db_diagnostics`, `debug_commands(ui_debug_log)`, `product_details_analytics` 정비
- 상위 중복 파일 제거 및 `lib.rs` 등록/재수출 일치화
- Build/TS 타입 체크 그린 확인

### 진행 스냅샷 (2025-09-03 2차)
- dev-tools 게이팅 일관화: simple_actor_test를 모듈/재노출/상태 manage/핸들러 등록 전반에 적용
- retry_manager: 락 스코프 축소 및 `as u32` 캐스팅을 `u32::try_from`으로 치환, Result 반환 함수에 `# Errors` 문서화 추가
- parsing_error: 동일 match arm 병합으로 clippy 경고 감소
- simple_http_client: 락 스코프 축소 및 Option 처리 개선(is_none_or)

---

## 다음 작업 계획 (Next Steps)

단기(Phase 2 마무리)
- [ ] `simple_actor_test`를 `#[cfg(feature = "dev-tools")]`로 제한하고, lib.rs의 manage/invoke 등록도 동일 게이트 적용
- [ ] 남은 dev-only 커맨드 중 FE 미사용 함수의 `#[tauri::command]` 제거(내부 util로 전환)
- [ ] cargo clippy --all-targets -- -D warnings 그린 달성(ASCII 로그, 불필요 allow 정리, dead_code 잔여 제거)
- [ ] 최소 단위 테스트 추가: system_analysis happy path + db_diagnostics gate 동작

중기(Phase 0 보완 및 품질 게이트 강화)
- [x] `_archive` 디렉터리 완전 삭제 전 최종 참조 점검 후 제거 (완료: 2025-09-04)
- [ ] `mod.rs` 잔여 제거 및 모듈 시스템 통일
- [ ] 통합 문서/README 업데이트(Commands 경로 변경 사항과 FE invoke 경로 안내)

장기(Phase 1/엔진 정제)
- [ ] crawl_engine 공개 API 표면 정리 및 에러 타입 일관화
- [ ] 정책/전략 타입(CrawlingPolicy 등) 명시화 및 ExecutionPlan 포함
