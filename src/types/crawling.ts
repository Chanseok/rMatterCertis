// Crawling related types for frontend

export interface StartCrawlingRequest {
  start_url: string;
  target_domains: string[];
  max_pages?: number;
  concurrent_requests?: number;
  delay_ms?: number;
}

export interface CrawlingSessionState {
  session_id: string;
  start_time: string;
  status: SessionStatus;
  stage: CrawlingStage;
  pages_crawled: number;
  max_pages: number;
  current_url?: string;
  errors: string[];
  config: any; // JSON value
}

export type SessionStatus = 
  | "NotStarted"
  | "Running" 
  | "Paused"
  | "Stopped"
  | "Completed"
  | "Failed";

export type CrawlingStage = 
  | "ProductList"
  | "ProductDetails" 
  | "Certification"
  | "Completed";

export interface CrawlingStats {
  total_sessions: number;
  active_sessions: number;
  completed_sessions: number;
  total_pages_crawled: number;
  average_success_rate: number;
}

export interface CrawlingStatusResponse {
  session_id: string;
  status: SessionStatus;
  stage: CrawlingStage;
  pages_crawled: number;
  max_pages: number;
  current_url?: string;
  error_count: number;
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
