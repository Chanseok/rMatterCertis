# Crawl Events Catalog (Schema v1)

이 문서는 신규 구조화 이벤트 시스템(`crawl_updates` 채널, discriminated union)에서 발행되는 이벤트 타입을 정의합니다.

## 공통 메타 필드
| 필드 | 타입 | 설명 |
|------|------|------|
| event_type | string | Discriminant (PascalCase) |
| event_id | string (UUID) | 이벤트 인스턴스 고유 식별자 |
| schema_version | number | 스키마 버전 (현재 1) |
| sequence | number | 백엔드 단조 증가 시퀀스 (중복/순서 감지) |
| timestamp | string (RFC3339) | 발생 시각 (UTC) |
| session_id | string | 크롤링 세션 식별자 |
| batch_id | string? | (선택) 배치 식별자 |
| message | string? | 사람 친화적 메시지 (UI Helper) |

Additive Only Rule: 기존 필드 제거/타입 변경은 금지. 신규 필드는 optional로 추가.

## 이벤트 타입 상세

### CrawlSessionStarted
필드: total_stages?
의미: 세션이 시작되었으며 스테이지 계획이 확정되었을 때.

### CrawlSessionCompleted
필드: total_duration_ms?, successful_items_count?, failed_items_count?
의미: 정상 종료. 통계 집계 완료.

### CrawlSessionFailed
필드: error_code?, error_message, retriable?
의미: 세션 치명적 오류로 중단.

### StageStarted
필드: stage_name, stage_index, total_items_in_stage?, batch_id?
의미: 특정 Stage 실행 시작.

### StageProgress
필드: stage_name, stage_index, current_item_index?, total_items_in_stage?, progress_percentage?
의미: Stage 진행 상황 주기/스로틀 업데이트.

### StageItemStarted
필드: stage_name, item_id, batch_id?
의미: 개별 아이템 처리 시작.

### StageItemCompleted
필드: stage_name, item_id, duration_ms?, batch_id?
의미: 아이템 처리 완료.

### StageItemFailed
필드: stage_name, item_id, error_code?, error_message, retry_possible?, batch_id?
의미: 아이템 처리 실패.

### StageItemRetrying
필드: stage_name, item_id, retry_attempt, max_retries?, reason?, batch_id?
의미: 실패 후 재시도 진행.

### OverallProgressUpdate
필드: current_stage_index?, total_stages?, overall_progress_percentage?, completed_items_count?, total_items_count?
의미: 전체 세션 진행 상황(집계 레벨) 업데이트.

## 예제 JSON (StageItemCompleted)
```json
{
  "event_type": "StageItemCompleted",
  "event_id": "7c0c9d5c-8a2c-4b9d-9b71-9c9b3ecc1c11",
  "schema_version": 1,
  "sequence": 42,
  "timestamp": "2025-09-20T00:58:37.919Z",
  "session_id": "actor_session_1758297517",
  "batch_id": "actor_session_1758297517-pre-1",
  "stage_name": "list_page_crawling",
  "item_id": "https://example.com/page/1",
  "duration_ms": 123
}
```

## 마이그레이션 전략 요약
1. Dual Emission: 기존 `actor-event` 유지, 신규 `crawl_updates` 병렬 송출.
2. FE Dual Listener: 새 이벤트 파싱 성공 시 우선 사용.
3. Metrics 검증: 스키마 파싱 실패수/성공률 추적.
4. Cutover: 안정화 후 구 채널 Deprecation 경고.
5. 제거: 구 채널 제거 & schema_version 관리.

## 검증 / 테스트
- Rust: snapshot 테스트 (serde_json 직렬화 결과 고정)
- FE: 타입 가드 + 예제 fixture 파싱.

## 향후 확장 예정 (예약 필드)
- severity?, source?, error taxonomy 확장
- Heartbeat, RateLimitHit, NetworkDegraded 등 운영 이벤트

---
이 파일 업데이트 시 CHANGELOG(Event Schema)도 함께 갱신하십시오.
