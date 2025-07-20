// Advanced Crawling Engine TypeScript Types
// Auto-generated from Rust types via ts-rs

// Advanced Crawling Engine 설정
export interface AdvancedCrawlingConfig {
  start_page: number;
  end_page: number;
  batch_size: number;
  concurrency: number;
  delay_ms: number;
  retry_max: number;
  enable_real_time_updates: boolean;
}

// 크롤링 진행 정보
export interface CrawlingProgressInfo {
  stage: number;
  stage_name: string;
  progress_percentage: number;
  items_processed: number;
  current_message: string;
  estimated_remaining_time?: number;
  session_id: string;
  timestamp: string; // ISO string
}

// 사이트 상태 정보
export interface SiteStatusInfo {
  is_accessible: boolean;
  total_pages: number;
  health_score: number;
  response_time_ms: number;
  products_on_last_page: number;
  estimated_total_products: number;
}

// 제품 정보
export interface ProductInfo {
  id: string;
  url: string;
  name: string;
  company: string;
  certification_number: string;
  description?: string;
  created_at: string; // ISO string
  updated_at?: string; // ISO string
}

// 크롤링 세션 정보
export interface CrawlingSession {
  session_id: string;
  started_at: string; // ISO string
  config: AdvancedCrawlingConfig;
  status: SessionStatus;
  total_products_processed: number;
  success_rate: number;
}

// 세션 상태
export enum SessionStatus {
  Running = "Running",
  Completed = "Completed",
  Failed = "Failed",
  Paused = "Paused",
}

// 제품 페이지 (페이지네이션)
export interface ProductPage {
  products: ProductInfo[];
  current_page: number;
  page_size: number;
  total_items: number;
  total_pages: number;
}

// 데이터베이스 통계
export interface DatabaseStats {
  total_products: number;
  products_added_today: number;
  last_updated?: string; // ISO string
  database_size_bytes: number;
}

// API 응답 래퍼
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
  };
}

// 크롤링 시작 요청
export interface StartCrawlingRequest {
  config: AdvancedCrawlingConfig;
}

// 크롤링 범위 계산 요청
export interface CrawlingRangeRequest {
  total_pages_on_site: number;
  products_on_last_page: number;
}

// 크롤링 범위 계산 응답
export interface CrawlingRangeResponse {
  success: boolean;
  range?: [number, number]; // [start_page, end_page] 튜플
  progress: {
    total_products: number;
    saved_products: number;
    progress_percentage: number;
    max_page_id?: number;
    max_index_in_page?: number;
    is_completed: boolean;
  };
  message: string;
}

// Tauri 명령어 타입들
export interface TauriCommands {
  // Advanced Crawling Engine 사이트 상태 확인
  check_advanced_site_status(): Promise<ApiResponse<SiteStatusInfo>>;
  
  // Advanced Crawling Engine 시작
  start_advanced_crawling(request: StartCrawlingRequest): Promise<ApiResponse<CrawlingSession>>;
  
  // 크롤링 범위 계산
  calculate_crawling_range(request: CrawlingRangeRequest): Promise<ApiResponse<CrawlingRangeResponse>>;
  
  // 최근 제품 목록 조회
  get_recent_products(page?: number, limit?: number): Promise<ApiResponse<ProductPage>>;
  
  // 데이터베이스 통계 조회
  get_database_stats(): Promise<ApiResponse<DatabaseStats>>;
}

// Tauri 이벤트 타입들
export interface TauriEvents {
  'crawling-progress': CrawlingProgressInfo;
  'crawling-completed': CrawlingSession;
  'crawling-failed': CrawlingSession;
}
