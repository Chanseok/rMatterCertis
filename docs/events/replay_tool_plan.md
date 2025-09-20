# Replay Tool Plan

## 1. 목적 (Why)
크롤링 세션 동안 발생한 `CrawlEvent` 스트림을 재생(Replay)하여 다음을 가능하게 한다:
- 프론트엔드 UI 회귀 테스트 (구조화 이벤트만으로 상태 재구성 검증)
- 성능/병목 분석: 타임라인 기반 지연 구간 식별
- 장애 재현: 특정 `error_category` / 시퀀스 갭 / Stage 실패 상황 빠른 복원
- 메트릭 재계산: 수집 누락, 처리율(throughput), 스테이지별 처리 편차
- 데이터 품질 검사: 이벤트 간 논리적 순서 제약(세션 시작 전 Stage 없음 등) 검증

## 2. 입력 소스 (Inputs)
| 소스 | 포맷 | 장점 | 단점 |
|------|------|------|------|
| 런타임 로그(Log tail) | JSON line(내장) 또는 raw | 즉시 사용 | 파싱 비용, 혼합 로그 노이즈 |
| 구조화 이벤트 JSONL(권장) | 1행=직렬화된 `CrawlEvent` | 경량, 순서 명확 | 외부 수집 파이프 필요 |
| SQLite (저장 시) | events 테이블 (id, seq, json, ts) | 인덱싱/쿼리 필터 용이 | 삭제/압축 전략 필요 |
| In-memory buffer snapshot | Vec<CrawlEvent> 직렬화 | 저지연 | 휘발성, 재기동 불가 |

1차 구현: JSONL (단순, 스트림 friendly) → 2차에 SQLite 백엔드 추가.

## 3. 아웃풋 (Outputs)
- UI 재구성: FE store API 에 동일 이벤트 순서로 inject
- 분석 리포트(JSON):
  - 총 이벤트 수 / 변형별 카운트
  - 첫/마지막 타임스탬프 → 세션 wall-clock duration
  - 시퀀스 갭 목록 (expected vs got)
  - `error_category` 분포
  - Stage별 처리량(Completed / Failed / Retry)
  - Progress 이벤트 간 평균 간격(ms)
- 선택적 그래프(미래): flame-like timeline (Stage vs time)

## 4. 동작 모드 (Modes)
| 모드 | 설명 | 사용 사례 |
|------|------|-----------|
| real-time | 원래 간격 근사(Δtimestamp 적용) | UX 동작 감시 | 
| accelerated (factor=N) | 간격 / N | 빠른 회귀 | 
| step | 키/명령 입력마다 1 이벤트 | 디버깅 | 
| digest-only | 이벤트 ingest+리포트, emit 생략 | 메트릭 계산 | 
| filter-play | 필터 통과 이벤트만 emit | 특정 Stage 집중 |

## 5. 핵심 컴포넌트 (Architecture)
```
[SourceReader] -> [Parser] -> [Validator] -> [Sequencer] -> [FilterChain] -> [Dispatcher]
                                         \-> [MetricsAccumulator]
```
- SourceReader: 파일/STDIN/DB 커서 추상화 (trait) `next_line() -> Option<String>`
- Parser: JSON → `CrawlEvent` (serde_json) + reject count
- Validator: 스키마 버전, 필수 필드, 시간 역행 검사
- Sequencer: 시퀀스 증가 모니터, 갭/역순 기록
- FilterChain: predicate 배열 (stage_name, error_category, time window 등)
- Dispatcher: FE 모드 = IPC emit(mock) / CLI 모드 = no-op
- MetricsAccumulator: 변형 카운트, 구간 통계, 레이턴시 분포(histogram mock)

## 6. Rust 초기 API 스케치
```rust
pub struct ReplayConfig {
    pub mode: ReplayMode,
    pub speed_factor: f32,            // accelerated 모드
    pub filters: Vec<EventFilter>,
    pub strict_sequence: bool,
    pub stop_on_error: bool,
}

pub enum ReplayMode { RealTime, Accelerated, Step, DigestOnly }

pub trait EventSink { fn handle(&self, ev: &CrawlEvent); }

pub struct ReplayEngine<S: SourceReader, K: EventSink> { /* fields */ }
impl<S: SourceReader, K: EventSink> ReplayEngine<S, K> {
    pub async fn run(&mut self) -> ReplayReport { /* ... */ }
    pub fn step(&mut self) -> Option<CrawlEvent>; // Step 모드 전용
}
```

## 7. 보고서 (ReplayReport)
```rust
pub struct ReplayReport {
  pub total_events: u64,
  pub by_variant: HashMap<String,u64>,
  pub first_ts: Option<DateTime<Utc>>,
  pub last_ts: Option<DateTime<Utc>>,
  pub sequence_gaps: Vec<(u64,u64)>, // (expected, got)
  pub error_categories: HashMap<String,u64>,
  pub avg_progress_interval_ms: Option<f64>,
  pub retries: u64,
  pub failed: u64,
  pub duration_wall_ms: u128,
}
```

## 8. 필터 (EventFilter)
- StageNameIn(Vec<String>)
- ErrorCategoryIn(Vec<String>)
- SequenceRange { start, end }
- TimeWindow { from, to }
- Predicate(Box<dyn Fn(&CrawlEvent)->bool>) (고급: feature guard)

## 9. 검증 규칙 (Validation Rules)
| 규칙 | 설명 | 위반 시 |
|------|------|---------|
| 세션 순서 | SessionStarted 앞 이벤트 금지 | discard/warn |
| 단조 증가 | sequence 증가 (동일/역행 금지) | gap 기록 |
| 타임스탬프 역행 | 이전 ts 보다 과거 ts | warn |
| 스키마 버전 | mismatch | warn(or drop strict) |
| StageItemCompleted 전 StageStarted | 논리 위반 | warn |
| OverallProgress 역행 | 퍼센트 감소 | warn |

## 10. CLI UX (초기)
```
replay-events \
  --input session_20250920.jsonl \
  --mode accelerated --factor 20 \
  --filter stage=ProductDetailCrawling \
  --strict-sequence --report report.json
```
옵션 파서: `clap`  / 출력: human + JSON 병행

## 11. 성능/메모리 목표
- 스트리밍 처리: 라인당 O(1) 추가 메모리 (이벤트 전체 보관 금지; 집계만 유지)
- 100k 이벤트 파일 < 2s digest-only (SSD 기준)
- Parser 에러 허용율: <0.1% (초과 시 오류 코드 반환)

## 12. 확장 로드맵
| 단계 | 기능 | 비고 |
|------|------|------|
| v1 | JSONL ingest + digest report | CLI | 
| v1.1 | Accelerated/RealTime 재생 + mock sink | FE 통합 준비 |
| v1.2 | Step 모드 + 인터랙티브 TUI | `crossterm` |
| v1.3 | SQLite source 지원 | 인덱스 필요 |
| v2 | Selective backfill (gap auto-fetch) | API 연동 |
| v2.1 | Timeline flame graph export (SVG) | 시 بص각화 |
| v2.2 | Metrics direct Prometheus exporter | pushgateway 혹은 pull adapter |

## 13. 위험 & 완화
| 위험 | 영향 | 완화 |
|------|------|------|
| 대용량 파일 메모리 폭주 | OOM | 스트리밍 + bounded channel |
| Timestamp 불균일 (clock skew) | 잘못된 속도 계산 | progress 기반 synthetic 간격 fallback |
| 스키마 진화로 필드 누락 | 역직렬화 실패 | `#[serde(default)]` + fallback category |
| 이벤트 재생 도중 sink panic | 중단 | sink 호출 try/catch (Result 수집) |

## 14. Acceptance Criteria (v1)
- JSONL(>=1 line) 입력, 0 오류 시 정상 종료 코드 0
- 리포트 JSON 생성 & `total_events` = 변형별 합계 검증
- 최소 5가지 Validation 룰 위반 시 warning 카운트 증가
- `--strict-sequence` 사용 시 첫 갭 발견 즉시 non-zero 종료
- 100k synthetic 이벤트 digest-only < 2s (벤치 스크립트 별도)

## 15. 구현 우선순위 (v1 Sprint Backlog)
1. Data model (`ReplayConfig`, `ReplayReport`, `SourceReader` trait)
2. JSONL Reader + Parser
3. Validator + Sequencer
4. MetricsAccumulator + Report 생성
5. CLI (digest-only)
6. Basic tests (fixture 1: happy, fixture 2: gap, fixture 3: malformed)
7. Docs + README snippet

## 16. 추가 메모
- FE 연동시 실제 IPC emit 대신 콜백 sink → Store 통합 리그레션 테스트 가능.
- 향후 codegen 통합 시 `CrawlEvent` 변형 delta 자동 비교하여 replay 회귀 테스트 자동화 가능.

(End of Plan)
