// Ambient placeholder declarations to satisfy generated types that reference global names without imports.
// Temporary shim: replace with proper generated imports or fix generator to emit imports.

// Basic utility aliases
type Timestamp = string;

// Actor / Session related
type SessionActorStatus = any;
type BatchActorStatus = any;
type StageActorStatus = any;
type ChannelStatus = any;
type PerformanceMetrics = any;

// Concurrency / Task system
type TaskExecutionContext = any;
type TaskLifecycleEvent = any; // Detailed union should replace
type TaskPriority = string;
type ConcurrencySnapshot = any;
type ConcurrencyInsight = any;
type BottleneckAlert = any;
type OptimizationHint = any;
type ResourceForecast = any;
type PerformanceTrend = any;
type TaskState = any;
type StageType = any;
type SessionEventType = string;
type BatchEventType = string;
type TaskTypeInfo = any;
type ResourceAllocation = any;
type ResourceUsage = any;
type ErrorCategory = string;
type RetryStrategy = any;

// Crawling progress / plans
type OverallProgress = any;
type StageProgress = any;
type CrawlingPerformanceStats = any;
type CrawlingEvent = any;
type TimeStats = any;
type SiteStatusInfo = any;
type BatchPlan = any;
type DetailedStats = any;

// Domain specific additions referenced by UI
interface SiteStatus {
  accessible?: boolean; // aligns with 'accessible' field
  is_accessible?: boolean; // legacy code reference; will be deprecated
  products_on_last_page?: number | null;
  data_change_status?: string | null;
  crawling_range_recommendation?: string | null;
  [key: string]: any;
}

// Allow BigInt-like placeholders (backend serialized maybe as number or string)
// eslint-disable-next-line @typescript-eslint/no-unused-vars
// noinspection JSUnusedGlobalSymbols
// Provide a typing for bigint usage in generated d.ts if needed
// Note: This does not override native bigint, it's just to silence references where generator used a custom BigInt placeholder.
type bigint = any;
