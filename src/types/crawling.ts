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
