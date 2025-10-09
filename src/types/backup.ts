/**
 * Backup and restore types for database operations
 * 
 * These types represent the structure returned by backup, restore, import, and export operations.
 * Used in tauri-api.ts and LocalDB dashboard components.
 */

/**
 * Result of dataset import operation (vendors, device_types, analytics)
 */
export interface ImportResult {
  dataset: string;
  processed: number;
  inserted: number;
  updated: number;
  errors: string[];
  backup_file?: string | null;
}

/**
 * Result of full database Excel export operation
 */
export interface ExportFullDatabaseResult {
  file_path: string;
  products_count: number;
  product_details_count: number;
  device_types_count: number;
  vendors_count: number;
}

/**
 * Result of full database Excel import/restore operation
 */
export interface ImportFullDatabaseResult {
  products_imported: number;
  products_updated: number;
  details_imported: number;
  details_updated: number;
  device_types_imported: number;
  device_types_updated: number;
  vendors_imported: number;
  vendors_updated: number;
  errors: string[];
  backup_file?: string | null;
}

/**
 * Result of delete all records operation
 */
export interface DeleteAllRecordsResult {
  deleted_products: number;
  deleted_product_details: number;
  backup_file?: string | null;
}

/**
 * Result of page range deletion operation
 */
export interface DeleteRangeResult {
  from_page: number;
  to_page: number;
  deleted_product_details: number;
  deleted_products: number;
}

/**
 * Preview result for page range deletion (no actual deletion)
 */
export interface PreviewDeleteRangeResult {
  from_page: number;
  to_page: number;
  product_details_count: number;
  products_count: number;
}
