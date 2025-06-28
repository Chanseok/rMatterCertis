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
