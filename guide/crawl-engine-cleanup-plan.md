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

- 준비
  - [x] 작업 브랜치 생성: `crawl-engine-cleanup`
  - [x] 최신 main에 변경 반영 및 푸시
  - [ ] 안전망 확인: 기본 빌드/테스트/린트 그린 상태 확인
  - [x] 인벤토리 1차 스냅샷 생성(구조 파악):
    - crawl_engine 루트: actor_event_bridge.rs, actor_system.rs, actors.rs, channels.rs, config/, context/, events/, integrated_context.rs, runtime/, services/, stages/, system_config.rs, ts_gen.rs, validation/
    - actors/: batch_actor.rs, contract.rs, session_actor.rs, stage_actor.rs, traits.rs, types.rs
    - channels/: types.rs (게이트: channels.rs)
    - stages/: mod.rs, strategies/default/*, traits.rs
    - services/: crawling_integration.rs, crawling_planner.rs, data_consistency_checker.rs, data_quality_analyzer.rs, performance_optimizer.rs, planning_service.rs, real_crawling_commands.rs, real_crawling_integration.rs, mod.rs
  - [x] 빈/미사용 스텁 제거: channels/channels.rs, services/services.rs 삭제
- 얇은 사전 정리(삭제 전용, API 영향 없음)
  - [ ] 명백한 죽은 파일/폴더 제거(주석만, 실험/백업 잔재, 완전히 미사용 테스트 등)
  - [ ] 아카이브 폴더는 유지하되 프로덕션 경로 의존성 차단 확인
  - [ ] 중복 구현/이름만 다른 파일 통합 계획 수립
- 구조 정리(mod.rs 제거 및 게이트 파일화)
  - [ ] crawl_engine 루트: `src-tauri/src/crawl_engine.rs`에서 하위 모듈 선언 및 재익스포트 정리
  - [ ] 각 디렉토리에서 `mod.rs` 제거, `모듈명.rs`/게이트 파일로 통일
  - [ ] 파일/모듈 이름을 역할 기반으로 정리(예: `actor_system.rs` → `system.rs`)
- 코드 정리(모듈 단위 반복)
  - [ ] dead_code/unused_imports 제거(도구: rust-analyzer, clippy)
  - [ ] 불필요한 `clone()` 제거: `&T`, `&str`, `&[T]`, `Arc<T>`, `Cow<'_ , T>` 적용
  - [ ] 함수 시그니처 정리: 입력→출력(가능하면 stateless), 명확한 에러 타입
  - [ ] 명확한 네이밍/단일 책임: 거대 파일 분리 또는 공통부 통합
- 횡단 관심사 정리
  - [ ] PlanningService/Strategy 패턴 정착(자동/수동/재개 전략 확장 용이)
  - [ ] CrawlingPolicy 명시화: 재시도/중복/성능 옵션을 구조체로 관리하고 ExecutionPlan에 포함
- 타입/프론트 연동
  - [x] ts-rs 기반 타입 자동 생성 재검증(`scripts/generate_types.sh`)
  - [x] 타입 변경 시 생성물 갱신 및 FE 타입 정합성 확인(프로젝트 TS 타입체크 실행)
- 품질 게이트
  - [ ] cargo check
  - [ ] cargo test --all-features
  - [ ] cargo clippy --all-targets -- -D warnings
    - 현 상태: clippy 경고 다수 존재(범위: commands/*, bin/* 중심). crawl_engine 모듈 리팩토링 중 단계적으로 해결 예정.
  - [ ] (선택) cargo +nightly udeps — 미사용 의존성 제거

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
