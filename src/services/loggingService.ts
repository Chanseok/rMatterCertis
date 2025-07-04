/**
 * Frontend logging service that writes to backend log files
 * Provides unified logging interface for frontend components
 */

import { invoke } from '@tauri-apps/api/core';

export interface LogEntry {
  level: string;
  message: string;
  timestamp: string;
  component?: string;
}

export type LogLevel = 'error' | 'warn' | 'info' | 'debug';

class LoggingService {
  private component: string = 'Frontend';

  /**
   * Set the component name for all subsequent logs
   */
  setComponent(component: string) {
    this.component = component;
  }

  /**
   * Write a log entry to the backend log file (unified or separated based on config)
   */
  private async writeLog(level: LogLevel, message: string, component?: string): Promise<void> {
    try {
      const entry: LogEntry = {
        level,
        message,
        timestamp: new Date().toISOString(),
        component: component || this.component,
      };

      await invoke('write_frontend_log', { entry });
      
      // Also log to browser console for development
      const consoleMethod = level === 'error' ? 'error' 
                          : level === 'warn' ? 'warn'
                          : level === 'debug' ? 'debug' 
                          : 'log';
      console[consoleMethod](`[${component || this.component}] ${message}`);
    } catch (error) {
      // Fallback to console only if backend logging fails
      console.error('Failed to write to backend log:', error);
      console[level === 'error' ? 'error' : level === 'warn' ? 'warn' : 'log'](
        `[${component || this.component}] ${message}`
      );
    }
  }

  /**
   * Clean up old log files, keeping only the latest
   */
  async cleanupLogs(): Promise<string> {
    try {
      const result = await invoke('cleanup_logs');
      console.log('Log cleanup result:', result);
      return result as string;
    } catch (error) {
      const errorMessage = `Failed to cleanup logs: ${error}`;
      console.error(errorMessage);
      throw new Error(errorMessage);
    }
  }

  /**
   * Log an error message
   */
  async error(message: string, component?: string): Promise<void> {
    return this.writeLog('error', message, component);
  }

  /**
   * Log a warning message
   */
  async warn(message: string, component?: string): Promise<void> {
    return this.writeLog('warn', message, component);
  }

  /**
   * Log an info message
   */
  async info(message: string, component?: string): Promise<void> {
    return this.writeLog('info', message, component);
  }

  /**
   * Log a debug message
   */
  async debug(message: string, component?: string): Promise<void> {
    return this.writeLog('debug', message, component);
  }

  /**
   * Get the log directory path from backend
   */
  async getLogDirectory(): Promise<string> {
    try {
      return await invoke('get_log_directory_path');
    } catch (error) {
      console.error('Failed to get log directory:', error);
      return 'Unknown';
    }
  }

  /**
   * Log user actions for audit trail
   */
  async logUserAction(action: string, details?: Record<string, any>): Promise<void> {
    const message = details 
      ? `User Action: ${action} - ${JSON.stringify(details)}`
      : `User Action: ${action}`;
    return this.info(message, 'UserAction');
  }

  /**
   * Log API calls and responses
   */
  async logApiCall(method: string, endpoint: string, duration?: number, error?: Error): Promise<void> {
    const durationStr = duration ? ` (${duration}ms)` : '';
    if (error) {
      return this.error(`API ${method} ${endpoint} failed${durationStr}: ${error.message}`, 'API');
    } else {
      return this.info(`API ${method} ${endpoint} completed${durationStr}`, 'API');
    }
  }

  /**
   * Log component lifecycle events
   */
  async logComponentEvent(component: string, event: string, details?: string): Promise<void> {
    const message = details ? `${event}: ${details}` : event;
    return this.debug(message, component);
  }
}

// Export singleton instance
export const loggingService = new LoggingService();

// Export for manual instantiation if needed
export { LoggingService };
