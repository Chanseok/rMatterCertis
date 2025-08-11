// Tauri service functions
// This file contains functions for communicating with the Tauri backend

import { invoke } from '@tauri-apps/api/core';

export async function greet(name: string): Promise<string> {
  return await invoke('greet', { name });
}

// Add more Tauri commands here as they are implemented

export interface DataConsistencyReport {
  total_checked: number;
  valid: number;
  invalid: number;
  skipped_null_page_id: number;
  total_pages_site: number;
  products_on_last_page_site: number;
  sample_inconsistencies: Array<{
    product_id: number;
    url: string;
    stored_page_id: number;
    stored_index_in_page: number;
    recomputed_page_id: number;
    recomputed_index_in_page: number;
    physical_page: number;
    physical_index: number;
    reason: string;
  }>;
  max_samples: number;
}

export async function checkPageIndexConsistency(): Promise<DataConsistencyReport> {
  const json = await invoke<string>('check_page_index_consistency');
  return JSON.parse(json) as DataConsistencyReport;
}
