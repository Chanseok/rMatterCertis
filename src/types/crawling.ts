// Crawling related types for frontend - Modern Real-time Event System
// TypeScript types matching the Rust backend event types
// This ensures type safety between Rust backend and SolidJS frontend

// Modern real-time event system types
export enum CrawlingStage {
  Idle = "Idle",
  TotalPages = "TotalPages",
  ProductList = "ProductList", 
  ProductDetail = "ProductDetail",
  Database = "Database",
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
  Cancelled = "Cancelled",
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

// Legacy types for backward compatibility
export namespace Legacy {
  export type LegacySessionStatus = 
    | "NotStarted"
    | "Running" 
    | "Paused"
    | "Stopped"
    | "Completed"
    | "Failed";

  export type LegacyCrawlingStage = 
    | "ProductList"
    | "ProductDetails" 
    | "Certification"
    | "Completed";

  export interface CrawlingSessionState {
    session_id: string;
    start_time: string;
    status: LegacySessionStatus;
    stage: LegacyCrawlingStage;
    pages_crawled: number;
    max_pages: number;
    current_url?: string;
    errors: string[];
    config: any; // JSON value
  }

  export interface CrawlingStatusResponse {
    session_id: string;
    status: LegacySessionStatus;
    stage: LegacyCrawlingStage;
    pages_crawled: number;
    max_pages: number;
    current_url?: string;
    error_count: number;
  }
}

// Export legacy types for backward compatibility
export interface StartCrawlingRequest {
  start_url: string;
  target_domains: string[];
  max_pages?: number;
  concurrent_requests?: number;
  delay_ms?: number;
}

export interface CrawlingStats {
  total_sessions: number;
  active_sessions: number;
  completed_sessions: number;
  total_pages_crawled: number;
  average_success_rate: number;
}

// Re-export legacy types with original names
export type CrawlingSessionState = Legacy.CrawlingSessionState;
export type CrawlingStatusResponse = Legacy.CrawlingStatusResponse;

// Product-related types for crawling results
export interface Product {
  url: string;
  manufacturer?: string;
  model?: string;
  certificate_id?: string;
  page_id?: number;
  index_in_page?: number;
  created_at: string;
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

// Crawler Configuration Types (based on guide documents)
export interface CrawlerConfig {
  // === 핵심 크롤링 설정 ===
  pageRangeLimit: number;              // 기본값: 10
  productListRetryCount: number;       // 기본값: 9
  productDetailRetryCount: number;     // 기본값: 9
  productsPerPage: number;             // 기본값: 12
  autoAddToLocalDB: boolean;           // 기본값: true
  autoStatusCheck: boolean;            // 기본값: true
  crawlerType: 'axios' | 'playwright'; // 기본값: 'axios'

  // === 배치 처리 설정 ===
  batchSize: number;                   // 기본값: 30
  batchDelayMs: number;                // 기본값: 2000
  enableBatchProcessing: boolean;      // 기본값: true
  batchRetryLimit: number;             // 기본값: 3

  // === 핵심 URL 설정 ===
  baseUrl: string;                     // CSA-IoT 기본 URL
  matterFilterUrl: string;             // Matter 필터 적용된 URL
  
  // === 타임아웃 설정 ===
  pageTimeoutMs: number;               // 기본값: 90000
  productDetailTimeoutMs: number;      // 기본값: 90000
  
  // === 동시성 및 성능 설정 ===
  initialConcurrency: number;          // 기본값: 16
  detailConcurrency: number;           // 기본값: 16
  retryConcurrency: number;            // 기본값: 9
  minRequestDelayMs: number;           // 기본값: 100
  maxRequestDelayMs: number;           // 기본값: 2200
  retryStart: number;                  // 기본값: 2
  retryMax: number;                    // 기본값: 10
  cacheTtlMs: number;                  // 기본값: 300000

  // === 브라우저 설정 ===
  headlessBrowser: boolean;            // 기본값: true
  maxConcurrentTasks: number;          // 기본값: 16
  requestDelay: number;                // 기본값: 100
  customUserAgent?: string;            // 선택적
  
  // === 로깅 설정 ===
  logging: LoggingConfig;
}

export interface LoggingConfig {
  level: 'ERROR' | 'WARN' | 'INFO' | 'DEBUG';
  enableStackTrace: boolean;
  enableTimestamp: boolean;
  components: Record<string, string>;
}

// 기본 설정값
export const DEFAULT_CRAWLER_CONFIG: CrawlerConfig = {
  // URL Configuration
  baseUrl: 'https://csa-iot.org/csa-iot_products/',
  matterFilterUrl: 'https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=',
  
  // Performance & Timing
  pageTimeoutMs: 90000,
  productDetailTimeoutMs: 90000,
  minRequestDelayMs: 100,
  maxRequestDelayMs: 2200,
  
  // Concurrency Settings
  initialConcurrency: 16,
  detailConcurrency: 16,
  retryConcurrency: 9,
  maxConcurrentTasks: 16,
  
  // Batch Processing
  batchSize: 30,
  batchDelayMs: 2000,
  enableBatchProcessing: true,
  batchRetryLimit: 3,
  
  // Crawler Behavior
  pageRangeLimit: 10,
  productListRetryCount: 9,
  productDetailRetryCount: 9,
  productsPerPage: 12,
  autoAddToLocalDB: true,
  autoStatusCheck: true,
  crawlerType: 'axios',
  
  // Browser Settings
  headlessBrowser: true,
  requestDelay: 100,
  customUserAgent: undefined,
  
  // Cache & Retry
  cacheTtlMs: 300000,
  retryStart: 2,
  retryMax: 10,
  
  // Logging
  logging: {
    level: 'INFO',
    enableStackTrace: false,
    enableTimestamp: true,
    components: {}
  }
};

// 설정 프리셋
export interface ConfigPreset {
  name: string;
  description: string;
  config: Partial<CrawlerConfig>;
}

export const CONFIG_PRESETS: ConfigPreset[] = [
  {
    name: 'Development',
    description: '개발용 빠른 테스트 설정',
    config: {
      pageRangeLimit: 3,
      batchSize: 5,
      pageTimeoutMs: 30000,
      productDetailTimeoutMs: 30000,
      headlessBrowser: false,
      logging: {
        level: 'DEBUG',
        enableStackTrace: true,
        enableTimestamp: true,
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
      pageRangeLimit: 50,
      batchSize: 30,
      pageTimeoutMs: 90000,
      productDetailTimeoutMs: 90000,
      headlessBrowser: true,
      logging: {
        level: 'INFO',
        enableStackTrace: false,
        enableTimestamp: true,
        components: {}
      }
    }
  },
  {
    name: 'Conservative',
    description: '안정성 우선 보수적 설정',
    config: {
      pageRangeLimit: 10,
      batchSize: 10,
      initialConcurrency: 8,
      detailConcurrency: 8,
      minRequestDelayMs: 500,
      maxRequestDelayMs: 3000,
      pageTimeoutMs: 120000,
      productDetailTimeoutMs: 120000
    }
  }
];

// Helper functions for stage and status display
export const getCrawlingStageDisplayName = (stage: CrawlingStage): string => {
  const stageNames: Record<CrawlingStage, string> = {
    [CrawlingStage.Idle]: "대기",
    [CrawlingStage.TotalPages]: "총 페이지 수 확인",
    [CrawlingStage.ProductList]: "제품 목록 수집",
    [CrawlingStage.ProductDetail]: "제품 상세정보 수집",
    [CrawlingStage.Database]: "데이터베이스 저장",
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

export const getTaskStatusColor = (status: TaskStatus): string => {
  switch (status) {
    case TaskStatus.Pending: return "text-gray-500";
    case TaskStatus.Running: return "text-blue-500";
    case TaskStatus.Completed: return "text-green-500";
    case TaskStatus.Failed: return "text-red-500";
    case TaskStatus.Cancelled: return "text-orange-500";
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
