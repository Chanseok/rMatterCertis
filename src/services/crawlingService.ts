import { invoke } from "@tauri-apps/api/core";
import type { 
  StartCrawlingRequest, 
  CrawlingSessionState, 
  CrawlingStats,
  CrawlingStatusResponse,
  Product,
  MatterProduct,
  ProductSearchRequest,
  ProductSearchResult,
  MatterProductFilter,
  DatabaseSummary
} from "../types/crawling";

export class CrawlingService {
  
  /**
   * Start a new crawling session
   */
  static async startCrawling(request: StartCrawlingRequest): Promise<string> {
    return await invoke<string>("start_crawling", { request });
  }

  /**
   * Get status of a crawling session
   */
  static async getCrawlingStatus(sessionId: string): Promise<CrawlingStatusResponse> {
    return await invoke<CrawlingStatusResponse>("get_crawling_status", { sessionId });
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
   * Get overall crawling statistics
   */
  static async getCrawlingStats(): Promise<CrawlingStats> {
    return await invoke<CrawlingStats>("get_enhanced_crawling_stats");
  }

  /**
   * Get all active crawling sessions
   */
  static async getActiveSessions(): Promise<CrawlingSessionState[]> {
    return await invoke<CrawlingSessionState[]>("get_active_crawling_sessions");
  }

  /**
   * Get crawling session history
   */
  static async getSessionHistory(): Promise<CrawlingSessionState[]> {
    return await invoke<CrawlingSessionState[]>("get_crawling_session_history");
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
