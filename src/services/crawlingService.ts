import { invoke } from "@tauri-apps/api/core";
import type { 
  Product,
  MatterProduct,
  ProductSearchRequest,
  ProductSearchResult,
  MatterProductFilter,
  DatabaseSummary,
  BackendCrawlerConfig
} from "../types/crawling";

// 현대적인 타입 정의 (레거시 코드 제거)
export interface ModernStartCrawlingRequest {
  config: BackendCrawlerConfig;
}

export interface SessionStatusDto {
  session_id: string;
  status: string;
  progress: number;
  current_step: string;
  started_at: string;
  last_updated: string;
}

export interface ModernCrawlingStats {
  total_sessions: number;
  active_sessions: number;
  completed_sessions: number;
  total_pages_crawled: number;
  average_success_rate: number;
  database_stats: DatabaseSummary;
}

export class CrawlingService {
  
  /**
   * Start a new crawling session with modern configuration
   */
  static async startCrawling(config: BackendCrawlerConfig): Promise<string> {
    return await invoke<string>("start_crawling", { config });
  }

  /**
   * Get status of a crawling session (modern)
   */
  static async getCrawlingStatus(sessionId: string): Promise<SessionStatusDto> {
    return await invoke<SessionStatusDto>("get_crawling_status", { sessionId });
  }

  /**
   * Stop a crawling session
   */
  static async stopCrawling(sessionId: string): Promise<void> {
    await invoke("stop_crawling", { sessionId });
  }

  /**
   * Pause a crawling session
   */
  static async pauseCrawling(sessionId: string): Promise<void> {
    await invoke("pause_crawling", { sessionId });
  }

  /**
   * Resume a crawling session
   */
  static async resumeCrawling(sessionId: string): Promise<void> {
    await invoke("resume_crawling", { sessionId });
  }

  /**
   * Get overall crawling statistics (modern)
   */
  static async getCrawlingStats(): Promise<ModernCrawlingStats> {
    return await invoke<ModernCrawlingStats>("get_enhanced_crawling_stats");
  }

  /**
   * Get all active crawling sessions (modern)
   */
  static async getActiveSessions(): Promise<SessionStatusDto[]> {
    return await invoke<SessionStatusDto[]>("get_active_crawling_sessions");
  }

  /**
   * Get crawling session history (modern)
   */
  static async getSessionHistory(): Promise<SessionStatusDto[]> {
    return await invoke<SessionStatusDto[]>("get_crawling_session_history");
  }

  /**
   * Get all basic products from crawling results
   */
  static async getProducts(): Promise<Product[]> {
    return await invoke<Product[]>("get_products");
  }

  /**
   * Get all Matter products from crawling results
   */
  static async getMatterProducts(): Promise<MatterProduct[]> {
    return await invoke<MatterProduct[]>("get_matter_products");
  }

  /**
   * Search products with filters and pagination
   */
  static async searchProducts(searchRequest: ProductSearchRequest): Promise<ProductSearchResult> {
    return await invoke<ProductSearchResult>("search_products", { searchDto: searchRequest });
  }

  /**
   * Get products by manufacturer
   */
  static async getProductsByManufacturer(manufacturer: string): Promise<Product[]> {
    return await invoke<Product[]>("get_products_by_manufacturer", { manufacturer });
  }

  /**
   * Filter Matter products with advanced criteria
   */
  static async filterMatterProducts(filter: MatterProductFilter): Promise<MatterProduct[]> {
    return await invoke<MatterProduct[]>("filter_matter_products", { filterDto: filter });
  }

  /**
   * Get database summary with counts and statistics
   */
  static async getDatabaseSummary(): Promise<DatabaseSummary> {
    return await invoke<DatabaseSummary>("get_database_summary");
  }

  /**
   * Get recently added products
   */
  static async getRecentProducts(limit?: number): Promise<Product[]> {
    return await invoke<Product[]>("get_recent_products", { limit });
  }
}
