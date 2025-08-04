// Dashboard Types - 실시간 크롤링 대시보드를 위한 타입 정의

export interface DashboardState {
  is_active: boolean;
  active_sessions: ActiveCrawlingSession[];
  performance_metrics: RealtimePerformanceMetrics;
  chart_data: ChartDataPoint[];
  alerts: AlertMessage[];
  system_health: SystemHealthStatus;
}

export interface ActiveCrawlingSession {
  session_id: string;
  start_time: string; // ISO 8601 date string
  progress: CrawlingProgress;
  current_stage: string;
  estimated_completion: string | null;
  performance_metrics: PerformanceMetrics;
}

export interface RealtimePerformanceMetrics {
  requests_per_second: number;
  success_rate: number;
  average_response_time_ms: number;
  active_connections: number;
  memory_usage_mb: number;
  cpu_usage_percent: number;
}

export interface ChartDataPoint {
  timestamp: string; // ISO 8601 date string
  value: number;
  metric_type: ChartMetricType;
  session_id?: string;
}

export type ChartMetricType = 
  | "requests_per_second"
  | "success_rate" 
  | "response_time"
  | "memory_usage"
  | "cpu_usage"
  | "pages_processed"
  | "products_collected";

export interface AlertMessage {
  id: string;
  level: AlertLevel;
  message: string;
  timestamp: string;
  category: string;
  auto_dismiss: boolean;
}

export type AlertLevel = "info" | "warning" | "error" | "success";

export interface SystemHealthStatus {
  overall_status: "healthy" | "warning" | "critical";
  database_status: "connected" | "disconnected" | "error";
  network_status: "stable" | "unstable" | "offline";
  actor_system_status: "running" | "degraded" | "stopped";
  last_health_check: string;
}

export interface PerformanceMetrics {
  pages_processed: number;
  products_collected: number;
  errors_count: number;
  current_speed: number; // items per minute
  estimated_completion: string | null;
}

export interface CrawlingProgress {
  total_pages: number;
  completed_pages: number;
  failed_pages: number;
  total_products: number;
  collected_products: number;
  failed_products: number;
  current_stage: string;
  percentage_complete: number;
}

// Dashboard Event Types for real-time updates
export interface DashboardEvent {
  event_type: DashboardEventType;
  timestamp: string;
  data: DashboardEventData;
}

export type DashboardEventType = 
  | "session_started"
  | "session_completed" 
  | "session_failed"
  | "metrics_updated"
  | "progress_updated"
  | "alert_triggered"
  | "health_status_changed";

export type DashboardEventData = 
  | SessionEvent
  | MetricsEvent  
  | ProgressEvent
  | AlertEvent
  | HealthEvent;

export interface SessionEvent {
  session_id: string;
  session_data: ActiveCrawlingSession;
}

export interface MetricsEvent {
  session_id?: string;
  metrics: RealtimePerformanceMetrics;
  chart_data: ChartDataPoint[];
}

export interface ProgressEvent {
  session_id: string;
  progress: CrawlingProgress;
}

export interface AlertEvent {
  alert: AlertMessage;
}

export interface HealthEvent {
  health_status: SystemHealthStatus;
}

// Chart Configuration Types
export interface ChartConfig {
  chart_type: ChartType;
  metrics: ChartMetricType[];
  time_range_minutes: number;
  update_interval_ms: number;
  max_data_points: number;
}

export type ChartType = "line" | "area" | "bar" | "pie" | "gauge";

// Dashboard UI State
export interface DashboardUIState {
  selected_session_id: string | null;
  visible_charts: ChartConfig[];
  alert_panel_open: boolean;
  auto_refresh_enabled: boolean;
  refresh_interval_ms: number;
}
