# UI Metrics Mapping & Persist Normalization

본 문서는 Stage 패널별로 어떤 내부 상태/신호를 사용하여 어떤 지표를 표시하는지, 그리고 Persist 정규화(normalization) 설계가 UI에 어떻게 반영되는지 요약합니다.

## 개요
- Stage 1 (목록 페이지 수집): `listStats()` + 진행률 바 (추정 vs 관측된 detail 시작/완료 카운트 기반)
- Stage 2 (상세 페이지 수집): `detailStats()` + delta 변환(누적 → 증분)으로 과대 집계 제거
- Stage 3 (검증): `validationStats()` 경량 신호 (pagesScanned / divergences / anomalies 등)
- Stage 4 (DB 스냅샷): `dbSnapshot()` (total / minPage / maxPage / inserted / updated)
- Stage 5 (Persist 요약): `persistStats()` (attempted / inserted / updated / duplicates / unchanged / failedTrue / successRate / mode)

## Persist Normalization 핵심
Phase 1 프론트엔드 accumulator가 아래 이벤트들을 통합:
1. 그룹 이벤트: `actor-product-lifecycle-group` (phase=persist) → group-only 모드에서 attempted/duplicates/unchanged 누적
2. 결과 이벤트: `actor-product-lifecycle` (status=persist_*) → result 관측되면 모드가 `mixed`로 전환, inserted/updated 등 실제 결과 기반 누적
3. 빈 결과: status=`persist_empty` → empty 상태 반영
4. Fallback: batch 이벤트(`actor-batch-completed`)에서 persist 이벤트 누락 시 삽입/업데이트 추정 반영(결과 이벤트 등장 시 모드 전환 후 group 값은 duration 누적만)

### Snapshot 구조 (`persistAccumulator.snapshot()`)
| 필드 | 의미 |
|------|------|
| mode | `group-only` 또는 `mixed` (결과 이벤트 관측 여부) |
| attempted | 처리 대상 개수 누적 |
| inserted | 실제 DB 신규 삽입 수 |
| updated | 실제 DB 업데이트 수 |
| duplicates | 중복 판정 수 |
| unchanged | 내용 변화 없음(no-op) |
| succeeded | inserted + updated |
| failed | attempted - succeeded (중간 단계: failedTrue 계산 전) |
| failedTrue | attempted - succeeded - duplicates - unchanged |
| durationMs | 그룹+결과 이벤트의 누적 시간 합 |
| statusCounts | (내부 통계: insertedOnly, updatedOnly, mixed, allDuplicate, noop, failed, empty) |

### UI 사용 필드
PersistPanel 현재 표시: attempted / (inserted+updated) 성공 / failedTrue / duplicates / unchanged / successRate(%) / mode 배지.

`statusCounts`는 내부 품질 분석용으로 유지하지만 **UI에는 노출하지 않음**. 이유:
- 초기 화면 과밀 문제 감소 (인지 부하 최소화)
- 운영 판단에 1차로 필요한 지표는 성공/실패/중복/무변경/성공률/모드
- 필요 시 2단계 진단 패널이나 확장(tooltip, advanced toggle)로 재도입 가능

## Stage 패널 ↔ 신호 매핑
| 패널 | 컴포넌트 | 신호/소스 | 주요 필드 | 비고 |
|------|----------|-----------|-----------|------|
| Stage 1 | StageStatsPanels | `listStats()` | started / completed / failed / retried / inflight | 진행률: completed / (observed 또는 추정) |
| Stage 2 | StageStatsPanels | `detailStats()` (delta 변환) | started / completed / failed / retried / inflight | 누적→증분 변환으로 과대 제거 |
| Stage 3 | ValidationPanel | `validationStats()` | targetPages / pagesScanned / divergences / anomalies / lastPage | lastAssignedStart/End 표시 |
| Stage 4 | DbSnapshotPanel | `dbSnapshot()` | total / minPage / maxPage / inserted / updated | persist 실패 시 batch fallback 가능 |
| Stage 5 | PersistPanel | `persistStats()` | attempted / inserted / updated / duplicates / unchanged / failedTrue / successRate / mode | statusCounts 미표시 |

## 성공률 정의
`successRate = (succeeded / attempted) * 100`, 단 attempted=0이면 0. succeeded = inserted + updated.

## Mode 의미
- group-only: 아직 개별 persist result 이벤트 미수신 → 그룹 레벨(시도/중복/무변경)만 근사
- mixed: 결과 이벤트 출현 → inserted/updated/duplicates/unchanged 실제값 기반 정밀 반영

## 향후 확장 아이디어
- statusCounts breakdown을 Advanced 탭으로 이동 또는 hover tooltip에서 요약 (예: 혼합 배치 비율)
- PersistPanel에 최근 N개 배치 추적(rolling window) 그래프화
- Backend Phase 2: group_id 제공 시 정확한 배치 분류 및 지연 분석

## 변경 이력
- 2025-09-15: 초기 문서 작성 (패널 분리 + persist normalization Phase 1 반영, statusCounts 비노출 전략 기록)

---
문서 개선/확장 필요 시 `docs/persist-metrics-normalization.md`와 동기화하십시오.
