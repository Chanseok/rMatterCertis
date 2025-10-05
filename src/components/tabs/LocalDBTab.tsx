/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, For, onMount, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { localDbDashboardStore, initializeLocalDbDashboard } from '../../stores/localDbDashboardStore';
import { listen } from '@tauri-apps/api/event';
import type { VendorSyncResult } from '../../types/domain';
import { DateRangeSlider } from '../DateRangeSlider';

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

  onMount(() => {
    initializeLocalDbDashboard().catch(console.error);
    // Vendor sync progress events (coarse-grained)
    listen<any>('vendor_sync_progress', (evt) => {
      const p = evt.payload || {};
      localDbDashboardStore.setUi({ ...localDbDashboardStore.ui, vendorResult: { ...(localDbDashboardStore.ui.vendorResult||{}), progress: p.stage } });
    }).catch(()=>{});
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

    // 5. 기존 filterDraft (quick search)와 결합
    const quickSearch = ui.filterDraft?.trim();
    if (quickSearch) {
      filters.push(quickSearch);
    }

    // 5. 최종 필터 문자열 생성
    const finalFilter = filters.join(' AND ');
    
    console.log('[applyFilters] 최종 필터:', finalFilter);
    
    // 6. Store에 적용하고 Analytics 재로드
    localDbDashboardStore.applyFilter(finalFilter);
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

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 space-y-4">
          <h2 class="text-3xl font-bold bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">�️ 로컬DB 데이터 분석</h2>
          <p class="text-sm text-gray-600">인증 데이터를 필터링하고 분석하세요</p>
        </div>
        
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

        {/* 카테고리 & 디바이스 타입 & 벤더 필터 버튼 */}
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
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
              <div class="flex items-center justify-center gap-2">
                <span>카테고리 선택</span>
                <Show when={ui.selectedCategories.length > 0}>
                  <span class="bg-white text-indigo-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                    {ui.selectedCategories.length}
                  </span>
                </Show>
              </div>
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
                <div class="text-xs text-emerald-600">{s()?.total_device_types || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={() => setShowDeviceTypeDialog(true)}
            >
              <div class="flex items-center justify-center gap-2">
                <span>디바이스 타입 선택</span>
                <Show when={ui.selectedDeviceTypes.length > 0}>
                  <span class="bg-white text-emerald-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                    {ui.selectedDeviceTypes.length}
                  </span>
                </Show>
              </div>
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
                <div class="text-xs text-blue-600">{s()?.total_vendors || 0}개 항목</div>
              </div>
            </div>
            
            <button 
              class="w-full px-4 py-3 bg-gradient-to-r from-blue-500 to-cyan-600 hover:from-blue-600 hover:to-cyan-700 text-white rounded-lg font-semibold shadow-lg transition-all transform hover:scale-[1.02] active:scale-95"
              onClick={() => setShowVendorDialog(true)}
            >
              <div class="flex items-center justify-center gap-2">
                <span>벤더 선택</span>
                <Show when={ui.selectedVendors.length > 0}>
                  <span class="bg-white text-blue-600 rounded-full px-2.5 py-0.5 text-xs font-bold shadow-md">
                    {ui.selectedVendors.length}
                  </span>
                </Show>
              </div>
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
        </div>

        {/* Summary */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="flex items-center justify-between mb-6">
            <div>
              <h3 class="text-xl font-bold text-gray-800">📈 데이터 요약</h3>
              <p class="text-xs text-gray-500 mt-1">전체 데이터베이스 통계 현황</p>
              <Show when={analytics.loading}>
                <p class="text-[10px] text-indigo-500 mt-1 animate-pulse">⏳ 분석 데이터 로딩...</p>
              </Show>
              {/* 디버깅: 현재 적용된 필터 표시 */}
              <Show when={analytics.filterApplied}>
                <p class="text-xs text-red-600 mt-1 font-mono">🔍 필터: {analytics.filterApplied}</p>
              </Show>
              <p class="text-xs text-blue-600 mt-1 font-mono">📅 날짜 범위: {ui.certDateRange[0] || '없음'} ~ {ui.certDateRange[1] || '없음'}</p>
            </div>
            <div class="flex gap-2">
              <button 
                class="px-4 py-2 rounded-lg bg-gradient-to-r from-red-500 to-pink-600 hover:from-red-600 hover:to-pink-700 text-white text-sm font-semibold shadow-md transition-all"
                onClick={() => {
                  localDbDashboardStore.resetFilter();
                  localDbDashboardStore.setUi({ ...ui, certDateRange: ["", ""], selectedCategories: [], selectedVendors: [], selectedDeviceTypes: [] });
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
          
          <Show when={!ui.loadingSummary && s()} fallback={
            <div class="flex items-center justify-center py-12">
              <div class="text-sm text-gray-400 animate-pulse">📊 데이터를 불러오는 중...</div>
            </div>
          }>
            {/* 주요 지표 카드 */}
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              <div class="rounded-xl bg-gradient-to-br from-blue-50 to-blue-100 border-2 border-blue-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🏭</span>
                  <span class="text-xs font-semibold text-blue-700 bg-blue-200 px-2 py-1 rounded-full">TOTAL</span>
                </div>
                {(() => {
                  const filtered = analytics.total;
                  const total = s()!.total_products;
                  const pct = total ? (filtered / total * 100) : 0;
                  return (
                    <>
                      <div class="text-2xl font-bold text-blue-900">
                        <span class="text-blue-600">{filtered.toLocaleString()}</span>
                        <span class="text-lg text-blue-400 mx-1">/</span>
                        <span class="text-blue-800">{total.toLocaleString()}</span>
                      </div>
                      <div class="text-xs text-blue-700 mt-1">인증 제품 (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                    </>
                  );
                })()}
              </div>

              <div class="rounded-xl bg-gradient-to-br from-purple-50 to-purple-100 border-2 border-purple-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🏢</span>
                  <span class="text-xs font-semibold text-purple-700 bg-purple-200 px-2 py-1 rounded-full">VENDORS</span>
                </div>
                {(() => {
                  const totalVen = s()!.total_vendors;
                  return (
                    <>
                      <div class="text-2xl font-bold text-purple-900">
                        <span class="text-purple-600">{totalVen.toLocaleString()}</span>
                        <span class="text-lg text-purple-400 mx-1">/</span>
                        <span class="text-purple-800">{totalVen.toLocaleString()}</span>
                      </div>
                      <div class="text-xs text-purple-700 mt-1">벤더 수 (전체 범위 기준 동일)</div>
                    </>
                  );
                })()}
              </div>

              <div class="rounded-xl bg-gradient-to-br from-green-50 to-green-100 border-2 border-green-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">📋</span>
                  <span class="text-xs font-semibold text-green-700 bg-green-200 px-2 py-1 rounded-full">DETAILS</span>
                </div>
                {(() => {
                  const filtered = analytics.total;
                  const totalDetails = s()!.total_product_details;
                  const pct = totalDetails ? (filtered / totalDetails * 100) : 0;
                  return (
                    <>
                      <div class="text-2xl font-bold text-green-900">
                        <span class="text-green-600">{filtered.toLocaleString()}</span>
                        <span class="text-lg text-green-400 mx-1">/</span>
                        <span class="text-green-800">{totalDetails.toLocaleString()}</span>
                      </div>
                      <div class="text-xs text-green-700 mt-1">상세 정보 (필터링 / 전체 · {pct.toFixed(1)}%)</div>
                    </>
                  );
                })()}
              </div>

              <div class="rounded-xl bg-gradient-to-br from-amber-50 to-amber-100 border-2 border-amber-200 p-5 shadow-sm">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-3xl">🔧</span>
                  <span class="text-xs font-semibold text-amber-700 bg-amber-200 px-2 py-1 rounded-full">TYPES</span>
                </div>
                <div class="text-2xl font-bold text-amber-900">
                  <span class="text-amber-600">{s()!.total_device_types.toLocaleString()}</span>
                  <span class="text-lg text-amber-400 mx-1">/</span>
                  <span class="text-amber-800">{s()!.total_device_types.toLocaleString()}</span>
                </div>
                <div class="text-xs text-amber-700 mt-1">디바이스 타입 (필터링 / 전체)</div>
              </div>
            </div>
          </Show>
          
          <Show when={ui.summaryError}>
            <div class="bg-red-50 border-2 border-red-200 rounded-lg p-4 text-red-700 text-sm">
              <span class="font-semibold">⚠️ 에러:</span> {ui.summaryError}
            </div>
          </Show>
        </div>

        {/* 분석 인사이트 섹션 */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="mb-6">
            <h3 class="text-xl font-bold text-gray-800">🔍 분석 인사이트</h3>
            <p class="text-xs text-gray-500 mt-1">카테고리, 디바이스 타입, 벤더, 인증 활동 통계</p>
          </div>
          
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
                    <div class="text-xs text-indigo-600">{s()!.all_device_categories?.length || 0}개 전체</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-indigo-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={s()!.top_device_categories || []}>
                    {(c, idx) => {
                      const maxCount = s()!.top_device_categories[0]?.[1] || 1;
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
                    <div class="text-xs text-emerald-600">총 {s()!.total_device_types}개 타입</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-emerald-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={s()!.top_device_types?.slice(0, 10) || []}>
                    {(dt, idx) => {
                      const maxCount = s()!.top_device_types?.[0]?.[1] || 1;
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
                    <div class="text-xs text-blue-600">총 {s()!.total_vendors}개 벤더</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-3 border border-blue-200 space-y-1.5 overflow-y-auto max-h-[240px]" style="scrollbar-width: thin;">
                  <For each={s()!.top_vendors?.slice(0, 10) || []}>
                    {(v, idx) => {
                      const maxCount = s()!.top_vendors?.[0]?.[1] || 1;
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

              {/* 최근 인증 활동 */}
              <div class="bg-gradient-to-br from-amber-50 to-orange-50 border-2 border-amber-200 rounded-xl p-5 shadow-sm">
                <div class="flex items-center gap-2 mb-4">
                  <span class="text-2xl">📅</span>
                  <div>
                    <div class="text-sm font-bold text-amber-900">최근 인증 활동</div>
                    <div class="text-xs text-amber-600">최신 인증 트렌드</div>
                  </div>
                </div>
                <div class="bg-white rounded-lg p-4 border border-amber-200 space-y-4">
                  <div class="flex items-center justify-between p-3 bg-gradient-to-r from-emerald-100 to-green-100 rounded-lg">
                    <div>
                      <div class="text-[10px] text-emerald-700 font-semibold uppercase tracking-wide">24시간 내</div>
                      <div class="text-3xl font-bold text-emerald-600 mt-1">{s()!.new_products_24h || 0}</div>
                    </div>
                    <div class="text-4xl">🆕</div>
                  </div>
                  <div class="flex items-center justify-between p-3 bg-gradient-to-r from-blue-100 to-cyan-100 rounded-lg">
                    <div>
                      <div class="text-[10px] text-blue-700 font-semibold uppercase tracking-wide">7일 내</div>
                      <div class="text-3xl font-bold text-blue-600 mt-1">{s()!.new_products_7d || 0}</div>
                    </div>
                    <div class="text-4xl">📈</div>
                  </div>
                  <div class="pt-3 border-t border-emerald-200">
                    <div class="text-xs text-emerald-700 mb-1 font-semibold">전체 인증 제품</div>
                    <div class="text-2xl font-bold text-gray-800">{s()!.total_products.toLocaleString()}</div>
                  </div>
                </div>
              </div>
            </div>
          </Show>
        </div>

        {/* Analytics + DSL Filter Placeholder */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 space-y-4">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-lg font-semibold text-gray-800">📊 Analytics 데이터</h3>
            <div class="flex gap-2 items-center">
              <span class="text-xs text-gray-500">
                총 <span class="font-bold text-indigo-600">{analytics.total}</span>건
              </span>
            </div>
          </div>

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
                  localDbDashboardStore.setUi({ ...ui, filterDraft: '', selectedCategories: [] });
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
          <Show when={analytics.filterApplied || analytics.filterError}>
            <div class="text-xs text-gray-500 flex items-center gap-2">
              <span>적용 필터: <code class="bg-gray-100 px-1 py-0.5 rounded">{analytics.filterApplied || '없음'}</code></span>
              <Show when={analytics.filterError}><span class="text-rose-600">에러: {analytics.filterError}</span></Show>
            </div>
          </Show>
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
                          <th class="p-2 text-left select-none cursor-pointer group hover:bg-indigo-50 transition" onClick={e => cycle(c.key, e.shiftKey)} title={`클릭: 정렬 / Shift+클릭: 다중정렬`}>
                            <span class="inline-flex items-center gap-1">
                              <span class="font-semibold">{c.label}</span>
                              <span class="text-sm text-indigo-600 font-bold min-w-[20px]">{indicator(c.key) || '↕'}</span>
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
                      <a
                        href={r.product_detail_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-indigo-600 hover:underline"
                      >{r.model || '(no model)'}</a>
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
        </div>

        {/* Export & Data Management */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-indigo-200 p-6 space-y-4">
          <h3 class="text-lg font-semibold text-gray-800">📥 Export & 데이터 관리</h3>
          <div class="space-y-3">
            <div class="flex flex-wrap gap-2 items-center">
              <button class="px-3 py-1.5 rounded bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium" onClick={() => localDbDashboardStore.exportCurrentView()}>📥 현재 뷰 Export</button>
              <span class="text-xs text-gray-400">|</span>
              <button class="px-3 py-1.5 rounded bg-indigo-600 hover:bg-indigo-700 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('vendors')}>Vendors (전체)</button>
              <button class="px-3 py-1.5 rounded bg-indigo-600 hover:bg-indigo-700 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('device_types')}>Device Types (전체)</button>
              <button class="px-3 py-1.5 rounded bg-indigo-600 hover:bg-indigo-700 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('analytics')}>Analytics (전체)</button>
            </div>
            <div class="text-xs text-gray-500">{localDbDashboardStore.ui.exportStatus}</div>
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

        {/* Device Types Editor - 개발자 모드 */}
        <details class="bg-slate-50 border border-slate-300 rounded-2xl shadow-lg overflow-hidden">
          <summary class="cursor-pointer p-4 hover:bg-slate-100 transition select-none">
            <div class="flex items-center justify-between">
              <h3 class="text-lg font-semibold text-slate-800">🔧 Device Types JSON Editor (개발자 모드)</h3>
              <span class="text-xs text-slate-600">클릭하여 펼치기/접기</span>
            </div>
          </summary>
          <div class="p-6 pt-2 space-y-3 bg-white border-t border-slate-200">
            <div class="flex items-center gap-3 text-xs text-gray-500 mb-3">
              <span>파일 개수: {localDbDashboardStore.ui.deviceTypesMeta?.count_in_file}</span>
              <span>DB: {localDbDashboardStore.ui.deviceTypesMeta?.count_in_db}</span>
            </div>
            <textarea class="w-full h-56 text-xs font-mono border rounded p-2" value={localDbDashboardStore.ui.deviceTypesJson} onInput={e => localDbDashboardStore.updateDeviceTypesJson(e.currentTarget.value)} />
            <Show when={localDbDashboardStore.ui.deviceTypesDiff}>
              <div class="text-xs text-gray-600 space-y-1">
                <div>Diff: +{localDbDashboardStore.ui.deviceTypesDiff?.added} / upd {localDbDashboardStore.ui.deviceTypesDiff?.updated} / -{localDbDashboardStore.ui.deviceTypesDiff?.removed}</div>
                <Show when={localDbDashboardStore.ui.deviceTypesDiff?.parseError}><div class="text-rose-600">Parse Error: {localDbDashboardStore.ui.deviceTypesDiff?.parseError}</div></Show>
              </div>
            </Show>
            <label class="flex items-center gap-2 text-xs text-gray-600">
              <input type="checkbox" checked={localDbDashboardStore.ui.reseedAfterSave} onChange={e => localDbDashboardStore.setUi({ ...localDbDashboardStore.ui, reseedAfterSave: e.currentTarget.checked })} />
              Save 후 Reseed 수행
            </label>
            <div class="flex gap-2 flex-wrap">
              <button class="px-3 py-1.5 rounded bg-gray-200 text-gray-800 text-xs" onClick={() => localDbDashboardStore.initDeviceTypes()}>Reload</button>
              <button class="px-3 py-1.5 rounded bg-fuchsia-600 text-white text-xs" onClick={() => localDbDashboardStore.saveDeviceTypes()}>Save</button>
              <span class="text-xs text-gray-500">{localDbDashboardStore.ui.deviceTypesSaveResult?.inserted != null && `ins ${localDbDashboardStore.ui.deviceTypesSaveResult.inserted} / upd ${localDbDashboardStore.ui.deviceTypesSaveResult.updated} / skip ${localDbDashboardStore.ui.deviceTypesSaveResult.skipped}`}</span>
            </div>
            <pre class="bg-gray-900 text-[10px] text-pink-300 p-2 rounded max-h-40 overflow-auto">{JSON.stringify(localDbDashboardStore.ui.deviceTypesSaveResult || {}, null, 2)}</pre>
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
                          <a href={product().product_detail_url} target="_blank" rel="noopener noreferrer" class="text-indigo-600 hover:underline text-sm break-all">
                            {product().product_detail_url}
                          </a>
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
                      const maxCount = s()!.top_device_categories[0]?.[1] || 1;
                      const percentage = (c[1] / maxCount) * 100;
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            isSelected() 
                              ? 'bg-indigo-100 border-indigo-400 shadow-md' 
                              : 'bg-white border-gray-200 hover:border-indigo-300 hover:bg-indigo-50'
                          }`}
                          onClick={() => {
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
                                class="pointer-events-none w-4 h-4"
                              />
                              <span class="text-gray-400 text-xs font-mono">#{idx() + 1}</span>
                              <span class="text-sm font-semibold text-gray-800 truncate" title={c[0]}>
                                {c[0]}
                              </span>
                            </div>
                            <span class="text-indigo-600 font-bold ml-2 text-sm">{c[1]}</span>
                          </div>
                          <div class="h-1.5 bg-gray-200 rounded-full overflow-hidden">
                            <div 
                              class="h-full bg-gradient-to-r from-indigo-500 to-purple-500 rounded-full transition-all" 
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
                        const allVendors = s()?.top_vendors?.map(v => v[0]) || [];
                        const isAllSelected = ui.selectedVendors.length === allVendors.length;
                        localDbDashboardStore.setUi({ ...ui, selectedVendors: isAllSelected ? [] : allVendors });
                      }}
                    >
                      {ui.selectedVendors.length === (s()?.top_vendors?.length || 0) ? '전체 해제' : '전체 선택'}
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
                  <For each={s()?.top_vendors || []}>
                    {(v, idx) => {
                      const isSelected = () => ui.selectedVendors.includes(v[0]);
                      const maxCount = s()!.top_vendors?.[0]?.[1] || 1;
                      const percentage = (v[1] / maxCount) * 100;
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            isSelected() 
                              ? 'bg-blue-100 border-blue-400 shadow-md' 
                              : 'bg-white border-gray-200 hover:border-blue-300 hover:bg-blue-50'
                          }`}
                          onClick={() => {
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
                                class="pointer-events-none w-4 h-4"
                              />
                              <span class="text-gray-400 text-xs font-mono">#{idx() + 1}</span>
                              <span class="text-sm font-semibold text-gray-800 truncate" title={v[0]}>
                                {v[0]}
                              </span>
                            </div>
                            <span class="text-blue-600 font-bold ml-2 text-sm">{v[1]}</span>
                          </div>
                          <div class="h-1.5 bg-gray-200 rounded-full overflow-hidden">
                            <div 
                              class="h-full bg-gradient-to-r from-blue-500 to-cyan-500 rounded-full transition-all" 
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
                  <For each={s()?.all_device_type_names || []}>
                    {(dtype, idx) => {
                      const isSelected = () => ui.selectedDeviceTypes.includes(dtype);
                      return (
                        <div 
                          class={`p-3 rounded-lg cursor-pointer transition-all border-2 ${
                            isSelected() 
                              ? 'bg-emerald-100 border-emerald-400 shadow-md' 
                              : 'bg-white border-gray-200 hover:border-emerald-300 hover:bg-emerald-50'
                          }`}
                          onClick={() => {
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
                              class="pointer-events-none w-4 h-4"
                            />
                            <span class="text-gray-400 text-xs font-mono">#{idx() + 1}</span>
                            <span class="text-sm font-semibold text-gray-800 truncate" title={dtype}>
                              {dtype}
                            </span>
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
      </div>
    </div>
  );
};
