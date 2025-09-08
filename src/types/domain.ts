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

// Removed unused UpdateProductDto.

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

// Removed unused StartSessionDto.

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

// Removed unused ProductSearchDto.

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

// Removed unused DatabaseSummaryDto.

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

// Removed unused CrawlingConfigDto.

// Removed unused CrawlingResultDto.

// Removed unused CrawlingProgressDto.

// ============================================================================
// Union Types for State Management
// ============================================================================

// Removed unused CrawlingStatus union.

// Removed unused DeviceType union.

// Removed unused SortOrder union.

// ============================================================================
// Utility Types
// ============================================================================

// Removed unused ApiResponse.

// Removed unused PaginatedResponse.

export interface ApiError {
  message: string;
  code?: string;
  details?: Record<string, any>;
}

// Result returned by update_vendors_from_csa Tauri command
export interface VendorSyncResult {
  inserted: number;
  updated: number;
  skipped: number;
  api_total: number;
  final_count: number;
  pages: number;
  finished_at: string; // ISO timestamp
}
