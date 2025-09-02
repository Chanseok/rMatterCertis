# Crawl Engine Cleanup & Refactor Plan (Modern Rust 2024)

문서 목적: `src-tauri/src/crawl_engine`의 소스 트리와 코드를 Modern Rust 2024, Clippy, Clean Code 원칙에 맞춰 체계적으로 정리/개선하기 위한 실행 계획과 체크리스트를 제공합니다. 이 문서는 진행 중 수시로 업데이트합니다.

## 목표와 범위

- 범위: `src-tauri/src/crawl_engine` 이하 소스 트리 및 공개 표면(API) 정리
- 목표:
  - mod.rs 제거 및 모듈 게이트 파일(모듈명.rs)로 통일
  - Clippy 경고 0, dead_code/unused_imports 0
  - ts-rs 타입 자동 생성 흐름 정상화(프론트엔드와 타입 동기화)
  - 불필요한 clone() 제거, 참조 전달 우선
  - 단일 책임, 명확한 네이밍, 최소 의존성 적용
  - 함수형 스타일 지향(불변/순수 함수 우선)

## 진행 체크리스트 (살아있는 문서)

  - [x] 작업 브랜치 생성: `crawl-engine-cleanup`
  - [x] 최신 main에 변경 반영 및 푸시
  - [x] 안전망 확인: 기본 빌드/테스트 그린 상태 확인(cargo test 238/238)
  - [x] 인벤토리 1차 스냅샷 생성(구조 파악):
    - crawl_engine 루트: actor_event_bridge.rs, actor_system.rs, actors.rs, channels.rs, config/, context/, events/, integrated_context.rs, runtime/, services/, stages/, system_config.rs, ts_gen.rs, validation/
    - actors/: batch_actor.rs, contract.rs, session_actor.rs, stage_actor.rs, traits.rs, types.rs
    - channels/: types.rs (게이트: channels.rs)
    - stages/: mod.rs, strategies/default/*, traits.rs
    - services/: crawling_integration.rs, crawling_planner.rs, data_consistency_checker.rs, data_quality_analyzer.rs, performance_optimizer.rs, planning_service.rs, real_crawling_commands.rs, real_crawling_integration.rs, mod.rs
  - [x] 빈/미사용 스텁 제거: channels/channels.rs, services/services.rs 삭제
  - [ ] 명백한 죽은 파일/폴더 제거(주석만, 실험/백업 잔재, 완전히 미사용 테스트 등)
  - [ ] 아카이브 폴더는 유지하되 프로덕션 경로 의존성 차단 확인
  - [ ] 중복 구현/이름만 다른 파일 통합 계획 수립
  - [ ] crawl_engine 루트: `src-tauri/src/crawl_engine.rs`에서 하위 모듈 선언 및 재익스포트 정리
  - [x] 각 디렉토리에서 `mod.rs` 제거, `모듈명.rs`/게이트 파일로 통일(충돌 제거)
  - [ ] 파일/모듈 이름을 역할 기반으로 정리(예: `actor_system.rs` → `system.rs`)
  - [ ] dead_code/unused_imports 제거(도구: rust-analyzer, clippy)
  - [x] 불필요한 `clone()` 제거(1차): 전략 단계와 Actor 경로에서 참조 우선 적용
  - [ ] 함수 시그니처 정리: 입력→출력(가능하면 stateless), 명확한 에러 타입
  - [ ] 명확한 네이밍/단일 책임: 거대 파일 분리 또는 공통부 통합
  - [x] Strategy 패턴 경로 고정: stages/strategies/default/* 경량화 및 일관화
  - [ ] CrawlingPolicy 명시화: 재시도/중복/성능 옵션을 구조체로 관리하고 ExecutionPlan에 포함
  - [x] ts-rs 기반 타입 자동 생성 재검증(`scripts/generate_types.sh`)
  - [x] 타입 변경 시 생성물 갱신 및 FE 타입 정합성 확인(프로젝트 TS 타입체크 실행)
  - [x] 이벤트 발행 경로 정리: SessionActor/BatchActor/StageActor(스테이지 레벨) `emit` 헬퍼 도입
- 품질 게이트
  - [x] cargo check (테스트와 함께 검증)
  - [x] cargo test --all-features
  - [ ] cargo clippy --all-targets -- -D warnings

## 제안 구조(타깃)

```
src-tauri/src/
├─ crawl_engine.rs            # 루트 게이트: 하위 모듈 선언 및 re-export
└─ crawl_engine/
   ├─ actors/
   │  ├─ session_actor.rs
   │  ├─ stage_actor.rs
   │  └─ batch_actor.rs
   ├─ channels/
   │  └─ ...
   ├─ services/
   │  ├─ planning_service.rs  # PlanningService/Strategy 구현
   │  └─ real_crawling_integration.rs
   ├─ stages/
   │  ├─ traits.rs
   │  └─ strategies/
   │     └─ default/
   │        ├─ list_page.rs
   │        ├─ product_detail.rs
   │        ├─ data_validation.rs
   │        ├─ data_saving.rs
   │        └─ status_check.rs
   ├─ system.rs               # (구) actor_system.rs
   ├─ actor_event_bridge.rs
   ├─ integrated_context.rs
   └─ types.rs (필요 시)
```

원칙
- mod.rs 사용 금지, 게이트 파일에서 `pub use`로 외부 surface 최소/명시화
- 파일명은 역할을 드러내도록 간결하게(actors.rs처럼 모호한 이름 지양)

## 작업 방식(하이브리드)

1) 얇은 선제 삭제(안전한 범위) → 2) 모듈별 리팩토링과 병행 삭제 → 3) 품질 게이트 통과 후 커밋/PR
- 삭제/리팩토링을 분리 커밋으로 관리해 리뷰와 롤백 용이성 확보
- 애매한 코드/실험 기능은 `cfg(feature = "experimental")` 또는 `_archive/`로 격리
- 경고를 에러로 승격(deny warnings)은 모듈별 점진 적용

## 도구/명령 관례

- 정적 점검: `cargo clippy --all-targets -- -D warnings`
- 자동 수정(선택): `cargo clippy --fix -Z unstable-options --allow-dirty --allow-staged`
- 테스트: `cargo test --all-features`
- 타입 생성: `bash scripts/generate_types.sh`
- 미사용 의존성(선택): `cargo +nightly udeps`

## Definition of Done

- crawl_engine 하위 dead_code/unused_imports 0
- clippy/테스트 모두 그린, 빌드 캐치업 완료
- ts-rs 타입 생성물 최신, FE 타입 불일치 0
- 공개 API 표면이 `crawl_engine.rs`와 게이트 파일에 명확히 정의됨
- 문서(본 계획서) 최신 상태 유지 및 변경 로그 반영

## 위험/롤백

- 대규모 삭제/이동 시 회귀 위험: 커밋을 작게 유지, 각 단계 후 품질 게이트로 검증
- 타입 변경으로 인한 FE 영향: 타입 생성 후 프런트 타입 검사/간단 스모크 테스트 병행
- 롤백: 단계별 태그/커밋 메시지에 컨텍스트 포함, PR마다 스쿼시 대신 병합 커밋 유지(이력 보존)

## 초기 작업 항목(다음 액션 제안)

- [ ] crawl_engine 트리 인벤토리 생성(현재 파일/모듈 맵 작성)
- [ ] 명백한 죽은 테스트/백업 파일 1차 삭제 커밋
- [ ] `crawl_engine.rs`의 re-export 표면 점검 및 주석 보강
- [ ] 첫 대상 모듈 선정(예: `actors/session_actor.rs`) → dead_code/clone 최소화 적용
- [ ] 타입 생성 스크립트 실행 및 변경 영향 리뷰

---

문서 버전: v0.1 (초안) — 브랜치: `crawl-engine-cleanup`

---

## 변경 로그(요약)

- 2025-09-01
  - StageActor: `emit` 헬퍼 추가(`AppContext::emit_event` -> `Result<usize, _>`를 무시하고 `Result<(), StageError>`로 매핑), stage 수명주기 이벤트 경로 통일(StageStarted/Completed/Failed/Timeout)
  - BatchActor: `emit` 헬퍼 도입 및 배치 이벤트 경로 통합(시작/진행/실패/완료/리포트)
  - SessionActor: `emit` 헬퍼 도입 및 호출 경로 통합(진단/플래닝/루프/완료)
  - stages/strategies/default/*: clone 최소화 및 duration_ms 수집 일관화
  - 전체 테스트 통과(238/238)

## 다음 단계(제안)

- StageActor per-item 경로 중 오류 전파가 의미 있는 지점은 `emit`로 통일하고, RAII/Drop 기반 베스트에포트 경로는 유지
- clippy 경고와 문서 주석(# Errors 등) 보강
- 선택: 공통 이벤트 생성 유틸(팩토리/빌더) 도입으로 중복 포맷 정리

## Commands 정리(Actor 기반 이행에 따른 레거시 정돈)

배경: `src-tauri/src/commands` 하위에 Actor 기반 통합 진입점(`unified_crawling`) 도입 전의 명령들이 혼재합니다. 현재 `src-tauri/src/lib.rs`의 `generate_handler!`에 등록된 커맨드와 디렉터리 목록을 비교하여 다음과 같이 정리합니다.

### 현재 사용 중(핵심/보조)
- unified: `unified_crawling.rs`
- actor system: `actor_system_commands.rs`, `simple_actor_test.rs`
- crawling: `real_crawling_commands.rs`, `crawling_test_commands.rs`, `smart_crawling.rs`
- 분석/동기화: `system_analysis.rs`, `validation_commands.rs`, `sync_commands.rs`
- 데이터 조회: `data_queries.rs`, `advanced_engine_api.rs`(status/info 전용)
- 성능: `performance_commands.rs`
- 설정/윈도우: `config_commands.rs` 중 아래 API만 FE 노출
  - settings store: `get_app_settings`, `save_app_settings`
  - window/log: `save_window_state`, `load_window_state`, `set_window_position`, `set_window_size`, `maximize_window`, `show_window`, `write_frontend_log`

### 사용 중단/레거시(등록되지 않음 또는 주석 처리)
- UI/대시보드: `dashboard_commands.rs`(lib.rs에서 주석 처리)
- 모니터링: `actor_system_monitoring.rs`(등록 없음)
- DB 유틸: `db_cleanup.rs`, `db_diagnostics.rs`, `db_repair.rs`(등록 없음)
- 기타: `debug_commands.rs`, `product_details_analytics.rs`(등록 없음)
- 서비스 기반 레거시: `real_actor_commands.rs`(등록 없음/주석 코멘트에 의거 비활성)
- config_commands.rs 내 미사용 Tauri 커맨드(등록되지 않음):
  - `get_site_config`, `build_page_url`, `resolve_url`,
  - `get_default_crawling_config`, `get_comprehensive_crawler_config`,
  - `cleanup_logs`, `get_log_directory_path`

### 실행 계획(안전한 정리 단계)
1) Archive 또는 feature-gate 도입
   - 파일 이동: 위 “사용 중단/레거시” 목록을 `src-tauri/src/commands/archive/`로 이동하거나
     `#[cfg(feature = "legacy-ui")]`/`#[cfg(feature = "dev-tools")]`로 가드합니다.
   - lib.rs에는 등록하지 않습니다(현 상태 유지). 빌드 영향 최소화.

2) config_commands API 표면 축소
   - FE에 노출하지 않는 미사용 `#[tauri::command]`는 아래 중 하나로 처리
     - (권장) 내부 유틸 함수로 전환(annot 제거) 혹은 모듈 분리 `commands/internal/`
     - (대안) `#[cfg(feature = "dev-tools")]`로 가드하여 기본 빌드에서 제외

3) 네이밍/역할 기반 모듈 정리
   - `actor_system_commands.rs` → `actor_system.rs` 등 역할 중심 이름으로 조정(선택)
   - `advanced_engine_api.rs`는 status/info 전용임을 주석과 타입으로 명시

4) 빌드/품질 게이트
   - 단계별 커밋: 이동/가드 후 `cargo clippy --all-targets -- -D warnings`와 테스트 실행
   - FE 타입 생성 영향 없음 확인(`scripts/generate_types.sh`)

### 체크리스트(Commands 정리)
- [x] 레거시 커맨드 파일 아카이브 또는 feature-gate 적용
  - 파일 레벨 cfg 추가: dashboard_commands(legacy-ui), actor_system_monitoring/dev DB 유틸/디버그/애널리틱스/real_actor_commands(dev-tools)
- [x] config_commands 미사용 커맨드 비노출화(annot 제거 또는 가드)
- [x] lib.rs 등록 커맨드 목록과 문서 싱크
- [ ] 네이밍 조정(선택) 및 모듈 경로 정리
- [ ] clippy/테스트 그린 확인 및 문서 갱신

### 진행 업데이트 (2025-09-03)
- `commands/actor_system.rs` 추가: 기존 `actor_system_commands.rs`를 임시 re-export하는 역할 기반 별칭 모듈 도입. `lib.rs`의 커맨드 등록도 새 별칭을 사용하도록 변경. 이후 실제 파일 이동은 단계적으로 진행.
- `db_diagnostics`는 dev-tools 기능 플래그 또는 debug 빌드에서 사용 가능하도록 게이트 조정. 개발 중 “진단 실행” 버튼이 동작하도록 dev 스크립트에 `--features dev-tools` 추가.

다음 단계:
- `actor_system_commands.rs` → `actor_system.rs`로 실제 소스 이동 및 내부 import 경로 정리
- gate 파일 주석/공개 표면 보강(crawl_engine.rs, services/mod.rs)
- dead_code/unused_imports 추가 제거 및 clippy 경고 축소

참고: 사용자가 남긴 `.local/prompts7` 노트는 워크스페이스에서 찾을 수 없어 반영하지 못했습니다. 경로를 공유해 주시면 해당 메모의 세부 항목까지 본 섹션에 병합하겠습니다.
