# 2025-09-14 Progress & Attempt Event Enhancements

## 변경 사항
 ProductDetailCrawling 단계: Collector 결과 수집 이후 replay 방식으로 per-product incremental 스냅샷 생성 (progress_emitter 활용). 최종 스냅샷은 partial 생략 & done == group_size.
 Persist 단계(ProductLifecycleGroup, phase="persist"): 모든 최종 스냅샷에 done 필드 채움 (group_size == attempted_count). guard/skip/empty/fallback/error 경로 모두 일관성.

 Detail 단계 replay: detail phase ProductLifecycleGroup 이벤트들은 timestamp 증가 순으로 per-product 단위 done 값을 1..N..group_size 로 올리며 partial=true. 마지막 이벤트만 partial 없음.
 Persist 단계: 항상 단일 (혹은 에러/스킵 경로) 최종 이벤트 1개, partial 미사용, done=group_size.
- 키 구성: `(session_id, batch_id || 'none', phase)`.
~ProductDetailCrawling per-product incremental 이벤트~ 구현 완료 (replay 방식).
~Persist 단계 done 누락~ 모든 persist 이벤트 done 필드 채움 완료.
추가 개선 여지: Detail 단계에서 실시간(컬렉션 중) 스트리밍하려면 Collector 내부 루프 직접 계측 필요 (현재는 post-collection synthetic replay).
- ProductDetailCrawling 에 대한 per-product incremental ProductLifecycleGroup 이벤트 (현재는 그룹 완료 단일 이벤트만). → 세부 전략 로직 내부(상세 페이지 개별 fetch loop) 계층에 계측 필요.
- Persist 단계에서도 `done` 누적 방식 통일 (현재는 필요시 스냅샷만). 필요 시 동일 패턴 적용.

## 마이그레이션 노트
- 이전 UI 코드가 ProductLifecycleGroup variant를 패턴 매칭할 때 새 필드를 무시해도 호환 유지 (optional).
- Attempt 이벤트 필터링을 위해 사용하던 `MC_ATTEMPT_EVENTS` 제거: 필요 시 런타임 필터(로그 레벨 or 프론트단 discard) 적용 권장.

## 테스트 권장 항목
- 다양한 batch 크기에서 ListPageCrawling 진행률 바가 0→1.0 선형 증가 확인.
- Out-of-order 이벤트 (인위적으로 timestamp 조작) 시 reducer가 무시하는지 확인.
- Attempt 실패/재시도/타임아웃 시 UI 노이즈 허용 범위 점검 후 필요시 프론트단 샘플링.

---
작성: 자동화 도구 (progress enhancements summary)
