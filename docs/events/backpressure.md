# Backpressure 정책 설계

## 1. 목표
구조화 이벤트 및 크롤링 파이프라인에서 과도한 이벤트/작업 생성으로 인해
- UI 렌더 지연
- 메모리/IPC 큐 폭주
- 중요 이벤트 지연/손실
이 발생하지 않도록 선제적으로 완화(adaptive throttling + prioritization)하는 체계 수립.

## 2. 압력(Pressure) 소스 분류
| 소스 | 신호(Observable) | 영향 |
|------|------------------|------|
| Stage 세부 진행 이벤트 다량 (아이템 단위) | 초당 이벤트 레이트 (EPS) 상승 | IPC 포화, 렌더 부담 |
| 느린 Consumer (FE listener) | 처리 지연 (process_latency_ms) 증가 | 큐 적체, 지연 누적 |
| DB I/O 지연 | query_duration p95 ↑ | Stage 완료 지연 → progress event 폭발 | 
| 재시도 폭증 | retry_ratio ↑ | 동일 아이템 반복 이벤트 증가 |
| 세션 동시성 확대 | active_sessions ↑ | 전역 이벤트 레이트 상승 |

## 3. 계층(Layered) 제어 구조
1. 생성 단 (Stage/엔진)
   - 이벤트 합성(coalescing): 동일 아이템 progress 연속 발생 시 최신값만 유지
2. 브릿지 단 (ActorEventBridge → CrawlEvent)
   - 동적 쓰로틀 슬라이딩 윈도우 (기존 고정 500ms → adaptive)
   - 우선순위 큐 삽입 (priority tiers)
3. Emitter/Queue 단
   - Bounded MPSC ring buffer (capacity N)
   - Backpressure 신호(드롭/지연) 메트릭화
4. Consumer(FE) 단
   - Frame budget aware pull (requestAnimationFrame 시 배치 적용)
   - Batch dispatch (한 frame 당 최대 K 이벤트 통합)

## 4. 이벤트 우선순위 등급
| Priority | 이벤트 타입 | 드롭 허용 | 설명 |
|----------|-------------|-----------|------|
| P0 (Critical) | SessionStarted, SessionCompleted, SessionFailed, StageStarted, StageCompleted, StageFailed(예정) | 불가 | 상태 전이 핵심 |
| P1 (Important) | StageItemCompleted (status=success/fail), OverallProgressUpdate (간헐) | 제한적 (샘플링) | 진행/성과 |
| P2 (Verbose) | StageItemRetrying, 세부 progress granular update | 높음 | 과다 발생 가능 |

## 5. 드롭 & Coalescing 정책
- 동일 Stage / item 에 대한 연속 progress numeric 업데이트는 마지막 1개만 유지 (coalesce window W=300ms)
- P2 이벤트가 큐 70% 초과 시 확률적 샘플링 (기본 30%)
- 큐 85% 초과 시 P2 즉시 드롭 + 드롭 카운터 증가
- 큐 95% 초과 시 P1 중 progress 성격(OverallProgressUpdate) coalescing 강제 (최근 1개만)
- P0 절대 드롭 금지 (큐 full 직전이면 force flush: 낮은 우선순위 제거)

## 6. Adaptive Throttle 알고리즘
Sliding window (최근 2초) 이벤트 레이트 R(EPS) 관측.
- Target EPS(T) = 120 (튜닝 값)
- if R <= T: 기본 최소 interval Imin = 200ms (progress 타입에만 적용)
- if R in (T, 2T]: interval I = Imin * (R/T)
- if R > 2T: interval I = Imin * 2 + penalty * log2(R/T)
- 상한 Imax = 1500ms
- 감소(완화) 히스테리시스: R < 0.6T for 4 윈도우 → I = max(Imin, I / 2)

간단 식:
```
let ratio = R / T;
if ratio <= 1 { I = Imin; }
else if ratio <= 2 { I = Imin * ratio; }
else { I = min(Imax, Imin * 2 + penalty * log2(ratio)); }
```
`penalty` 기본 150ms. (추후 메트릭 기반 조정)

## 7. 샘플링 전략
| 전략 | 적용 범위 | 조건 | 적용 방식 |
|------|-----------|------|-----------|
| 확률 샘플링 | P2 verbose | 큐 사용률 > 0.7 | p=0.3 keep |
| 비율 제한(Leaky bucket) | OverallProgressUpdate | interval throttle 후에도 >5/sec | 초과 즉시 드롭 |
| Coalescing | Numeric progress | window 300ms | 마지막만 유지 |

## 8. 큐 설계
- 자료구조: lock-free bounded ring (crossbeam or tokio mpsc bounded)
- 크기 N 기본 2048 (메모리/폭주 트레이드오프)
- Enqueue 실패(Full) 시 정책:
  1. 낮은 priority candidate 제거 (scan O(k) 제한 k=8)
  2. 실패 시 P2 전체 중 하나 random eviction
  3. 그래도 실패 → 드롭 카운터 + 경고 로그 (once per 5s rate-limit)

## 9. 메트릭 연계 (metrics_plan.md 연동)
| 메트릭 | Backpressure 해석 |
|--------|-------------------|
| event_throttled_total | Adaptive throttle 활성 정도 |
| event_emitted_total / throttled | 실제 전달율 vs 원천 발생율 |
| event_gap_total | (Backpressure 직접 원인 X, 단 조합지표) |
| backpressure_queue_occupancy_percent (신규 Gauge) | 즉시 압력 수준 |
| backpressure_dropped_total (신규 Counter) | 정책 드롭 발생 |
| backpressure_coalesced_total (신규 Counter) | 병합된 이벤트 수 |
| backpressure_sampling_kept_total / dropped_total | 샘플링 효과 측정 |

## 10. Alert/Hints
| 조건 | 심각도 | 액션 |
|------|--------|------|
| queue_occupancy_percent > 90% 1분 지속 | Warning | 큐 크기/샘플 파라미터 검토 |
| dropped_total / emitted_total > 0.05 5분 평균 | Critical | 쓰로틀 파라미터 상향, 이벤트 소스 분석 |
| coalesced_total / emitted_total > 0.40 | Info | Progress 이벤트 과다 빈도 조사 |

## 11. 설정 파라미터 (Configurable)
| 키 | 기본 | 범위 | 설명 |
|----|------|------|------|
| bp.target_eps | 120 | 60–300 | 목표 초당 이벤트 레이트 |
| bp.queue_capacity | 2048 | 512–8192 | 링 버퍼 크기 |
| bp.progress_min_interval_ms | 200 | 100–500 | progress throttle 최소 간격 |
| bp.coalesce_window_ms | 300 | 100–600 | progress coalescing 윈도우 |
| bp.sampling_p2_probability | 0.3 | 0.1–0.5 | P2 유지 확률 |

## 12. 롤아웃 단계
| 단계 | 내용 | 안전장치 |
|------|------|----------|
| Phase 1 | Static throttle 고정값 → ring buffer + 드롭 카운터 도입 | P0 확정 보호 테스트 |
| Phase 2 | Adaptive interval & coalescing | Dry-run 모드(계산만) 1일 |
| Phase 3 | P2 샘플링 활성 | 샘플링 비활성 toggle 제공 |
| Phase 4 | Priority eviction & eviction 메트릭 | 로깅 강화 |
| Phase 5 | 파라미터 동적 조정 (config reload) | 변경 감사 로그 |

## 13. 테스트 전략
- 단위: 알고리즘 함수 (interval 계산) 입력 R 값 스냅샷 검증
- 부하 시뮬: 10k synthetic events -> 최종 emit 수/드롭 비율 기대 범위 체크
- P0 보존 테스트: 혼합 스트림에서 P0 드롭=0 assert
- 회귀: 파라미터 변경 시 historical baseline 비교

## 14. 리스크 & 완화
| 리스크 | 설명 | 완화 |
|--------|------|------|
| 과도한 throttle 로 UI 갱신 지연 | Progress 반응성 저하 | Imax 제한 및 히스테리시스 다운조정 |
| 샘플링으로 진짜 문제 은닉 | 중요 실패 이벤트 손실 | P0/P1 실패는 절대 샘플링 제외 |
| Priority eviction 비용 | 큐 스캔 비용 증가 | 상위 k=8 제한 + O(1) random fallback |

## 15. Acceptance Criteria (Phase 1)
- 큐 capacity 설정 및 occupancy gauge 노출
- 드롭/쓰로틀/emit 카운터 모두 증가 확인되는 통합 테스트
- P0 이벤트 100% 전달 검증
- Synthetic burst (EPS = 5T) 시 progress 이벤트 일부 throttle/드롭 기록

(End of Backpressure Policy Doc)
