// API types for communication with the backend
// This file contains types for API requests and responses

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

// Request types
export interface CreateVendorRequest {
  name: string;
  baseUrl: string;
  crawlingConfig: Record<string, any>;
}

export interface UpdateVendorRequest extends Partial<CreateVendorRequest> {
  isActive?: boolean;
}

export interface StartCrawlingRequest {
  vendorId: string;
  options?: {
    maxPages?: number;
    delayMs?: number;
  };
}
