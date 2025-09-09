import { createStore } from 'solid-js/store';
import { tauriApi } from '../services/tauri-api';
import { createSignal } from 'solid-js';

// 단일 로컬 DB 대시보드 상태 Store (Phase 6B)

interface SummaryData {
  total_products: number;
  total_product_details: number;
  total_vendors: number;
  total_device_types: number;
  new_products_24h: number;
  new_products_7d: number;
  top_device_categories: [string, number][];
}

interface AnalyticsState {
  rows: any[];
  total: number;
  offset: number;
  limit: number;
  filterDraft: string;
  filterApplied: string;
  filterError?: string | null;
  loading: boolean;
}

interface UiFlags {
  loadingSummary: boolean;
  summaryError?: string | null;
  exportStatus?: string;
  importStatus?: string;
  importLog?: string;
  deletePreview?: any;
  deleteResult?: any;
  vendorDryRun?: any;
  vendorResult?: any;
  deviceTypesMeta?: any;
  deviceTypesJson: string;
  deviceTypesOriginal?: string; // 초기 로드 버전 (diff 비교용)
  deviceTypesSaveResult?: any;
  reseedAfterSave: boolean;
  working?: boolean; // global action lock (simple)
  deviceTypesDiff?: {
    added: number; updated: number; removed: number;
    samples?: { added: any[]; updated: any[]; removed: any[] };
    parseError?: string;
  } | null;
}

export const DEFAULT_PAGE_SIZE = 50;

const [summary, setSummary] = createSignal<SummaryData | null>(null);
const [ui, setUi] = createStore<UiFlags>({ loadingSummary: false, deviceTypesJson: '[]', reseedAfterSave: true });
const [analytics, setAnalytics] = createStore<AnalyticsState>({ rows: [], total: 0, offset: 0, limit: DEFAULT_PAGE_SIZE, filterDraft: '', filterApplied: '', loading: false });

async function loadSummary() {
  setUi({ ...ui, loadingSummary: true, summaryError: null });
  try {
    const data = await tauriApi.getDbSummary();
    setSummary(data as any);
  } catch (e: any) {
    setUi({ ...ui, summaryError: String(e) });
  } finally {
    setUi({ ...ui, loadingSummary: false });
  }
}

async function loadAnalytics(offset = 0) {
  setAnalytics({ ...analytics, loading: true });
  try {
    const res = await tauriApi.analyticsQuery({ offset, limit: analytics.limit, filter: analytics.filterApplied || undefined });
    setAnalytics({ ...analytics, rows: res.rows || [], total: res.total || 0, offset: res.offset || offset, limit: res.limit || analytics.limit, filterError: res.filter_error || null, loading: false });
  } catch (e: any) {
    setAnalytics({ ...analytics, rows: [], total: 0, filterError: String(e), loading: false });
  }
}

async function applyFilter() {
  setAnalytics({ ...analytics, filterApplied: analytics.filterDraft });
  await loadAnalytics(0);
}

async function resetFilter() {
  setAnalytics({ ...analytics, filterDraft: '', filterApplied: '' });
  await loadAnalytics(0);
}

async function initDeviceTypes() {
  try {
    const res = await tauriApi.getDeviceTypesJson();
  setUi({ ...ui, deviceTypesMeta: res, deviceTypesJson: res.json, deviceTypesOriginal: res.json, deviceTypesDiff: null });
  } catch (e: any) {
    setUi({ ...ui, deviceTypesMeta: { error: String(e) } });
  }
}

async function saveDeviceTypes() {
  try {
    const res = await tauriApi.saveDeviceTypesJson(ui.deviceTypesJson, ui.reseedAfterSave);
    setUi({ ...ui, deviceTypesSaveResult: res });
    await loadSummary();
  } catch (e: any) {
    setUi({ ...ui, deviceTypesSaveResult: { error: String(e) } });
  }
}

// Device Types diff 계산
function computeDeviceTypesDiff(newJson: string) {
  try {
    if (!ui.deviceTypesOriginal) return null;
    const origArr = JSON.parse(ui.deviceTypesOriginal);
    const newArr = JSON.parse(newJson);
  if (!Array.isArray(origArr) || !Array.isArray(newArr)) return { added: 0, updated: 0, removed: 0, parseError: 'Root is not array' };
    const byIdOrig = new Map<number, any>();
    for (const o of origArr) if (o && typeof o.id === 'number') byIdOrig.set(o.id, o);
    const byIdNew = new Map<number, any>();
    for (const n of newArr) if (n && typeof n.id === 'number') byIdNew.set(n.id, n);
    const added: any[] = []; const updated: any[] = []; const removed: any[] = [];
    for (const [id, val] of byIdNew) {
      if (!byIdOrig.has(id)) added.push(val);
      else {
        const o = byIdOrig.get(id);
        if (o.name !== val.name || o.category !== val.category || o.hex !== val.hex || o.introduced_in !== val.introduced_in) {
          updated.push({ id, before: o, after: val });
        }
      }
    }
    for (const [id, val] of byIdOrig) {
      if (!byIdNew.has(id)) removed.push(val);
    }
    return {
      added: added.length,
      updated: updated.length,
      removed: removed.length,
      samples: {
        added: added.slice(0, 5),
        updated: updated.slice(0, 5),
        removed: removed.slice(0, 5)
      }
    };
  } catch (e: any) {
    return { added: 0, updated: 0, removed: 0, parseError: String(e) };
  }
}

function updateDeviceTypesJson(newText: string) {
  const diff = computeDeviceTypesDiff(newText);
  setUi({ ...ui, deviceTypesJson: newText, deviceTypesDiff: diff });
}

// ================= Maintenance Helpers =================
async function exportDataset(dataset: 'vendors' | 'device_types' | 'analytics') {
  try {
    setUi({ ...ui, exportStatus: `Exporting ${dataset}...` });
    const path = await tauriApi.exportDataset(dataset);
    setUi({ ...ui, exportStatus: `완료: ${path}` });
  } catch (e: any) {
    setUi({ ...ui, exportStatus: `실패: ${e}` });
  }
}

async function importDataset(dataset: 'vendors' | 'device_types', csvText: string) {
  try {
    setUi({ ...ui, importStatus: 'Import 중...', importLog: '' });
    const res = await tauriApi.importDataset(dataset, csvText);
    setUi({ ...ui, importStatus: '완료', importLog: JSON.stringify(res, null, 2) });
    await loadSummary();
  } catch (e: any) {
    setUi({ ...ui, importStatus: '실패', importLog: String(e) });
  }
}

async function previewDeleteRange(fromPage: number, toPage: number) {
  try {
    const res = await tauriApi.previewDeleteRange(fromPage, toPage);
    setUi({ ...ui, deletePreview: res });
  } catch (e: any) {
    setUi({ ...ui, deletePreview: { error: String(e) } });
  }
}

async function executeDeleteRange(fromPage: number, toPage: number) {
  if (!ui.deletePreview || ui.deletePreview.error) return;
  try {
    const res = await tauriApi.deleteRange(fromPage, toPage);
    setUi({ ...ui, deleteResult: res });
    await Promise.all([loadSummary(), loadAnalytics(0)]);
  } catch (e: any) {
    setUi({ ...ui, deleteResult: { error: String(e) } });
  }
}

async function vendorDryRun() {
  try {
    const res = await tauriApi.vendorSyncDryRun();
    setUi({ ...ui, vendorDryRun: res });
  } catch (e: any) {
    setUi({ ...ui, vendorDryRun: { error: String(e) } });
  }
}

async function vendorSync() {
  try {
    setUi({ ...ui, vendorResult: { status: 'running' } });
    const res = await tauriApi.dashboardVendorSync({ dry_run: false });
    setUi({ ...ui, vendorResult: res });
    await loadSummary();
  } catch (e: any) {
    setUi({ ...ui, vendorResult: { error: String(e) } });
  }
}

export const localDbDashboardStore = {
  summary,
  ui,
  analytics,
  setUi,
  setAnalytics,
  setSummary,
  loadSummary,
  loadAnalytics,
  applyFilter,
  resetFilter,
  initDeviceTypes,
  saveDeviceTypes,
  updateDeviceTypesJson,
  exportDataset,
  importDataset,
  previewDeleteRange,
  executeDeleteRange,
  vendorDryRun,
  vendorSync,
};

// 초기 로드 헬퍼 (탭 진입 시 호출)
export async function initializeLocalDbDashboard() {
  await Promise.all([loadSummary(), loadAnalytics(0), initDeviceTypes()]);
}

// TODO: Vendor sync / delete range / import-export helper 함수들은 추후 이동 예정