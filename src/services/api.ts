// API service functions
// This file contains higher-level API functions that use Tauri commands

import * as tauri from './tauri';

export class ApiService {
  static async greet(name: string): Promise<string> {
    return tauri.greet(name);
  }

  // Add more API methods here as backend commands are implemented
  // static async getVendors(): Promise<Vendor[]> { ... }
  // static async createVendor(vendor: CreateVendorRequest): Promise<Vendor> { ... }
  // static async startCrawling(request: StartCrawlingRequest): Promise<CrawlingSession> { ... }
}
