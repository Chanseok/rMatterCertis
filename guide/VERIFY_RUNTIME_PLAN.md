# Runtime Crawling Plan Verification (Log-Based)

목표: 코드 내 mock / 복잡한 단위 테스트 없이 실제 실행 로그를 기반으로 단일 계획 수립과 일관된 배치 실행을 검증합니다.

## 검증 항목
1. 단일 📋 `CrawlingPlan created` 로그만 존재 (중복 계획 금지)
2. Plan 내 `phase_type: ListPageCrawling` 의 개수 = Stage 실행 로그
   - `Starting Stage 2: ListPageCrawling` 개수 (시작)
   - `Stage 2 (ListPageCrawling) completed` 개수 (완료)
3. `RangeLoopAnomaly` 경고가 없어야 함
4. `Partial page re-included` 로그는 0 또는 1회 (조건: 마지막 DB 페이지가 partial)
5. (선택) 각 ListPageCrawling phase 의 pages 배열은 최신→과거 내림차순
6. (모니터링) `📋 배치 계획 수립:` 로그와 phases 구조의 불일치가 없어야 함

## 스크립트 사용
```
./scripts/verify_runtime_plan.sh path/to/logfile.log
```
또는:
```
LOG_FILE=path/to/logfile.log ./scripts/verify_runtime_plan.sh
```

## 종료 코드 의미
| Code | 의미 |
|------|------|
| 0 | 성공 |
| 10 | 로그 파일 없음 |
| 11 | CrawlingPlan 로그 미검출 |
| 12 | CrawlingPlan 로그 중복 |
| 13 | Plan ListPageCrawling phase 수 ≠ Stage 2 start 수 |
| 14 | Plan ListPageCrawling phase 수 ≠ Stage 2 completion 수 |
| 15 | RangeLoopAnomaly 경고 발견 |
| 16 | Partial page reinclusion 2회 이상 |
| 17 | 페이지 내림차순 위반 (실험적) |

## 절차
1. 크롤링 세션 실행 (SessionActor 경유).
2. 세션 종료 후 `target/debug/logs/*.log` 혹은 지정 로그 파일 경로를 스크립트에 전달.
3. 스크립트 결과가 `[OK]` 여야 하며 실패 시 반환 코드와 메시지로 원인 파악.

## 문제 발생 시 대응 가이드
| 실패 유형 | 원인 후보 | 조치 |
|-----------|-----------|------|
| 12 (중복 계획) | SessionActor 외부 재계획 경로 미제거 | 외부 planner 호출 제거 / 단일 진입점 확인 |
| 13/14 | phases → Stage 실행 전환 로직 누락 또는 조기 종료 | SessionActor batch loop / StageActor 에러 로그 점검 |
| 15 | 레거시 range loop 또는 강제 중단 | range 관련 코드 제거 / 종료 조건 점검 |
| 16 | partial 재삽입 이중 실행 | planner 내부 재삽입 로직 중복 호출 여부 확인 |
| 17 | 페이지 순서 재정렬 누락 | planner newest-first 정렬 로직 확인 (sort_by b.cmp(a)) |

## 향후 확장 아이디어
- JSON 구조 로그 (target=kpi.execution_plan) 를 jq 활용해 더 정교한 검증
- GitHub Actions에서 nightly crawl 후 스크립트 실행 자동화
- 실패 시 마지막 CrawlingPlan 라인을 별도 아티팩트로 보존

---
본 절차는 단순성(코드 변경 최소) + 운영 환경과의 동일성(실제 HTTP/DB 경로) 확보를 위해 설계되었습니다.
