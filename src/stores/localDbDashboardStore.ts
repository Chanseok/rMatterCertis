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
  top_device_types?: [string, number][]; // Top 10 디바이스 타입
  all_device_types?: [string, number][]; // 전체 디바이스 타입 리스트 (count와 함께)
  top_vendors?: [string, number][]; // Top 10 벤더
  all_vendors?: [string, number][]; // 전체 벤더 리스트
  all_device_type_names?: string[]; // 전체 디바이스 타입 이름 리스트 (Matter 표준 정의용)
  top_transport_interfaces?: [string, number][]; // Top transport interfaces
  all_transport_interfaces?: string[]; // 전체 transport interface 리스트
  device_types_by_category?: Record<string, string[]>; // 카테고리별 디바이스 타입 매핑
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
  selectedDeviceTypes: string[]; // 선택된 디바이스 타입
  selectedTransportInterfaces: string[]; // 선택된 transport interface
  certDateRange: [string, string]; // [startDate, endDate] YYYY-MM-DD 형식
  certDateMin?: string; // 데이터의 최소 날짜 (YYYY-MM-DD)
  certDateMax?: string; // 데이터의 최대 날짜 (YYYY-MM-DD)
  filtersExpanded: boolean; // 필터 섹션 접기/펼치기 상태
}

export const DEFAULT_PAGE_SIZE = 50;

const [summary, setSummary] = createSignal<SummaryData | null>(null);
const [ui, setUi] = createStore<UiFlags>({ 
  loadingSummary: false, 
  deviceTypesJson: '[]', 
  reseedAfterSave: true,
  selectedCategories: [],
  selectedVendors: [],
  selectedDeviceTypes: [],
  selectedTransportInterfaces: [],
  certDateRange: ["", ""], // 초기에는 비우고 loadSummary에서 설정
  filtersExpanded: true, // 기본적으로 펼쳐진 상태
});
const [analytics, setAnalytics] = createStore<AnalyticsState>({ rows: [], total: 0, offset: 0, limit: DEFAULT_PAGE_SIZE, filterDraft: '', filterApplied: '', loading: false, sort: [] });

async function loadSummary() {
  console.log('[loadSummary] 시작...');
  setUi({ ...ui, loadingSummary: true, summaryError: null });
  try {
    const data = await tauriApi.getDbSummary();
    console.log('[loadSummary] getDbSummary 응답:', data);
    setSummary(data as any);
    console.log('[loadSummary] summary 설정 완료');
    
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
    console.log('[loadSummary] 완료, summary:', summary());
  } catch (e: any) {
    console.error('[loadSummary] 에러:', e);
    setUi({ ...ui, summaryError: String(e) });
  } finally {
    console.log('[loadSummary] finally, loadingSummary를 false로 설정');
    setUi({ ...ui, loadingSummary: false });
  }
}

async function loadAnalytics(offset = 0) {
  console.log('[loadAnalytics] 시작:', { offset, limit: analytics.limit, filter: analytics.filterApplied });
  setAnalytics({ ...analytics, loading: true });
  try {
  const res = await tauriApi.analyticsQuery({ offset, limit: analytics.limit, filter: analytics.filterApplied || undefined, sort: analytics.sort.length ? analytics.sort : undefined });
    console.log('[loadAnalytics] 응답 받음:', { 
      rowsCount: res.rows?.length || 0, 
      total: res.total, 
      offset: res.offset,
      limit: res.limit,
      filterError: res.filter_error,
      firstRow: res.rows?.[0]
    });
    setAnalytics({ ...analytics, rows: res.rows || [], total: res.total || 0, offset: res.offset || offset, limit: res.limit || analytics.limit, filterError: res.filter_error || null, loading: false });
    console.log('[loadAnalytics] analytics.rows 업데이트 완료:', { rowsCount: analytics.rows.length });
  } catch (e: any) {
    console.error('[loadAnalytics] 에러:', e);
    setAnalytics({ ...analytics, rows: [], total: 0, filterError: String(e), loading: false });
  }
}

async function applyFilter(filter?: string) {
  const filterToApply = filter !== undefined ? filter : analytics.filterDraft;
  console.log('[applyFilter] 필터 적용:', { filterToApply, currentFilter: analytics.filterApplied });
  setAnalytics({ ...analytics, filterApplied: filterToApply });
  console.log('[applyFilter] analytics.filterApplied 업데이트 완료, loadAnalytics 호출');
  await loadAnalytics(0);
  console.log('[applyFilter] loadAnalytics 완료');
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

async function exportFullDatabaseExcel() {
  try {
    setUi({ ...ui, exportStatus: 'Excel 파일로 전체 DB 내보내는 중...', working: true });
    const result = await tauriApi.exportFullDatabaseExcel();
    setUi({ 
      ...ui, 
      exportStatus: `완료: ${result.file_path}\n제품: ${result.products_count}개, 상세정보: ${result.product_details_count}개`,
      working: false 
    });
  } catch (e: any) {
    setUi({ ...ui, exportStatus: `실패: ${e}`, working: false });
  }
}

async function importFullDatabaseExcel(filePath: string) {
  try {
    setUi({ ...ui, importStatus: 'Excel 파일에서 전체 DB 가져오는 중...', importLog: '', working: true });
    const result = await tauriApi.importFullDatabaseExcel(filePath);
    const summary = `완료!\n` +
      `제품: ${result.products_imported}개 추가, ${result.products_updated}개 업데이트\n` +
      `상세정보: ${result.details_imported}개 추가, ${result.details_updated}개 업데이트\n` +
      `Device Types: ${result.device_types_imported}개 추가, ${result.device_types_updated}개 업데이트\n` +
      `Vendors: ${result.vendors_imported}개 추가, ${result.vendors_updated}개 업데이트`;
    const errors = result.errors.length > 0 ? `\n\n오류 (${result.errors.length}개):\n${result.errors.slice(0, 10).join('\n')}` : '';
    const backup = result.backup_file ? `\n\n백업 파일: ${result.backup_file}` : '';
    setUi({ 
      ...ui, 
      importStatus: '완료', 
      importLog: summary + errors + backup,
      working: false 
    });
    await loadSummary();
    await loadAnalytics(0);
  } catch (e: any) {
    setUi({ ...ui, importStatus: '실패', importLog: String(e), working: false });
  }
}

async function deleteAllRecordsConfirmed() {
  try {
    // Use Tauri's native dialog for better control
    const { ask } = await import('@tauri-apps/plugin-dialog');
    
    const confirmed = await ask(
      '이 작업은 되돌릴 수 없으며, 자동으로 백업 파일이 생성됩니다.\n\n정말로 모든 데이터를 삭제하시겠습니까?',
      {
        title: '⚠️ 경고: 모든 제품 데이터 삭제',
        kind: 'warning',
        okLabel: '삭제',
        cancelLabel: '취소'
      }
    );
    
    if (!confirmed) {
      setUi({ ...ui, deleteResult: { cancelled: true } });
      return;
    }
    
    // Only after user confirms, start the deletion process
    setUi({ ...ui, deleteResult: { status: 'deleting...' }, working: true });
    const result = await tauriApi.deleteAllRecords('DELETE_ALL_CONFIRMED');
    setUi({ 
      ...ui, 
      deleteResult: {
        ...result,
        message: `삭제 완료!\n제품: ${result.deleted_products}개\n상세정보: ${result.deleted_product_details}개\n백업: ${result.backup_file || 'N/A'}`
      },
      working: false 
    });
    await loadSummary();
    await loadAnalytics(0);
  } catch (e: any) {
    setUi({ ...ui, deleteResult: { error: String(e) }, working: false });
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
  console.log('🔍 previewDeleteRange called:', { fromPage, toPage });
  try {
    const res = await tauriApi.previewDeleteRange(fromPage, toPage);
    console.log('✅ previewDeleteRange result:', res);
    setUi({ ...ui, deletePreview: res });
  } catch (e: any) {
    console.error('❌ previewDeleteRange error:', e);
    console.error('❌ Error details:', JSON.stringify(e, null, 2));
    setUi({ ...ui, deletePreview: { error: String(e) } });
    alert(`미리보기 실패: ${e}`);
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
  exportFullDatabaseExcel,
  importDataset,
  importFullDatabaseExcel,
  previewDeleteRange,
  executeDeleteRange,
  deleteAllRecordsConfirmed,
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