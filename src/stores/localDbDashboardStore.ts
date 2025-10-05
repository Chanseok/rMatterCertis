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
  all_device_categories?: [string, number][]; // 전체 카테고리 리스트
  top_vendors?: [string, number][]; // Top 10 벤더
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
  sort: string[]; // e.g. ['device_category:asc','vendor_name:desc']
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
  analyticsDiagnostics?: any;
  // 필터 상태
  filterDraft?: string; // Quick Search 입력 필드
  selectedCategories: string[]; // 선택된 카테고리
  selectedVendors: string[]; // 선택된 벤더
  certDateRange: [string, string]; // [startDate, endDate] YYYY-MM-DD 형식
  certDateMin?: string; // 데이터의 최소 날짜 (YYYY-MM-DD)
  certDateMax?: string; // 데이터의 최대 날짜 (YYYY-MM-DD)
}

export const DEFAULT_PAGE_SIZE = 50;

const [summary, setSummary] = createSignal<SummaryData | null>(null);
const [ui, setUi] = createStore<UiFlags>({ 
  loadingSummary: false, 
  deviceTypesJson: '[]', 
  reseedAfterSave: true,
  selectedCategories: [],
  selectedVendors: [],
  certDateRange: ["", ""] // 초기에는 비우고 loadSummary에서 설정
});
const [analytics, setAnalytics] = createStore<AnalyticsState>({ rows: [], total: 0, offset: 0, limit: DEFAULT_PAGE_SIZE, filterDraft: '', filterApplied: '', loading: false, sort: [] });

async function loadSummary() {
  setUi({ ...ui, loadingSummary: true, summaryError: null });
  try {
    const data = await tauriApi.getDbSummary();
    setSummary(data as any);
    
    // Cert Date 범위 초기화: 실제 데이터에서 최소/최대값 조회
    if (!ui.certDateMin) {
      try {
        // 최소 날짜 조회
        const minRes = await tauriApi.analyticsQuery({ 
          offset: 0, 
          limit: 1, 
          sort: ['certification_date:asc'] 
        });
        
        // 최대 날짜 조회
        const maxRes = await tauriApi.analyticsQuery({ 
          offset: 0, 
          limit: 1, 
          sort: ['certification_date:desc'] 
        });
        
        console.log('[loadSummary] 날짜 범위 조회 결과:', {
          minRes: minRes.rows?.[0],
          maxRes: maxRes.rows?.[0]
        });
        
        if (minRes.rows?.[0]?.certification_date && maxRes.rows?.[0]?.certification_date) {
          const minDate = minRes.rows[0].certification_date;
          const maxDate = maxRes.rows[0].certification_date;
          
          console.log('[loadSummary] 설정할 날짜 범위:', {
            minDate,
            maxDate
          });
          
          // certDateMin/Max와 certDateRange를 모두 업데이트
          setUi({ 
            ...ui, 
            certDateMin: minDate, 
            certDateMax: maxDate,
            certDateRange: [minDate, maxDate] // 실제 데이터 범위로 초기화
          });
        } else {
          // 데이터가 없으면 기본값 사용
          const today = new Date().toISOString().split('T')[0];
          console.log('[loadSummary] 데이터 없음, 기본값 사용');
          setUi({ 
            ...ui, 
            certDateMin: "2020-01-01", 
            certDateMax: today,
            certDateRange: ["2020-01-01", today]
          });
        }
      } catch (e) {
        // 에러 시 기본값 사용
        const today = new Date().toISOString().split('T')[0];
        console.error('[loadSummary] 날짜 범위 조회 실패:', e);
        setUi({ 
          ...ui, 
          certDateMin: "2020-01-01", 
          certDateMax: today,
          certDateRange: ["2020-01-01", today]
        });
      }
    }
  } catch (e: any) {
    setUi({ ...ui, summaryError: String(e) });
  } finally {
    setUi({ ...ui, loadingSummary: false });
  }
}

async function loadAnalytics(offset = 0) {
  setAnalytics({ ...analytics, loading: true });
  try {
  const res = await tauriApi.analyticsQuery({ offset, limit: analytics.limit, filter: analytics.filterApplied || undefined, sort: analytics.sort.length ? analytics.sort : undefined });
    setAnalytics({ ...analytics, rows: res.rows || [], total: res.total || 0, offset: res.offset || offset, limit: res.limit || analytics.limit, filterError: res.filter_error || null, loading: false });
  } catch (e: any) {
    setAnalytics({ ...analytics, rows: [], total: 0, filterError: String(e), loading: false });
  }
}

async function applyFilter(filter?: string) {
  const filterToApply = filter !== undefined ? filter : analytics.filterDraft;
  setAnalytics({ ...analytics, filterApplied: filterToApply });
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

async function exportCurrentView() {
  try {
    setUi({ ...ui, exportStatus: '현재 뷰 내보내는 중...' });
    // 현재 analytics 상태(필터, 정렬 포함)로 전체 데이터 조회 후 Export
    const res = await tauriApi.analyticsQuery({ 
      offset: 0, 
      limit: analytics.total || 10000, // 전체 데이터 가져오기
      filter: analytics.filterApplied || undefined, 
      sort: analytics.sort.length ? analytics.sort : undefined 
    });
    
    // CSV 형식으로 변환
    const rows = res.rows || [];
    if (rows.length === 0) {
      setUi({ ...ui, exportStatus: '내보낼 데이터가 없습니다.' });
      return;
    }
    
    const headers = ['Category', 'Device Type', 'Model', 'Vendor', 'Cert Date', 'Transport IF', 'Created', 'URL'];
    const csvContent = [
      headers.join(','),
      ...rows.map((r: any) => [
        `"${(r.device_category || '').replace(/"/g, '""')}"`,
        `"${(r.device_type_name || '').replace(/"/g, '""')}"`,
        `"${(r.model || '').replace(/"/g, '""')}"`,
        `"${(r.vendor_name || '').replace(/"/g, '""')}"`,
        `"${r.certification_date || ''}"`,
        `"${(r.transport_interface || r.transport_if || '').replace(/"/g, '""')}"`,
        `"${r.detail_created_at || ''}"`,
        `"${(r.product_detail_url || '').replace(/"/g, '""')}"`
      ].join(','))
    ].join('\n');
    
    // 파일 다운로드
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `analytics_filtered_${new Date().toISOString().slice(0, 10)}.csv`;
    link.click();
    URL.revokeObjectURL(url);
    
    setUi({ ...ui, exportStatus: `완료: ${rows.length}행 내보냄` });
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
  exportCurrentView,
  importDataset,
  previewDeleteRange,
  executeDeleteRange,
  vendorDryRun,
  vendorSync,
  // Sorting helpers
  updateSort(order: string[]) { setAnalytics({ ...analytics, sort: order }); },
};

// 초기 로드 헬퍼 (탭 진입 시 호출)
export async function initializeLocalDbDashboard() {
  await Promise.all([loadSummary(), loadAnalytics(0), initDeviceTypes()]);
}

// TODO: Vendor sync / delete range / import-export helper 함수들은 추후 이동 예정