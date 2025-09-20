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
// Removed unused DetailStatus.

// ---------------------------------------------------------------------
// New optional events (concurrency, validation, db-save)
// ---------------------------------------------------------------------

// Removed unused ConcurrencyEvent.

// Removed unused ValidationEvent.

// Removed unused DatabaseSaveEvent.

// ---------------------------------------------------------------------
// Stage Item Level Events (실시간 개별 아이템 진행상황)
// ---------------------------------------------------------------------

export type StageItemType = 
  | { Page: { page_number: number } }
  | { Product: { page_number: number } }
  | { Url: { url_type: string } }
  | { ProductUrls: { urls: string[] } }
  | { ProductDetail: { url: string; page_id: number; index_in_page: number } }
  | 'SiteCheck';

export interface StageItemStartedEvent {
  session_id: string;
  batch_id?: string;
  stage_type: string; // "ListPageCrawling" | "ProductDetailCrawling"
  item_id: string;
  item_type: StageItemType;
  timestamp: string; // ISO datetime string
}

export interface StageItemCompletedEvent {
  session_id: string;
  batch_id?: string;
  stage_type: string; // "ListPageCrawling" | "ProductDetailCrawling" 
  item_id: string;
  item_type: StageItemType;
  success: boolean;
  error?: string;
  duration_ms: number;
  retry_count: number;
  collected_count?: number; // 수집된 엔트리 수 (ListPage: URL 개수, ProductDetail: 필드 개수)
  timestamp: string; // ISO datetime string
}
