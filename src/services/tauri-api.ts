/**
 * Modern Tauri API Service for Real-time Crawling Operations
 * 
 * This service provides a clean interface for communicating with the Rust backend
 * using the new real-time event system. It encapsulates all Tauri invoke calls
 * and event listening logic.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  BackendCrawlerConfig,
  CrawlingProgress,
  CrawlingResult,
  CrawlingTaskStatus,
  DatabaseStats
} from '../types/crawling';

/**
 * Service class for communicating with the Rust backend
 */
export class TauriApiService {
  private eventListeners: Map<string, UnlistenFn> = new Map();

  // =========================================================================
  // Crawling Control Commands
  // =========================================================================

  /**
   * Start a new crawling session with the provided comprehensive configuration
   */
  async startCrawling(config: BackendCrawlerConfig): Promise<string> {
    try {
      const sessionId = await invoke<string>('start_crawling', { config });
      return sessionId;
    } catch (error) {
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
      callback(event.payload);
    });
    
    this.eventListeners.set('crawling-progress', unlisten);
    return unlisten;
  }

  /**
   * Subscribe to individual task status updates
   */
  async subscribeToTaskStatus(callback: (status: CrawlingTaskStatus) => void): Promise<UnlistenFn> {
    const unlisten = await listen<CrawlingTaskStatus>('crawling-task-update', (event) => {
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

  // =========================================================================
  // Event Management
  // =========================================================================

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
   * Get frontend configuration (complete config)
   */
  async getFrontendConfig(): Promise<any> {
    try {
      return await invoke<any>('get_frontend_config');
    } catch (error) {
      throw new Error(`Failed to get frontend config: ${error}`);
    }
  }

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
}

// Create a singleton instance for use throughout the application
export const tauriApi = new TauriApiService();
