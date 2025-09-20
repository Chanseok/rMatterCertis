# Prometheus Metrics Plan

## 1. 목적
크롤링 파이프라인 및 이벤트 서브시스템 헬스/성능/품질을 가시화하여 다음을 달성:
- 병목 지점 식별 (Stage 처리율, 재시도 비율)
- 신뢰성 지표 추적 (실패율, 시퀀스 갭, 이벤트 드롭)
- 용량 계획 (동시 세션, 이벤트 레이트, DB I/O)
- SLA/에러 버짓 관리 (error_category 기반 비율)

## 2. 수집 주체 / 노출 방식
| 컴포넌트 | 수집 방법 | 방식 |
|----------|-----------|------|
| 백엔드 Rust (Tauri) | 내장 HTTP exporter (feature flag) | Pull (Prometheus scrape) |
| 향후 독립 크롤링 서비스 (서버 모드) | Actix/Tonic exporter | Pull |
| 프론트엔드 측 경량 지표 (선택) | window.performance + postMessage -> backend counter | Push (internal channel) |

초기: 백엔드 프로세스에 `/metrics` (로컬) 바인딩. Tauri 앱 특성상 보안: loopback 인터페이스 + opt-in.

## 3. 네이밍 규칙
`mattercertis_<domain>_<metric>`
- 도메인 예: `crawl`, `event`, `stage`, `db`, `http`
- Counter: `_total` 접미사
- Histogram: `_seconds` (시간) / `_bytes`
- Gauge: 상태 값, 접미사 없음

## 4. 메트릭 정의 (초기 세트)
### 4.1 Event Channel
| 이름 | 타입 | 라벨 | 설명 |
|------|------|------|------|
| `mattercertis_event_emitted_total` | Counter | `event_type` | 성공적으로 emit 된 구조화 이벤트 수 |
| `mattercertis_event_emit_fail_total` | Counter | `event_type`, `error_category` | emit 실패(IPC/직렬화) 카운트 |
| `mattercertis_event_throttled_total` | Counter | `event_type` | 쓰로틀로 생략된 이벤트 |
| `mattercertis_event_gap_total` | Counter |  | 시퀀스 갭 감지 건수 |
| `mattercertis_event_version_mismatch_total` | Counter | `found_version` | 스키마 버전 불일치 |

### 4.2 Stage / 처리 흐름
| 이름 | 타입 | 라벨 | 설명 |
|------|------|------|------|
| `mattercertis_stage_started_total` | Counter | `stage_name` | Stage 시작 횟수 |
| `mattercertis_stage_completed_total` | Counter | `stage_name` | Stage 성공 종료 횟수 (추론: 모든 item 처리) |
| `mattercertis_stage_failed_total` | Counter | `stage_name`, `error_category` | Stage 수준 실패 |
| `mattercertis_stage_item_processed_total` | Counter | `stage_name`, `status`(success|fail) | Stage 아이템 처리 결과 |
| `mattercertis_stage_item_retry_total` | Counter | `stage_name` | 재시도 발생 |
| `mattercertis_stage_duration_seconds` | Histogram | `stage_name` | Stage wall-clock 소요 시간 |

### 4.3 세션 / 상위 흐름
| 이름 | 타입 | 라벨 | 설명 |
|------|------|------|------|
| `mattercertis_session_started_total` | Counter |  | 세션 시작 수 |
| `mattercertis_session_completed_total` | Counter |  | 세션 정상 완료 수 |
| `mattercertis_session_failed_total` | Counter | `error_category` | 세션 실패 수 |
| `mattercertis_session_duration_seconds` | Histogram |  | 세션 전체 duration (start~end) |
| `mattercertis_session_items_total` | Gauge |  | 현재 세션 목표 아이템 총량(동적으로 업데이트) |
| `mattercertis_session_overall_progress_percent` | Gauge |  | 최근 OverallProgressUpdate 기반 진행률 |

### 4.4 DB / Storage (선택 포함)
| 이름 | 타입 | 라벨 | 설명 |
|------|------|------|------|
| `mattercertis_db_query_duration_seconds` | Histogram | `query_class` | 주요 쿼리 그룹별 실행 시간 |
| `mattercertis_db_pool_acquire_seconds` | Histogram |  | 커넥션 획득 지연 |
| `mattercertis_db_errors_total` | Counter | `error_class` | DB 오류 카운트 |

### 4.5 HTTP / 네트워크 (옵션)
| 이름 | 타입 | 라벨 | 설명 |
|------|------|------|------|
| `mattercertis_http_request_duration_seconds` | Histogram | `target`(site|api) | 요청 지연 |
| `mattercertis_http_errors_total` | Counter | `target`, `error_category` | HTTP 레벨 실패 |

## 5. 레이블 전략 & Cardinality 가이드
- `stage_name`: Stage 타입 수 제한적 (저카디널리티)
- `event_type`: enum 고정 (저카디널리티)
- `error_category`: taxonomy 통제 (SessionFailure, StageFailure, Network, Parse, Validation, Storage, Internal, Unknown)
- 피해야 할 것: 동적 URL, item_id, session_id 직접 라벨 사용 금지 (→ 메트릭 폭증 위험)
- 세션/아이템 특정 분석은 로그/Replay 도구로 분리

## 6. 수집 경로 & Export 흐름
```
[CrawlEvent Bridge] -> (atomic counters + hist buckets) -> /metrics scrape -> Prometheus -> Grafana
```
- Stage/Session duration: 시작 시 timestamp 기록, 종료 시 observe
- Retry/Fail: 이벤트 수신 지점에서 즉시 inc

## 7. 구현 단계 (Roadmap)
| 단계 | 내용 | 완료 조건 |
|------|------|-----------|
| v1 | Counter 5개 (event emit/ fail/ throttled/ gap/ version_mismatch) | /metrics 노출, 값 증가 확인 |
| v1.1 | Session & Stage counters + duration 히스토그램 | 세션 단위 그래프 생성 |
| v1.2 | Retry/Item 처리 counters | 실패율 대시보드 |
| v1.3 | DB / HTTP histogram 통합 | P95 노출 |
| v1.4 | Error taxonomy별 비율 계산 패널 | 분포 그래프 |
| v2 | Adaptive sampling (저빈도/고빈도 구분) | 높은 QPS 시 CPU 안정 |

## 8. Histogram 버킷 제안
- Stage duration: `[0.5, 1, 2, 5, 10, 30, 60, 120]` (seconds)
- Session duration: `[30, 60, 120, 300, 600, 1200, 1800]`
- DB query: `[0.005, 0.010, 0.025, 0.050, 0.1, 0.25, 0.5, 1.0]`

## 9. 성능 고려
- Atomic inc 비용 미미: 고빈도(StageItemCompleted 등)도 허용
- Histogram observe 는 상대적으로 비용↑ → 고빈도 path (아이템 단위) 에 남용 금지
- 필요 시 Counter + TDigest(미래) 로Latency 추정 실험 고려

## 10. 에러 핸들링 / 강건성
- Export 실패(포트 점유 등) → 경고 로그 후 비활성화 fallback
- 메트릭 레지스트리 중복 등록 → lazy_init + once_cell 방어
- Panic 안전: observe 시 unwrap 금지

## 11. 테스트 전략
- 단위: emit 함수 호출 후 counter 값 assert
- 통합: 인메모리 registry 사용해 /metrics endpoint scrape 결과 텍스트 검사
- 회귀: Replay 도구로 synthetic 이벤트 1000개 ingest 후 기대 카운트 비교

## 12. 보안
- Tauri 로컬 앱: 127.0.0.1 바인딩 + 랜덤 high 포트 → UI 디버그 화면에 포트 노출
- 서버 모드 전환 시 Basic Auth(옵션) 또는 IP allowlist

## 13. Grafana 대시보드 초안 (섹션)
1. Sessions Overview: Started/Completed/Failed rate + Duration histogram
2. Stage Throughput: per stage success vs fail stacked area
3. Event Health: emit vs throttled vs gaps
4. Error Categories: pie + time series stacked
5. DB & HTTP Latency: P50/P95/P99

## 14. 향후 확장
- Exemplars (에러 이벤트에 trace id 연결)
- OpenTelemetry metrics bridge (prom -> otlp)
- Dynamic bucket tuning (관측 분포 기반 재구성)

## 15. Acceptance Criteria (v1)
- `/metrics` 호출 시 최소 5개 counter 노출
- 이벤트 10개 emit 후 `event_emitted_total` 증가값 정확
- 의도적 gap 1회 발생 시 `event_gap_total` +1
- Version mismatch 시 `event_version_mismatch_total` +1
- Throttle 조건 충족 시 `event_throttled_total` 증가

(End of Plan)
