
╭─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│  > 로그파일, event.log를 살펴보면 name은 모두 actor-event 이군요. 모든 event의 이름이 actor-event 라는 건 의미가 없지 않나 하는 생각인데요.        │
│    그리고 variant 가 너무 추상적이지 않나요? (예: StageItemStarted ) FE 입장에서 각 Stage별 진행 상황을 쉽게 parsing할 수 있도록 구조를 개선해야   │
│    할 것 같은데요.                                                                                                                                 │
│    stage 정보, stage의 현재 item index, action or progress 상태, timestamp, 완료 시에는 총 소요시간 추가 이런 식으로 되어 있어야 FE에서            │
│    parsing해서 바로 쓰기 좋을 것 같습니다.                                                                                                         │
│    물론 현재 BE의 Crawling Engine 의 구조는 횡단 관심사는 StageLogic에 영향을 전혀 주지 않도록 하는 등 견고하게 유지하거나 더 담백하고 견고하게    │
│    발전시키면서 말이죠.                                                                                                                            │
│    이 이벤트 구조는 BE의 SW구조는 물론 FE의 SW구조에 큰 영향을 미치는 매우 매우 중요한 부분이므로 어떻게 개선하면 좋을 지 차분하고 깊게 고민하셔서 │
│     제안 해 주시기 바랍니다.                                                                                                                       │
│                                                                                                                                                    │
│    2025-09-20 00:58:37.919 +09:00  INFO actor-event: 🌉 actor-event seq=4 name=actor-event variant=StageItemStarted                                │
│    session_id=Some("actor_session_1758297517") batch_id=Some("actor_session_1758297517-pre-1")                                                     │
╰─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

✦ 정확하고 심도 깊은 분석 감사합니다. name이 항상 actor-event인 점, variant가 추상적인 점, 그리고 FE 처리 용이성 부족에 대한 지적은 매우 타당하며,
  이는 이벤트 시스템의 근본적인 개선이 필요하다는 것을 의미합니다. 말씀하신 대로 이 이벤트 구조는 BE와 FE의 SW 구조에 지대한 영향을 미치므로,
  신중하게 접근해야 합니다.

  현재의 이벤트 시스템은 백엔드 내부 로깅에는 적합할 수 있으나, FE가 실시간으로 진행 상황을 파싱하고 UI를 업데이트하기에는 비효율적이고 오류 발생
  가능성이 높습니다.

  제안: 명시적이고 구조화된 이벤트 시스템으로의 전환

  현재의 name=actor-event, variant=StageItemStarted와 같은 방식 대신, 이벤트의 목적과 내용을 명확히 드러내는 구조화된 이벤트 시스템으로 전환할
  것을 제안합니다. 이는 FE가 이벤트를 쉽게 소비하고, BE는 자신의 로직을 견고하게 유지하면서도 필요한 정보를 정확히 전달할 수 있도록 합니다.

  핵심 아이디어:

   1. 단일 이벤트 채널 유지: Tauri의 emit_all을 사용할 때, 이벤트 이름(채널)은 crawl_updates와 같이 단일화하여 FE의 리스너 설정을 단순화합니다.
   2. 구조화된 이벤트 페이로드: 이 단일 채널을 통해 전송되는 페이로드는 모든 크롤링 관련 업데이트를 담을 수 있는 공통 구조를 가지되, 내부적으로는
      event_type 필드를 통해 실제 이벤트의 종류를 명확히 구분하고, 각 event_type에 맞는 상세 payload를 포함하도록 합니다.

  ---

  제안하는 이벤트 페이로드 구조 (TypeScript 예시)

     1 // src/types/events.ts (또는 crawling.ts)
     2
     3 /**
     4  * 모든 크롤링 관련 이벤트의 공통 구조
     5  */
     6 interface CrawlEvent {
     7   event_id: string; // 이 이벤트 인스턴스의 고유 ID (디버깅 및 추적 용이)
     8   timestamp: string; // 이벤트 발생 시각 (ISO 8601 형식)
     9   session_id: string; // 특정 크롤링 세션을 식별하는 ID
    10   batch_id?: string; // (선택 사항) 세션 내의 특정 배치 작업을 식별하는 ID
    11
    12   event_type: CrawlEventType; // 이벤트의 종류를 명확히 나타내는 Enum 또는 Literal Union
    13
    14   // event_type에 따라 구조가 달라지는 상세 페이로드
    15   payload:
    16     | CrawlSessionStartedPayload
    17     | CrawlSessionCompletedPayload
    18     | CrawlSessionFailedPayload
    19     | StageStartedPayload
    20     | StageProgressPayload
    21     | StageItemStartedPayload
    22     | StageItemCompletedPayload
    23     | StageItemFailedPayload
    24     | StageItemRetryingPayload
    25     | OverallProgressUpdatePayload; // 기타 필요한 페이로드들
    26 }
    27
    28 /**
    29  * 이벤트 종류를 정의하는 Enum (Rust의 Enum과 매핑)
    30  */
    31 enum CrawlEventType {
    32   CrawlSessionStarted = "CrawlSessionStarted",
    33   CrawlSessionCompleted = "CrawlSessionCompleted",
    34   CrawlSessionFailed = "CrawlSessionFailed",
    35   StageStarted = "StageStarted",
    36   StageProgress = "StageProgress", // Stage 전체의 진행률 업데이트
    37   StageItemStarted = "StageItemStarted",
    38   StageItemCompleted = "StageItemCompleted",
    39   StageItemFailed = "StageItemFailed",
    40   StageItemRetrying = "StageItemRetrying",
    41   OverallProgressUpdate = "OverallProgressUpdate", // 전체 크롤링 작업의 진행률 업데이트
    42   // ... 필요한 다른 이벤트 타입 추가
    43 }
    44
    45 // --- 각 event_type에 따른 상세 페이로드 인터페이스 ---
    46
    47 interface CrawlSessionStartedPayload {
    48   total_stages: number;
    49   message?: string;
    50 }
    51
    52 interface CrawlSessionCompletedPayload {
    53   total_duration_ms: number; // 총 소요 시간
    54   successful_items_count: number;
    55   failed_items_count: number;
    56   message?: string;
    57 }
    58
    59 interface CrawlSessionFailedPayload {
    60   error_message: string;
    61   error_code?: string;
    62   message?: string;
    63 }
    64
    65 interface StageStartedPayload {
    66   stage_name: string; // 예: "PageFetching", "ProductParsing"
    67   stage_index: number; // 전체 Stage 중 몇 번째 Stage인지
    68   total_items_in_stage: number; // 이 Stage에서 처리할 총 아이템 수
    69   message?: string;
    70 }
    71
    72 interface StageProgressPayload {
    73   stage_name: string;
    74   stage_index: number;
    75   current_item_index: number; // 현재 처리 중인 아이템의 인덱스 (0부터 시작)
    76   total_items_in_stage: number;
    77   progress_percentage: number; // 해당 Stage의 진행률 (0-100)
    78   message?: string;
    79 }
    80
    81 interface StageItemStartedPayload {
    82   stage_name: string;
    83   item_id: string; // 처리 시작된 아이템의 고유 ID (예: URL, Product ID)
    84   message?: string;
    85 }
    86
    87 interface StageItemCompletedPayload {
    88   stage_name: string;
    89   item_id: string;
    90   duration_ms: number; // 해당 아이템 처리 소요 시간
    91   message?: string;
    92 }
    93
    94 interface StageItemFailedPayload {
    95   stage_name: string;
    96   item_id: string;
    97   error_message: string;
    98   retry_possible: boolean; // 재시도 가능한 에러인지 여부
    99   message?: string;
   100 }
   101
   102 interface StageItemRetryingPayload {
   103   stage_name: string;
   104   item_id: string;
   105   retry_attempt: number; // 현재 재시도 횟수
   106   max_retries: number; // 최대 재시도 횟수
   107   reason?: string; // 재시도 사유
   108   message?: string;
   109 }
   110
   111 interface OverallProgressUpdatePayload {
   112   current_stage_index: number;
   113   total_stages: number;
   114   overall_progress_percentage: number; // 전체 크롤링 작업의 진행률 (0-100)
   115   completed_items_count: number;
   116   total_items_count: number; // 전체 크롤링할 아이템 수 (알 수 있다면)
   117   message?: string;
   118 }

  ---

  제안하는 구조의 장점
   1. FE 처리 용이성 극대화:
       * FE는 event_type 필드만으로 어떤 종류의 이벤트인지 즉시 파악하고, 해당 event_type에 맞는 payload 구조를 타입 안전하게 사용할 수 있습니다.
         switch 문이나 if/else if 문으로 쉽게 분기 처리가 가능합니다.
       * message 필드 파싱과 같은 복잡하고 오류 발생 가능성이 높은 작업이 필요 없어집니다.
       * 사용자께서 제안하신 stage 정보, item index, action/progress 상태, timestamp, 총 소요시간 등이 모두 명시적인 필드로 제공됩니다.

   2. 명확성과 가독성:
       * 이벤트 자체의 의미가 event_type을 통해 명확하게 드러납니다.
       * 각 이벤트가 어떤 데이터를 포함하는지 payload 인터페이스를 통해 쉽게 이해할 수 있습니다.

   3. 타입 안전성 (Type Safety):
       * Rust의 enum과 struct를 #[derive(Serialize)]와 함께 사용하고, FE의 TypeScript enum과 interface로 미러링하면, 컴파일 시점에 이벤트 구조의
         유효성을 검증할 수 있어 런타임 오류를 크게 줄일 수 있습니다.

   4. 유지보수성 및 확장성:
       * 새로운 이벤트 타입이 필요하면 CrawlEventType에 추가하고 해당 payload 인터페이스만 정의하면 됩니다. 기존 이벤트 처리 로직에 미치는 영향이
         최소화됩니다.
       * BE의 StageLogic은 자신의 상태 변화를 이 명시적인 이벤트 타입으로 발행하기만 하면 되므로, 횡단 관심사로부터 분리되어 견고함을 유지할 수
         있습니다.

   5. 디버깅 용이성:
       * 로그 파일에 기록되는 이벤트들이 event_type과 구조화된 payload를 포함하게 되어, 문제 발생 시 어떤 이벤트가 어떤 데이터를 가지고 전송되었는지
         훨씬 쉽게 추적하고 분석할 수 있습니다.

  구현 단계 (개요)

   1. `src/types/events.ts` (FE) 및 `src-tauri/src/events.rs` (BE) 정의: 위에 제안된 CrawlEvent, CrawlEventType 및 각 payload 인터페이스/구조체를
      정의합니다. (BE에서는 serde 크레이트를 활용하여 enum과 struct를 정의하고 #[derive(Serialize)]를 적용합니다.)
   2. 백엔드 이벤트 발행 로직 수정: crawl_engine/actor_event_bridge.rs 및 크롤링 엔진의 다른 관련 부분에서 이벤트를 발행하는 코드를 수정하여 새로운
      CrawlEvent 구조에 맞춰 데이터를 구성하고 emit_all("crawl_updates", ...)를 호출하도록 합니다.
   3. 프론트엔드 이벤트 수신 및 처리 로직 수정:
       * src/services/tauri-api.ts에서 listenToCrawlProgress 함수를 listenToCrawlUpdates 등으로 변경하고, crawl_updates 채널을 구독합니다.
       * src/stores/crawlerStore.ts에서 수신된 CrawlEvent의 event_type에 따라 switch 문 등으로 분기하여 payload의 상세 정보를 바탕으로 스토어 상태를
         업데이트합니다.
   4. 프론트엔드 UI 컴포넌트 수정: src/components/tabs/CrawlingEngineTabSimple.tsx 및 src/components/tabs/parts/StageStatsPanels.tsx에서
      crawlerStore의 새로운 상태 구조를 사용하여 UI를 렌더링하도록 업데이트합니다.

  ---

  이러한 방식으로 이벤트 시스템을 개선하면, 백엔드의 견고한 로직을 유지하면서도 프론트엔드가 크롤링 진행 상황을 훨씬 더 효율적이고 정확하게 표시할
  수 있게 될 것입니다.