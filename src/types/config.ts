// Configuration types matching the actual backend config structure

export interface LoggingConfig {
  level: string;
  json_format: boolean;
  console_output: boolean;
  file_output: boolean;
  separate_frontend_backend: boolean;
  file_naming_strategy: string;
  max_file_size_mb: number;
  max_files: number;
  auto_cleanup_logs: boolean;
  keep_only_latest: boolean;
  module_filters: Record<string, string>;
}

export interface BatchConfig {
  batch_size: number;
  batch_delay_ms: number;
  enable_batch_processing: boolean;
  batch_retry_limit: number;
}

export interface WorkerConfig {
  list_page_max_concurrent: number;
  product_detail_max_concurrent: number;
  request_timeout_seconds: number;
  max_retries: number;
  db_batch_size: number;
  db_max_concurrency: number;
}

export interface TimingConfig {
  scheduler_interval_ms: number;
  shutdown_timeout_seconds: number;
  stats_interval_seconds: number;
  retry_delay_ms: number;
  operation_timeout_seconds: number;
}

export interface CrawlingConfigSection {
  page_range_limit: number;
  product_list_retry_count: number;
  product_detail_retry_count: number;
  auto_add_to_local_db: boolean;
  workers: WorkerConfig;
  timing: TimingConfig;
}

export interface UserConfig {
  max_pages: number;
  request_delay_ms: number;
  max_concurrent_requests: number;
  verbose_logging: boolean;
  logging: LoggingConfig;
  batch: BatchConfig;
  crawling: CrawlingConfigSection;
}

export interface AdvancedConfig {
  last_page_search_start: number;
  max_search_attempts: number;
  retry_attempts: number;
  retry_delay_ms: number;
  product_selectors: string[];
  request_timeout_seconds: number;
}

export interface AppManagedConfig {
  last_known_max_page: number | null;
  last_successful_crawl: string | null;
  last_crawl_product_count: number | null;
  avg_products_per_page: number | null;
  config_version: number;
  window_state: string | null;
}

export interface AppConfig {
  user: UserConfig;
  advanced: AdvancedConfig;
  app_managed: AppManagedConfig;
}

// Configuration Presets
export interface ConfigPreset {
  name: string;
  description: string;
  config: AppConfig;
}

export const CONFIG_PRESETS: ConfigPreset[] = [
  {
    name: '개발용',
    description: '빠른 테스트를 위한 설정',
    config: {
      user: {
        max_pages: 3,
        request_delay_ms: 1000,
        max_concurrent_requests: 5,
        verbose_logging: true,
        logging: {
          level: 'debug',
          json_format: false,
          console_output: true,
          file_output: true,
          separate_frontend_backend: false,
          file_naming_strategy: 'unified',
          max_file_size_mb: 10,
          max_files: 5,
          auto_cleanup_logs: true,
          keep_only_latest: false,
          module_filters: {
            wry: 'warn',
            tokio: 'debug',
            hyper: 'warn',
            tauri: 'info',
            reqwest: 'info',
            sqlx: 'warn',
            matter_certis_v2: 'debug'
          }
        },
        batch: {
          batch_size: 5,
          batch_delay_ms: 500,
          enable_batch_processing: true,
          batch_retry_limit: 2
        },
        crawling: {
          page_range_limit: 3,
          product_list_retry_count: 2,
          product_detail_retry_count: 2,
          auto_add_to_local_db: true,
          workers: {
            list_page_max_concurrent: 5,
            product_detail_max_concurrent: 10,
            request_timeout_seconds: 15,
            max_retries: 2,
            db_batch_size: 100,
            db_max_concurrency: 10
          },
          timing: {
            scheduler_interval_ms: 1000,
            shutdown_timeout_seconds: 30,
            stats_interval_seconds: 10,
            retry_delay_ms: 1000,
            operation_timeout_seconds: 30
          }
        }
      },
      advanced: {
        last_page_search_start: 100,
        max_search_attempts: 5,
        retry_attempts: 2,
        retry_delay_ms: 1000,
        product_selectors: ['div.post-feed article.type-product'],
        request_timeout_seconds: 15
      },
      app_managed: {
        last_known_max_page: 482,
        last_successful_crawl: new Date().toISOString(),
        last_crawl_product_count: 0,
        avg_products_per_page: 12.0,
        config_version: 1,
        window_state: ''
      }
    }
  },
  {
    name: '프로덕션',
    description: '안정적인 운영을 위한 설정',
    config: {
      user: {
        max_pages: 100,
        request_delay_ms: 800,
        max_concurrent_requests: 20,
        verbose_logging: false,
        logging: {
          level: 'info',
          json_format: false,
          console_output: true,
          file_output: true,
          separate_frontend_backend: false,
          file_naming_strategy: 'unified',
          max_file_size_mb: 50,
          max_files: 10,
          auto_cleanup_logs: true,
          keep_only_latest: false,
          module_filters: {
            wry: 'warn',
            tokio: 'info',
            hyper: 'warn',
            tauri: 'info',
            reqwest: 'info',
            sqlx: 'warn',
            matter_certis_v2: 'info'
          }
        },
        batch: {
          batch_size: 20,
          batch_delay_ms: 1000,
          enable_batch_processing: true,
          batch_retry_limit: 3
        },
        crawling: {
          page_range_limit: 20,
          product_list_retry_count: 3,
          product_detail_retry_count: 3,
          auto_add_to_local_db: true,
          workers: {
            list_page_max_concurrent: 20,
            product_detail_max_concurrent: 40,
            request_timeout_seconds: 30,
            max_retries: 3,
            db_batch_size: 200,
            db_max_concurrency: 20
          },
          timing: {
            scheduler_interval_ms: 500,
            shutdown_timeout_seconds: 60,
            stats_interval_seconds: 30,
            retry_delay_ms: 2000,
            operation_timeout_seconds: 60
          }
        }
      },
      advanced: {
        last_page_search_start: 450,
        max_search_attempts: 10,
        retry_attempts: 3,
        retry_delay_ms: 2000,
        product_selectors: ['div.post-feed article.type-product'],
        request_timeout_seconds: 30
      },
      app_managed: {
        last_known_max_page: 482,
        last_successful_crawl: new Date().toISOString(),
        last_crawl_product_count: 0,
        avg_products_per_page: 12.0,
        config_version: 1,
        window_state: ''
      }
    }
  }
];
