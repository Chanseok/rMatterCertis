# 로컬 DB 대시보드 구현/개선 계획 (갱신됨 2025-09-10)

단일 탭(기존 `로컬DB`)에서 로컬 데이터베이스 운영/분석/유지보수를 수행하는 기능을 단계적으로 구축한다. 중복 탭(`Local DB`)은 제거하고, 향후 필터 DSL 반영을 위한 구조를 먼저 정리한다.

## 1. 목표
1. 핵심 요약 지표 조회
2. 데이터 Export / Import (CSV → 향후 XLSX 고려)
3. 범위 제한 정리 작업 (페이지 범위 삭제, 벤더 동기화, Device Types JSON 편집/재시드)
4. 경량 DSL 기반 분석 뷰 필터링
5. 안전장치(백업, 프리뷰, 트랜잭션) 확보 및 향후 확장 대비 구조화

## 2. 단계 (Phase) 정의
| Phase | 범위 | 산출물 | 비고 |
|-------|------|--------|------|
| 1 | 백엔드 기초 (Summary + Analytics 기본 + DSL Stub) | `get_db_summary`, `analytics_query` | 페이지네이션 기본형 |
| 2 | Export / Import (CSV) | `export_data`, `import_data` + 백업 | vendors/device_types, analytics는 export 전용 |
| 3 | 페이지 범위 삭제 | `preview_delete_range`, `delete_range` | 50페이지 가드, 카운트 프리뷰 |
| 4 | 벤더 동기화 래퍼 | `dashboard_vendor_sync` | 진행 이벤트(start/up_to_date/finished) |
| 5 | Device Types JSON 편집 백엔드 | `get_device_types_json`, `save_device_types_json(reseed)` | 원자적 쓰기 + 백업 |
| 6 | 초기 프론트엔드 연결 | 대시보드 UI 기본 골격 | (기존 UI + 신규 시도) |
| 6B | FE 재정비(이번) | 단일 탭 통합, 상태 Store, 섹션 재구성 | DSL 자리 확보(Mock) |
| 7 | DSL 고도화 | 실제 파서 + 비교/부분/범위 연산 | 에러 메시지, 바인딩 안전성 |
| 8 | 품질 & 하드닝 | 테스트, 캐싱, 성능 | 선택적 최적화 |

## 3. 데이터 소스
- 테이블: `products`, `product_details`, `vendors`, `device_types`
- 브리지: `product_primary_device_types`
- 뷰: `v_product_detail_analytics`

## 4. Summary 지표 (Phase 1)
- total_products / total_product_details / total_vendors / total_device_types
- new_products_24h / new_products_7d
- top_device_categories (상위 5개; 없으면 빈 배열)

## 5. Analytics Query (현재)
정렬: `detail_created_at DESC`
필드: product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at
입력: offset + limit (limit 최대 200)
필터 DSL: 아직 Stub (무시)

## 6. DSL 사양 초안 (Phase 7 구현용)
목표: 사용자 입력 문자열 → 안전한 WHERE 절 + 바인딩 파라미터 목록

### 6.1 토큰 형태
1. `field:값` 패턴 (연산자 생략 시 기본은 부분(~) 또는 정확(=) 중 필드 타입별 결정)
2. `field=값`, `field~값`, `field>=값`, `field<=값`
3. 단순 단어(필드 지정 없음)는 기본 검색 대상(예: model LIKE, vendor_name LIKE) 다중 OR 묶음

### 6.2 지원 필드 매핑
| 입력 | 내부 컬럼 |
|------|-----------|
| vendor, v, ven | vendor_name |
| vnum | vendor_number |
| category, cat | device_category |
| dtype, dname, dt | device_type_name |
| model, m | model |
| date | certification_date |
| created, c | detail_created_at |

### 6.3 연산자 의미
- `=`: 정확 일치 (대소문자 기본 그대로; 필요 시 COLLATE NOCASE 고려)
- `~`: 부분 일치 (`%...%`)
- `>=`, `<=`: 날짜/숫자 비교 (필드가 날짜형이면 ISO 비교 문자열, 숫자이면 CAST)

### 6.4 구문 규칙
- 공백으로 토큰 분리 (따옴표 포함 구문은 Phase 7.5 이후 고려; 초기엔 미지원 시 에러)
- 잘못된 토큰 → 전체 결과 0 + `filter_error` 메시지 (HTTP 200)
- 1개 필드에 다수 조건 → AND
- 서로 다른 필드 토큰 → AND
- 필드 없는 단어 여러 개 → (model LIKE ? OR vendor_name LIKE ?) AND … (다른 필드 토큰들)

### 6.5 에러 포맷 예시
`{"filter_error":"Unexpected token '>=abc' after field 'date' (expect YYYY-MM-DD)", "rows":[], "total":0}`

### 6.6 반환 확장 (Phase 7)
`applied_filter_sql` (디버깅용, 바인딩 플레이스홀더만 표시), `filter_tokens` 배열(파싱 결과)

## 7. 안전장치 요약
- Import: 실행 전 CSV 백업 `exports/backup_<dataset>_<ts>.csv`
- Device Types JSON: 기존 파일 timestamp 백업 + tmp → rename
- Delete Range: 사전 preview + 50페이지 제한
- 벤더 동기화: dry_run 경로 제공

## 8. Phase 별 진행 현황 (업데이트)
| Phase | 상태 | 메모 |
|-------|------|------|
| 1 ~ 5 | 완료 | 백엔드 명령 동작, 기본 검증 OK |
| 6 (초기) | 완료 | `LocalDbDashboard` 시도 (중복 탭 문제 발생) |
| 6B (재정비) | 진행중 | 탭 통합, Store 구성, Maintenance / Device Types / Vendor Sync Dry Run / Diff / 이벤트 일부 구현 |
| 7 | 대기 | DSL 사양 확정(본 문서) → 구현 준비 완료 |
| 8 | 대기 | 테스트/캐싱/성능 패스 예정 |

## 9. 6B (FE 재정비) 세부 계획
1. 탭 정리: `localDbDashboard` 제거, 기존 `localDB` 탭에 기능 통합
2. 상태 Store 생성: summary, analytics, loading/error, filterDraft, filterApplied
3. 섹션 구조: Summary / Analytics / Maintenance(Export, Import, Delete) / Vendor Sync / Device Types Editor / Filter(Placeholder)
4. 공통 로딩 & 에러 UI 컴포넌트화
5. DSL 입력창 + 적용/초기화 버튼 (백엔드 미구현 시 mock 결과)
6. Device Types 편집: 변경 라인 diff(간단: 기존 JSON → parse → ID/Name 비교) 추후
7. Vendor Sync 진행 이벤트 수신 (향후 page 단위 event 확장 예정)

## 10. 이후 우선순위 (요약)
1. 6B 구현 → UI 단일화 및 상태 정리
2. Phase 7 DSL 파서 + 테스트
3. Vendor Sync 중간 진행율/취소
4. Device Types prune 옵션 + dry-run 검증
5. 성능/캐싱 + 통합 테스트 스위트

## 11. 테스트 항목 초안 (Phase 7 포함)
- DSL: 유효/에러 케이스 파싱, SQL 바인딩 개수 매칭
- Export/Import: round-trip 동일 행 수
- Delete Range: 프리뷰 수 == 실제 삭제 수
- Device Types reseed: inserted/updated 합 = JSON 길이
- Vendor Sync dry_run: will_sync > 0 → full sync 후 local_count 증가

## 12. 오픈 이슈 / 추후 결정
- DSL 인용부호/정확 다단어 검색 지원 범위
- Timezone 처리 (detail_created_at 비교 시 UTC vs local)
- 대량 analytics 페이지네이션 성능 (필요 시 인덱스 추가)
- Summary 캐싱 TTL (예: 5초) 도입 여부

---
문서 한글화 및 6B 재정비 계획 반영 완료.
