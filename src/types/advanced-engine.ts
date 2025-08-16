// Minimal type stubs for advanced engine to keep type-checking green during cleanup.
// Replace with generated types when ready.
export type CrawlingProgressInfo = any;
export type SiteStatusInfo = any;
export type ProductInfo = any;
export type CrawlingSession = any;
export type DatabaseStats = any;
export type ApiResponse<T = any> = { success?: boolean; message?: string; data?: T } & any;
export type CrawlingRangeRequest = { total_pages_on_site: number; products_on_last_page: number } | any;
export type CrawlingRangeResponse = { success: boolean; range: [number, number]; message?: string } | any;
