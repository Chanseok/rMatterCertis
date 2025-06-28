// Domain types for the application
// This file contains TypeScript interfaces and types that represent
// the core business entities and concepts

export interface Vendor {
  id: string;
  name: string;
  baseUrl: string;
  crawlingConfig: Record<string, any>;
  isActive: boolean;
  lastCrawledAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}

export interface Product {
  id: string;
  name: string;
  price?: number;
  currency: string;
  description?: string;
  imageUrl?: string;
  productUrl: string;
  vendorId: string;
  category?: string;
  inStock: boolean;
  collectedAt: Date;
  updatedAt: Date;
}

export interface CrawlingSession {
  id: string;
  vendorId: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  totalPages?: number;
  processedPages: number;
  productsFound: number;
  errorsCount: number;
  startedAt: Date;
  completedAt?: Date;
}
