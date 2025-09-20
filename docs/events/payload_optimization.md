# 이벤트 페이로드 최적화 전략

## 1. 목표
- IPC / JSON 직렬화 오버헤드 감소
- FE 파싱 비용 & 메모리 footprint 축소
- 네트워크 (향후 원격 모드) 전송량 절감
- 구조화 이벤트 스키마 안정성 유지하면서 선택적 최소화 경로 제공

## 2. 현재 문제 징후 (예상)
| 징후 | 원인 | 영향 |
|------|------|------|
| Progress 이벤트 빈번히 중복 필드 반복 | 정적 메타(세션, stage) 매 이벤트 포함 | 불필요한 직렬화 비용 |
| 문자열 키 길이 과다 | snake_case 긴 키 반복 | JSON 크기 증가 |
| 중간 상태 numeric 이 급격히 많음 | item-level granular update 과다 | 렌더/GC 부담 |

(실측은 추후 metrics + 샘플 파싱 프로파일링 예정)

## 3. 원칙
1. 가독성 < 성능 필요 시 "Short Form" 채널 별도 도입 (기본은 Readable)
2. 필수/옵셔널 필드를 명확히 (옵셔널은 Null 생략)
3. Enumerated string → compact code (단, FE 매핑 테이블 캐싱)
4. 구조적 재사용 (session/stage context 캐시)

## 4. 최적화 레이어
| 레이어 | 전략 | 우선순위 |
|--------|------|----------|
| Schema 레벨 | 필드 분리: static context vs dynamic delta | High |
| 직렬화 레벨 | serde `skip_serializing_if` 활용 | High |
| 전송 레벨 | Short key alias map | Medium |
| Aggregation | 여러 low-value progress 합산 후 배치 | High |
| 압축 (미래) | zstd frame (원격 transport 시) | Low |

## 5. Static vs Dynamic Context 분리
- Static (세션 시작 시 1회 전송): `session_id`, `total_items`, `origin`, `schema_version`
- Stage static (StageStarted): `stage_name`, `planned_count`
- Dynamic delta: `completed_count`, `failed_count`, `retrying_item_id`, `progress_percent`

FE 측 캐시 구조:
```
contextCache = {
  session: {...},
  stages: { stage_name: {...static...} }
}
```
수신 이벤트는 delta만 포함 → 머지 후 최종 뷰 구성.

## 6. Short Form Key 매핑 (옵션 채널)
| Long | Short |
|------|-------|
| event_type | t |
| sequence | s |
| session_id | si |
| stage_name | gn |
| completed_count | cc |
| failed_count | fc |
| progress_percent | pp |
| error_category | ec |

Short 채널: `crawl_updates_compact`
- Dual emission 기간: 기존 readable + compact (옵션)
- Opt-in 설정: FE가 지원시 compact 구독

## 7. Enum 압축
| Enum | 전략 |
|------|------|
| event_type | 이미 discriminator → Short form 시 1~N code map (e.g. SessionStarted=1) |
| error_category | 소량 집합 → 1바이트 코드 (Rust -> u8) |

FE 초기 bootstrap 에 code→string 테이블 주입

## 8. serde / Rust 구현 전술
- `#[serde(skip_serializing_if = "Option::is_none")]` 모든 Option 필드 적용
- Compact 변환: 별도 struct `CompactCrawlEvent` + `From<&CrawlEvent>` 구현
- Emission 경로에서 feature/flag 로 분기

## 9. 배치(Aggregation) 정책
- Progress (P2) 아이템-level 업데이트: 내부 aggregator가 250ms 윈도우에서 마지막 값만 flush
- 전체 progress 계산: 마지막 StageItemCompleted 시점에만 OverallProgressUpdate 생성 (중간 연속 생략)

## 10. 예상 절감 효과 (예비 산정)
| 전략 | 절감(크기) | 근거 |
|------|-----------|------|
| Static context 제거 | ~25% | 반복 head 필드 제거 |
| Short key | 추가 10–15% | 키 길이 평균 8→2 |
| skip_serializing_if | 5–10% | Null/Empty 생략 |
| Aggregation | 이벤트 수 감소 (최대 50% P2) | 중복 progress 억제 |

## 11. 호환성 전략
- Schema version 증가 없음 (Readable 유지)
- Compact = 파생 표현 (derivative format) → schema_change_log 에 "non-breaking" 기록
- Replay 도구: readable 포맷 우선, compact 는 변환 후 ingest

## 12. 롤아웃 단계
| 단계 | 내용 | 검증 |
|------|------|------|
| Phase 1 | skip_serializing_if + progress aggregation | 이벤트/byte 전후 비교 로그 |
| Phase 2 | Static vs Delta 분리 구현 | 세션 시작 후 delta만 수신 FE 검사 |
| Phase 3 | Compact 포맷 파생 출력 | 동일 의미 diff 테스트 (snapshot) |
| Phase 4 | FE compact 구독 옵셔널 지원 | Fallback 정상 |
| Phase 5 | 최종 크기/레이트 보고서 작성 | 절감 % 문서화 |

## 13. 테스트
- Snapshot: readable vs compact → semantic 동등성(assert 재구성 후 eq)
- Fuzz: random event stream → delta merge 후 상태 모델과 일치
- Size benchmark: 1000 synthetic events 직렬화 전/후 byte 합산

## 14. 메트릭 연계
신규:
- `payload_bytes_total` (Counter, readable vs compact label)
- `payload_events_compacted_total` (Counter)
- `payload_progress_aggregated_total` (Counter)
- `payload_avg_event_size_bytes` (Gauge: sliding window 평균)

## 15. Acceptance Criteria (Phase 1)
- skip_serializing_if 적용된 옵션 필드 100% 확인
- Progress aggregation 윈도우 적용 후 P2 이벤트 수 20% 이상 감소(샘플 런)
- 기능 플래그 비활성 시 기존 출력 동일

(End of Payload Optimization Doc)
