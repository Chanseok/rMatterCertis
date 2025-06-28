// TypeScript domain types synchronized with Rust DTOs
// Generated from: src-tauri/src/application/dto.rs

// ============================================================================
// Vendor Types
// ============================================================================

export interface CreateVendorDto {
  vendor_number: number;           // Matter 인증 벤더 번호 (숫자)
  vendor_name: string;            // 벤더명
  company_legal_name: string;     // 법인명 (Matter 인증 필수)
  vendor_url?: string;            // 벤더 웹사이트 URL
  csa_assigned_number?: string;   // CSA 할당 번호
}

export interface UpdateVendorDto {
  vendor_name?: string;
  company_legal_name?: string;
  vendor_url?: string;
  csa_assigned_number?: string;
}

export interface VendorResponseDto {
  vendor_id: string;
  vendor_number: number;
  vendor_name: string;
  company_legal_name: string;
  created_at: string;
}

// ============================================================================
// Product Types
// ============================================================================

export interface CreateProductDto {
  url: string;                    // 제품 상세 페이지 URL
  manufacturer?: string;
  model?: string;
  certificate_id?: string;
  page_id?: number;
  index_in_page?: number;
}

export interface UpdateProductDto {
  manufacturer?: string;
  model?: string;
  certificate_id?: string;
}

export interface ProductResponseDto {
  url: string;
  manufacturer?: string;
  model?: string;
  certificate_id?: string;
  page_id?: number;
  index_in_page?: number;
  created_at: string;
}

// ============================================================================
// MatterProduct Types (Matter 인증 특화)
// ============================================================================

export interface CreateMatterProductDto {
  url: string;                    // Product와 연결되는 URL
  page_id?: number;
  json_data?: string;             // Raw JSON data from crawling
  vid?: string;                   // Vendor ID (Matter 특화)
  pid?: string;                   // Product ID (Matter 특화)
  device_name?: string;           // Device name
  device_type?: string;           // Device type
  manufacturer?: string;
  certification_date?: string;
  commissioning_method?: string;
  transport_protocol?: string;
  application_categories?: string; // JSON string
  clusters_client?: string;        // JSON string
  clusters_server?: string;        // JSON string
}

export interface MatterProductResponseDto {
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

// ============================================================================
// Session Management Types (Memory-based)
// ============================================================================

export interface StartSessionDto {
  session_id: string;
  start_url: string;
  target_domains: string[];
}

export interface SessionStatusDto {
  session_id: string;
  status: string;
  progress: number;
  current_step: string;
  started_at: string;
  last_updated: string;
}

// ============================================================================
// Search and Filter Types
// ============================================================================

export interface ProductSearchDto {
  query?: string;
  page?: number;
  page_size?: number;
}

export interface MatterProductFilterDto {
  manufacturer?: string;
  device_type?: string;
  vid?: string;
  certification_date_start?: string;
  certification_date_end?: string;
  page?: number;
  page_size?: number;
}

export interface ProductSearchResultDto {
  products: MatterProductResponseDto[];
  total_count: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// ============================================================================
// Database Summary Types
// ============================================================================

export interface DatabaseSummaryDto {
  total_vendors: number;
  total_products: number;
  total_matter_products: number;
  database_size_mb: number;
  last_crawling_date?: string;
}

// ============================================================================
// Crawling Engine Types (for Phase 3 implementation)
// ============================================================================

export interface StartCrawlingDto {
  start_url: string;
  target_domains: string[];
  max_pages?: number;
  concurrent_requests?: number;
  delay_ms?: number;
}

export interface CrawlingConfigDto {
  max_concurrent_requests: number;
  request_delay_ms: number;
  timeout_seconds: number;
  retry_attempts: number;
  user_agent: string;
  respect_robots_txt: boolean;
}

export interface CrawlingResultDto {
  session_id: string;
  status: string;
  total_pages_crawled: number;
  products_found: number;
  errors_count: number;
  started_at: string;
  completed_at?: string;
  execution_time_seconds?: number;
  error_details: string[];
}

export interface CrawlingProgressDto {
  session_id: string;
  current_page: number;
  total_pages: number;
  progress_percentage: number;
  current_url?: string;
  products_found: number;
  last_updated: string;
  estimated_completion?: string;
}

// ============================================================================
// Union Types for State Management
// ============================================================================

export type CrawlingStatus = 'idle' | 'running' | 'paused' | 'completed' | 'error';

export type DeviceType = 
  | 'bridge'
  | 'light'
  | 'switch'
  | 'sensor'
  | 'thermostat'
  | 'lock'
  | 'camera'
  | 'speaker'
  | 'display'
  | 'appliance'
  | 'other';

export type SortOrder = 'asc' | 'desc';

// ============================================================================
// Utility Types
// ============================================================================

export interface ApiResponse<T> {
  data: T;
  success: boolean;
  error?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  total_count: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface ApiError {
  message: string;
  code?: string;
  details?: Record<string, any>;
}
