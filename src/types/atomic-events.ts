/**
 * proposal5.md 구현 - "Dual Channel" 이벤트 시스템의 Atomic Task Events
 * 
 * 고빈도 원자적 작업 이벤트를 위한 TypeScript 타입 정의
 * Rust 백엔드의 AtomicTaskEvent enum과 동기화
 */

/** UUID 타입 정의 */
export type UUID = string;

/**
 * 원자적 작업 이벤트 타입 - 개별 작업의 생명주기 추적
 */
export interface AtomicTaskEvent {
  /** 작업 식별자 */
  task_id: UUID;
  /** 이벤트 타임스탬프 (ISO 8601) */
  timestamp: string;
  /** 이벤트 종류 */
  event_type: AtomicEventType;
}

/**
 * 원자적 이벤트 타입 열거형
 */
export type AtomicEventType = 
  | { type: 'TaskStarted' }
  | { type: 'TaskCompleted'; data: TaskCompletedData }
  | { type: 'TaskFailed'; data: TaskFailedData }
  | { type: 'TaskRetrying'; data: TaskRetryingData };

/**
 * 작업 완료 이벤트 데이터
 */
export interface TaskCompletedData {
  /** 처리 시간 (밀리초) */
  duration_ms: number;
  /** 추출된 데이터 크기 */
  data_size: number;
}

/**
 * 작업 실패 이벤트 데이터
 */
export interface TaskFailedData {
  /** 오류 메시지 */
  error: string;
  /** 재시도 가능 여부 */
  retryable: boolean;
}

/**
 * 작업 재시도 이벤트 데이터
 */
export interface TaskRetryingData {
  /** 현재 재시도 횟수 */
  attempt: number;
  /** 이전 오류 메시지 */
  previous_error: string;
  /** 다음 재시도까지 대기 시간 (밀리초) */
  delay_ms: number;
}

/**
 * 원자적 이벤트 통계
 */
export interface AtomicEventStats {
  /** 총 이벤트 수 */
  total_events: number;
  /** 활성 작업 수 */
  active_tasks: number;
  /** 완료된 작업 수 */
  completed_tasks: number;
  /** 실패한 작업 수 */
  failed_tasks: number;
  /** 평균 처리 시간 (밀리초) */
  avg_duration_ms: number;
}
