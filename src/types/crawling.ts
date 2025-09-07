// Crawling related types for frontend - Modern Real-time Event System
// TypeScript types matching the Rust backend event types
// This ensures type safety between Rust backend and SolidJS frontend

// Modern real-time event system types
export enum CrawlingStage {
  Idle = "Idle",
  StatusCheck = "StatusCheck",
  DatabaseAnalysis = "DatabaseAnalysis", 
  TotalPages = "TotalPages",
  ProductList = "ProductList", 
  ProductDetails = "ProductDetails",
  DatabaseSave = "DatabaseSave",
  Database = "Database", // Legacy compatibility
}

export enum CrawlingStatus {
  Idle = "Idle",
  Running = "Running",
  Paused = "Paused",
  Completed = "Completed",
  Error = "Error",
  Cancelled = "Cancelled",
}

export enum TaskStatus {
  Pending = "Pending",
  Running = "Running",
  Completed = "Completed",
  Failed = "Failed",
  Retrying = "Retrying", // 추가
  Cancelled = "Cancelled", // 추가
}

export enum DatabaseHealth {
  Healthy = "Healthy",
  Warning = "Warning",
  Critical = "Critical",
}

export interface CrawlingProgress {
  current: number;
  total: number;
  percentage: number;
  current_stage: CrawlingStage;
  current_step: string;
  status: CrawlingStatus;
  message: string;
  remaining_time?: number;
  elapsed_time: number;
  new_items: number;
  updated_items: number;
  current_batch?: number;
  total_batches?: number;
  errors: number;
  timestamp: string; // ISO string format
}

export interface CrawlingTaskStatus {
  task_id: string;
  url: string;
  status: TaskStatus;
  message: string;
  timestamp: string;
  stage: CrawlingStage;
  details?: Record<string, any>;
}

export interface DatabaseStats {
  total_products: number;
  total_devices: number;
  last_updated: string;
  storage_size: string;
  incomplete_records: number;
  health_status: DatabaseHealth;
}

// Site data change detection types - matching Rust enum structure
export type SiteDataChangeStatus = 
  | { Increased: { new_count: number; previous_count: number } }
  | { Stable: { count: number } }
  | { Decreased: { current_count: number; previous_count: number; decrease_amount: number } }
  | { Initial: { count: number } }
  | "Inaccessible";

export enum RecommendedAction {
  WaitAndRetry = "WaitAndRetry",
  BackupAndRecrawl = "BackupAndRecrawl", 
  ManualVerification = "ManualVerification",
  PartialRecrawl = "PartialRecrawl",
}

export enum SeverityLevel {
  Low = "Low",
  Medium = "Medium",
  High = "High", 
  Critical = "Critical",
}

export interface DataDecreaseRecommendation {
  action_type: RecommendedAction;
  description: string;
  severity: SeverityLevel;
  action_steps: string[];
}

export interface SiteStatus {
  is_accessible: boolean;
  response_time_ms: number;
  total_pages: number;
  estimated_products: number;
  last_check_time: string;
  health_score: number; // 0.0 ~ 1.0
  data_change_status: SiteDataChangeStatus;
  decrease_recommendation?: DataDecreaseRecommendation;
}

export interface PerformanceMetrics {
  avg_processing_time_ms: number;
  items_per_second: number;
  memory_usage_mb: number;
  network_requests: number;
  cache_hit_rate: number;
}

export interface CrawlingResult {
  total_processed: number;
  new_items: number;
  updated_items: number;
  errors: number;
  duration_ms: number;
  stages_completed: CrawlingStage[];
  start_time: string;
  end_time: string;
  performance_metrics: PerformanceMetrics;
}

// Removed: CrawlingEvent union was unused.

// Removed: CrawlingConfig was unused.

// Removed: Product interface was unused.

// Removed: MatterProduct was unused.

// Removed: ProductSearchRequest was unused.

// Removed: ProductSearchResult was unused.

// Removed: MatterProductFilter was unused.

// Removed: DatabaseSummary was unused.

// Backend-provided configuration types (loaded via IPC)
export interface BackendCrawlerConfig {
  // Core settings
  start_page: number;
  end_page: number;
  concurrency: number;
  delay_ms: number;
  
  // Advanced settings
  page_range_limit: number;
  product_list_retry_count: number;
  product_detail_retry_count: number;
  products_per_page: number;
  auto_add_to_local_db: boolean;
  auto_status_check: boolean;
  crawler_type: string;

  // Batch processing
  batch_size: number;
  batch_delay_ms: number;
  enable_batch_processing: boolean;
  batch_retry_limit: number;

  // URLs
  base_url: string;
  matter_filter_url: string;
  
  // Timeouts
  page_timeout_ms: number;
  product_detail_timeout_ms: number;
  
  // Concurrency & Performance
  initial_concurrency: number;
  detail_concurrency: number;
  retry_concurrency: number;
  min_request_delay_ms: number;
  max_request_delay_ms: number;
  retry_start: number;
  retry_max: number;
  cache_ttl_ms: number;

  // Browser settings
  headless_browser: boolean;
  max_concurrent_tasks: number;
  request_delay: number;
  custom_user_agent?: string;
  
  // Logging
  logging: BackendLoggingConfig;
}

export interface BackendLoggingConfig {
  level: string;
  enable_stack_trace: boolean;
  enable_timestamp: boolean;
  components: Record<string, string>;
}

// Configuration Presets for BackendCrawlerConfig
// Removed: ConfigPreset and CONFIG_PRESETS were unused.

// Helper functions for stage and status display
// Removed: getCrawlingStageDisplayName was unused.

// Removed: getCrawlingStatusDisplayName was unused.

// Removed: getTaskStatusDisplayName was unused.

// Removed: getDatabaseHealthDisplayName was unused.

// Color utilities for UI components
// Removed: getCrawlingStatusColor was unused.

// Removed: getDatabaseHealthColor was unused.

// Removed: getSeverityLevelColor was unused.

// Removed: getSeverityLevelDisplayName was unused.

// Removed: getRecommendedActionDisplayName was unused.

// Removed: getDataChangeStatusDisplayName was unused.

// Removed: getDataChangeStatusColor was unused.

// Removed: getTaskStatusColor was unused.

// Time formatting utilities
// Removed: formatElapsedTime was unused.

// Removed: formatRemainingTime was unused.

// Progress calculation utilities
// Removed: calculateProgressPercentage was unused.

// Removed: getProgressBarColor was unused.

// =========================================================================
// Crawling Status Check Types - Improved Structure
// =========================================================================

export interface DatabaseStatus {
  total_products: number;
  last_crawl_time?: string;
  page_range: [number, number]; // [min_page, max_page]
  health: DatabaseHealth;
  size_mb: number;
  last_updated: string;
}

export interface SiteStatus {
  is_accessible: boolean;
  response_time_ms: number;
  total_pages: number;
  estimated_products: number;
  last_check_time: string;
  health_score: number; // 0.0 ~ 1.0
  data_change_status: SiteDataChangeStatus;
  decrease_recommendation?: DataDecreaseRecommendation;
}

export interface SmartRecommendation {
  action: 'crawl' | 'cleanup' | 'wait' | 'manual_check';
  priority: 'low' | 'medium' | 'high' | 'critical';
  reason: string;
  suggested_range?: [number, number]; // [start_page, end_page]
  estimated_new_items: number;
  efficiency_score: number; // 0.0 - 1.0
  next_steps: string[];
}

export interface CrawlingStatusCheck {
  database_status: DatabaseStatus;
  site_status: SiteStatus;
  recommendation: SmartRecommendation;
  sync_comparison: {
    database_count: number;
    site_estimated_count: number;
    sync_percentage: number;
    last_sync_time?: string;
  };
}

// =========================================================================
// 원자적 태스크 이벤트 타입 (proposal5.md 구현)
// =========================================================================

// Removed: AtomicTaskEvent and AtomicEventStats were unused.

// D3.js 시각화를 위한 새로운 타입 정의
// Removed: D3 visualization-related types and interfaces were unused.
