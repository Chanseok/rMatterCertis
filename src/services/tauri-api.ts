/**
 * Modern Tauri API Service for Real-time Crawling Operations
 * 
 * This service provides a clean interface for communicating with the Rust backend
 * using the new real-time event system. It encapsulates all Tauri invoke calls
 * and event listening logic.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { loggingService } from './loggingService';
import type {
  BackendCrawlerConfig,
  CrawlingProgress,
  CrawlingResult,
  CrawlingTaskStatus,
  CrawlingStatusCheck,
  DatabaseStats
} from '../types/crawling';
import type { 
  AtomicTaskEvent,
  SystemStatePayload,
  LiveSystemState
} from '../types/events';

/**
 * Service class for communicating with the Rust backend
 */
export class TauriApiService {
  private eventListeners: Map<string, UnlistenFn> = new Map();

  // =========================================================================
  // Crawling Control Commands
  // =========================================================================

  /**
   * Start the unified Actor-based crawling pipeline.
   * mode: 'advanced' | 'live' to align with UI tabs. Overrides are optional.
   */
  async startUnifiedCrawling(options: {
    mode?: 'advanced' | 'live';
    overrideBatchSize?: number;
    overrideConcurrency?: number;
    delayMs?: number;
  } = {}): Promise<{ success: boolean; message: string; session_id?: string }> {
    const req = {
      mode: options.mode,
      override_batch_size: options.overrideBatchSize,
      override_concurrency: options.overrideConcurrency,
      delay_ms: options.delayMs,
    };
    const res = await invoke<any>('start_unified_crawling', { request: req });
    return res as { success: boolean; message: string; session_id?: string };
  }

  /**
   * Start a new crawling session with page range
   */
  async startCrawling(startPage?: number, endPage?: number): Promise<string> {
    try {
      console.log('🚀 TauriApiService.startCrawling 호출됨');
      console.log('📋 파라미터:', { startPage, endPage });
      
      // 1. 먼저 크롤링 엔진을 초기화합니다
      console.log('🔧 크롤링 엔진 초기화 시도...');
      try {
        const initResponse = await invoke<any>('init_crawling_engine');
        console.log('✅ 크롤링 엔진 초기화 응답:', initResponse);
        
        if (initResponse && !initResponse.success && initResponse.message !== "Crawling engine is already initialized") {
          throw new Error(`엔진 초기화 실패: ${initResponse.message}`);
        }
      } catch (initError) {
        console.error('❌ 크롤링 엔진 초기화 실패:', initError);
        throw new Error(`크롤링 엔진 초기화 실패: ${initError}`);
      }
      
      // 2. 백엔드에서 기대하는 StartCrawlingRequest 형태로 파라미터 전달
      // start_page와 end_page가 0이면 백엔드에서 지능형 계산 사용
      const request = {
        start_page: startPage || 0,     // 0이면 백엔드에서 지능형 계산
        end_page: endPage || 0,         // 0이면 백엔드에서 지능형 계산
        max_products_per_page: null,
        concurrent_requests: null,
        request_timeout_seconds: null
      };
      
      console.log('📞 Tauri invoke 호출 시도: start_crawling');
      console.log('📋 Request 구조:', request);
      
      const response = await invoke<any>('start_crawling', { request });
      console.log('✅ 백엔드 응답 받음:', response);
      
      // 응답이 객체인 경우 적절히 처리
      if (typeof response === 'object' && response.success) {
        return response.message || 'Crawling started successfully';
      } else if (typeof response === 'string') {
        return response;
      } else {
        return 'Crawling started successfully';
      }
    } catch (error) {
      console.error('❌ TauriApiService.startCrawling 실패:', error);
      throw new Error(`Failed to start crawling: ${error}`);
    }
  }

  /**
   * Pause the current crawling session
   */
  async pauseCrawling(): Promise<void> {
    try {
      await invoke<void>('pause_crawling');
    } catch (error) {
      throw new Error(`Failed to pause crawling: ${error}`);
    }
  }

  /**
   * Resume the paused crawling session
   */
  async resumeCrawling(): Promise<void> {
    try {
      await invoke<void>('resume_crawling');
    } catch (error) {
      throw new Error(`Failed to resume crawling: ${error}`);
    }
  }

  /**
   * Stop the current crawling session
   */
  async stopCrawling(): Promise<void> {
    try {
      await invoke<void>('stop_crawling');
    } catch (error) {
      throw new Error(`Failed to stop crawling: ${error}`);
    }
  }

  // =========================================================================
  // Status and Data Retrieval Commands
  // =========================================================================

  /**
   * Start a validation pass against the database/site. Oldest-forward with optional range.
   */
  async startValidation(options: {
    scanPages?: number;
    startPhysicalPage?: number;
    endPhysicalPage?: number;
    rangesExpr?: string;
  } = {}): Promise<any> {
    const args = {
      scanPages: options.scanPages ?? null,
      startPhysicalPage: options.startPhysicalPage ?? null,
      endPhysicalPage: options.endPhysicalPage ?? null,
      rangesExpr: options.rangesExpr ?? null,
    } as any;
    return await invoke<any>('start_validation', args);
  }

  /**
   * Start a partial sync over provided ranges string (e.g., "498-492,489,487-485").
   */
  async startPartialSync(ranges: string, dryRun?: boolean): Promise<any> {
    return await invoke<any>('start_partial_sync', {
      ranges,
      dryRun: dryRun ?? null,
    } as any);
  }

  /**
   * Start an anomaly-repair sync sweep around divergent pages.
   */
  async startRepairSync(buffer?: number, dryRun?: boolean): Promise<any> {
    return await invoke<any>('start_repair_sync', {
      buffer: buffer ?? null,
      dryRun: dryRun ?? null,
    } as any);
  }

  /**
   * Get the current crawling progress and status
   */
  async getCrawlingStatus(): Promise<CrawlingProgress> {
    try {
      return await invoke<CrawlingProgress>('get_crawling_status');
    } catch (error) {
      throw new Error(`Failed to get crawling status: ${error}`);
    }
  }

  /**
   * Get database statistics
   */
  async getDatabaseStats(): Promise<DatabaseStats> {
    try {
      return await invoke<DatabaseStats>('get_database_stats');
    } catch (error) {
      throw new Error(`Failed to get database stats: ${error}`);
    }
  }

  /**
   * Get active crawling sessions
   */
  async getActiveSessions(): Promise<string[]> {
    try {
      return await invoke<string[]>('get_active_sessions');
    } catch (error) {
      throw new Error(`Failed to get active sessions: ${error}`);
    }
  }

  /**
   * Get products from database with pagination
   */
  async getProducts(page?: number, limit?: number): Promise<any> {
    try {
      return await invoke<any>('get_products', { page, limit });
    } catch (error) {
      throw new Error(`Failed to get products: ${error}`);
    }
  }

  /**
   * Get local database statistics
   */
  async getLocalDbStats(): Promise<any> {
    try {
      return await invoke<any>('get_local_db_stats');
    } catch (error) {
      throw new Error(`Failed to get local DB stats: ${error}`);
    }
  }

  /**
   * Get analysis data for Analysis tab
   */
  async getAnalysisData(): Promise<any> {
    try {
      return await invoke<any>('get_analysis_data');
    } catch (error) {
      throw new Error(`Failed to get analysis data: ${error}`);
    }
  }

  // =========================================================================
  // Database Management Commands
  // =========================================================================

  /**
   * Create a backup of the database
   */
  async backupDatabase(): Promise<string> {
    try {
      return await invoke<string>('backup_database');
    } catch (error) {
      throw new Error(`Failed to backup database: ${error}`);
    }
  }

  /**
   * Optimize the database for better performance
   */
  async optimizeDatabase(): Promise<void> {
    try {
      await invoke<void>('optimize_database');
    } catch (error) {
      throw new Error(`Failed to optimize database: ${error}`);
    }
  }

  /**
   * Export database data in the specified format
   */
  async exportDatabaseData(format: 'csv' | 'json' | 'excel'): Promise<string> {
    try {
      return await invoke<string>('export_database_data', { format });
    } catch (error) {
      throw new Error(`Failed to export database data: ${error}`);
    }
  }

  /**
   * Clear all crawling error logs
   */
  async clearCrawlingErrors(): Promise<void> {
    try {
      await invoke<void>('clear_crawling_errors');
    } catch (error) {
      throw new Error(`Failed to clear crawling errors: ${error}`);
    }
  }

  /**
   * Export crawling results to a file
   */
  async exportCrawlingResults(): Promise<string> {
    try {
      return await invoke<string>('export_crawling_results');
    } catch (error) {
      throw new Error(`Failed to export crawling results: ${error}`);
    }
  }

  // =========================================================================
  // Real-time Event Subscription
  // =========================================================================

  /**
   * Subscribe to crawling progress updates
   */
  async subscribeToProgress(callback: (progress: CrawlingProgress) => void): Promise<UnlistenFn> {
    const unlisten = await listen<CrawlingProgress>('crawling-progress', (event) => {
      // 저수준 이벤트 수신 로깅
      loggingService.debug(
        `Raw Event Received: crawling-progress - ${JSON.stringify(event.payload)}`,
        'TauriApiService'
      );
      callback(event.payload);
    });
    
    this.eventListeners.set('crawling-progress', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to concurrency broadcast events (optional)
   */
  async subscribeToConcurrency(
    callback: (event: any) => void
  ): Promise<UnlistenFn> {
    const unlisten = await listen<any>('concurrency-event', (event) => {
      callback(event.payload);
    });
    this.eventListeners.set('concurrency-event', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to validation detailed events (optional)
   */
  async subscribeToValidation(
    callback: (event: any) => void
  ): Promise<UnlistenFn> {
    const unlisten = await listen<any>('validation-event', (event) => {
      callback(event.payload);
    });
    this.eventListeners.set('validation-event', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to database save detailed events (optional)
   */
  async subscribeToDbSave(
    callback: (event: any) => void
  ): Promise<UnlistenFn> {
    const unlisten = await listen<any>('db-save-event', (event) => {
      callback(event.payload);
    });
    this.eventListeners.set('db-save-event', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to individual task status updates
   */
  async subscribeToTaskStatus(callback: (status: CrawlingTaskStatus) => void): Promise<UnlistenFn> {
    const unlisten = await listen<CrawlingTaskStatus>('crawling-task-update', (event) => {
      // 저수준 이벤트 수신 로깅
      loggingService.debug(
        `Raw Event Received: crawling-task-update - ${JSON.stringify(event.payload)}`,
        'TauriApiService'
      );
      callback(event.payload);
    });
    
    this.eventListeners.set('crawling-task-update', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to crawling stage changes
   */
  async subscribeToStageChange(
    callback: (data: { from: string; to: string; message: string }) => void
  ): Promise<UnlistenFn> {
    const unlisten = await listen<{ from: string; to: string; message: string }>(
      'crawling-stage-change',
      (event) => {
        callback(event.payload);
      }
    );
    
    this.eventListeners.set('crawling-stage-change', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to error notifications
   */
  async subscribeToErrors(
    callback: (error: { error_id: string; message: string; stage: string; recoverable: boolean }) => void
  ): Promise<UnlistenFn> {
    const unlisten = await listen<{ error_id: string; message: string; stage: string; recoverable: boolean }>(
      'crawling-error',
      (event) => {
        callback(event.payload);
      }
    );
    
    this.eventListeners.set('crawling-error', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to database statistics updates
   */
  async subscribeToDatabaseUpdates(callback: (stats: DatabaseStats) => void): Promise<UnlistenFn> {
    const unlisten = await listen<DatabaseStats>('database-update', (event) => {
      callback(event.payload);
    });
    
    this.eventListeners.set('database-update', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to crawling completion events
   */
  async subscribeToCompletion(callback: (result: CrawlingResult) => void): Promise<UnlistenFn> {
    const unlisten = await listen<CrawlingResult>('crawling-completed', (event) => {
      callback(event.payload);
    });
    
    this.eventListeners.set('crawling-completed', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to crawling stop events
   */
  async subscribeToCrawlingStopped(callback: (data: { status: string; message: string; timestamp: string }) => void): Promise<UnlistenFn> {
    const unlisten = await listen<{ status: string; message: string; timestamp: string }>(
      'crawling-stopped',
      (event) => {
        callback(event.payload);
      }
    );
    
    this.eventListeners.set('crawling-stopped', unlisten);
    return unlisten;
  }

  // =========================================================================
  // 원자적 태스크 이벤트 구독 (proposal5.md 구현)
  // =========================================================================
  // Live Production Line Event Subscriptions
  // =========================================================================

  /**
   * Subscribe to task started events specifically
   */
  async subscribeToTaskStarted(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn> {
    return this.subscribeToAtomicTaskUpdates((event) => {
      if (event.status === 'Active') {
        callback(event);
      }
    });
  }

  /**
   * Subscribe to task completed events specifically
   */
  async subscribeToTaskCompleted(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn> {
    return this.subscribeToAtomicTaskUpdates((event) => {
      if (event.status === 'Success') {
        callback(event);
      }
    });
  }

  /**
   * Subscribe to task failed events specifically
   */
  async subscribeToTaskFailed(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn> {
    return this.subscribeToAtomicTaskUpdates((event) => {
      if (event.status === 'Error') {
        callback(event);
      }
    });
  }

  /**
   * Subscribe to task retrying events specifically
   */
  async subscribeToTaskRetrying(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn> {
    return this.subscribeToAtomicTaskUpdates((event) => {
      if (event.status === 'Retrying') {
        callback(event);
      }
    });
  }

  // =========================================================================
  // Live Production Line Event Subscriptions
  // =========================================================================

  /**
   * Subscribe to system state updates (macro-level information)
   */
  async subscribeToSystemStateUpdates(callback: (state: SystemStatePayload) => void): Promise<UnlistenFn> {
    const unlisten = await listen<SystemStatePayload>('system-state-update', (event) => {
      callback(event.payload);
    });
    
    this.eventListeners.set('system-state-update', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to atomic task events (micro-level information)
   */
  async subscribeToAtomicTaskUpdates(callback: (event: AtomicTaskEvent) => void): Promise<UnlistenFn> {
    const unlisten = await listen<AtomicTaskEvent>('atomic-task-update', (event) => {
      callback(event.payload);
    });
    
    this.eventListeners.set('atomic-task-update', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to live system state updates (comprehensive Live Production Line data)
   */
  async subscribeToLiveSystemState(callback: (state: LiveSystemState) => void): Promise<UnlistenFn> {
    const unlisten = await listen<LiveSystemState>('live-state-update', (event) => {
      callback(event.payload);
    });
    
    this.eventListeners.set('live-state-update', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to detailed crawling events (hierarchical event monitor)
   */
  async subscribeToDetailedCrawlingEvents(callback: (event: any) => void): Promise<UnlistenFn> {
    const unlisten = await listen<any>('detailed-crawling-event', (event) => {
      console.log('[Debug] [TauriApiService] Raw Detailed Event Received:', event.payload);
      callback(event.payload);
    });
    
    this.eventListeners.set('detailed-crawling-event', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to all Live Production Line events with proper typing
   */
  async subscribeToLiveProductionLineEvents(callbacks: {
    onSystemStateUpdate?: (state: SystemStatePayload) => void;
    onAtomicTaskUpdate?: (event: AtomicTaskEvent) => void;
    onLiveStateUpdate?: (state: LiveSystemState) => void;
  }): Promise<UnlistenFn[]> {
    const unlisteners: UnlistenFn[] = [];

    if (callbacks.onSystemStateUpdate) {
      const unlisten = await this.subscribeToSystemStateUpdates(callbacks.onSystemStateUpdate);
      unlisteners.push(unlisten);
    }

    if (callbacks.onAtomicTaskUpdate) {
      const unlisten = await this.subscribeToAtomicTaskUpdates(callbacks.onAtomicTaskUpdate);
      unlisteners.push(unlisten);
    }

    if (callbacks.onLiveStateUpdate) {
      const unlisten = await this.subscribeToLiveSystemState(callbacks.onLiveStateUpdate);
      unlisteners.push(unlisten);
    }

    return unlisteners;
  }

  // =========================================================================
  // Event Management
  // =========================================================================

  /**
   * Subscribe to all Actor bridge events emitted by the Rust ActorEventBridge.
   * This listens to a curated set of 'actor-*' event names and invokes the callback with (eventName, payload).
   */
  async subscribeToActorBridgeEvents(
    callback: (eventName: string, payload: any) => void
  ): Promise<() => void> {
    const names = [
      // Session
      'actor-session-started',
      'actor-session-paused',
      'actor-session-resumed',
      'actor-session-completed',
      'actor-session-failed',
      'actor-session-timeout',
      // Batch/Stage
      'actor-batch-started',
      'actor-batch-completed',
      'actor-batch-failed',
      'actor-stage-started',
      'actor-stage-completed',
      'actor-stage-failed',
      // Progress / Metrics / Reports
      'actor-progress',
      'actor-performance-metrics',
      'actor-batch-report',
      'actor-session-report',
      // Phases / Shutdown
      'actor-phase-started',
      'actor-phase-completed',
      'actor-phase-aborted',
      'actor-shutdown-requested',
      'actor-shutdown-completed',
      // Page & Detail lifecycle (Stage2/3)
      'actor-page-task-started',
      'actor-page-task-completed',
      'actor-page-task-failed',
      'actor-detail-task-started',
      'actor-detail-task-completed',
      'actor-detail-task-failed',
      'actor-detail-concurrency-downshifted',
      'actor-stage-item-started',
      'actor-stage-item-completed',
      // High-level lifecycle and timing
      'actor-page-lifecycle',
      'actor-product-lifecycle',
      'actor-product-lifecycle-group',
      'actor-http-request-timing',
      // Validation / Persistence / DB
      'actor-preflight-diagnostics',
      'actor-persistence-anomaly',
      'actor-database-stats',
      'actor-validation-started',
      'actor-validation-page-scanned',
      'actor-validation-divergence',
      'actor-validation-anomaly',
      'actor-validation-completed',
      // Sync (optional)
      'actor-sync-started',
      'actor-sync-page-started',
      'actor-sync-upsert-progress',
      'actor-sync-page-completed',
      'actor-sync-warning',
      'actor-sync-completed',
    ];

    const unsubs: UnlistenFn[] = [];
    for (const name of names) {
      try {
        const un = await listen<any>(name, (evt) => callback(name, evt.payload));
        this.eventListeners.set(name, un);
        unsubs.push(un);
      } catch (e) {
        // If some names are not emitted in a build, ignore subscription failures.
        console.warn(`[ActorBridge] Failed to subscribe '${name}':`, e);
      }
    }
    return () => unsubs.forEach((u) => u());
  }

  /**
   * Unsubscribe from a specific event type
   */
  unsubscribeFromEvent(eventType: string): void {
    const unlisten = this.eventListeners.get(eventType);
    if (unlisten) {
      unlisten();
      this.eventListeners.delete(eventType);
    }
  }

  /**
   * Unsubscribe from all events and clean up resources
   */
  cleanup(): void {
    for (const [, unlisten] of this.eventListeners) {
      unlisten();
    }
    this.eventListeners.clear();
  }

  /**
   * Get list of currently subscribed event types
   */
  getSubscribedEvents(): string[] {
    return Array.from(this.eventListeners.keys());
  }

  // =========================================================================
  // Convenience Methods
  // =========================================================================

  /**
   * Subscribe to all crawling-related events at once
   */
  async subscribeToAllCrawlingEvents(callbacks: {
    onProgress?: (progress: CrawlingProgress) => void;
    onTaskUpdate?: (status: CrawlingTaskStatus) => void;
    onStageChange?: (data: { from: string; to: string; message: string }) => void;
    onError?: (error: { error_id: string; message: string; stage: string; recoverable: boolean }) => void;
    onDatabaseUpdate?: (stats: DatabaseStats) => void;
    onCompletion?: (result: CrawlingResult) => void;
  }): Promise<void> {
    const subscriptions: Promise<UnlistenFn>[] = [];

    if (callbacks.onProgress) {
      subscriptions.push(this.subscribeToProgress(callbacks.onProgress));
    }
    if (callbacks.onTaskUpdate) {
      subscriptions.push(this.subscribeToTaskStatus(callbacks.onTaskUpdate));
    }
    if (callbacks.onStageChange) {
      subscriptions.push(this.subscribeToStageChange(callbacks.onStageChange));
    }
    if (callbacks.onError) {
      subscriptions.push(this.subscribeToErrors(callbacks.onError));
    }
    if (callbacks.onDatabaseUpdate) {
      subscriptions.push(this.subscribeToDatabaseUpdates(callbacks.onDatabaseUpdate));
    }
    if (callbacks.onCompletion) {
      subscriptions.push(this.subscribeToCompletion(callbacks.onCompletion));
    }

    await Promise.all(subscriptions);
  }

  // =========================================================================
  // Configuration Management Commands
  // =========================================================================

  /**
   * Get comprehensive crawler configuration from backend
   */
  async getComprehensiveCrawlerConfig(): Promise<BackendCrawlerConfig> {
    try {
      return await invoke<BackendCrawlerConfig>('get_comprehensive_crawler_config');
    } catch (error) {
      throw new Error(`Failed to get comprehensive crawler config: ${error}`);
    }
  }

  /**
   * Get site configuration (URLs and domains)
   */
  async getSiteConfig(): Promise<any> {
    try {
      return await invoke<any>('get_site_config');
    } catch (error) {
      throw new Error(`Failed to get site config: ${error}`);
    }
  }

  /**
   * Scan DB for pagination mismatches (page_id/index_in_page invariants)
   */
  async scanDbPaginationMismatches(): Promise<any> {
    try {
      return await invoke<any>('scan_db_pagination_mismatches');
    } catch (error) {
      throw new Error(`Failed to scan DB pagination mismatches: ${error}`);
    }
  }

  /**
   * Cleanup duplicate rows by URL across products and product_details.
   */
  async cleanupDuplicateUrls(): Promise<any> {
    try {
      return await invoke<any>('cleanup_duplicate_urls');
    } catch (error) {
      throw new Error(`Failed to cleanup duplicate URLs: ${error}`);
    }
  }

  // ❌ REMOVED: getFrontendConfig - 설정 전송 API 제거 (아키텍처 원칙 준수)

  /**
   * Get default crawling configuration
   */
  async getDefaultCrawlingConfig(): Promise<any> {
    try {
      return await invoke<any>('get_default_crawling_config');
    } catch (error) {
      throw new Error(`Failed to get default crawling config: ${error}`);
    }
  }

  // =========================================================================
  // Application Configuration Commands
  // =========================================================================

  /**
   * Initialize app configuration on first run
   */
  async initializeAppConfig(): Promise<any> {
    try {
      return await invoke<any>('initialize_app_config');
    } catch (error) {
      throw new Error(`Failed to initialize app config: ${error}`);
    }
  }

  /**
   * Check if this is the first run of the application
   */
  async isFirstRun(): Promise<boolean> {
    try {
      return await invoke<boolean>('is_first_run');
    } catch (error) {
      throw new Error(`Failed to check first run: ${error}`);
    }
  }

  /**
   * Reset configuration to defaults
   */
  async resetConfigToDefaults(): Promise<any> {
    try {
      return await invoke<any>('reset_config_to_defaults');
    } catch (error) {
      throw new Error(`Failed to reset config: ${error}`);
    }
  }

  /**
   * Get application directories information
   */
  async getAppDirectories(): Promise<any> {
    try {
      return await invoke<any>('get_app_directories');
    } catch (error) {
      throw new Error(`Failed to get app directories: ${error}`);
    }
  }

  // =========================================================================
  // Logging Configuration Commands
  // =========================================================================

  /**
   * Update logging settings
   */
  async updateLoggingSettings(settings: {
    level: string;
    separate_frontend_backend: boolean;
    max_file_size_mb: number;
    max_files: number;
    auto_cleanup_logs: boolean;
    keep_only_latest: boolean;
    module_filters: Record<string, string>;
  }): Promise<void> {
    try {
      await invoke<void>('update_logging_settings', {
        level: settings.level,
        separateFrontendBackend: settings.separate_frontend_backend,
        maxFileSizeMb: settings.max_file_size_mb,
        maxFiles: settings.max_files,
        autoCleanupLogs: settings.auto_cleanup_logs,
        keepOnlyLatest: settings.keep_only_latest,
        moduleFilters: settings.module_filters
      });
    } catch (error) {
      throw new Error(`Failed to update logging settings: ${error}`);
    }
  }

  /**
   * Get log directory path
   */
  async getLogDirectoryPath(): Promise<string> {
    try {
      return await invoke<string>('get_log_directory_path');
    } catch (error) {
      throw new Error(`Failed to get log directory path: ${error}`);
    }
  }

  /**
   * Clean up old log files
   */
  async cleanupLogs(): Promise<string> {
    try {
      return await invoke<string>('cleanup_logs');
    } catch (error) {
      throw new Error(`Failed to cleanup logs: ${error}`);
    }
  }

  /**
   * Update batch processing settings
   */
  async updateBatchSettings(settings: {
    batch_size: number;
    batch_delay_ms: number;
    enable_batch_processing: boolean;
    batch_retry_limit: number;
  }): Promise<void> {
    try {
      await invoke<void>('update_batch_settings', {
        batchSize: settings.batch_size,
        batchDelayMs: settings.batch_delay_ms,
        enableBatchProcessing: settings.enable_batch_processing,
        batchRetryLimit: settings.batch_retry_limit
      });
    } catch (error) {
      throw new Error(`Failed to update batch settings: ${error}`);
    }
  }

  /**
   * Update crawling configuration settings
   */
  async updateCrawlingSettings(settings: {
    page_range_limit: number;
  validation_page_limit?: number | null;
    product_list_retry_count: number;
    product_detail_retry_count: number;
    auto_add_to_local_db: boolean;
  }): Promise<void> {
    try {
      await invoke<void>('update_crawling_settings', {
        pageRangeLimit: settings.page_range_limit,
    validationPageLimit: settings.validation_page_limit ?? null,
        productListRetryCount: settings.product_list_retry_count,
        productDetailRetryCount: settings.product_detail_retry_count,
        autoAddToLocalDb: settings.auto_add_to_local_db
      });
    } catch (error) {
      throw new Error(`Failed to update crawling settings: ${error}`);
    }
  }

  /**
   * Get crawling status check with recommendations (for real-time monitoring during crawling)
   */
  async getCrawlingStatusCheck(): Promise<CrawlingStatusCheck> {
    try {
      return await invoke<CrawlingStatusCheck>('get_crawling_status_check');
    } catch (error) {
      throw new Error(`Failed to get crawling status check: ${error}`);
    }
  }

  /**
   * Check site status comprehensively (for pre-crawling analysis)
   * This performs active site analysis including page discovery and DB comparison
   */
  async checkSiteStatus(): Promise<any> {
    try {
      return await invoke<any>('analyze_system_status');
    } catch (error) {
      throw new Error(`Failed to check site status: ${error}`);
    }
  }

  /**
   * Get retry statistics - INTEGRATED_PHASE2_PLAN Week 1 Day 5
   */
  async getRetryStats(): Promise<any> {
    try {
      return await invoke<any>('get_retry_stats');
    } catch (error) {
      throw new Error(`Failed to get retry stats: ${error}`);
    }
  }

  /**
   * Check backend connection status
   */
  async checkBackendConnection(): Promise<boolean> {
    try {
      console.log('🔌 백엔드 연결 상태 확인 중...');
      
      // 간단한 ping 명령으로 백엔드 응답 확인
      const result = await invoke<any>('ping_backend');
      console.log('✅ 백엔드 연결 확인:', result);
      
      return result && result.status === 'ok';
    } catch (error) {
      console.error('❌ 백엔드 연결 확인 실패:', error);
      return false;
    }
  }

  // =========================================================================
  // Actor System Integration (Future Enhancement)
  // =========================================================================

  /**
   * Start Actor-based crawling (experimental new architecture)
   * This method demonstrates how the UI would integrate with the new Actor system
   */
  async startActorBasedCrawling(config: {
    startPage: number;
    endPage: number;
    batchSize?: number;
    concurrencyLimit?: number;
  }): Promise<string> {
    try {
      console.log('🎭 Starting Actor-based crawling (simulated)...', config);
      
      // For now, simulate actor system by calling existing crawling with enhanced events
      const result = await this.startCrawling(config.startPage, config.endPage);
      
      // Emit simulated actor events to demonstrate UI integration
      this.simulateActorSystemEvents(config);
      
      return result;
    } catch (error) {
      throw new Error(`Failed to start actor-based crawling: ${error}`);
    }
  }

  /**
   * Simulate Actor system events for UI testing
   * This demonstrates how the real Actor system would emit events
   */
  private simulateActorSystemEvents(config: any): void {
    console.log('🎭 Simulating Actor System events for UI integration...');
    
    // Simulate session start
    setTimeout(() => {
      // In the real implementation, this would come from SessionActor
      window.dispatchEvent(new CustomEvent('actor-session-started', {
        detail: {
          session_id: 'session-' + Date.now(),
          total_pages: config.endPage - config.startPage + 1,
          batch_size: config.batchSize || 10,
          timestamp: new Date().toISOString()
        }
      }));
    }, 100);

    // Simulate batch processing
    const totalPages = config.endPage - config.startPage + 1;
    const batchSize = config.batchSize || 10;
    const batches = Math.ceil(totalPages / batchSize);
    
    for (let i = 0; i < batches; i++) {
      setTimeout(() => {
        // Simulate BatchActor events
        window.dispatchEvent(new CustomEvent('actor-batch-started', {
          detail: {
            batch_id: `batch-${i + 1}`,
            batch_number: i + 1,
            total_batches: batches,
            pages_in_batch: Math.min(batchSize, totalPages - i * batchSize),
            timestamp: new Date().toISOString()
          }
        }));

        // Simulate stage events within each batch
        setTimeout(() => {
          ['collection', 'processing', 'storage'].forEach((stage, stageIndex) => {
            setTimeout(() => {
              window.dispatchEvent(new CustomEvent('actor-stage-completed', {
                detail: {
                  batch_id: `batch-${i + 1}`,
                  stage_name: stage,
                  stage_index: stageIndex,
                  success: Math.random() > 0.1, // 90% success rate
                  items_processed: Math.min(batchSize, totalPages - i * batchSize),
                  processing_time_ms: 1000 + Math.random() * 2000,
                  timestamp: new Date().toISOString()
                }
              }));
            }, stageIndex * 500);
          });
        }, 200);

        // Simulate batch completion
        setTimeout(() => {
          window.dispatchEvent(new CustomEvent('actor-batch-completed', {
            detail: {
              batch_id: `batch-${i + 1}`,
              success: true,
              total_items_processed: Math.min(batchSize, totalPages - i * batchSize),
              batch_duration_ms: 2000 + Math.random() * 1000,
              timestamp: new Date().toISOString()
            }
          }));
        }, 2500);

      }, i * 3000); // Stagger batches
    }

    // Simulate session completion
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent('actor-session-completed', {
        detail: {
          session_id: 'session-' + Date.now(),
          total_pages_processed: totalPages,
          total_batches: batches,
          session_duration_ms: batches * 3000,
          success_rate: 0.95,
          timestamp: new Date().toISOString()
        }
      }));
    }, batches * 3000 + 1000);
  }

  /**
   * Subscribe to Actor system events (for future real implementation)
   */
  async subscribeToActorSystemEvents(callbacks: {
    onSessionStarted?: (data: any) => void;
    onBatchStarted?: (data: any) => void;
    onStageCompleted?: (data: any) => void;
    onBatchCompleted?: (data: any) => void;
    onSessionCompleted?: (data: any) => void;
  }): Promise<() => void> {
    const eventListeners: Array<() => void> = [];

    if (callbacks.onSessionStarted) {
      const handler = (event: any) => callbacks.onSessionStarted!(event.detail);
      window.addEventListener('actor-session-started', handler);
      eventListeners.push(() => window.removeEventListener('actor-session-started', handler));
    }

    if (callbacks.onBatchStarted) {
      const handler = (event: any) => callbacks.onBatchStarted!(event.detail);
      window.addEventListener('actor-batch-started', handler);
      eventListeners.push(() => window.removeEventListener('actor-batch-started', handler));
    }

    if (callbacks.onStageCompleted) {
      const handler = (event: any) => callbacks.onStageCompleted!(event.detail);
      window.addEventListener('actor-stage-completed', handler);
      eventListeners.push(() => window.removeEventListener('actor-stage-completed', handler));
    }

    if (callbacks.onBatchCompleted) {
      const handler = (event: any) => callbacks.onBatchCompleted!(event.detail);
      window.addEventListener('actor-batch-completed', handler);
      eventListeners.push(() => window.removeEventListener('actor-batch-completed', handler));
    }

    if (callbacks.onSessionCompleted) {
      const handler = (event: any) => callbacks.onSessionCompleted!(event.detail);
      window.addEventListener('actor-session-completed', handler);
      eventListeners.push(() => window.removeEventListener('actor-session-completed', handler));
    }

    // Return cleanup function
    return () => {
      eventListeners.forEach(cleanup => cleanup());
    };
  }
}

// Create a singleton instance for use throughout the application
export const tauriApi = new TauriApiService();
