import { invoke } from "@tauri-apps/api/core";
import type { 
  StartCrawlingRequest, 
  CrawlingSessionState, 
  CrawlingStats,
  CrawlingStatusResponse 
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
}
