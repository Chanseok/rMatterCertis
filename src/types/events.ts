// 백엔드 events.rs와 호환되는 TypeScript 타입 정의

export interface DbCursor {
  page: number;
  index: number;
}

export interface SystemStatePayload {
  is_running: boolean;
  total_pages: number;
  db_total_products: number;
  last_db_cursor: DbCursor | null;
  session_target_items: number;
  session_collected_items: number;
  session_eta_seconds: number;
  items_per_minute: number;
  current_stage: string;
  analyzed_at: string | null; // ISO datetime string
}

export type TaskStatus = 'Pending' | 'Active' | 'Retrying' | 'Success' | 'Error';

export interface AtomicTaskEvent {
  task_id: string;
  batch_id: number;
  stage_name: string; // "ListPageCollection", "DetailPageCollection", "DatabaseSave"
  status: TaskStatus;
  progress: number; // 0.0 - 1.0
  message: string | null;
  timestamp: string; // ISO datetime string
}

export interface BatchInfo {
  id: number;
  status: string;
  progress: number;
  items_total: number;
  items_completed: number;
  current_page: number;
  pages_range: [number, number]; // [start, end]
}

export interface StageInfo {
  name: string;
  status: string;
  items_total: number;
  items_completed: number;
  items_active: number;
  items_failed: number;
}

export interface LiveSystemState {
  basic_state: SystemStatePayload;
  current_batch: BatchInfo | null;
  stages: StageInfo[];
  recent_completions: AtomicTaskEvent[];
}

// Detail crawling status exposed via get_session_status + incremental actor events
export interface DetailStatus {
  total: number;
  completed: number;
  failed: number;
  failed_ids_sample?: string[];
  remaining_ids?: string[] | null;
  retries_total?: number;
  retry_histogram?: { retries: number; count: number }[];
  retry_counts_sample?: { id: string; count: number }[];
  failure_threshold?: number;
  downshifted?: boolean;
  downshift_meta?: {
    timestamp?: string | null;
    old_limit?: number | null;
    new_limit?: number | null;
    trigger?: string | null;
  } | null;
}

// ---------------------------------------------------------------------
// New optional events (concurrency, validation, db-save)
// ---------------------------------------------------------------------

export type ConcurrencyEvent =
  | {
      type: 'ConcurrentBatchStarted';
      session_id: string;
      batch_id: string;
      stage: string;
      concurrent_tasks: number;
      max_concurrency: number;
      timestamp: string;
    }
  | {
      type: 'ConcurrentTaskStatusUpdate';
      session_id: string;
      batch_id: string;
      active_tasks: number;
      queued_tasks: number;
      completed_tasks: number;
      failed_tasks: number;
      timestamp: string;
    };

export type ValidationEvent =
  | { type: 'ValidationStarted'; batch_id: string; total_items: number; timestamp: string }
  | {
      type: 'ValidationIssueFound';
      batch_id: string;
      item_id: string;
      issue_type: 'MissingRequiredField' | 'InvalidDataFormat' | 'DuplicateEntry' | 'DataInconsistency';
      details: string;
      timestamp: string;
    }
  | { type: 'ValidationCompleted'; batch_id: string; passed: number; failed: number; timestamp: string };

export type DatabaseSaveEvent =
  | { type: 'SaveBatchStarted'; batch_id: string; total_items: number; timestamp: string }
  | {
      type: 'SaveItemResult';
      batch_id: string;
      item_id: string;
      page_number: number;
      index_in_page: number;
      result: 'Saved' | 'Skipped' | 'Failed';
      reason?: string | null;
      timestamp: string;
    }
  | {
      type: 'SaveBatchCompleted';
      batch_id: string;
      saved: number;
      skipped: number;
      failed: number;
      failed_items: { page_number: number; index_in_page: number; product_url: string; reason: string }[];
      timestamp: string;
    };
