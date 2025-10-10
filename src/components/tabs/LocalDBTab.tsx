/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, createMemo, For, onMount, Show, createEffect, onCleanup } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { localDbDashboardStore } from '../../stores/localDbDashboardStore';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { VendorSyncResult } from '../../types/domain';
import { DateRangeSlider } from '../DateRangeSlider';
import { CertificationTimeline } from '../charts/CertificationTimeline.tsx';
import { DeviceTypeTableEditor } from '../DeviceTypeTableEditor';

// TypeScript types for filter-aware APIs
interface AvailableFilterOptions {
  categories: string[];
  device_types: string[];
  vendors: string[];
  transport_interfaces: string[];
}

export const LocalDBTab: Component = () => {
  // Phase 6B: 통합된 Summary / Analytics / Maintenance UI
  const s = localDbDashboardStore.summary;
  const analytics = localDbDashboardStore.analytics;
  const ui = localDbDashboardStore.ui;

  // 제품 상세 모달 상태
  const [selectedProduct, setSelectedProduct] = createSignal<any | null>(null);
  const [showProductModal, setShowProductModal] = createSignal(false);
  
  // 필터 다이얼로그 상태
  const [showCategoryDialog, setShowCategoryDialog] = createSignal(false);
  const [showVendorDialog, setShowVendorDialog] = createSignal(false);
  const [showDeviceTypeDialog, setShowDeviceTypeDialog] = createSignal(false);
  const [showTransportInterfaceDialog, setShowTransportInterfaceDialog] = createSignal(false);
  
  // 필터링된 인사이트 데이터
  const [useFilteredInsights, setUseFilteredInsights] = createSignal(true); // 기본값을 true로 변경
  const [filteredInsights, setFilteredInsights] = createSignal<any>(null);
  const [loadingFilteredInsights, setLoadingFilteredInsights] = createSignal(false);
  
  // 디버깅: 필터 변경부터 데이터 로드까지 걸린 시간 측정
  const [filterLoadTime, setFilterLoadTime] = createSignal<number | null>(null);
  const [isTimerRunning, setIsTimerRunning] = createSignal(false);
  let filterStartTime: number | null = null;
  let timerInterval: number | undefined;

  // 섹션 접기 상태
  const [isSummaryCollapsed, setIsSummaryCollapsed] = createSignal(false);
  const [isTimelineCollapsed, setIsTimelineCollapsed] = createSignal(false);
  const [isInsightsCollapsed, setIsInsightsCollapsed] = createSignal(false);
  const [isAnalyticsCollapsed, setIsAnalyticsCollapsed] = createSignal(false);
  
  // Computed data for insights - switches between filtered and full data
  const topCategories = createMemo(() => 
    useFilteredInsights() && filteredInsights() 
      ? filteredInsights().top_categories 
      : s()?.top_device_categories || []
  );
  const topDeviceTypes = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().top_device_types
      : s()?.top_device_types || []
  );
  const topVendors = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().top_vendors
      : s()?.top_vendors || []
  );
  const topTransport = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().top_transport_interfaces
      : s()?.top_transport_interfaces || []
  );
  
  const totalCategories = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().total_categories
      : s()?.all_device_categories?.length || 0
  );
  const totalDeviceTypes = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().total_device_types
      : s()?.total_device_types || 0
  );
  const totalVendors = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().total_vendors
      : s()?.total_vendors || 0
  );
  const totalTransport = createMemo(() => 
    useFilteredInsights() && filteredInsights()
      ? filteredInsights().total_transport_interfaces
      : s()?.all_transport_interfaces?.length || 0
  );
  
  // 유효한 필터 옵션 (다른 필터에 따라 동적으로 변경)
  const [availableOptions, setAvailableOptions] = createSignal<AvailableFilterOptions>({
    categories: [],
    device_types: [],
    vendors: [],
    transport_interfaces: [],
  });
  
  // analytics.filterApplied 기반으로 특정 차원만 제외한 필터 생성
  const buildFilterExcluding = (excludeFilter: 'category' | 'device_type' | 'vendor' | 'transport') => {
    // analytics.filterApplied가 신뢰할 수 있는 원천 정보
    const currentFilter = analytics.filterApplied || '';
    
    console.log('[buildFilterExcluding] 📋 현재 적용된 필터:', currentFilter);
    console.log('[buildFilterExcluding] 🚫 제외할 차원:', excludeFilter);
    
    if (!currentFilter.trim()) {
      console.log('[buildFilterExcluding] ⚠️ 적용된 필터가 없음, 빈 문자열 반환');
      return '';
    }
    
    // DSL 파싱: AND로 구분된 토큰들
    const tokens = currentFilter.split(' AND ').map(t => t.trim()).filter(t => t.length > 0);
    
    // 제외할 차원에 해당하는 토큰 필터링
    const filteredTokens = tokens.filter(token => {
      const lowerToken = token.toLowerCase();
      
      switch (excludeFilter) {
        case 'category':
          // device_category:in:[...] 제외
          return !lowerToken.startsWith('device_category:');
        case 'device_type':
          // dtype:in:[...] 제외
          return !lowerToken.startsWith('dtype:') && !lowerToken.startsWith('device_type_name:');
        case 'vendor':
          // vendor:in:[...] 제외
          return !lowerToken.startsWith('vendor:');
        case 'transport':
          // transport:in:[...] 제외
          return !lowerToken.startsWith('transport:');
        default:
          return true;
      }
    });
    
    const result = filteredTokens.join(' AND ');
    console.log('[buildFilterExcluding] ✅ 결과 필터:', result);
    return result;
  };
  
  // 유효한 옵션 업데이트
  const updateAvailableOptions = async () => {
    try {
      console.log('[updateAvailableOptions] 🔍 시작 - 현재 필터:', {
        categories: ui.selectedCategories,
        deviceTypes: ui.selectedDeviceTypes,
        vendors: ui.selectedVendors,
        transport: ui.selectedTransportInterfaces,
        dateRange: ui.certDateRange
      });
      
      const vendorFilter = buildFilterExcluding('vendor');
      console.log('[updateAvailableOptions] 🔧 벤더 필터 DSL:', vendorFilter);
      
      // 각 필터별로 해당 필터를 제외한 다른 필터들을 적용하여 유효한 옵션 조회
      console.log('[updateAvailableOptions] 📡 백엔드 API 호출 시작...');
      
      const categoryFilter = buildFilterExcluding('category');
      const deviceTypeFilter = buildFilterExcluding('device_type');
      const transportFilter = buildFilterExcluding('transport');
      
      console.log('[updateAvailableOptions] 🔧 생성된 필터들:', {
        category: categoryFilter,
        deviceType: deviceTypeFilter,
        vendor: vendorFilter,
        transport: transportFilter
      });
      
      console.log('[updateAvailableOptions] 🚀 invoke 호출 직전...');
      console.log('[updateAvailableOptions] 🔍 invoke 함수 타입:', typeof invoke);
      console.log('[updateAvailableOptions] 🔍 invoke 함수:', invoke);
      
      console.log('[updateAvailableOptions] 🎬 Promise.all 시작...');
      
      // 🔧 TEST: 먼저 하나만 호출해서 백엔드 응답 확인
      console.log('[updateAvailableOptions] 🧪 TEST: 단일 invoke 테스트 시작...');
      console.log('[updateAvailableOptions] 🧪 TEST: vendorFilter =', vendorFilter);
      console.log('[updateAvailableOptions] 🧪 TEST: 파라미터 =', { current_filter: vendorFilter });
      
      try {
        const testResult = await invoke<AvailableFilterOptions>('get_available_filter_options', { 
          current_filter: vendorFilter
        });
        console.log('[updateAvailableOptions] ✅ TEST 성공! 응답:', testResult);
      } catch (testError) {
        console.error('[updateAvailableOptions] ❌ TEST 실패:', testError);
        console.error('[updateAvailableOptions] ❌ TEST 에러 타입:', typeof testError);
        console.error('[updateAvailableOptions] ❌ TEST 에러 상세:', JSON.stringify(testError, null, 2));
      }
      
      const [categories, deviceTypes, vendors, transportInterfaces] = await Promise.all([
        invoke<AvailableFilterOptions>('get_available_filter_options', { 
          current_filter: categoryFilter
        }).then(result => {
          console.log('[updateAvailableOptions] ✅ categories 응답:', result);
          return result;
        }).catch(err => {
          console.error('[updateAvailableOptions] ❌ categories 에러:', err);
          throw err;
        }),
        invoke<AvailableFilterOptions>('get_available_filter_options', { 
          current_filter: deviceTypeFilter
        }).then(result => {
          console.log('[updateAvailableOptions] ✅ deviceTypes 응답:', result);
          return result;
        }).catch(err => {
          console.error('[updateAvailableOptions] ❌ deviceTypes 에러:', err);
          throw err;
        }),
        invoke<AvailableFilterOptions>('get_available_filter_options', { 
          current_filter: vendorFilter
        }).then(result => {
          console.log('[updateAvailableOptions] ✅ vendors 응답:', result);
          return result;
        }).catch(err => {
          console.error('[updateAvailableOptions] ❌ vendors 에러:', err);
          throw err;
        }),
        invoke<AvailableFilterOptions>('get_available_filter_options', { 
          current_filter: transportFilter
        }).then(result => {
          console.log('[updateAvailableOptions] ✅ transportInterfaces 응답:', result);
          return result;
        }).catch(err => {
          console.error('[updateAvailableOptions] ❌ transportInterfaces 에러:', err);
          throw err;
        }),
      ]);
      
      console.log('[updateAvailableOptions] 🎉 Promise.all 완료! 결과:', {
        categories,
        deviceTypes,
        vendors,
        transportInterfaces
      });
      
      console.log('[updateAvailableOptions] ✅ 백엔드 응답:', {
        vendorFilter: vendorFilter,
        availableVendorsCount: vendors.vendors?.length || 0,
        availableVendors: vendors.vendors?.slice(0, 5) || [],
        allResponses: {
          categories: categories.categories?.length,
          deviceTypes: deviceTypes.device_types?.length,
          vendors: vendors.vendors?.length,
          transport: transportInterfaces.transport_interfaces?.length
        }
      });
      
      setAvailableOptions({
        categories: categories.categories || [],
        device_types: deviceTypes.device_types || [],
        vendors: vendors.vendors || [],
        transport_interfaces: transportInterfaces.transport_interfaces || [],
      });
      
      console.log('[updateAvailableOptions] ✅ availableOptions 업데이트 완료');
    } catch (error) {
      console.error('[updateAvailableOptions] ❌ 실패:', error);
      console.error('[updateAvailableOptions] ❌ 에러 타입:', typeof error);
      console.error('[updateAvailableOptions] ❌ 에러 객체:', JSON.stringify(error, null, 2));
      if (error instanceof Error) {
        console.error('[updateAvailableOptions] ❌ 에러 메시지:', error.message);
        console.error('[updateAvailableOptions] ❌ 에러 스택:', error.stack);
      }
      // 에러 발생 시에도 빈 배열로 설정하여 UI가 멈추지 않도록
      setAvailableOptions({
        categories: [],
        device_types: [],
        vendors: [],
        transport_interfaces: [],
      });
    }
  };

  // 초기화 완료 플래그
  const [isInitialized, setIsInitialized] = createSignal(false);

  // 필터 조합 변경 시 자동으로 유효 옵션 재계산 (카테고리 선택이 벤더/Transport에 반영되도록)
  createEffect(() => {
    // 초기화가 완료된 후에만 실행
    if (!isInitialized()) return;
    
    // 의존성 읽기: Solid은 값 접근만으로 추적
    ui.selectedCategories.length;
    ui.selectedDeviceTypes.length;
    ui.selectedVendors.length;
    ui.selectedTransportInterfaces.length;
    ui.certDateRange[0];
    ui.certDateRange[1];
    updateAvailableOptions().catch(err => console.warn('[auto updateAvailableOptions] 실패', err));
  });
  
  // 필터링된 인사이트 업데이트
  const updateFilteredInsights = async () => {
    if (!useFilteredInsights()) return;
    
    try {
      setLoadingFilteredInsights(true);
      const currentFilter = analytics.filterApplied || '';
      
      console.log('[updateFilteredInsights] Fetching filtered insights with filter:', currentFilter);
      
      const result = await invoke<any>('get_filtered_analytics_summary', {
        filter: currentFilter || null,
      });
      
      setFilteredInsights(result);
      console.log('[updateFilteredInsights] Filtered insights updated:', result);
    } catch (error) {
      console.error('[updateFilteredInsights] Failed to fetch filtered insights:', error);
    } finally {
      setLoadingFilteredInsights(false);
    }
  };

  onMount(async () => {
    console.log('[onMount] 시작');
    
    // 1. 초기 날짜 범위 설정
    const today = new Date().toISOString().split('T')[0];
    const defaultStartDate = "2020-01-01";
    const defaultEndDate = today;
    const initialFilter = `date>=${defaultStartDate} AND date<=${defaultEndDate}`;
    
    // 2. Summary 먼저 로드 (날짜 범위 설정을 위해)
    console.log('[onMount] Summary 로드 시작');
    try {
      await localDbDashboardStore.loadSummary();
      console.log('[onMount] ✅ Summary 로드 완료');
    } catch (error) {
      console.error('[onMount] ❌ Summary 로드 실패:', error);
    }
    
    // 3. Analytics 필터 적용하고 데이터 로드
    console.log('[onMount] Analytics 필터 적용:', initialFilter);
    try {
      await localDbDashboardStore.applyFilter(initialFilter);
      console.log('[onMount] ✅ Analytics 데이터 로드 완료');
    } catch (error) {
      console.error('[onMount] ❌ Analytics 로드 실패:', error);
    }
    
    // 4. Device Types 로드
    try {
      await localDbDashboardStore.initDeviceTypes();
      console.log('[onMount] ✅ Device Types 로드 완료');
    } catch (error) {
      console.error('[onMount] ❌ Device Types 로드 실패:', error);
    }
    
    // 5. filteredInsights 로드
    try {
      setLoadingFilteredInsights(true);
      console.log('[onMount] 초기 filteredInsights 로드:', { filter: initialFilter });
      
      const result = await invoke<any>('get_filtered_analytics_summary', {
        filter: initialFilter,
      });
      
      setFilteredInsights(result);
      console.log('[onMount] 초기 filteredInsights 설정 완료:', result);
      
      setLoadingFilteredInsights(false);
    } catch (error) {
      console.error('[onMount] 초기 filteredInsights 로드 실패:', error);
      setLoadingFilteredInsights(false);
    }
    
    // 6. 유효한 필터 옵션 가져오기
    updateAvailableOptions().catch(console.error);
    
    // 7. 초기화 완료 플래그 설정
    setIsInitialized(true);
    console.log('[onMount] ✅ 초기화 완료');
    
    // Vendor sync progress events (coarse-grained)
    listen<any>('vendor_sync_progress', (evt) => {
      const p = evt.payload || {};
      localDbDashboardStore.setUi({ ...localDbDashboardStore.ui, vendorResult: { ...(localDbDashboardStore.ui.vendorResult||{}), progress: p.stage } });
    }).catch(()=>{});
    
    // Cleanup: 타이머 정리
    onCleanup(() => {
      if (timerInterval) {
        window.clearInterval(timerInterval);
        timerInterval = undefined;
      }
    });
  });

  // 4. summary가 로드되고 실제 날짜 범위가 설정되면 filteredInsights를 실제 범위로 업데이트
  let hasInitializedWithRealDates = false;
  createEffect(() => {
    // summary가 로드되고, certDateMin/Max가 설정되었으며, 아직 초기화하지 않았을 때만 실행
    const certMin = ui.certDateMin;
    const certMax = ui.certDateMax;
    const summaryExists = s();
    
    if (summaryExists && certMin && certMax && !hasInitializedWithRealDates) {
      hasInitializedWithRealDates = true;
      
      console.log('[Effect] 실제 날짜 범위로 filteredInsights 업데이트:', {
        certMin,
        certMax
      });
      
      const realFilter = `date>=${certMin} AND date<=${certMax}`;
      
      setLoadingFilteredInsights(true);
      invoke<any>('get_filtered_analytics_summary', {
        filter: realFilter,
      }).then(result => {
        setFilteredInsights(result);
        localDbDashboardStore.applyFilter(realFilter);
        console.log('[Effect] 실제 날짜 범위로 filteredInsights 업데이트 완료');
      }).catch(error => {
        console.error('[Effect] filteredInsights 업데이트 실패:', error);
      }).finally(() => {
        setLoadingFilteredInsights(false);
      });
    }
  });

  // 기존 로컬 제품 목록/검색 기능은 새로운 Analytics DSL UI 도입 전 임시 제거 (필요 시 별도 섹션 재추가)
  // (legacy 검색 신호 제거됨)

  // 디바운스 타이머 (날짜 범위 변경 최적화)
  let dateDebounce: number | undefined;
  const DATE_DEBOUNCE_MS = 250;

  // Analytics 테이블 페이지 계산
  const totalPages = () => Math.max(1, Math.ceil(analytics.total / analytics.limit));

  // 필터 적용 함수: 날짜 범위 + 카테고리 + 벤더 선택을 DSL 필터로 변환
  const applyFilters = () => {
    console.log('[applyFilters] 🚀 필터 적용 시작');
    console.log('[applyFilters] 📊 현재 선택 상태:', {
      selectedCategories: ui.selectedCategories,
      selectedVendors: ui.selectedVendors,
      selectedDeviceTypes: ui.selectedDeviceTypes,
      certDateRange: ui.certDateRange
    });
    
    const filters: string[] = [];

    // 1. 날짜 범위 필터 (항상 적용) - date 필드 -> certification_date 매핑
    let [startDate, endDate] = ui.certDateRange;
    
    // 날짜가 비어있으면 전체 범위 사용
    if (!startDate || !endDate) {
      startDate = ui.certDateMin || "2020-01-01";
      endDate = ui.certDateMax || new Date().toISOString().split('T')[0];
      console.log('[applyFilters] 날짜 범위 없음, 전체 범위 사용:', { startDate, endDate });
    }
    
    if (startDate && endDate) {
      filters.push(`date>=${startDate}`);
      filters.push(`date<=${endDate}`);
    }
    console.log('[applyFilters] 날짜 필터 적용:', { startDate, endDate, filterAdded: startDate && endDate });

    // 2. 카테고리 필터 (device_category:in:[...] 형식)
    console.log('[applyFilters] 카테고리 필터 검사:', { 
      length: ui.selectedCategories.length, 
      categories: ui.selectedCategories 
    });
    if (ui.selectedCategories.length > 0) {
      const hasNull = ui.selectedCategories.includes('null');
      const normalCats = ui.selectedCategories.filter(c => c !== 'null');
      
      if (hasNull && normalCats.length === 0) {
        // null만 선택
        filters.push('device_category=null');
        console.log('[applyFilters] 카테고리 필터: 카테고리 없음');
      } else if (normalCats.length > 0) {
        // 일반 카테고리 선택 (UI에서 null과 동시 선택 불가능하도록 제어됨)
        const catList = normalCats.map(c => `"${c.replace(/"/g, '\\"')}"`).join(',');
        filters.push(`device_category:in:[${catList}]`);
        console.log(`[applyFilters] 카테고리 필터: ${normalCats.length}개 선택`);
      }
    }

    // 3. 벤더 필터 (vendor_name:in:[...] 형식)
    if (ui.selectedVendors.length > 0) {
      const vendorList = ui.selectedVendors.map(v => `"${v.replace(/"/g, '\\"')}"`).join(',');
      filters.push(`vendor_name:in:[${vendorList}]`);
    }

    // 4. 디바이스 타입 필터 (dtype:in:[...] 형식)
    if (ui.selectedDeviceTypes.length > 0) {
      const dtypeList = ui.selectedDeviceTypes.map(d => `"${d.replace(/"/g, '\\"')}"`).join(',');
      filters.push(`dtype:in:[${dtypeList}]`);
    }

    // 5. Transport Interface 필터 (transport_interface:in:[...] 형식)
    if (ui.selectedTransportInterfaces.length > 0) {
      const tiList = ui.selectedTransportInterfaces.map(ti => `"${ti.replace(/"/g, '\\"')}"`).join(',');
      filters.push(`transport_interface:in:[${tiList}]`);
    }

    // 6. 기존 filterDraft (quick search)와 결합
    const quickSearch = ui.filterDraft?.trim();
    if (quickSearch) {
      filters.push(quickSearch);
    }

    // 7. 최종 필터 문자열 생성
    const finalFilter = filters.join(' AND ');
    
    console.log('[applyFilters] 최종 필터:', finalFilter);
    
    // 8. Store에 적용하고 Analytics 재로드
    localDbDashboardStore.applyFilter(finalFilter);
    
    // 9. 유효한 필터 옵션 업데이트 (필터 변경 후)
    updateAvailableOptions().catch(console.error);
    
    // 10. filteredInsights 업데이트 (데이터 요약 카드용)
    console.log('[applyFilters] filteredInsights 업데이트 요청:', {
      finalFilter,
      dateRange: { startDate, endDate },
      hasDateFilter: finalFilter.includes('date>=') && finalFilter.includes('date<=')
    });
    
    // 타이머 시작
    filterStartTime = performance.now();
    setIsTimerRunning(true);
    setFilterLoadTime(null);
    
    // 실시간 타이머 업데이트 (매 10ms)
    if (timerInterval) {
      window.clearInterval(timerInterval);
    }
    timerInterval = window.setInterval(() => {
      if (filterStartTime !== null) {
        const elapsed = performance.now() - filterStartTime;
        setFilterLoadTime(elapsed);
      }
    }, 10);
    
    setLoadingFilteredInsights(true);
    
    invoke<any>('get_filtered_analytics_summary', {
      filter: finalFilter || null,
    }).then(result => {
      setFilteredInsights(result);
      
      // 타이머 중지
      const endTime = performance.now();
      const totalTime = filterStartTime !== null ? endTime - filterStartTime : 0;
      setFilterLoadTime(totalTime);
      setIsTimerRunning(false);
      if (timerInterval) {
        window.clearInterval(timerInterval);
        timerInterval = undefined;
      }
      
      console.log('[applyFilters] filteredInsights 업데이트 완료:', {
        total_products: result.total_products,
        total_device_types: result.total_device_types,
        total_vendors: result.total_vendors,
        filter_applied: finalFilter,
        load_time_ms: totalTime.toFixed(2)
      });
    }).catch(error => {
      console.error('[applyFilters] filteredInsights 업데이트 실패:', error);
      
      // 타이머 중지 (에러 시)
      setIsTimerRunning(false);
      if (timerInterval) {
        window.clearInterval(timerInterval);
        timerInterval = undefined;
      }
    }).finally(() => {
      setLoadingFilteredInsights(false);
    });
  };

  // Export/Import/삭제/Vendor Sync/Device Types 등은 차례로 store helper로 이동 예정 (현재는 backend API 직접 호출 유지)

  const [vendorSyncLoading, setVendorSyncLoading] = createSignal(false);
  const [vendorSyncResult, setVendorSyncResult] = createSignal<VendorSyncResult | null>(null);
  const syncVendors = async () => {
    try {
      setVendorSyncLoading(true);
      setVendorSyncResult(null);
      const res = await tauriApi.dashboardVendorSync({ dry_run: false });
      setVendorSyncResult(res);
      await localDbDashboardStore.loadSummary();
    } catch (e) {
      alert(`벤더 동기화 실패: ${e}`);
    } finally {
      setVendorSyncLoading(false);
    }
  };

  // ================= Maintenance Handlers =================
  // CSV Import 관련 코드 제거됨
  // Delete Range 관련 코드 제거됨
  
  // Excel Export/Import Handlers
  
  const handleExcelExport = async () => {
    try {
      localDbDashboardStore.setUi({ ...ui, exportStatus: 'Excel 파일 생성 중...', working: true });
      
      // 백엔드에서 Excel 파일 생성
      const result = await tauriApi.exportFullDatabaseExcel();
      
      const statusMessage = `완료!\n저장 위치: ${result.file_path}\n` +
        `제품: ${result.products_count}개\n` +
        `상세정보: ${result.product_details_count}개\n` +
        `Device Types: ${result.device_types_count}개\n` +
        `Vendors: ${result.vendors_count}개`;
      
      localDbDashboardStore.setUi({ 
        ...ui, 
        exportStatus: statusMessage,
        working: false 
      });
      
      // 파일 위치를 클립보드에 복사하거나 알림
      const confirmMessage = `백업 파일이 생성되었습니다.\n\n` +
        `제품: ${result.products_count}개, 상세정보: ${result.product_details_count}개\n` +
        `Device Types: ${result.device_types_count}개, Vendors: ${result.vendors_count}개\n\n` +
        `파일 위치를 클립보드에 복사하시겠습니까?\n\n${result.file_path}`;
      
      if (confirm(confirmMessage)) {
        try {
          await navigator.clipboard.writeText(result.file_path);
          alert('파일 경로가 클립보드에 복사되었습니다!');
        } catch (e) {
          console.error('클립보드 복사 실패:', e);
        }
      }
    } catch (e: any) {
      localDbDashboardStore.setUi({ ...ui, exportStatus: `실패: ${e}`, working: false });
    }
  };
  
  const handleExcelImport = async () => {
    try {
      // 기본 내보내기 폴더 경로 가져오기
      const defaultDir = await tauriApi.getExportsDirectory();
      
      // 파일 선택 다이얼로그
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Excel Files', extensions: ['xlsx'] }],
        title: 'Excel 백업 파일 선택',
        defaultPath: defaultDir
      });
      
      if (selected && typeof selected === 'string') {
        await localDbDashboardStore.importFullDatabaseExcel(selected);
      } else if (!selected) {
        localDbDashboardStore.setUi({ ...ui, importStatus: '취소됨' });
      }
    } catch (e: any) {
      localDbDashboardStore.setUi({ ...ui, importStatus: `실패: ${e}`, working: false });
    }
  };
  
  // Page Range Delete Handlers
  const [deleteFromPage, setDeleteFromPage] = createSignal(1);
  const [deleteToPage, setDeleteToPage] = createSignal(20);
  const [maxPageId, setMaxPageId] = createSignal<number | null>(null);
  
  // Load max page_id on mount
  onMount(async () => {
    try {
      const max = await tauriApi.getMaxPageId();
      setMaxPageId(max);
      // Set default to latest 20 pages (max-19 to max)
      if (max > 19) {
        setDeleteFromPage(max - 19);
        setDeleteToPage(max);
      } else if (max > 0) {
        setDeleteFromPage(Math.max(0, max - 19));
        setDeleteToPage(max);
      } else {
        setDeleteFromPage(0);
        setDeleteToPage(max || 20);
      }
    } catch (e) {
      console.error('Failed to get max page_id:', e);
    }
  });
  
  const handlePreviewDelete = async () => {
    console.log('🔍 handlePreviewDelete clicked! from:', deleteFromPage(), 'to:', deleteToPage());
    await localDbDashboardStore.previewDeleteRange(deleteFromPage(), deleteToPage());
  };
  
  const handleExecuteDelete = async () => {
    if (!ui.deletePreview) {
      alert('먼저 미리보기를 실행하세요');
      return;
    }
    
    if (confirm(`페이지 ${deleteFromPage()}-${deleteToPage()} 범위를 삭제하시겠습니까?`)) {
      const currentFrom = deleteFromPage();
      await localDbDashboardStore.executeDeleteRange(deleteFromPage(), deleteToPage());
      
      // 삭제 성공 후 자동으로 다음 20페이지 범위 설정
      if (!ui.deleteResult?.error) {
        const newTo = currentFrom - 1;
        const newFrom = Math.max(0, newTo - 19);
        setDeleteFromPage(newFrom);
        setDeleteToPage(newTo);
        
        // 자동으로 미리보기 실행
        if (newTo > 0) {
          setTimeout(() => {
            localDbDashboardStore.previewDeleteRange(newFrom, newTo);
          }, 500);
        }
      }
    }
  };
  
  const handleDeleteAll = async () => {
    await localDbDashboardStore.deleteAllRecordsConfirmed();
  };
  
  // 레코드 삭제 섹션 접기 상태
  const [deletesSectionExpanded, setDeletesSectionExpanded] = createSignal(false);

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-3xl font-bold bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">✨ 로컬DB 데이터 분석</h2>
              <p class="text-sm text-gray-600">인증 데이터를 필터링하고 분석하세요</p>
            </div>
            <button
              class="flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-blue-500 to-purple-500 hover:from-blue-600 hover:to-purple-600 text-white rounded-lg shadow-md transition-all duration-200 font-semibold text-sm"
              onClick={() => localDbDashboardStore.setUi({ ...ui, filtersExpanded: !ui.filtersExpanded })}
            >
              <span>{ui.filtersExpanded ? '▲' : '▼'}</span>
              <span>{ui.filtersExpanded ? '필터 접기' : '필터 펼치기'}</span>
            </button>
          </div>
        </div>
        
        <Show when={ui.filtersExpanded}>
        {/* Date Range Slider */}
        <DateRangeSlider 
          minDate={ui.certDateMin || "2020-01-01"}
          maxDate={ui.certDateMax || new Date().toISOString().split('T')[0]}
          startDate={ui.certDateRange[0] || ui.certDateMin || "2020-01-01"}
          endDate={ui.certDateRange[1] || ui.certDateMax || new Date().toISOString().split('T')[0]}
          onChange={(start, end) => {
            localDbDashboardStore.setUi({ ...ui, certDateRange: [start, end] });
            if (dateDebounce) window.clearTimeout(dateDebounce);
            dateDebounce = window.setTimeout(() => applyFilters(), DATE_DEBOUNCE_MS);
          }}
        />
        <div class="mt-1 text-xs">
          <Show when={ui.certDateRange[0] && ui.certDateRange[1]}>
            {(() => {
              const [start, end] = ui.certDateRange;
              if (!start || !end) return null;
              const fullStart = ui.certDateMin;
              const fullEnd = ui.certDateMax;
              const span = (a: string, b: string) => (Math.round((new Date(b).getTime() - new Date(a).getTime())/86400000) + 1);
              const isFull = start === fullStart && end === fullEnd;
              const fullSpan = fullStart && fullEnd ? span(fullStart, fullEnd) : undefined;
              const curSpan = span(start, end);
              const pct = fullSpan ? Math.round(curSpan / fullSpan * 100) : 100;
              return <span class={`px-2 py-1 rounded-full font-semibold ${isFull ? 'bg-green-100 text-green-700' : 'bg-amber-100 text-amber-700'}`}>{isFull ? 'FULL RANGE' : 'PARTIAL'} · {curSpan}d{fullSpan ? ` / ${fullSpan}d (${pct}%)` : ''}</span>;
            })()}
          </Show>
        </div>

        {/* 카테고리 & 디바이스 타입 & 벤더 & Transport Interface 필터 버튼 */}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {/* 카테고리 필터 */}
          <div class="bg-gradient-to-br from-indigo-50 to-purple-50 border-2 border-indigo-200 rounded-xl p-5 shadow-sm">
            <div class="flex items-center gap-2 mb-3">
              <span class="text-2xl">🏷️</span>
              <div>
                <div class="text-sm font-bold text-indigo-900">카테고리 필터</div>
                <div class="text-xs text-indigo-600">{s()?.all_device_categories?.length || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-indigo-500 to-purple-600 hover:from-indigo-600 hover:to-purple-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={() => setShowCategoryDialog(true)}
            >
              <Show
                when={ui.selectedCategories.length > 0}
                fallback={
                  <div class="flex items-center justify-center gap-2">
                    <span>카테고리 선택</span>
                  </div>
                }
              >
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-center gap-2">
                    <span class="text-xs opacity-90">선택함:</span>
                    <span class="bg-white text-indigo-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                      {ui.selectedCategories.length}
                    </span>
                  </div>
                  <div class="text-xs opacity-90 truncate">
                    {ui.selectedCategories.slice(0, 2).map(c => c === 'null' ? '(없음)' : c).join(', ')}
                    {ui.selectedCategories.length > 2 ? '...' : ''}
                  </div>
                </div>
              </Show>
            </button>
            
            <Show when={ui.selectedCategories.length > 0}>
              <div class="mt-3 bg-white rounded-lg p-3 border border-indigo-200">
                <div class="text-xs font-semibold text-indigo-700 mb-2">선택됨:</div>
                <div class="flex flex-wrap gap-1.5">
                  <For each={ui.selectedCategories.slice(0, 3)}>
                    {cat => (
                      <span class="px-2 py-1 bg-indigo-100 text-indigo-700 rounded-full text-[10px] font-medium truncate max-w-[120px]" title={cat}>
                        {cat === 'null' ? '(없음)' : cat}
                      </span>
                    )}
                  </For>
                  <Show when={ui.selectedCategories.length > 3}>
                    <span class="px-2 py-1 bg-gray-200 text-gray-600 rounded-full text-[10px] font-semibold">
                      +{ui.selectedCategories.length - 3}
                    </span>
                  </Show>
                </div>
              </div>
            </Show>
          </div>

          {/* 디바이스 타입 필터 */}
          <div class="bg-gradient-to-br from-emerald-50 to-teal-50 border-2 border-emerald-200 rounded-xl p-5 shadow-sm">
            <div class="flex items-center gap-2 mb-3">
              <span class="text-2xl">🔧</span>
              <div>
                <div class="text-sm font-bold text-emerald-900">디바이스 타입 필터</div>
                <div class="text-xs text-emerald-600">{s()?.all_device_types?.length || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={() => setShowDeviceTypeDialog(true)}
            >
              <Show
                when={ui.selectedDeviceTypes.length > 0}
                fallback={
                  <div class="flex items-center justify-center gap-2">
                    <span>디바이스 타입 선택</span>
                  </div>
                }
              >
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-center gap-2">
                    <span class="text-xs opacity-90">선택함:</span>
                    <span class="bg-white text-emerald-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                      {ui.selectedDeviceTypes.length}
                    </span>
                  </div>
                  <div class="text-xs opacity-90 truncate">
                    {ui.selectedDeviceTypes.slice(0, 2).join(', ')}
                    {ui.selectedDeviceTypes.length > 2 ? '...' : ''}
                  </div>
                </div>
              </Show>
            </button>
            
            <Show when={ui.selectedDeviceTypes.length > 0}>
              <div class="mt-3 bg-white rounded-lg p-3 border border-emerald-200">
                <div class="text-xs font-semibold text-emerald-700 mb-2">선택됨:</div>
                <div class="flex flex-wrap gap-1.5">
                  <For each={ui.selectedDeviceTypes.slice(0, 3)}>
                    {dtype => (
                      <span class="px-2 py-1 bg-emerald-100 text-emerald-700 rounded-full text-[10px] font-medium truncate max-w-[120px]" title={dtype}>
                        {dtype}
                      </span>
                    )}
                  </For>
                  <Show when={ui.selectedDeviceTypes.length > 3}>
                    <span class="px-2 py-1 bg-gray-200 text-gray-600 rounded-full text-[10px] font-semibold">
                      +{ui.selectedDeviceTypes.length - 3}
                    </span>
                  </Show>
                </div>
              </div>
            </Show>
          </div>

          {/* 벤더 필터 */}
          <div class="bg-gradient-to-br from-blue-50 to-cyan-50 border-2 border-blue-200 rounded-xl p-5 shadow-sm">
            <div class="flex items-center gap-2 mb-3">
              <span class="text-2xl">🏢</span>
              <div>
                <div class="text-sm font-bold text-blue-900">벤더 필터</div>
                <div class="text-xs text-blue-600">{s()?.all_vendors?.length || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-blue-500 to-cyan-600 hover:from-blue-600 hover:to-cyan-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={async () => {
                setShowVendorDialog(true);
                await updateAvailableOptions();
              }}
            >
              <Show
                when={ui.selectedVendors.length > 0}
                fallback={
                  <div class="flex items-center justify-center gap-2">
                    <span>벤더 선택</span>
                  </div>
                }
              >
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-center gap-2">
                    <span class="text-xs opacity-90">선택함:</span>
                    <span class="bg-white text-blue-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                      {ui.selectedVendors.length}
                    </span>
                  </div>
                  <div class="text-xs opacity-90 truncate">
                    {ui.selectedVendors.slice(0, 2).join(', ')}
                    {ui.selectedVendors.length > 2 ? '...' : ''}
                  </div>
                </div>
              </Show>
            </button>
            
            <Show when={ui.selectedVendors.length > 0}>
              <div class="mt-3 bg-white rounded-lg p-3 border border-blue-200">
                <div class="text-xs font-semibold text-blue-700 mb-2">선택됨:</div>
                <div class="flex flex-wrap gap-1.5">
                  <For each={ui.selectedVendors.slice(0, 3)}>
                    {vendor => (
                      <span class="px-2 py-1 bg-blue-100 text-blue-700 rounded-full text-[10px] font-medium truncate max-w-[120px]" title={vendor}>
                        {vendor}
                      </span>
                    )}
                  </For>
                  <Show when={ui.selectedVendors.length > 3}>
                    <span class="px-2 py-1 bg-gray-200 text-gray-600 rounded-full text-[10px] font-semibold">
                      +{ui.selectedVendors.length - 3}
                    </span>
                  </Show>
                </div>
              </div>
            </Show>
          </div>

          {/* Transport Interface 필터 */}
          <div class="bg-gradient-to-br from-violet-50 to-purple-50 border-2 border-violet-200 rounded-xl p-5 shadow-sm">
            <div class="flex items-center gap-2 mb-3">
              <span class="text-2xl">📡</span>
              <div>
                <div class="text-sm font-bold text-violet-900">Transport Interface 필터</div>
                <div class="text-xs text-violet-600">{s()?.all_transport_interfaces?.length || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={async () => {
                setShowTransportInterfaceDialog(true);
                await updateAvailableOptions();
              }}
            >
              <Show
                when={ui.selectedTransportInterfaces.length > 0}
                fallback={
                  <div class="flex items-center justify-center gap-2">
                    <span>Transport Interface 선택</span>
                  </div>
                }
              >
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-center gap-2">
                    <span class="text-xs opacity-90">선택함:</span>
                    <span class="bg-white text-violet-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                      {ui.selectedTransportInterfaces.length}
                    </span>
                  </div>
                  <div class="text-xs opacity-90 truncate">
                    {ui.selectedTransportInterfaces.slice(0, 2).join(', ')}
                    {ui.selectedTransportInterfaces.length > 2 ? '...' : ''}
                  </div>
                </div>
              </Show>
            </button>
            
            <Show when={ui.selectedTransportInterfaces.length > 0}>
              <div class="mt-3 bg-white rounded-lg p-3 border border-violet-200">
                <div class="text-xs font-semibold text-violet-700 mb-2">선택됨:</div>
                <div class="flex flex-wrap gap-1.5">
                  <For each={ui.selectedTransportInterfaces.slice(0, 3)}>
                    {ti => (
                      <span class="px-2 py-1 bg-violet-100 text-violet-700 rounded-full text-[10px] font-medium truncate max-w-[120px]" title={ti}>
                        {ti}
                      </span>
                    )}
                  </For>
                  <Show when={ui.selectedTransportInterfaces.length > 3}>
                    <span class="px-2 py-1 bg-gray-200 text-gray-600 rounded-full text-[10px] font-semibold">
                      +{ui.selectedTransportInterfaces.length - 3}
                    </span>
                  </Show>
                </div>
              </div>
            </Show>
          </div>
        </div>
        </Show>

        {/* Summary */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="flex items-center justify-between mb-6">
            <button
              onClick={() => setIsSummaryCollapsed(!isSummaryCollapsed())}
              class="flex items-center gap-3 hover:text-indigo-600 transition-colors"
            >
              <div>
                <h3 class="text-xl font-bold text-gray-800 flex items-center gap-2">
                  📈 데이터 요약
                  <span class="text-gray-400 text-base">{isSummaryCollapsed() ? '▼' : '▲'}</span>
                </h3>
                <p class="text-xs text-gray-500 mt-1">전체 데이터베이스 통계 현황</p>
                <Show when={analytics.loading}>
                  <p class="text-[10px] text-indigo-500 mt-1 animate-pulse">⏳ 분석 데이터 로딩...</p>
                </Show>
                {/* 디버깅: 현재 적용된 필터 표시 */}
                <Show when={analytics.filterApplied}>
                  <p class="text-xs text-red-600 mt-1 font-mono">🔍 필터: {analytics.filterApplied}</p>
                </Show>
                <p class="text-xs text-blue-600 mt-1 font-mono">📅 날짜 범위: {ui.certDateRange[0] || '없음'} ~ {ui.certDateRange[1] || '없음'}</p>
                <p class="text-xs text-purple-600 mt-1 font-mono">
                  📊 filteredInsights: {filteredInsights() ? `${filteredInsights()!.total_products} products` : 'null'}
                </p>
                {/* 디버깅: 필터 로드 타이머 */}
                <Show when={isTimerRunning() || filterLoadTime() !== null}>
                  <p class="text-xs font-mono mt-1 flex items-center gap-1">
                    <Show when={isTimerRunning()} fallback={
                      <span class="text-green-600">✅ 로드 완료: {filterLoadTime()?.toFixed(0)}ms</span>
                    }>
                      <span class="text-orange-500 animate-pulse">⏱️ 로딩 중: {filterLoadTime()?.toFixed(0) || '0'}ms</span>
                    </Show>
                  </p>
                </Show>
              </div>
            </button>
            <div class="flex gap-2">
              <button 
                class="px-4 py-2 rounded-lg bg-gradient-to-r from-red-500 to-pink-600 hover:from-red-600 hover:to-pink-700 text-white text-sm font-semibold shadow-md transition-all"
                onClick={async () => {
                  // 날짜 범위를 전체 범위로 리셋
                  const fullStartDate = ui.certDateMin || "2020-01-01";
                  const fullEndDate = ui.certDateMax || new Date().toISOString().split('T')[0];
                  
                  localDbDashboardStore.resetFilter();
                  localDbDashboardStore.setUi({ 
                    ...ui, 
                    certDateRange: [fullStartDate, fullEndDate], 
                    selectedCategories: [], 
                    selectedVendors: [], 
                    selectedDeviceTypes: [], 
                    selectedTransportInterfaces: [] 
                  });
                  
                  // filteredInsights도 초기화 (전체 날짜 범위 포함)
                  const dateFilters = [`date>=${fullStartDate}`, `date<=${fullEndDate}`];
                  const fullRangeFilter = dateFilters.join(' AND ');
                  
                  const result = await invoke<any>('get_filtered_analytics_summary', {
                    filter: fullRangeFilter,
                  });
                  setFilteredInsights(result);
                  console.log('[Reset] filteredInsights 초기화 완료 (전체 날짜 범위 적용):', result);
                  
                  // 유효한 필터 옵션도 업데이트
                  updateAvailableOptions().catch(console.error);
                }}
              >
                🗑️ 필터 초기화
              </button>
              <button 
                class="px-4 py-2 rounded-lg bg-gradient-to-r from-blue-500 to-indigo-600 hover:from-blue-600 hover:to-indigo-700 text-white text-sm font-semibold shadow-md transition-all"
                onClick={() => localDbDashboardStore.loadSummary()}
              >
                🔄 새로고침
              </button>
            </div>
          </div>
          
          <Show when={!isSummaryCollapsed()}>
          <Show when={s()} fallback={
            <div class="flex items-center justify-center py-12">
              <div class="flex flex-col items-center gap-2">
                <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
                <div class="text-sm text-gray-400">📊 초기 데이터 로딩 중...</div>
                <Show when={ui.loadingSummary}>
                  <div class="text-xs text-gray-500 mt-1">
                    (summary 로드 중...)
                  </div>
                </Show>
              </div>
            </div>
          }>
            {/* 주요 지표 카드 */}
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              {/* 1. PRODUCTS (카테고리 색상) - 필터 버튼 1번과 일치 */}
              <div class="rounded-xl bg-gradient-to-br from-indigo-50 to-purple-100 border-2 border-indigo-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🏷️</span>
                  <span class="text-xs font-semibold text-indigo-700 bg-indigo-200 px-2 py-1 rounded-full">PRODUCTS</span>
                </div>
                <Show
                  when={!loadingFilteredInsights()}
                  fallback={
                    <div class="animate-pulse">
                      <div class="h-8 bg-indigo-200 rounded mb-2"></div>
                      <div class="h-3 bg-indigo-100 rounded w-3/4"></div>
                    </div>
                  }
                >
                  {(() => {
                    const filtered = filteredInsights()?.total_products || 0;
                    const total = s()!.total_products;
                    const pct = total ? (filtered / total * 100) : 0;
                    return (
                      <>
                        <div class="text-2xl font-bold text-indigo-900 transition-all duration-300">
                          <span class="text-indigo-600">{filtered.toLocaleString()}</span>
                          <span class="text-lg text-indigo-400 mx-1">/</span>
                          <span class="text-indigo-800">{total.toLocaleString()}</span>
                        </div>
                        <div class="text-xs text-indigo-700 mt-1">인증 제품 (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                      </>
                    );
                  })()}
                </Show>
              </div>

              {/* 2. TYPES (디바이스 타입) - 필터 버튼 2번과 일치 */}
              <div class="rounded-xl bg-gradient-to-br from-emerald-50 to-teal-100 border-2 border-emerald-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🔧</span>
                  <span class="text-xs font-semibold text-emerald-700 bg-emerald-200 px-2 py-1 rounded-full">TYPES</span>
                </div>
                <Show
                  when={!loadingFilteredInsights()}
                  fallback={
                    <div class="animate-pulse">
                      <div class="h-8 bg-emerald-200 rounded mb-2"></div>
                      <div class="h-3 bg-emerald-100 rounded w-3/4"></div>
                    </div>
                  }
                >
                  {(() => {
                    const filtered = filteredInsights()?.total_device_types || 0;
                    const total = s()?.all_device_types?.length || 0;
                    const pct = total ? (filtered / total * 100) : 0;
                    return (
                      <>
                        <div class="text-2xl font-bold text-emerald-900 transition-all duration-300">
                          <span class="text-emerald-600">{filtered.toLocaleString()}</span>
                          <span class="text-lg text-emerald-400 mx-1">/</span>
                          <span class="text-emerald-800">{total.toLocaleString()}</span>
                        </div>
                        <div class="text-xs text-emerald-700 mt-1">디바이스 타입 (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                      </>
                    );
                  })()}
                </Show>
              </div>

              {/* 3. VENDORS (벤더) - 필터 버튼 3번과 일치 */}
              <div class="rounded-xl bg-gradient-to-br from-blue-50 to-cyan-100 border-2 border-blue-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🏢</span>
                  <span class="text-xs font-semibold text-blue-700 bg-blue-200 px-2 py-1 rounded-full">VENDORS</span>
                </div>
                <Show
                  when={!loadingFilteredInsights()}
                  fallback={
                    <div class="animate-pulse">
                      <div class="h-8 bg-blue-200 rounded mb-2"></div>
                      <div class="h-3 bg-blue-100 rounded w-3/4"></div>
                    </div>
                  }
                >
                  {(() => {
                    const filtered = filteredInsights()?.total_vendors || 0;
                    const total = s()?.all_vendors?.length || 0;
                    const pct = total ? (filtered / total * 100) : 0;
                    return (
                      <>
                        <div class="text-2xl font-bold text-blue-900 transition-all duration-300">
                          <span class="text-blue-600">{filtered.toLocaleString()}</span>
                          <span class="text-lg text-blue-400 mx-1">/</span>
                          <span class="text-blue-800">{total.toLocaleString()}</span>
                        </div>
                        <div class="text-xs text-blue-700 mt-1">제조사 (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                      </>
                    );
                  })()}
                </Show>
              </div>

              {/* 4. TRANSPORT (트랜스포트) - 필터 버튼 4번과 일치 */}
              <div class="rounded-xl bg-gradient-to-br from-violet-50 to-purple-100 border-2 border-violet-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">📡</span>
                  <span class="text-xs font-semibold text-violet-700 bg-violet-200 px-2 py-1 rounded-full">TRANSPORT</span>
                </div>
                <Show
                  when={!loadingFilteredInsights()}
                  fallback={
                    <div class="animate-pulse">
                      <div class="h-8 bg-violet-200 rounded mb-2"></div>
                      <div class="h-3 bg-violet-100 rounded w-3/4"></div>
                    </div>
                  }
                >
                  {(() => {
                    const filtered = filteredInsights()?.total_transport_interfaces || 0;
                    const total = s()?.all_transport_interfaces?.length || 0;
                    const pct = total ? (filtered / total * 100) : 0;
                    return (
                      <>
                        <div class="text-2xl font-bold text-violet-900 transition-all duration-300">
                          <span class="text-violet-600">{filtered.toLocaleString()}</span>
                          <span class="text-lg text-violet-400 mx-1">/</span>
                          <span class="text-violet-800">{total.toLocaleString()}</span>
                        </div>
                        <div class="text-xs text-violet-700 mt-1">Transport IF (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                      </>
                    );
                  })()}
                </Show>
              </div>
            </div>
          </Show>
          </Show>
        </div>

        {/* 인증 추세 차트 섹션 */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <button
            onClick={() => setIsTimelineCollapsed(!isTimelineCollapsed())}
            class="flex items-center gap-2 hover:text-indigo-600 transition-colors mb-4 w-full"
          >
            <h3 class="text-xl font-bold text-gray-800 flex items-center gap-2">
              📈 인증 추세
              <span class="text-gray-400 text-base">{isTimelineCollapsed() ? '▼' : '▲'}</span>
            </h3>
          </button>
          <Show when={!isTimelineCollapsed()}>
        <CertificationTimeline 
          filter={analytics.filterApplied || null}
          startDate={ui.certDateRange[0] || ui.certDateMin || "2020-01-01"}
          endDate={ui.certDateRange[1] || ui.certDateMax || new Date().toISOString().split('T')[0]}
        />
          </Show>
        </div>

        {/* 분석 인사이트 섹션 */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="mb-6 flex items-center justify-between">
            <button
              onClick={() => setIsInsightsCollapsed(!isInsightsCollapsed())}
              class="flex items-center gap-2 hover:text-indigo-600 transition-colors"
            >
              <div>
                <h3 class="text-xl font-bold text-gray-800 flex items-center gap-2">
                  🔍 분석 인사이트
                  <span class="text-gray-400 text-base">{isInsightsCollapsed() ? '▼' : '▲'}</span>
                </h3>
                <p class="text-xs text-gray-500 mt-1">카테고리, 디바이스 타입, 벤더, Transport Interface 통계</p>
              </div>
            </button>
            <div class="flex items-center gap-3">
              <Show when={analytics.filterApplied}>
                <div class="text-xs text-gray-500 bg-blue-50 px-3 py-1.5 rounded-lg border border-blue-200">
                  <span class="font-semibold text-blue-700">필터 적용됨</span>
                </div>
              </Show>
              <button
                class={`flex items-center gap-2 px-4 py-2 rounded-lg font-medium text-sm transition-all ${
                  useFilteredInsights()
                    ? 'bg-gradient-to-r from-blue-500 to-purple-500 text-white shadow-md'
                    : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                }`}
                onClick={async () => {
                  const newValue = !useFilteredInsights();
                  setUseFilteredInsights(newValue);
                  if (newValue) {
                    await updateFilteredInsights();
                  }
                }}
              >
                <span>{useFilteredInsights() ? '✓' : '○'}</span>
                <span>{useFilteredInsights() ? '필터 적용 중' : '전체 데이터'}</span>
              </button>
            </div>
          </div>
          
          <Show when={!isInsightsCollapsed()}>
          <Show when={!ui.loadingSummary && s()} fallback={
            <div class="flex items-center justify-center py-12">
              <div class="text-sm text-gray-400 animate-pulse">📊 인사이트를 불러오는 중...</div>
            </div>
          }>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
              {/* Top 카테고리 통계 */}
              <div class="bg-gradient-to-br from-indigo-50 to-purple-50 border-2 border-indigo-200 rounded-xl p-5 shadow-sm">
                <div class="flex items-center gap-2 mb-4">
                  <span class="text-2xl">🏷️</span>
                  <div>
                    <div class="text-sm font-bold text-indigo-900">Top 카테고리</div>
                    <div class="text-xs text-indigo-600">{totalCategories()}개 {useFilteredInsights() && loadingFilteredInsights() ? '로딩...' : (useFilteredInsights() ? '(필터됨)' : '전체')}</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-indigo-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={topCategories()}>
                    {(c, idx) => {
                      const maxCount = topCategories()[0]?.[1] || 1;
                      const percentage = (c[1] / maxCount) * 100;
                      return (
                        <div class="group hover:bg-indigo-50 rounded p-2 transition-colors">
                          <div class="flex items-center justify-between mb-1">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <span class="text-indigo-400 text-[10px] font-mono w-6">#{idx() + 1}</span>
                              <span class="truncate font-medium text-xs text-gray-700" title={c[0]}>{c[0]}</span>
                            </div>
                            <span class="text-indigo-600 font-bold ml-2 text-xs">{c[1]}</span>
                          </div>
                          <div class="h-1 bg-gray-200 rounded-full overflow-hidden">
                            <div class="h-full bg-gradient-to-r from-indigo-500 to-purple-500 rounded-full transition-all" style={`width: ${percentage}%`}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>

              {/* Top 디바이스 타입 */}
              <div class="bg-gradient-to-br from-emerald-50 to-teal-50 border-2 border-emerald-200 rounded-xl p-5 shadow-sm">
                <div class="flex items-center gap-2 mb-4">
                  <span class="text-2xl">🔧</span>
                  <div>
                    <div class="text-sm font-bold text-emerald-900">Top 디바이스 타입</div>
                    <div class="text-xs text-emerald-600">총 {totalDeviceTypes()}개 타입 {useFilteredInsights() ? '(필터됨)' : '(전체)'}</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-emerald-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={topDeviceTypes().slice(0, 10) || []}>
                    {(dt, idx) => {
                      const maxCount = topDeviceTypes()[0]?.[1] || 1;
                      const percentage = (dt[1] / maxCount) * 100;
                      return (
                        <div class="group hover:bg-emerald-50 rounded p-2 transition-colors">
                          <div class="flex items-center justify-between mb-1">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <span class="text-emerald-400 text-[10px] font-mono w-6">#{idx() + 1}</span>
                              <span class="truncate font-medium text-xs text-gray-700" title={dt[0]}>{dt[0]}</span>
                            </div>
                            <span class="text-emerald-600 font-bold ml-2 text-xs">{dt[1]}</span>
                          </div>
                          <div class="h-1 bg-gray-200 rounded-full overflow-hidden">
                            <div class="h-full bg-gradient-to-r from-emerald-500 to-teal-500 rounded-full transition-all" style={`width: ${percentage}%`}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>

              {/* 주요 벤더 Top 10 */}
              <div class="bg-gradient-to-br from-blue-50 to-cyan-50 border-2 border-blue-200 rounded-xl p-5 shadow-sm">
                <div class="flex items-center gap-2 mb-4">
                  <span class="text-2xl">🏢</span>
                  <div>
                    <div class="text-sm font-bold text-blue-900">주요 벤더 Top 10</div>
                    <div class="text-xs text-blue-600">총 {totalVendors()}개 벤더 {useFilteredInsights() ? '(필터됨)' : '(전체)'}</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-blue-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={topVendors().slice(0, 10) || []}>
                    {(v, idx) => {
                      const maxCount = topVendors()[0]?.[1] || 1;
                      const percentage = (v[1] / maxCount) * 100;
                      return (
                        <div class="group hover:bg-blue-50 rounded p-2 transition-colors">
                          <div class="flex items-center justify-between mb-1">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <span class="text-blue-400 text-[10px] font-mono w-6">#{idx() + 1}</span>
                              <span class="truncate font-medium text-xs text-gray-700" title={v[0]}>{v[0]}</span>
                            </div>
                            <span class="text-blue-600 font-bold ml-2 text-xs">{v[1]}</span>
                          </div>
                          <div class="h-1 bg-gray-200 rounded-full overflow-hidden">
                            <div class="h-full bg-gradient-to-r from-blue-500 to-cyan-500 rounded-full transition-all" style={`width: ${percentage}%`}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>

              {/* Transport Interface Top 10 */}
              <div class="bg-gradient-to-br from-violet-50 to-purple-50 border-2 border-violet-200 rounded-xl p-5 shadow-sm">
                <div class="flex items-center gap-2 mb-4">
                  <span class="text-2xl">📡</span>
                  <div>
                    <div class="text-sm font-bold text-violet-900">Transport Interface</div>
                    <div class="text-xs text-violet-600">{totalTransport()}개 인터페이스 {useFilteredInsights() ? '(필터됨)' : '(전체)'}</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-violet-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={topTransport().slice(0, 10) || []}>
                    {(ti, idx) => {
                      const maxCount = topTransport()[0]?.[1] || 1;
                      const percentage = (ti[1] / maxCount) * 100;
                      return (
                        <div class="group hover:bg-violet-50 rounded p-2 transition-colors">
                          <div class="flex items-center justify-between mb-1">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <span class="text-violet-400 text-[10px] font-mono w-6">#{idx() + 1}</span>
                              <span class="truncate font-medium text-xs text-gray-700" title={ti[0]}>{ti[0]}</span>
                            </div>
                            <span class="text-violet-600 font-bold ml-2 text-xs">{ti[1]}</span>
                          </div>
                          <div class="h-1 bg-gray-200 rounded-full overflow-hidden">
                            <div class="h-full bg-gradient-to-r from-violet-500 to-purple-500 rounded-full transition-all" style={`width: ${percentage}%`}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>
            </div>
          </Show>
          </Show>
        </div>

        {/* Analytics + DSL Filter Placeholder */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 space-y-4">
          <div class="flex items-center justify-between mb-4">
            <button
              onClick={() => setIsAnalyticsCollapsed(!isAnalyticsCollapsed())}
              class="flex items-center gap-2 hover:text-indigo-600 transition-colors"
            >
              <h3 class="text-lg font-semibold text-gray-800 flex items-center gap-2">
                📊 Analytics 데이터
                <span class="text-gray-400 text-base">{isAnalyticsCollapsed() ? '▼' : '▲'}</span>
              </h3>
            </button>
            <div class="flex gap-2 items-center">
              <span class="text-xs text-gray-500">
                총 <span class="font-bold text-indigo-600">{analytics.total}</span>건
              </span>
            </div>
          </div>

          <Show when={!isAnalyticsCollapsed()}>
          {/* Quick Search + 필터 상태 표시 */}
          <div class="space-y-3">
            <div class="flex gap-2 items-center">
              <input
                type="text"
                class="flex-1 px-4 py-2 border-2 border-gray-200 rounded-lg text-sm focus:border-indigo-400 focus:outline-none transition-colors"
                placeholder="🔍 모델명, 벤더명, 카테고리 등 빠른 검색 (DSL 지원)..."
                value={ui.filterDraft || ''}
                onInput={e => localDbDashboardStore.setUi({ ...ui, filterDraft: e.currentTarget.value })}
                onKeyDown={e => { if (e.key === 'Enter') applyFilters(); }}
              />
              <button 
                class="px-5 py-2 rounded-lg bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-700 hover:to-purple-700 text-white text-sm font-semibold shadow-md transition-all disabled:opacity-50"
                onClick={() => applyFilters()} 
                disabled={analytics.loading}
              >
                검색
              </button>
              <button 
                class="px-4 py-2 rounded-lg bg-gray-200 hover:bg-gray-300 text-gray-700 text-sm font-medium"
                onClick={() => {
                  localDbDashboardStore.setUi({ ...ui, filterDraft: '', selectedCategories: [], selectedVendors: [], selectedDeviceTypes: [], selectedTransportInterfaces: [] });
                  localDbDashboardStore.resetFilter();
                }}
              >
                초기화
              </button>
            </div>

            {/* 현재 적용된 필터 표시 */}
            <Show when={analytics.filterApplied}>
              <div class="flex items-center gap-2 bg-indigo-50 border border-indigo-200 rounded-lg p-3">
                <span class="text-xs font-semibold text-indigo-700">✓ 적용된 필터:</span>
                <code class="text-xs text-indigo-900 bg-white px-2 py-1 rounded border border-indigo-200 flex-1 overflow-x-auto">
                  {analytics.filterApplied}
                </code>
              </div>
            </Show>

            {/* 필터 에러 표시 */}
            <Show when={analytics.filterError}>
              <div class="bg-red-50 border border-red-200 text-red-700 p-3 rounded-lg text-xs">
                <span class="font-semibold">❌ 필터 오류:</span> {analytics.filterError}
              </div>
            </Show>
          </div>

          {/* Analytics 테이블 */}
          <div class="overflow-auto max-h-[360px] border rounded">
            <table class="w-full text-xs">
              <thead class="bg-gray-50 sticky top-0">
                <tr>
                  {(() => {
                    const cols: { key: string; label: string }[] = [
                      { key: 'device_category', label: 'Category' },
                      { key: 'device_type_name', label: 'Device Type' },
                      { key: 'model', label: 'Model' },
                      { key: 'vendor_name', label: 'Vendor' },
                      { key: 'certification_date', label: 'Cert Date' },
                      { key: 'transport_interface', label: 'Transport IF' },
                    ];
                    const current = analytics.sort; // e.g. ['model:asc']
                    function cycle(col: string, multi: boolean) {
                      let order = [...current];
                      const idx = order.findIndex(o => o.startsWith(col + ':'));
                      const nextState = (prev?: string): string[] => {
                        // tri-state: none -> asc -> desc -> none
                        if (!prev) return [`${col}:asc`];
                        if (prev.endsWith(':asc')) return [`${col}:desc`];
                        return []; // remove
                      };
                      let replacement: string[] = [];
                      if (idx >= 0) {
                        replacement = nextState(order[idx]);
                        order.splice(idx, 1); // remove old
                      } else {
                        replacement = nextState(undefined);
                      }
                      if (replacement.length) {
                        if (!multi) order = []; // single sort if not multi
                        order.push(replacement[0]);
                      }
                      localDbDashboardStore.updateSort(order);
                      localDbDashboardStore.loadAnalytics(0);
                    }
                    function indicator(col: string) {
                      const idx = current.findIndex(o => o.startsWith(col + ':'));
                      if (idx < 0) return '';
                      const dir = current[idx].split(':')[1];
                      const n = idx + 1;
                      return dir === 'asc' ? `▲${current.length>1? n:''}` : `▼${current.length>1? n:''}`;
                    }
                    return (
                      <>
                        {cols.map(c => (
                          <th class="p-2 text-left select-none cursor-pointer group hover:bg-indigo-50 transition" onClick={e => cycle(c.key, e.altKey)} title={`클릭: 정렬 / Option+클릭: 다중정렬`}>
                            <span class="inline-flex items-center gap-1">
                              <span class="font-semibold">{c.label}</span>
                              <span class="text-sm text-indigo-600 font-bold min-w-[20px]">{indicator(c.key) || ''}</span>
                            </span>
                          </th>
                        ))}
                      </>
                    );
                  })()}
                </tr>
              </thead>
              <tbody>
                <Show when={!analytics.loading && analytics.rows.length === 0}>
                  <tr><td class="p-4 text-center text-gray-400" colSpan={6}>행 없음</td></tr>
                </Show>
                <For each={analytics.rows}>{r => (
                  <tr class="border-t border-gray-100 hover:bg-indigo-50 cursor-pointer transition" onClick={() => { setSelectedProduct(r); setShowProductModal(true); }}>
                    <td class="p-2 whitespace-nowrap">{r.device_category}</td>
                    <td class="p-2 whitespace-nowrap" title={r.device_type_name}>{r.device_type_name}</td>
                    <td class="p-2 max-w-[220px] truncate" title={r.model || r.product_detail_url}>
                      <button
                        onClick={async (e) => {
                          e.stopPropagation(); // 팝업 열리지 않도록 이벤트 전파 중단
                          if (r.product_detail_url) {
                            try {
                              await openUrl(r.product_detail_url);
                            } catch (err) {
                              console.error('Failed to open URL:', err);
                            }
                          }
                        }}
                        class="text-indigo-600 hover:underline text-left w-full"
                      >{r.model || '(no model)'}</button>
                    </td>
                    <td class="p-2 whitespace-nowrap" title={r.vendor_name}>{r.vendor_name}</td>
                    <td class="p-2 whitespace-nowrap">{r.certification_date}</td>
                    <td class="p-2 whitespace-nowrap">{(r.transport_interface || r.transport_if || '-') as any}</td>
                  </tr>
                )}</For>
              </tbody>
            </table>
            <Show when={analytics.loading}>
              <div class="p-6 text-center text-gray-500 text-sm">로딩 중...</div>
            </Show>
          </div>
          <div class="flex items-center gap-3 text-sm flex-wrap">
            <button class="px-2 py-1 rounded border hover:bg-gray-50" disabled={analytics.offset === 0 || analytics.loading} onClick={() => localDbDashboardStore.loadAnalytics(0)} title="처음으로">⏮️</button>
            <button class="px-2 py-1 rounded border hover:bg-gray-50" disabled={analytics.offset === 0 || analytics.loading} onClick={() => localDbDashboardStore.loadAnalytics(Math.max(0, analytics.offset - analytics.limit))}>이전</button>
            <div class="flex items-center gap-1">
              <span class="text-xs text-gray-500">페이지</span>
              <input
                type="number"
                class="w-16 px-2 py-1 border rounded text-center text-xs"
                value={Math.floor(analytics.offset / analytics.limit) + 1}
                min="1"
                max={totalPages()}
                onChange={e => {
                  const page = Math.max(1, Math.min(totalPages(), Number(e.currentTarget.value) || 1));
                  localDbDashboardStore.loadAnalytics((page - 1) * analytics.limit);
                }}
              />
              <span class="text-xs text-gray-500">/ {totalPages()}</span>
            </div>
            <button class="px-2 py-1 rounded border hover:bg-gray-50" disabled={analytics.loading || analytics.offset + analytics.limit >= analytics.total} onClick={() => localDbDashboardStore.loadAnalytics(analytics.offset + analytics.limit)}>다음</button>
            <button class="px-2 py-1 rounded border hover:bg-gray-50" disabled={analytics.loading || analytics.offset + analytics.limit >= analytics.total} onClick={() => localDbDashboardStore.loadAnalytics((totalPages() - 1) * analytics.limit)} title="마지막으로">⏭️</button>
            <select class="px-2 py-1 border rounded text-xs" value={analytics.limit} onChange={e => { localDbDashboardStore.setAnalytics({ ...analytics, limit: Number(e.currentTarget.value) }); localDbDashboardStore.loadAnalytics(0); }}>
              <option value="25">25개</option>
              <option value="50">50개</option>
              <option value="100">100개</option>
              <option value="200">200개</option>
            </select>
            <div class="ml-auto text-xs text-gray-400">총 {analytics.total} 행</div>
          </div>
          {/* Diagnostics UI 제거됨 */}
          </Show>
        </div>

        {/* Export & Data Management */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-indigo-200 p-6 space-y-4">
          <h3 class="text-lg font-semibold text-gray-800">📥 Export & 데이터 관리</h3>
          
          {/* Excel 백업/복원 섹션 */}
          <div class="bg-gradient-to-r from-blue-50 to-indigo-50 p-4 rounded-lg border border-blue-200">
            <h4 class="text-sm font-semibold text-blue-900 mb-3">💾 Excel 전체 백업/복원</h4>
            <div class="flex flex-wrap gap-2 mb-3">
              <button 
                class="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium shadow-sm transition-all disabled:bg-gray-400 disabled:cursor-not-allowed" 
                onClick={handleExcelExport}
                disabled={ui.working}
              >
                📊 전체 DB 내보내기 (저장 위치 선택)
              </button>
              <button 
                class="px-4 py-2 rounded-lg bg-green-600 hover:bg-green-700 text-white text-sm font-medium shadow-sm transition-all disabled:bg-gray-400 disabled:cursor-not-allowed" 
                onClick={handleExcelImport}
                disabled={ui.working}
              >
                📥 Excel 파일에서 복원 (파일 선택)
              </button>
            </div>
            <Show when={ui.exportStatus || ui.importStatus}>
              <div class="bg-white/80 rounded p-3 space-y-1">
                <Show when={ui.exportStatus}>
                  <div class="text-xs text-gray-700">
                    <span class="font-semibold">내보내기:</span> {ui.exportStatus}
                  </div>
                </Show>
                <Show when={ui.importStatus}>
                  <div class="text-xs text-gray-700">
                    <span class="font-semibold">가져오기:</span> {ui.importStatus}
                  </div>
                </Show>
                <Show when={ui.importLog}>
                  <pre class="text-[10px] bg-gray-100 p-2 rounded overflow-auto max-h-32 mt-2">{ui.importLog}</pre>
                </Show>
              </div>
            </Show>
          </div>
          
          {/* 레코드 삭제 섹션 (접을 수 있음) */}
          <div class="bg-gradient-to-r from-orange-50 to-red-50 p-4 rounded-lg border border-orange-300">
            <div class="flex items-center justify-between mb-3">
              <h4 class="text-sm font-semibold text-red-900">�️ 레코드 삭제</h4>
              <button
                class="px-3 py-1 rounded bg-orange-500 hover:bg-orange-600 text-white text-xs font-medium"
                onClick={() => setDeletesSectionExpanded(!deletesSectionExpanded())}
              >
                {deletesSectionExpanded() ? '▲ 접기' : '▼ 펼치기'}
              </button>
            </div>
            
            <Show when={deletesSectionExpanded()}>
              <div class="space-y-4">
                {/* 페이지 범위 삭제 */}
                <div class="bg-white/60 p-3 rounded border border-yellow-200">
                  <h5 class="text-sm font-semibold text-orange-800 mb-2">� 페이지 범위 삭제 (page_id 기준)</h5>
                  <div class="bg-blue-50 border border-blue-200 rounded p-2 mb-3 text-xs text-blue-800">
                    <strong>ℹ️ 중요:</strong> 물리 페이지가 아닌 <strong>page_id</strong> 기준으로 삭제합니다.<br/>
                    <strong>큰 값일수록 최신 데이터</strong>입니다. 
                    <Show when={maxPageId()}>
                      (현재 최대: {maxPageId()})
                    </Show>
                    <br/>
                    page_id는 0부터 시작 가능, 기본값은 최신 20페이지, 최대 100페이지까지 설정 가능합니다.
                  </div>
                  <div class="space-y-3">
                    <div class="flex gap-3 items-center flex-wrap">
                      <label class="flex items-center gap-2 text-sm">
                        <span class="text-gray-700">시작:</span>
                        <input
                          type="number"
                          value={deleteFromPage()}
                          onInput={(e) => setDeleteFromPage(parseInt(e.currentTarget.value) || 0)}
                          min="0"
                          max={maxPageId() || undefined}
                          class="border rounded px-2 py-1 w-24 text-sm"
                        />
                      </label>
                      <label class="flex items-center gap-2 text-sm">
                        <span class="text-gray-700">종료:</span>
                        <input
                          type="number"
                          value={deleteToPage()}
                          onInput={(e) => setDeleteToPage(parseInt(e.currentTarget.value) || 0)}
                          min="0"
                          max={maxPageId() || undefined}
                          class="border rounded px-2 py-1 w-24 text-sm"
                        />
                      </label>
                      <Show when={maxPageId()}>
                        <button
                          class="px-2 py-1 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs"
                          onClick={() => {
                            const max = maxPageId()!;
                            setDeleteFromPage(max > 19 ? max - 19 : 1);
                            setDeleteToPage(max);
                          }}
                          title="최신 20페이지 범위로 설정"
                        >
                          📌 최신 20
                        </button>
                      </Show>
                    </div>
                    <div class="flex gap-2">
                      <button 
                        class="px-3 py-1.5 rounded bg-yellow-600 hover:bg-yellow-700 text-white text-xs font-medium" 
                        onClick={handlePreviewDelete}
                      >
                        🔍 미리보기
                      </button>
                      <button 
                        class="px-3 py-1.5 rounded bg-red-600 hover:bg-red-700 text-white text-xs font-medium disabled:bg-gray-400 disabled:cursor-not-allowed" 
                        onClick={handleExecuteDelete}
                        disabled={!ui.deletePreview}
                      >
                        🗑️ 삭제 실행
                      </button>
                    </div>
                    <Show when={ui.deletePreview}>
                      <div class="bg-white/80 rounded p-3 text-xs space-y-1">
                        <div class="font-semibold text-gray-800">삭제 미리보기:</div>
                        <div>• 제품: {ui.deletePreview.products_count}개</div>
                        <div>• 상세정보: {ui.deletePreview.product_details_count}개</div>
                      </div>
                    </Show>
                    <Show when={ui.deleteResult && !ui.deleteResult.error}>
                      <div class="bg-green-100 rounded p-3 text-xs space-y-1">
                        <div class="font-semibold text-green-800">✅ 삭제 완료 (다음 범위로 자동 이동)</div>
                        <div>• 제품: {ui.deleteResult.deleted_products}개</div>
                        <div>• 상세정보: {ui.deleteResult.deleted_product_details}개</div>
                      </div>
                    </Show>
                  </div>
                </div>
                
                {/* 전체 삭제 */}
                <div class="bg-white/60 p-3 rounded border border-red-300">
                  <h5 class="text-sm font-semibold text-red-900 mb-2">⚠️ 전체 레코드 삭제</h5>
                  <p class="text-xs text-red-700 mb-3">모든 제품 데이터를 삭제합니다. 자동 백업이 생성됩니다.</p>
                  <button 
                    class="px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 text-white text-sm font-medium shadow-sm transition-all disabled:bg-gray-400 disabled:cursor-not-allowed" 
                    onClick={handleDeleteAll}
                    disabled={ui.working}
                  >
                    🗑️ 전체 데이터 삭제 (주의!)
                  </button>
                  <Show when={ui.deleteResult?.message}>
                    <pre class="text-xs bg-white/80 p-3 rounded mt-3 whitespace-pre-wrap">{ui.deleteResult.message}</pre>
                  </Show>
                  <Show when={ui.deleteResult?.cancelled}>
                    <div class="text-xs text-yellow-600 mt-2">취소되었습니다.</div>
                  </Show>
                  <Show when={ui.deleteResult?.error}>
                    <div class="text-xs text-red-600 mt-2">오류: {ui.deleteResult.error}</div>
                  </Show>
                </div>
              </div>
            </Show>
          </div>
        </div>

        {/* Vendor Sync */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">Vendor Sync</h3>
          <div class="flex items-center gap-3 flex-wrap">
            <button class={`px-4 py-2 rounded-lg text-white ${vendorSyncLoading() ? 'bg-gray-400 cursor-not-allowed' : 'bg-slate-500 hover:bg-slate-600'}`} disabled={vendorSyncLoading()} onClick={() => localDbDashboardStore.vendorDryRun()}>Dry Run</button>
            <button class={`px-4 py-2 rounded-lg text-white ${vendorSyncLoading() ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'}`} disabled={vendorSyncLoading()} onClick={syncVendors}>
              {vendorSyncLoading() ? '동기화 중...' : '벤더 동기화 실행'}
            </button>
            <Show when={vendorSyncResult()}>
              <span class="text-xs text-gray-600">완료: +{vendorSyncResult()!.inserted} / upd {vendorSyncResult()!.updated} / skip {vendorSyncResult()!.skipped} (총 {vendorSyncResult()!.final_count})</span>
            </Show>
            <Show when={localDbDashboardStore.ui.vendorDryRun}>
              <span class="text-xs text-indigo-600">dry_run: API {localDbDashboardStore.ui.vendorDryRun.api_total} / local {localDbDashboardStore.ui.vendorDryRun.local_count} / will_sync {String(localDbDashboardStore.ui.vendorDryRun.will_sync)}</span>
            </Show>
            <Show when={localDbDashboardStore.ui.vendorResult?.progress}>
              <span class="text-[10px] px-2 py-0.5 rounded bg-indigo-100 text-indigo-700">{localDbDashboardStore.ui.vendorResult.progress}</span>
            </Show>
          </div>
        </div>

        {/* Device Types Table Editor - 개선된 테이블 뷰 */}
        <details class="bg-white/90 backdrop-blur-sm border border-slate-200 rounded-2xl shadow-lg overflow-hidden">
          <summary class="cursor-pointer p-4 hover:bg-slate-100 transition select-none bg-gradient-to-r from-slate-50 to-gray-100">
            <div class="flex items-center justify-between">
              <div>
                <h3 class="text-lg font-semibold text-slate-800 flex items-center gap-2">
                  🔧 Device Types 관리
                  <span class="text-xs px-2 py-1 bg-emerald-100 text-emerald-700 rounded-full font-medium">테이블 뷰</span>
                </h3>
                <p class="text-xs text-gray-500 mt-1">Matter Device Types를 테이블 형식으로 추가/수정/삭제/정렬</p>
              </div>
              <span class="text-xs text-slate-600">클릭하여 펼치기/접기</span>
            </div>
          </summary>
          <div class="p-6 bg-white border-t border-slate-200">
            <div class="flex items-center gap-3 text-xs text-gray-500 mb-4 p-3 bg-blue-50 rounded-lg border border-blue-200">
              <span>📄 파일 개수: <b class="text-blue-700">{localDbDashboardStore.ui.deviceTypesMeta?.count_in_file || 0}</b></span>
              <span>💾 DB 개수: <b class="text-blue-700">{localDbDashboardStore.ui.deviceTypesMeta?.count_in_db || 0}</b></span>
              <Show when={localDbDashboardStore.ui.deviceTypesSaveResult}>
                <span class="ml-auto text-emerald-600 font-medium">
                  ✅ 마지막 저장: 
                  ins {localDbDashboardStore.ui.deviceTypesSaveResult?.inserted || 0} / 
                  upd {localDbDashboardStore.ui.deviceTypesSaveResult?.updated || 0} / 
                  skip {localDbDashboardStore.ui.deviceTypesSaveResult?.skipped || 0}
                </span>
              </Show>
            </div>
            
            <DeviceTypeTableEditor
              deviceTypes={(() => {
                try {
                  const parsed = JSON.parse(localDbDashboardStore.ui.deviceTypesJson || '[]');
                  return Array.isArray(parsed) ? parsed : [];
                } catch {
                  return [];
                }
              })()}
              onSave={(updatedTypes) => {
                const jsonStr = JSON.stringify(updatedTypes, null, 2);
                localDbDashboardStore.updateDeviceTypesJson(jsonStr);
                localDbDashboardStore.saveDeviceTypes();
              }}
              onReload={() => localDbDashboardStore.initDeviceTypes()}
            />

            <Show when={localDbDashboardStore.ui.deviceTypesDiff?.parseError}>
              <div class="mt-4 p-3 bg-rose-50 border border-rose-200 rounded-lg">
                <div class="text-sm text-rose-600 font-medium">⚠️ Parse Error</div>
                <div class="text-xs text-rose-700 mt-1">{localDbDashboardStore.ui.deviceTypesDiff?.parseError}</div>
              </div>
            </Show>
          </div>
        </details>

        {/* 제품 상세 모달 */}
        <Show when={showProductModal()}>
          <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={() => setShowProductModal(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-2xl w-full max-h-[80vh] overflow-auto" onClick={e => e.stopPropagation()}>
              <div class="sticky top-0 bg-white border-b px-6 py-4 flex items-center justify-between">
                <h3 class="text-xl font-bold text-gray-800">📝 제품 상세 정보</h3>
                <button class="text-gray-400 hover:text-gray-600 text-2xl" onClick={() => setShowProductModal(false)}>&times;</button>
              </div>
              <div class="p-6 space-y-4">
                <Show when={selectedProduct()}>
                  {(product) => (
                    <>
                      <div class="grid grid-cols-2 gap-4">
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Model</div>
                          <div class="font-semibold text-gray-800">{product().model || '(no model)'}</div>
                        </div>
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Vendor</div>
                          <div class="font-semibold text-gray-800">{product().vendor_name}</div>
                        </div>
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Category</div>
                          <div class="font-semibold text-gray-800">{product().device_category}</div>
                        </div>
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Device Type</div>
                          <div class="font-semibold text-gray-800">{product().device_type_name}</div>
                        </div>
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Certification Date</div>
                          <div class="font-semibold text-gray-800">{product().certification_date}</div>
                        </div>
                        <div>
                          <div class="text-xs text-gray-500 mb-1">Transport Interface</div>
                          <div class="font-semibold text-gray-800">{product().transport_interface || product().transport_if || '-'}</div>
                        </div>
                        <div class="col-span-2">
                          <div class="text-xs text-gray-500 mb-1">Product URL</div>
                          <button 
                            onClick={async () => {
                              const url = product().product_detail_url;
                              if (url) {
                                try {
                                  await openUrl(url);
                                } catch (err) {
                                  console.error('Failed to open URL:', err);
                                }
                              }
                            }}
                            class="text-indigo-600 hover:underline text-sm break-all text-left cursor-pointer hover:bg-indigo-50 px-2 py-1 rounded transition"
                          >
                            {product().product_detail_url}
                          </button>
                        </div>
                      </div>
                      <div class="pt-4 border-t">
                        <div class="text-xs text-gray-500 mb-2">Full Data (JSON)</div>
                        <pre class="bg-gray-900 text-green-300 text-[10px] p-3 rounded max-h-60 overflow-auto">{JSON.stringify(product(), null, 2)}</pre>
                      </div>
                    </>
                  )}
                </Show>
              </div>
              <div class="sticky bottom-0 bg-gray-50 border-t px-6 py-3 flex justify-end">
                <button class="px-4 py-2 rounded bg-gray-200 hover:bg-gray-300 text-gray-800" onClick={() => setShowProductModal(false)}>닫기</button>
              </div>
            </div>
          </div>
        </Show>

        {/* 카테고리 필터 다이얼로그 */}
        <Show when={showCategoryDialog()}>
          <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" onClick={() => setShowCategoryDialog(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-3xl w-full max-h-[85vh] overflow-hidden" onClick={e => e.stopPropagation()}>
              <div class="sticky top-0 bg-gradient-to-r from-indigo-500 to-purple-600 px-6 py-4 flex items-center justify-between">
                <h3 class="text-xl font-bold text-white">🏷️ 카테고리 필터 선택</h3>
                <button class="text-white hover:text-gray-200 text-3xl font-light" onClick={() => setShowCategoryDialog(false)}>&times;</button>
              </div>
              
              <div class="p-6">
                <div class="flex items-center justify-between mb-4">
                  <div class="text-sm text-gray-600">
                    선택됨: <span class="font-bold text-indigo-600">{ui.selectedCategories.length}</span> / {(s()?.all_device_categories?.length || 0) + 1}
                  </div>
                  <div class="flex gap-2">
                    <button 
                      class="px-3 py-1.5 bg-indigo-100 hover:bg-indigo-200 text-indigo-700 rounded font-medium text-sm"
                      onClick={() => {
                        const currentUi = localDbDashboardStore.ui;
                        // 일반 카테고리만 비교 (null 제외)
                        const normalCats = s()?.all_device_categories?.map(c => c[0]) || [];
                        const currentNormal = currentUi.selectedCategories.filter(c => c !== 'null');
                        const isAllNormalSelected = currentNormal.length === normalCats.length && normalCats.length > 0;
                        
                        // 토글: 전체 선택 상태면 전부 해제, 아니면 일반 카테고리 전체 선택 (null 제외)
                        const newSelected = isAllNormalSelected ? [] : normalCats;
                        
                        console.log('[전체 선택 버튼] 클릭:', {
                          currentSelected: currentUi.selectedCategories,
                          normalCatsCount: normalCats.length,
                          currentNormalCount: currentNormal.length,
                          isAllNormalSelected,
                          newSelected
                        });
                        localDbDashboardStore.setUi({ ...currentUi, selectedCategories: newSelected });
                      }}
                    >
                      {(() => {
                        const normalCats = s()?.all_device_categories?.map(c => c[0]) || [];
                        const currentNormal = ui.selectedCategories.filter(c => c !== 'null');
                        return currentNormal.length === normalCats.length && normalCats.length > 0 ? '전체 해제' : '전체 선택';
                      })()}
                    </button>
                    <button 
                      class="px-3 py-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded font-medium text-sm"
                      onClick={() => {
                        const currentUi = localDbDashboardStore.ui;
                        console.log('[초기화 버튼] 클릭');
                        localDbDashboardStore.setUi({ ...currentUi, selectedCategories: [] });
                      }}
                    >
                      초기화
                    </button>
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-3 overflow-y-auto max-h-[500px] pr-2" style="scrollbar-width: thin;">
                  {/* null 카테고리 (카테고리 없음) */}
                  <div 
                    class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                      ui.selectedCategories.includes('null') 
                        ? 'bg-amber-100 border-amber-400 shadow-md' 
                        : 'bg-gray-50 border-gray-200 hover:border-gray-300 hover:bg-gray-100'
                    }`}
                    onClick={() => {
                      const currentUi = localDbDashboardStore.ui;
                      const isSelected = currentUi.selectedCategories.includes('null');
                      let newSelected: string[];
                      
                      if (isSelected) {
                        // null 체크 해제
                        newSelected = currentUi.selectedCategories.filter(cat => cat !== 'null');
                      } else {
                        // null 체크: 다른 카테고리 모두 해제하고 null만 선택
                        newSelected = ['null'];
                        console.log('[null 카테고리] OR 미지원으로 null 선택 시 다른 카테고리 모두 해제');
                      }
                      
                      console.log('[null 카테고리 체크박스] 클릭:', { isSelected, resulting: newSelected });
                      localDbDashboardStore.setUi({ ...currentUi, selectedCategories: newSelected });
                    }}
                  >
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-2 flex-1 min-w-0">
                        <input 
                          type="checkbox" 
                          checked={ui.selectedCategories.includes('null')} 
                          class="pointer-events-none w-4 h-4"
                        />
                        <span class="text-sm font-semibold text-amber-700" title="카테고리 정보 없음">
                          📦 (카테고리 없음)
                        </span>
                      </div>
                    </div>
                  </div>
                  
                  {/* 실제 카테고리들 */}
                  <For each={s()?.all_device_categories || []}>
                    {(c, idx) => {
                      const isSelected = () => ui.selectedCategories.includes(c[0]);
                      // 옵션 배열이 비어있으면(초기 로딩 중) 모든 옵션을 활성화
                      const isAvailable = () => {
                        const available = availableOptions().categories;
                        return available.length === 0 || available.includes(c[0]);
                      };
                      const maxCount = s()!.top_device_categories[0]?.[1] || 1;
                      const percentage = (c[1] / maxCount) * 100;
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            !isAvailable() && !isSelected()
                              ? 'bg-gray-100 border-gray-200 opacity-50'
                              : isSelected() 
                                ? 'bg-indigo-100 border-indigo-400 shadow-md' 
                                : 'bg-white border-gray-200 hover:border-indigo-300 hover:bg-indigo-50'
                          }`}
                          onClick={() => {
                            if (!isAvailable() && !isSelected()) {
                              console.log('[카테고리] 비활성화된 옵션 클릭 무시:', c[0]);
                              return;
                            }
                            const currentUi = localDbDashboardStore.ui;
                            let newSelected: string[];
                            
                            if (isSelected()) {
                              // 체크 해제
                              newSelected = currentUi.selectedCategories.filter(cat => cat !== c[0]);
                            } else {
                              // 체크: null이 있으면 제거
                              newSelected = currentUi.selectedCategories.filter(cat => cat !== 'null');
                              newSelected.push(c[0]);
                              if (currentUi.selectedCategories.includes('null')) {
                                console.log('[일반 카테고리] OR 미지원으로 일반 카테고리 선택 시 null 제거');
                              }
                            }
                            console.log('[카테고리 체크박스] 클릭:', { 
                              category: c[0], 
                              wasSelected: isSelected(),
                              currentSelected: currentUi.selectedCategories,
                              resulting: newSelected 
                            });
                            localDbDashboardStore.setUi({ ...currentUi, selectedCategories: newSelected });
                          }}
                        >
                          <div class="flex items-center justify-between mb-2">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <input 
                                type="checkbox" 
                                checked={isSelected()} 
                                disabled={!isAvailable() && !isSelected()}
                                class="pointer-events-none w-4 h-4"
                              />
                              <span class="text-gray-400 text-xs font-mono">#{idx() + 1}</span>
                              <span class={`text-sm font-semibold truncate ${!isAvailable() && !isSelected() ? 'text-gray-400' : 'text-gray-800'}`} title={c[0]}>
                                {c[0]}
                              </span>
                              <Show when={!isAvailable() && !isSelected()}>
                                <span class="text-xs text-gray-400 italic">(다른 필터로 제외됨)</span>
                              </Show>
                            </div>
                            <span class={`font-bold ml-2 text-sm ${!isAvailable() && !isSelected() ? 'text-gray-400' : 'text-indigo-600'}`}>{c[1]}</span>
                          </div>
                          <div class="h-1.5 bg-gray-200 rounded-full overflow-hidden">
                            <div 
                              class={`h-full rounded-full transition-all ${!isAvailable() && !isSelected() ? 'bg-gray-300' : 'bg-gradient-to-r from-indigo-500 to-purple-500'}`}
                              style={`width: ${percentage}%`}
                            ></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>
              
              <div class="sticky bottom-0 bg-gray-50 border-t px-6 py-4 flex justify-between items-center">
                <div class="text-sm text-gray-600">
                  <span class="font-semibold text-indigo-600">{ui.selectedCategories.length}</span>개 카테고리 선택됨
                </div>
                <div class="flex gap-2">
                  <button 
                    class="px-4 py-2 rounded bg-gray-200 hover:bg-gray-300 text-gray-800 font-medium"
                    onClick={() => setShowCategoryDialog(false)}
                  >
                    취소
                  </button>
                  <button 
                    class="px-4 py-2 rounded bg-gradient-to-r from-indigo-500 to-purple-600 hover:from-indigo-600 hover:to-purple-700 text-white font-semibold shadow-md"
                    onClick={() => {
                      console.log('[카테고리 적용 버튼] 클릭됨! 현재 선택:', ui.selectedCategories);
                      setShowCategoryDialog(false);
                      applyFilters();
                    }}
                  >
                    적용
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* 벤더 필터 다이얼로그 */}
        <Show when={showVendorDialog()}>
          <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" onClick={() => setShowVendorDialog(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-3xl w-full max-h-[85vh] overflow-hidden" onClick={e => e.stopPropagation()}>
              <div class="sticky top-0 bg-gradient-to-r from-blue-500 to-cyan-600 px-6 py-4 flex items-center justify-between">
                <h3 class="text-xl font-bold text-white">🏢 벤더 필터 선택</h3>
                <button class="text-white hover:text-gray-200 text-3xl font-light" onClick={() => setShowVendorDialog(false)}>&times;</button>
              </div>
              
              <div class="p-6">
                <div class="flex items-center justify-between mb-4">
                  <div class="text-sm text-gray-600">
                    선택됨: <span class="font-bold text-blue-600">{ui.selectedVendors.length}</span> / {s()?.top_vendors?.length || 0}
                  </div>
                  <div class="flex gap-2">
                    <button 
                      class="px-3 py-1.5 bg-blue-100 hover:bg-blue-200 text-blue-700 rounded font-medium text-sm"
                      onClick={() => {
                        const allVendors = s()?.all_vendors?.map(v => v[0]) || [];
                        const isAllSelected = ui.selectedVendors.length === allVendors.length;
                        localDbDashboardStore.setUi({ ...ui, selectedVendors: isAllSelected ? [] : allVendors });
                      }}
                    >
                      {ui.selectedVendors.length === (s()?.all_vendors?.length || 0) ? '전체 해제' : '전체 선택'}
                    </button>
                    <button 
                      class="px-3 py-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded font-medium text-sm"
                      onClick={() => localDbDashboardStore.setUi({ ...ui, selectedVendors: [] })}
                    >
                      초기화
                    </button>
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-3 overflow-y-auto max-h-[500px] pr-2" style="scrollbar-width: thin;">
                  <For each={(() => {
                    // 전체 벤더 목록을 유효한 옵션 우선으로 정렬
                    const vendors = s()?.all_vendors || [];
                    const available = availableOptions().vendors;
                    
                    // 옵션이 로딩 중이면 원본 순서 유지
                    if (available.length === 0) return vendors;
                    
                    // 유효한 벤더와 비유효한 벤더 분리
                    const availableVendors = vendors.filter(v => available.includes(v[0]));
                    const unavailableVendors = vendors.filter(v => !available.includes(v[0]));
                    
                    // 유효한 벤더를 먼저, 그 다음 비유효한 벤더
                    return [...availableVendors, ...unavailableVendors];
                  })()}>
                    {(v, idx) => {
                      const isSelected = () => ui.selectedVendors.includes(v[0]);
                      const isAvailable = () => {
                        const available = availableOptions().vendors;
                        return available.length === 0 || available.includes(v[0]);
                      };
                      const maxCount = s()?.all_vendors?.[0]?.[1] || 1;
                      const percentage = (v[1] / maxCount) * 100;
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            !isAvailable() && !isSelected()
                              ? 'bg-gray-100 border-gray-200 opacity-50'
                              : isSelected() 
                                ? 'bg-blue-100 border-blue-400 shadow-md' 
                                : 'bg-white border-gray-200 hover:border-blue-300 hover:bg-blue-50'
                          }`}
                          onClick={() => {
                            if (!isAvailable() && !isSelected()) {
                              console.log('[벤더] 비활성화된 옵션 클릭 무시:', v[0]);
                              return;
                            }
                            const newSelected = isSelected() 
                              ? ui.selectedVendors.filter(vendor => vendor !== v[0])
                              : [...ui.selectedVendors, v[0]];
                            localDbDashboardStore.setUi({ ...ui, selectedVendors: newSelected });
                          }}
                        >
                          <div class="flex items-center justify-between mb-2">
                            <div class="flex items-center gap-2 flex-1 min-w-0">
                              <input 
                                type="checkbox" 
                                checked={isSelected()}
                                disabled={!isAvailable() && !isSelected()}
                                class="pointer-events-none w-4 h-4"
                              />
                              <span class="text-gray-400 text-xs font-mono">#{idx() + 1}</span>
                              <span class={`text-sm font-semibold truncate ${!isAvailable() && !isSelected() ? 'text-gray-400' : 'text-gray-800'}`} title={v[0]}>
                                {v[0]}
                              </span>
                              <Show when={!isAvailable() && !isSelected()}>
                                <span class="text-xs text-gray-400 italic">(제외됨)</span>
                              </Show>
                            </div>
                            <span class={`font-bold ml-2 text-sm ${!isAvailable() && !isSelected() ? 'text-gray-400' : 'text-blue-600'}`}>{v[1]}</span>
                          </div>
                          <div class="h-1.5 bg-gray-200 rounded-full overflow-hidden">
                            <div 
                              class={`h-full rounded-full transition-all ${!isAvailable() && !isSelected() ? 'bg-gray-300' : 'bg-gradient-to-r from-blue-500 to-cyan-500'}`}
                              style={`width: ${percentage}%`}
                            ></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>
              
              <div class="sticky bottom-0 bg-gray-50 border-t px-6 py-4 flex justify-between items-center">
                <div class="text-sm text-gray-600">
                  <span class="font-semibold text-blue-600">{ui.selectedVendors.length}</span>개 벤더 선택됨
                </div>
                <div class="flex gap-2">
                  <button 
                    class="px-4 py-2 rounded bg-gray-200 hover:bg-gray-300 text-gray-800 font-medium"
                    onClick={() => setShowVendorDialog(false)}
                  >
                    취소
                  </button>
                  <button 
                    class="px-4 py-2 rounded bg-gradient-to-r from-blue-500 to-cyan-600 hover:from-blue-600 hover:to-cyan-700 text-white font-semibold shadow-md"
                    onClick={() => {
                      setShowVendorDialog(false);
                      applyFilters();
                    }}
                  >
                    적용
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* 디바이스 타입 필터 다이얼로그 */}
        <Show when={showDeviceTypeDialog()}>
          <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" onClick={() => setShowDeviceTypeDialog(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-3xl w-full max-h-[85vh] overflow-hidden" onClick={e => e.stopPropagation()}>
              <div class="sticky top-0 bg-gradient-to-r from-emerald-500 to-teal-600 px-6 py-4 flex items-center justify-between">
                <h3 class="text-xl font-bold text-white">🔧 디바이스 타입 필터 선택</h3>
                <button class="text-white hover:text-gray-200 text-3xl font-light" onClick={() => setShowDeviceTypeDialog(false)}>&times;</button>
              </div>
              
              <div class="p-6">
                <div class="flex items-center justify-between mb-4">
                  <div class="text-sm text-gray-600">
                    선택됨: <span class="font-bold text-emerald-600">{ui.selectedDeviceTypes.length}</span> / {s()?.all_device_type_names?.length || 0}
                  </div>
                  <div class="flex gap-2">
                    <button 
                      class="px-3 py-1.5 bg-emerald-100 hover:bg-emerald-200 text-emerald-700 rounded font-medium text-sm"
                      onClick={() => {
                        const allTypes = s()?.all_device_type_names || [];
                        const isAllSelected = ui.selectedDeviceTypes.length === allTypes.length;
                        localDbDashboardStore.setUi({ ...ui, selectedDeviceTypes: isAllSelected ? [] : allTypes });
                      }}
                    >
                      {ui.selectedDeviceTypes.length === (s()?.all_device_type_names?.length || 0) ? '전체 해제' : '전체 선택'}
                    </button>
                    <button 
                      class="px-3 py-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded font-medium text-sm"
                      onClick={() => localDbDashboardStore.setUi({ ...ui, selectedDeviceTypes: [] })}
                    >
                      초기화
                    </button>
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-3 overflow-y-auto max-h-[500px] pr-2" style="scrollbar-width: thin;">
                  <For each={(() => {
                    // 1차: 카테고리 기반 정렬, 2차: 유효한 옵션 기반 정렬
                    const allTypes = s()?.all_device_type_names || [];
                    const categoryMapping = s()?.device_types_by_category || {};
                    const selectedCats = ui.selectedCategories.filter(c => c !== 'null');
                    const available = availableOptions().device_types;
                    
                    // 카테고리 기반 관련성 체크
                    const relevantTypes = new Set<string>();
                    if (selectedCats.length > 0) {
                      for (const cat of selectedCats) {
                        const types = categoryMapping[cat] || [];
                        types.forEach(t => relevantTypes.add(t));
                      }
                    }
                    
                    // 4개 그룹으로 분류
                    const group1: string[] = []; // 카테고리 관련 + 유효
                    const group2: string[] = []; // 카테고리 관련 + 비유효
                    const group3: string[] = []; // 카테고리 무관 + 유효
                    const group4: string[] = []; // 카테고리 무관 + 비유효
                    
                    for (const dtype of allTypes) {
                      const isRelevant = selectedCats.length === 0 || relevantTypes.has(dtype);
                      const isAvailable = available.length === 0 || available.includes(dtype);
                      
                      if (isRelevant && isAvailable) group1.push(dtype);
                      else if (isRelevant && !isAvailable) group2.push(dtype);
                      else if (!isRelevant && isAvailable) group3.push(dtype);
                      else group4.push(dtype);
                    }
                    
                    // 우선순위: 관련+유효 > 관련+비유효 > 무관+유효 > 무관+비유효
                    return [...group1, ...group2, ...group3, ...group4];
                  })()}>
                    {(dtype, idx) => {
                      const isSelected = () => ui.selectedDeviceTypes.includes(dtype);
                      const isRelevant = () => {
                        const selectedCats = ui.selectedCategories.filter(c => c !== 'null');
                        if (selectedCats.length === 0) return true;
                        const categoryMapping = s()?.device_types_by_category || {};
                        for (const cat of selectedCats) {
                          const types = categoryMapping[cat] || [];
                          if (types.includes(dtype)) return true;
                        }
                        return false;
                      };
                      const isAvailable = () => {
                        const available = availableOptions().device_types;
                        return available.length === 0 || available.includes(dtype);
                      };
                      
                      const canSelect = isAvailable() || isSelected();
                      
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            !canSelect
                              ? 'bg-gray-100 border-gray-200 opacity-50'
                              : isSelected() 
                                ? 'bg-emerald-100 border-emerald-400 shadow-md' 
                                : isRelevant()
                                  ? 'bg-white border-gray-200 hover:border-emerald-300 hover:bg-emerald-50'
                                  : 'bg-gray-50 border-gray-200 hover:border-gray-300'
                          }`}
                          onClick={() => {
                            if (!canSelect) {
                              console.log('[디바이스 타입] 비활성화된 옵션 클릭 무시:', dtype);
                              return;
                            }
                            const newSelected = isSelected() 
                              ? ui.selectedDeviceTypes.filter(dt => dt !== dtype)
                              : [...ui.selectedDeviceTypes, dtype];
                            localDbDashboardStore.setUi({ ...ui, selectedDeviceTypes: newSelected });
                          }}
                        >
                          <div class="flex items-center gap-2">
                            <input 
                              type="checkbox" 
                              checked={isSelected()}
                              disabled={!canSelect}
                              class="pointer-events-none w-4 h-4"
                            />
                            <span class={`text-xs font-mono ${canSelect ? (isRelevant() ? 'text-gray-400' : 'text-gray-300') : 'text-gray-200'}`}>#{idx() + 1}</span>
                            <span class={`text-sm font-semibold truncate ${!canSelect ? 'text-gray-400' : isRelevant() ? 'text-gray-800' : 'text-gray-500'}`} title={dtype}>
                              {dtype}
                            </span>
                            <Show when={!isRelevant() && canSelect}>
                              <span class="ml-auto text-[10px] text-gray-400">∅</span>
                            </Show>
                            <Show when={!canSelect}>
                              <span class="ml-auto text-[10px] text-gray-400 italic">(제외됨)</span>
                            </Show>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>
              
              <div class="sticky bottom-0 bg-gray-50 border-t px-6 py-4 flex justify-between items-center">
                <div class="text-sm text-gray-600">
                  <span class="font-semibold text-emerald-600">{ui.selectedDeviceTypes.length}</span>개 디바이스 타입 선택됨
                </div>
                <div class="flex gap-2">
                  <button 
                    class="px-4 py-2 rounded bg-gray-200 hover:bg-gray-300 text-gray-800 font-medium"
                    onClick={() => setShowDeviceTypeDialog(false)}
                  >
                    취소
                  </button>
                  <button 
                    class="px-4 py-2 rounded bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 text-white font-semibold shadow-md"
                    onClick={() => {
                      setShowDeviceTypeDialog(false);
                      applyFilters();
                    }}
                  >
                    적용
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* Transport Interface 필터 다이얼로그 */}
        <Show when={showTransportInterfaceDialog()}>
          <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" onClick={() => setShowTransportInterfaceDialog(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-3xl w-full max-h-[85vh] overflow-hidden" onClick={e => e.stopPropagation()}>
              <div class="sticky top-0 bg-gradient-to-r from-violet-500 to-purple-600 px-6 py-4 flex items-center justify-between">
                <h3 class="text-xl font-bold text-white">📡 Transport Interface 필터 선택</h3>
                <button class="text-white hover:text-gray-200 text-3xl font-light" onClick={() => setShowTransportInterfaceDialog(false)}>&times;</button>
              </div>
              
              <div class="p-6">
                <div class="flex items-center justify-between mb-4">
                  <div class="text-sm text-gray-600">
                    선택됨: <span class="font-bold text-violet-600">{ui.selectedTransportInterfaces.length}</span> / {s()?.all_transport_interfaces?.length || 0}
                  </div>
                  <div class="flex gap-2">
                    <button 
                      class="px-3 py-1.5 bg-violet-100 hover:bg-violet-200 text-violet-700 rounded font-medium text-sm"
                      onClick={() => {
                        const allTIs = s()?.all_transport_interfaces || [];
                        const isAllSelected = ui.selectedTransportInterfaces.length === allTIs.length;
                        localDbDashboardStore.setUi({ ...ui, selectedTransportInterfaces: isAllSelected ? [] : allTIs });
                      }}
                    >
                      {ui.selectedTransportInterfaces.length === (s()?.all_transport_interfaces?.length || 0) ? '전체 해제' : '전체 선택'}
                    </button>
                    <button 
                      class="px-3 py-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 rounded font-medium text-sm"
                      onClick={() => localDbDashboardStore.setUi({ ...ui, selectedTransportInterfaces: [] })}
                    >
                      초기화
                    </button>
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-3 overflow-y-auto max-h-[500px] pr-2" style="scrollbar-width: thin;">
                  <For each={(() => {
                    // Transport Interface를 유효한 옵션 우선으로 정렬
                    const allTIs = s()?.all_transport_interfaces || [];
                    const available = availableOptions().transport_interfaces;
                    
                    // 옵션이 로딩 중이면 원본 순서 유지
                    if (available.length === 0) return allTIs;
                    
                    // 유효한 TI와 비유효한 TI 분리
                    const availableTIs = allTIs.filter(ti => available.includes(ti));
                    const unavailableTIs = allTIs.filter(ti => !available.includes(ti));
                    
                    // 유효한 TI를 먼저, 그 다음 비유효한 TI
                    return [...availableTIs, ...unavailableTIs];
                  })()}>
                    {(ti, idx) => {
                      const isSelected = () => ui.selectedTransportInterfaces.includes(ti);
                      const isAvailable = () => {
                        const available = availableOptions().transport_interfaces;
                        return available.length === 0 || available.includes(ti);
                      };
                      
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            !isAvailable() && !isSelected()
                              ? 'bg-gray-100 border-gray-200 opacity-50'
                              : isSelected() 
                                ? 'bg-violet-100 border-violet-400 shadow-md' 
                                : 'bg-white border-gray-200 hover:border-violet-300 hover:bg-violet-50'
                          }`}
                          onClick={() => {
                            if (!isAvailable() && !isSelected()) {
                              console.log('[Transport Interface] 비활성화된 옵션 클릭 무시:', ti);
                              return;
                            }
                            const newSelected = isSelected() 
                              ? ui.selectedTransportInterfaces.filter(t => t !== ti)
                              : [...ui.selectedTransportInterfaces, ti];
                            localDbDashboardStore.setUi({ ...ui, selectedTransportInterfaces: newSelected });
                          }}
                        >
                          <div class="flex items-center gap-2">
                            <input 
                              type="checkbox" 
                              checked={isSelected()}
                              disabled={!isAvailable() && !isSelected()}
                              class="pointer-events-none w-4 h-4"
                            />
                            <span class={`text-xs font-mono ${!isAvailable() && !isSelected() ? 'text-gray-200' : 'text-gray-400'}`}>#{idx() + 1}</span>
                            <span class={`text-sm font-semibold truncate ${!isAvailable() && !isSelected() ? 'text-gray-400' : 'text-gray-800'}`} title={ti}>
                              {ti}
                            </span>
                            <Show when={!isAvailable() && !isSelected()}>
                              <span class="ml-auto text-xs text-gray-400 italic">(제외됨)</span>
                            </Show>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </div>
              
              <div class="sticky bottom-0 bg-gray-50 border-t px-6 py-4 flex justify-between items-center">
                <div class="text-sm text-gray-600">
                  <span class="font-semibold text-violet-600">{ui.selectedTransportInterfaces.length}</span>개 Transport Interface 선택됨
                </div>
                <div class="flex gap-2">
                  <button 
                    class="px-4 py-2 rounded bg-gray-200 hover:bg-gray-300 text-gray-800 font-medium"
                    onClick={() => setShowTransportInterfaceDialog(false)}
                  >
                    취소
                  </button>
                  <button 
                    class="px-4 py-2 rounded bg-gradient-to-r from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700 text-white font-semibold shadow-md"
                    onClick={() => {
                      setShowTransportInterfaceDialog(false);
                      applyFilters();
                    }}
                  >
                    적용
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};
