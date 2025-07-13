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

export type CrawlingEvent =
  | { type: "ProgressUpdate"; data: CrawlingProgress }
  | { type: "TaskUpdate"; data: CrawlingTaskStatus }
  | { type: "StageChange"; data: { from: CrawlingStage; to: CrawlingStage; message: string } }
  | { type: "Error"; data: { error_id: string; message: string; stage: CrawlingStage; recoverable: boolean } }
  | { type: "DatabaseUpdate"; data: DatabaseStats }
  | { type: "Completed"; data: CrawlingResult };

export interface CrawlingConfig {
  start_page: number;
  end_page: number;
  concurrency: number;
  delay_ms: number;
  auto_add_to_local_db: boolean;
  retry_max: number;
  page_timeout_ms: number;
}

// Product-related types for crawling results
export interface Product {
  url: string;
  manufacturer?: string;
  model?: string;
  certificate_id?: string;
  page_id?: number;
  index_in_page?: number;
  created_at: string;
  updated_at: string;
}

export interface MatterProduct {
  url: string;
  page_id?: number;
  index_in_page?: number;
  id?: string;
  manufacturer?: string;
  model?: string;
  device_type?: string;
  certificate_id?: string;
  certification_date?: string;
  software_version?: string;
  hardware_version?: string;
  vid?: string;
  pid?: string;
  family_sku?: string;
  family_variant_sku?: string;
  firmware_version?: string;
  family_id?: string;
  tis_trp_tested?: string;
  specification_version?: string;
  transport_interface?: string;
  primary_device_type_id?: string;
  application_categories: string[];
  created_at: string;
  updated_at: string;
}

export interface ProductSearchRequest {
  query?: string;
  manufacturer?: string;
  page?: number;
  limit?: number;
}

export interface ProductSearchResult {
  products: Product[];
  total_count: number;
  page: number;
  limit: number;
  total_pages: number;
}

export interface MatterProductFilter {
  manufacturer?: string;
  device_type?: string;
  vid?: string;
  certification_date_from?: string;
  certification_date_to?: string;
}

export interface DatabaseSummary {
  total_products: number;
  total_matter_products: number;
  total_vendors: number;
  unique_manufacturers: number;
  recent_crawling_sessions: number;
  last_updated: string;
}

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
export interface ConfigPreset {
  name: string;
  description: string;
  config: Partial<BackendCrawlerConfig>;
}

export const CONFIG_PRESETS: ConfigPreset[] = [
  {
    name: 'Development',
    description: '개발용 빠른 테스트 설정',
    config: {
      page_range_limit: 3,
      batch_size: 5,
      page_timeout_ms: 30000,
      product_detail_timeout_ms: 30000,
      headless_browser: false,
      logging: {
        level: 'DEBUG',
        enable_stack_trace: true,
        enable_timestamp: true,
        components: {
          crawler: 'DEBUG',
          database: 'DEBUG',
          ui: 'INFO'
        }
      }
    }
  },
  {
    name: 'Production',
    description: '프로덕션 환경 최적화 설정',
    config: {
      page_range_limit: 50,
      batch_size: 30,
      page_timeout_ms: 90000,
      product_detail_timeout_ms: 90000,
      headless_browser: true,
      logging: {
        level: 'INFO',
        enable_stack_trace: false,
        enable_timestamp: true,
        components: {}
      }
    }
  },
  {
    name: 'Conservative',
    description: '안정성 우선 보수적 설정',
    config: {
      page_range_limit: 10,
      batch_size: 10,
      initial_concurrency: 8,
      detail_concurrency: 8,
      min_request_delay_ms: 500,
      max_request_delay_ms: 3000,
      page_timeout_ms: 120000,
      product_detail_timeout_ms: 120000
    }
  }
];

// Helper functions for stage and status display
export const getCrawlingStageDisplayName = (stage: CrawlingStage): string => {
  const stageNames: Record<CrawlingStage, string> = {
    [CrawlingStage.Idle]: "대기",
    [CrawlingStage.StatusCheck]: "사이트 상태 확인",
    [CrawlingStage.DatabaseAnalysis]: "데이터베이스 분석",
    [CrawlingStage.TotalPages]: "총 페이지 수 확인",
    [CrawlingStage.ProductList]: "제품 목록 수집",
    [CrawlingStage.ProductDetails]: "제품 상세정보 수집",
    [CrawlingStage.DatabaseSave]: "데이터베이스 저장",
    [CrawlingStage.Database]: "데이터베이스 저장", // Legacy
  };
  return stageNames[stage] || stage;
};

export const getCrawlingStatusDisplayName = (status: CrawlingStatus): string => {
  const statusNames: Record<CrawlingStatus, string> = {
    [CrawlingStatus.Idle]: "대기",
    [CrawlingStatus.Running]: "실행 중",
    [CrawlingStatus.Paused]: "일시정지",
    [CrawlingStatus.Completed]: "완료",
    [CrawlingStatus.Error]: "오류",
    [CrawlingStatus.Cancelled]: "중단됨",
  };
  return statusNames[status] || status;
};

export const getTaskStatusDisplayName = (status: TaskStatus): string => {
  const statusNames: Record<TaskStatus, string> = {
    [TaskStatus.Pending]: "대기",
    [TaskStatus.Running]: "진행 중",
    [TaskStatus.Completed]: "완료",
    [TaskStatus.Failed]: "실패",
    [TaskStatus.Cancelled]: "취소",
    [TaskStatus.Retrying]: "재시도 중", // 추가
  };
  return statusNames[status] || status;
};

export const getDatabaseHealthDisplayName = (health: DatabaseHealth): string => {
  const healthNames: Record<DatabaseHealth, string> = {
    [DatabaseHealth.Healthy]: "정상",
    [DatabaseHealth.Warning]: "주의",
    [DatabaseHealth.Critical]: "위험",
  };
  return healthNames[health] || health;
};

// Color utilities for UI components
export const getCrawlingStatusColor = (status: CrawlingStatus): string => {
  switch (status) {
    case CrawlingStatus.Idle: return "text-gray-500";
    case CrawlingStatus.Running: return "text-blue-500";
    case CrawlingStatus.Paused: return "text-yellow-500";
    case CrawlingStatus.Completed: return "text-green-500";
    case CrawlingStatus.Error: return "text-red-500";
    case CrawlingStatus.Cancelled: return "text-orange-500";
    default: return "text-gray-500";
  }
};

export const getDatabaseHealthColor = (health: DatabaseHealth): string => {
  switch (health) {
    case DatabaseHealth.Healthy: return "text-green-500";
    case DatabaseHealth.Warning: return "text-yellow-500";
    case DatabaseHealth.Critical: return "text-red-500";
    default: return "text-gray-500";
  }
};

export const getSeverityLevelColor = (severity: SeverityLevel): string => {
  switch (severity) {
    case SeverityLevel.Low: return "text-green-500";
    case SeverityLevel.Medium: return "text-yellow-500";
    case SeverityLevel.High: return "text-orange-500";
    case SeverityLevel.Critical: return "text-red-500";
    default: return "text-gray-500";
  }
};

export const getSeverityLevelDisplayName = (severity: SeverityLevel): string => {
  const severityNames: Record<SeverityLevel, string> = {
    [SeverityLevel.Low]: "낮음",
    [SeverityLevel.Medium]: "보통",
    [SeverityLevel.High]: "높음",
    [SeverityLevel.Critical]: "심각",
  };
  return severityNames[severity] || severity;
};

export const getRecommendedActionDisplayName = (action: RecommendedAction): string => {
  const actionNames: Record<RecommendedAction, string> = {
    [RecommendedAction.WaitAndRetry]: "잠시 대기 후 재시도",
    [RecommendedAction.BackupAndRecrawl]: "백업 후 전체 재크롤링",
    [RecommendedAction.ManualVerification]: "수동 확인 필요",
    [RecommendedAction.PartialRecrawl]: "부분적 재크롤링",
  };
  return actionNames[action] || action;
};

export const getDataChangeStatusDisplayName = (status: SiteDataChangeStatus | any): string => {
  if (!status) return "정보 없음";
  
  if (typeof status === 'string') {
    switch (status) {
      case 'Inaccessible': return "사이트 접근 불가";
      default: return status;
    }
  }
  
  // Handle Rust enum variants with proper checks
  if (status && typeof status === 'object') {
    if ('Increased' in status && status.Increased) {
      const { new_count, previous_count } = status.Increased;
      return `데이터 증가 (${previous_count} → ${new_count})`;
    }
    if ('Stable' in status && status.Stable) {
      return `데이터 안정 (${status.Stable.count}개)`;
    }
    if ('Decreased' in status && status.Decreased) {
      const { current_count, previous_count, decrease_amount } = status.Decreased;
      return `데이터 감소 (${previous_count} → ${current_count}, -${decrease_amount})`;
    }
    if ('Initial' in status && status.Initial) {
      return `초기 상태 (${status.Initial.count}개)`;
    }
  }
  
  return "알 수 없음";
};

export const getDataChangeStatusColor = (status: SiteDataChangeStatus | any): string => {
  if (!status) return "text-gray-500";
  
  if (typeof status === 'string') {
    if (status === 'Inaccessible') return "text-red-500";
  }
  
  if (status && typeof status === 'object') {
    if ('Increased' in status) return "text-green-500";
    if ('Stable' in status) return "text-blue-500";
    if ('Decreased' in status) return "text-red-500";
    if ('Initial' in status) return "text-gray-500";
  }
  
  return "text-gray-500";
};

export const getTaskStatusColor = (status: TaskStatus): string => {
  switch (status) {
    case TaskStatus.Pending: return "text-gray-500";
    case TaskStatus.Running: return "text-blue-500";
    case TaskStatus.Completed: return "text-green-500";
    case TaskStatus.Failed: return "text-red-500";
    case TaskStatus.Cancelled: return "text-orange-500";
    case TaskStatus.Retrying: return "text-yellow-500"; // 추가
    default: return "text-gray-500";
  }
};

// Time formatting utilities
export const formatElapsedTime = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  
  if (hours > 0) {
    return `${hours}시간 ${minutes}분 ${secs}초`;
  } else if (minutes > 0) {
    return `${minutes}분 ${secs}초`;
  } else {
    return `${secs}초`;
  }
};

export const formatRemainingTime = (seconds?: number): string => {
  if (!seconds) return "계산 중...";
  return `약 ${formatElapsedTime(seconds)} 남음`;
};

// Progress calculation utilities
export const calculateProgressPercentage = (current: number, total: number): number => {
  if (total === 0) return 0;
  return Math.min(Math.max((current / total) * 100, 0), 100);
};

export const getProgressBarColor = (percentage: number, status: CrawlingStatus): string => {
  if (status === CrawlingStatus.Error) return "bg-red-500";
  if (status === CrawlingStatus.Completed) return "bg-green-500";
  if (status === CrawlingStatus.Paused) return "bg-yellow-500";
  
  if (percentage >= 80) return "bg-green-500";
  if (percentage >= 60) return "bg-blue-500";
  if (percentage >= 40) return "bg-yellow-500";
  return "bg-gray-500";
};

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

export type AtomicTaskEvent = 
  | { 
      type: 'TaskStarted';
      task_id: string;
      task_type: string;
      timestamp: string;
    }
  | { 
      type: 'TaskCompleted';
      task_id: string;
      task_type: string;
      duration_ms: number;
      timestamp: string;
    }
  | { 
      type: 'TaskFailed';
      task_id: string;
      task_type: string;
      error_message: string;
      retry_count: number;
      timestamp: string;
    }
  | { 
      type: 'TaskRetrying';
      task_id: string;
      task_type: string;
      retry_count: number;
      delay_ms: number;
      timestamp: string;
    };

export interface AtomicEventStats {
  events_emitted: number;
  events_per_second: number;
  last_emission_time: string;
  event_type_counts: Record<string, number>;
}
