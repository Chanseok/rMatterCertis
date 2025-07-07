// Platform API abstraction layer for Tauri commands
// This module provides type-safe wrappers around Tauri's invoke function

import { invoke } from '@tauri-apps/api/core';
import type {
  // Vendor DTOs
  CreateVendorDto, UpdateVendorDto, VendorResponseDto,
  // Product DTOs  
  CreateProductDto, CreateMatterProductDto, ProductResponseDto, MatterProductResponseDto,
  // Search DTOs
  MatterProductFilterDto, ProductSearchResultDto,
  // Session DTOs
  StartCrawlingDto, SessionStatusDto,
  // Utility Types
  ApiError
} from '../types/domain';

// ============================================================================
// Command Parameter Mapping
// ============================================================================

export interface MethodParamsMapping {
  // Database commands
  test_database_connection: void;
  get_database_info: void;
  
  // Vendor commands
  create_vendor: CreateVendorDto;
  get_all_vendors: void;
  get_vendor_by_id: { vendor_id: string };
  search_vendors_by_name: { name: string };
  update_vendor: { vendor_id: string; dto: UpdateVendorDto };
  delete_vendor: { vendor_id: string };
  
  // Product commands
  create_product: CreateProductDto;
  create_matter_product: CreateMatterProductDto;
  search_matter_products: MatterProductFilterDto;
  delete_product: { product_url: string };
  
  // Crawling commands
  start_crawling: StartCrawlingDto;
  get_crawling_status: { session_id?: string };
  stop_crawling: { session_id: string };
  pause_crawling: { session_id: string };
  resume_crawling: { session_id: string };
  get_crawling_stats: { session_id?: string };
  get_active_crawling_sessions: void;
  get_crawling_session_history: { limit?: number };
  get_enhanced_crawling_stats: { session_id?: string };
  get_retry_stats: void; // 재시도 통계 - INTEGRATED_PHASE2_PLAN Week 1 Day 5
}

// ============================================================================
// Command Return Type Mapping
// ============================================================================

export interface MethodReturnMapping {
  // Database commands
  test_database_connection: string;
  get_database_info: string;
  
  // Vendor commands
  create_vendor: VendorResponseDto;
  get_all_vendors: VendorResponseDto[];
  get_vendor_by_id: VendorResponseDto;
  search_vendors_by_name: VendorResponseDto[];
  update_vendor: VendorResponseDto;
  delete_vendor: string;
  
  // Product commands
  create_product: ProductResponseDto;
  create_matter_product: MatterProductResponseDto;
  search_matter_products: ProductSearchResultDto;
  delete_product: string;
  
  // Crawling commands
  start_crawling: SessionStatusDto;
  get_crawling_status: SessionStatusDto;
  stop_crawling: SessionStatusDto;
  pause_crawling: SessionStatusDto;
  resume_crawling: SessionStatusDto;
  get_crawling_stats: CrawlingStats;
  get_active_crawling_sessions: SessionStatusDto[];
  get_crawling_session_history: SessionStatusDto[];
  get_enhanced_crawling_stats: EnhancedCrawlingStats;
  get_retry_stats: RetryStats; // 재시도 통계 - INTEGRATED_PHASE2_PLAN Week 1 Day 5
}

// ============================================================================
// Crawling Statistics Types
// ============================================================================

interface ErrorInfo {
  id: string;
  message: string;
  timestamp: string;
  recoverable: boolean;
  count: number;
}

interface RetryStats {
  total_items: number;
  pending_retries: number;
  successful_retries: number;
  failed_retries: number;
  max_retries: number;
  status?: string;
}

interface CrawlingStats {
  totalItems: number;
  processedItems: number;
  successRate: number;
  errors: ErrorInfo[];
  startTime: string;
  elapsedTime: number;
  estimatedTimeRemaining: number;
}

interface EnhancedCrawlingStats extends CrawlingStats {
  performanceMetrics: {
    averageRequestTime: number;
    requestsPerSecond: number;
    memoryUsage: number;
    cpuUsage: number;
  };
  detailedProgress: {
    stageBreakdown: Record<string, number>;
    categoryStats: Record<string, number>;
    vendorStats: Record<string, number>;
  };
  qualityMetrics: {
    dataCompleteness: number;
    validationErrors: number;
    duplicateCount: number;
  };
}

// ============================================================================
// Tauri API Adapter Class
// ============================================================================

export class TauriApiAdapter {
  /**
   * Generic method to invoke Tauri commands with type safety
   */
  async invoke<K extends keyof MethodParamsMapping>(
    command: K,
    params?: MethodParamsMapping[K]
  ): Promise<MethodReturnMapping[K]> {
    try {
      console.log(`🔧 Invoking Tauri command: ${command}`, params);
      
      // Handle void parameters
      const invokeParams = params === undefined ? {} : params;
      const result = await invoke(command as string, invokeParams);
      
      console.log(`✅ Command ${command} succeeded:`, result);
      return result as MethodReturnMapping[K];
    } catch (error) {
      console.error(`❌ Command ${command} failed:`, error);
      throw this.normalizeError(error);
    }
  }

  /**
   * Normalize different error formats into a consistent ApiError structure
   */
  private normalizeError(error: unknown): ApiError {
    if (typeof error === 'string') {
      return { message: error };
    }
    
    if (error instanceof Error) {
      return { message: error.message };
    }
    
    if (error && typeof error === 'object' && 'message' in error) {
      return { 
        message: (error as any).message,
        code: (error as any).code,
        details: error as any
      };
    }
    
    return { message: 'Unknown error occurred' };
  }

  // ========================================================================
  // Database Methods
  // ========================================================================

  async testDatabaseConnection(): Promise<string> {
    return this.invoke('test_database_connection');
  }

  async getDatabaseInfo(): Promise<string> {
    return this.invoke('get_database_info');
  }

  // ========================================================================
  // Vendor Methods
  // ========================================================================

  async createVendor(dto: CreateVendorDto): Promise<VendorResponseDto> {
    return this.invoke('create_vendor', dto);
  }

  async getAllVendors(): Promise<VendorResponseDto[]> {
    return this.invoke('get_all_vendors');
  }

  async getVendorById(vendorId: string): Promise<VendorResponseDto> {
    return this.invoke('get_vendor_by_id', { vendor_id: vendorId });
  }

  async searchVendorsByName(name: string): Promise<VendorResponseDto[]> {
    return this.invoke('search_vendors_by_name', { name });
  }

  async updateVendor(vendorId: string, dto: UpdateVendorDto): Promise<VendorResponseDto> {
    return this.invoke('update_vendor', { vendor_id: vendorId, dto });
  }

  async deleteVendor(vendorId: string): Promise<string> {
    return this.invoke('delete_vendor', { vendor_id: vendorId });
  }

  // ========================================================================
  // Product Methods
  // ========================================================================

  async createProduct(dto: CreateProductDto): Promise<ProductResponseDto> {
    return this.invoke('create_product', dto);
  }

  async createMatterProduct(dto: CreateMatterProductDto): Promise<MatterProductResponseDto> {
    return this.invoke('create_matter_product', dto);
  }

  async searchMatterProducts(filter: MatterProductFilterDto): Promise<ProductSearchResultDto> {
    return this.invoke('search_matter_products', filter);
  }

  async deleteProduct(productUrl: string): Promise<string> {
    return this.invoke('delete_product', { product_url: productUrl });
  }

  // ========================================================================
  // Crawling Methods
  // ========================================================================

  async startCrawling(dto: StartCrawlingDto): Promise<SessionStatusDto> {
    return this.invoke('start_crawling', dto);
  }

  async getCrawlingStatus(sessionId?: string): Promise<SessionStatusDto> {
    return this.invoke('get_crawling_status', sessionId ? { session_id: sessionId } : undefined);
  }

  async stopCrawling(sessionId: string): Promise<SessionStatusDto> {
    return this.invoke('stop_crawling', { session_id: sessionId });
  }

  async pauseCrawling(sessionId: string): Promise<SessionStatusDto> {
    return this.invoke('pause_crawling', { session_id: sessionId });
  }

  async resumeCrawling(sessionId: string): Promise<SessionStatusDto> {
    return this.invoke('resume_crawling', { session_id: sessionId });
  }

  async getCrawlingStats(sessionId?: string): Promise<any> {
    return this.invoke('get_crawling_stats', sessionId ? { session_id: sessionId } : undefined);
  }

  async getActiveCrawlingSessions(): Promise<SessionStatusDto[]> {
    return this.invoke('get_active_crawling_sessions');
  }

  async getCrawlingSessionHistory(limit?: number): Promise<SessionStatusDto[]> {
    return this.invoke('get_crawling_session_history', limit ? { limit } : undefined);
  }

  async getEnhancedCrawlingStats(sessionId?: string): Promise<any> {
    return this.invoke('get_enhanced_crawling_stats', sessionId ? { session_id: sessionId } : undefined);
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

export const apiAdapter = new TauriApiAdapter();

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Wrapper for common API operations with error handling
 */
export async function safeApiCall<T>(
  operation: () => Promise<T>,
  fallback?: T
): Promise<{ data?: T; error?: ApiError }> {
  try {
    const data = await operation();
    return { data };
  } catch (error) {
    const apiError = error as ApiError;
    console.error('API call failed:', apiError);
    return { error: apiError, data: fallback };
  }
}

/**
 * Batch API calls with error isolation
 */
export async function batchApiCalls<T extends Record<string, () => Promise<any>>>(
  operations: T
): Promise<{ [K in keyof T]: { data?: Awaited<ReturnType<T[K]>>; error?: ApiError } }> {
  const results = {} as any;
  
  await Promise.all(
    Object.entries(operations).map(async ([key, operation]) => {
      results[key] = await safeApiCall(operation);
    })
  );
  
  return results;
}

// ============================================================================
// Type Guards
// ============================================================================

export function isApiError(value: unknown): value is ApiError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'message' in value &&
    typeof (value as any).message === 'string'
  );
}

export function hasApiError<T>(result: { data?: T; error?: ApiError }): result is { error: ApiError } {
  return !!result.error;
}

export function hasApiData<T>(result: { data?: T; error?: ApiError }): result is { data: T } {
  return !!result.data && !result.error;
}
