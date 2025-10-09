/**
 * Diagnostics result types for database health checks
 * 
 * These types represent the structure returned by the backend diagnostics command.
 * Used in DiagnosticsPanel and CrawlingEngineTabSimple.
 */

export interface DiagnosticsResult {
  // Product counts
  total_products: number;
  total_products_with_coords: number;
  total_products_without_coords: number;
  
  // Site statistics
  total_products_site?: number;
  total_pages_site?: number;
  items_on_last_page?: number;
  
  // Database metadata
  max_page_id_db: number | null;
  
  // Prepass statistics (optional)
  prepass?: {
    details_aligned: number;
    products_id_backfilled: number;
  };
  
  // Issue detection
  group_summaries: GroupSummary[];
  missing_pages: PageGap[];
  duplicate_positions: DuplicatePosition[];
  total_missing_pages: number;
}

export interface GroupSummary {
  page_id: number;
  current_page_number: number | null;
  status: 'ok' | 'warning' | 'error';
  count: number;
  distinct_indices: number;
  duplicate_indices?: number[];
  missing_indices?: number[];
  out_of_range_count?: number;
}

export interface PageGap {
  gap_type: 'single' | 'range';
  start_page: number;
  end_page?: number;
  start_physical_page?: number | null;
  end_physical_page?: number | null;
  missing_count: number;
}

export interface DuplicatePosition {
  page_id: number;
  current_page_number?: number | null;
  index_in_page: number;
  urls?: string[];
  count?: number;
  product_ids?: number[];
}
