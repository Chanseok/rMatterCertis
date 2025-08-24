import { invoke } from "@tauri-apps/api/core";

export interface SyncSummary {
  pages_processed: number;
  inserted: number;
  updated: number;
  skipped: number;
  failed: number;
  duration_ms: number;
}

// Parse a range expression like "498-492,489,487-485" into explicit pages (desc)
export function expandRangeExpr(expr: string): number[] {
  const parts = expr
    .split(/[,\s]+/)
    .map((p) => p.trim())
    .filter(Boolean);
  const pages: number[] = [];
  for (const part of parts) {
    const m = part.match(/^(\d+)(?:-(\d+))?$/);
    if (!m) continue;
    const a = parseInt(m[1], 10);
    const b = m[2] ? parseInt(m[2], 10) : a;
    const start = Math.max(a, b);
    const end = Math.min(a, b);
    for (let p = start; p >= end; p--) pages.push(p);
  }
  // normalize: unique + desc
  const uniq = Array.from(new Set(pages)).sort((x, y) => y - x);
  return uniq;
}

export async function startPartialSync(ranges: string, dryRun?: boolean): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_partial_sync", { ranges, dryRun });
}

export async function startBatchedSync(
  ranges: string,
  batchSizeOverride?: number,
  dryRun?: boolean
): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_batched_sync", {
    ranges,
    _batchSizeOverride: batchSizeOverride,
    dryRun,
  });
}

export async function startRepairSync(buffer?: number, dryRun?: boolean): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_repair_sync", { buffer, dryRun });
}

export async function startSyncPages(pages: number[], dryRun?: boolean): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_sync_pages", { pages, dryRun });
}

// New: basic engine path with explicit page list (avoids partial sync path)
export async function startBasicSyncPages(
  pages: number[],
  dryRun?: boolean
): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_basic_sync_pages", { pages, dryRun });
}

export interface DiagnosticPageInput {
  physical_page: number;
  miss_indices: number[];
}

export async function startDiagnosticSync(
  pages: DiagnosticPageInput[],
  dryRun?: boolean
): Promise<SyncSummary> {
  return await invoke<SyncSummary>("start_diagnostic_sync", { pages, dryRun });
}

export async function retryFailedDetails(limit?: number): Promise<{ retried: number }>{
  return await invoke<{ retried: number }>("retry_failed_details", { limit });
}
