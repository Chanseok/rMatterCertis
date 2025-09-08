/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, For, onMount, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { VendorSyncResult } from '../../types/domain';

export const LocalDBTab: Component = () => {
  // 데이터베이스 상태 (실제 데이터)
  const [dbStats, setDbStats] = createSignal({
    totalRecords: 0,
    lastUpdate: 'Loading...',
    databaseSize: '0MB',
    indexSize: '0MB'
  });

  // 실제 제품 데이터
  const [recentData, setRecentData] = createSignal<any[]>([]);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string>('');

  // 페이지네이션
  const [currentPage, setCurrentPage] = createSignal(1);
  const [totalPages, setTotalPages] = createSignal(1);
  const [totalProducts, setTotalProducts] = createSignal(0);

  // 검색 및 필터링
  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedCategory, setSelectedCategory] = createSignal('All');

  // 실제 데이터에서 카테고리 추출
  const categories = () => {
    const uniqueCompanies = new Set<string>();
    recentData().forEach(item => {
      if (item.company && item.company !== 'Unknown') {
        uniqueCompanies.add(item.company);
      }
    });
    return ['All', ...Array.from(uniqueCompanies).sort()];
  };

  // 실제 데이터베이스에서 통계 로드
  const loadDbStats = async () => {
    try {
      const stats = await tauriApi.getLocalDbStats();
      setDbStats(stats);
    } catch (err) {
      console.error('Failed to load DB stats:', err);
      setError(`DB 통계 로드 실패: ${err}`);
    }
  };

  // 실제 데이터베이스에서 제품 데이터 로드
  const loadProducts = async (page: number = 1) => {
    try {
      setIsLoading(true);
      const result = await tauriApi.getProducts(page, 20);
      setRecentData(result.products || []);
      setTotalPages(result.total_pages || 1);
      setTotalProducts(result.total || 0);
      setCurrentPage(page);
    } catch (err) {
      console.error('Failed to load products:', err);
      setError(`제품 데이터 로드 실패: ${err}`);
      setRecentData([]);
    } finally {
      setIsLoading(false);
    }
  };

  // 컴포넌트 마운트 시 데이터 로드
  onMount(() => {
    loadDbStats();
    loadProducts(1);
  });

  const filteredData = () => {
    return recentData().filter(item => {
      const matchesSearch = item.title?.toLowerCase().includes(searchTerm().toLowerCase()) || false;
      const matchesCategory = selectedCategory() === 'All' || item.company === selectedCategory();
      return matchesSearch && matchesCategory;
    });
  };

  const exportData = async () => {
    try {
      const exportPath = await tauriApi.exportDatabaseData('csv');
      alert(`데이터가 내보내졌습니다: ${exportPath}`);
    } catch (err) {
      alert(`데이터 내보내기 실패: ${err}`);
    }
  };

  const clearDatabase = () => {
    if (confirm('정말로 데이터베이스를 초기화하시겠습니까? 이 작업은 되돌릴 수 없습니다.')) {
      alert('데이터베이스 초기화 기능은 개발 중입니다.');
    }
  };

  const optimizeDatabase = async () => {
    try {
      await tauriApi.optimizeDatabase();
      alert('데이터베이스 최적화가 완료되었습니다.');
      loadDbStats(); // 통계 새로고침
    } catch (err) {
      alert(`데이터베이스 최적화 실패: ${err}`);
    }
  };

  // Vendor-only sync (CSA DCL)
  const [vendorSyncLoading, setVendorSyncLoading] = createSignal(false);
  const [vendorSyncResult, setVendorSyncResult] = createSignal<VendorSyncResult | null>(null);
  const syncVendors = async () => {
    try {
      setVendorSyncLoading(true);
      setVendorSyncResult(null);
      const res = await tauriApi.updateVendorsFromCsa();
      setVendorSyncResult(res);
      // Refresh stats after sync
      await loadDbStats();
    } catch (err) {
      alert(`벤더 동기화 실패: ${err}`);
    } finally {
      setVendorSyncLoading(false);
    }
  };

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h2 class="text-2xl md:text-3xl font-bold text-gray-800">🗄️ 로컬DB</h2>
        </div>

        {/* DB Stats */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">데이터베이스 통계</h3>
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            <div class="bg-gradient-to-br from-blue-50 to-blue-100 rounded-2xl p-4 border border-blue-200/50 text-center">
              <div class="text-2xl font-bold text-blue-600">{dbStats().totalRecords.toLocaleString()}</div>
              <div class="text-sm text-blue-700">총 레코드 수</div>
            </div>
            <div class="bg-gradient-to-br from-emerald-50 to-emerald-100 rounded-2xl p-4 border border-emerald-200/50 text-center">
              <div class="text-xl font-bold text-emerald-600">{dbStats().databaseSize}</div>
              <div class="text-sm text-emerald-700">데이터베이스 크기</div>
            </div>
            <div class="bg-gradient-to-br from-amber-50 to-amber-100 rounded-2xl p-4 border border-amber-200/50 text-center">
              <div class="text-xl font-bold text-amber-600">{dbStats().indexSize}</div>
              <div class="text-sm text-amber-700">인덱스 크기</div>
            </div>
            <div class="bg-gradient-to-br from-violet-50 to-violet-100 rounded-2xl p-4 border border-violet-200/50 text-center">
              <div class="text-sm font-semibold text-violet-700">{dbStats().lastUpdate}</div>
              <div class="text-sm text-violet-700">마지막 업데이트</div>
            </div>
          </div>
        </div>

        {/* Search & Filter */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">검색 및 필터링</h3>
          <div class="flex flex-wrap gap-4 mb-3">
            <div class="flex-1 min-w-[200px]">
              <label class="block text-sm font-medium text-gray-700 mb-1">검색어</label>
              <input type="text" placeholder="제품명으로 검색..." value={searchTerm()} onInput={(e) => setSearchTerm(e.currentTarget.value)} class="w-full px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300" />
            </div>
            <div class="min-w-[150px]">
              <label class="block text-sm font-medium text-gray-700 mb-1">카테고리</label>
              <select value={selectedCategory()} onChange={(e) => setSelectedCategory(e.currentTarget.value)} class="w-full px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300">
                <For each={categories()}>{(category) => <option value={category}>{category}</option>}</For>
              </select>
            </div>
          </div>
          <div class="text-sm text-gray-500">검색 결과: {filteredData().length}개 항목</div>
        </div>

        {/* Data Table */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 overflow-hidden">
          <div class="p-4 bg-gradient-to-r from-gray-50 to-gray-100 border-b border-white/30 flex items-center justify-between">
            <h3 class="text-base md:text-lg font-semibold text-gray-800">실제 데이터 ({totalProducts()}개 제품)</h3>
            <button class="px-3 py-1.5 text-sm rounded-lg text-white bg-indigo-600 hover:bg-indigo-700" onClick={() => loadProducts(currentPage())}>새로고침</button>
          </div>

          <Show when={!!error()}>
            <div class="p-4 bg-rose-50 border-b border-rose-200 text-rose-700 text-sm">{error()}</div>
          </Show>
          <Show when={isLoading()}>
            <div class="p-8 text-center text-gray-500">
              <div class="mb-2">데이터를 로드하는 중...</div>
              <div class="w-6 h-6 border-2 border-gray-200 border-t-indigo-500 rounded-full animate-spin mx-auto"></div>
            </div>
          </Show>

          <Show when={!isLoading() && !error() && filteredData().length === 0}>
            <div class="p-10 text-center text-gray-500">
              <div class="text-5xl mb-3">📭</div>
              <div class="text-lg font-medium mb-1">데이터가 없습니다</div>
              <div class="text-sm">크롤링을 실행하여 데이터를 수집해보세요.</div>
            </div>
          </Show>

          <Show when={!isLoading() && !error() && filteredData().length > 0}>
            <div class="overflow-x-auto">
              <table class="w-full border-collapse">
                <thead>
                  <tr class="bg-gray-50">
                    <th class="px-3 py-2 text-left text-sm font-medium text-gray-700 border-b">ID</th>
                    <th class="px-3 py-2 text-left text-sm font-medium text-gray-700 border-b">제품명</th>
                    <th class="px-3 py-2 text-left text-sm font-medium text-gray-700 border-b">회사</th>
                    <th class="px-3 py-2 text-left text-sm font-medium text-gray-700 border-b">인증일</th>
                    <th class="px-3 py-2 text-left text-sm font-medium text-gray-700 border-b">상태</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={filteredData()}>
                    {(item) => (
                      <tr class="border-b">
                        <td class="px-3 py-2 text-gray-500 font-mono text-sm">{item.id}</td>
                        <td class="px-3 py-2 text-gray-900 font-medium">{item.title || 'Unknown Product'}</td>
                        <td class="px-3 py-2 text-gray-600">
                          <span class="bg-blue-100 text-blue-800 px-2 py-0.5 rounded text-xs">{item.company || 'Unknown'}</span>
                        </td>
                        <td class="px-3 py-2 text-gray-600 font-mono text-xs">{item.certification_date || 'N/A'}</td>
                        <td class="px-3 py-2">
                          <span class={`px-2 py-0.5 rounded text-xs ${item.status === 'Valid' ? 'bg-emerald-100 text-emerald-700' : 'bg-amber-100 text-amber-700'}`}>{item.status}</span>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>

          <Show when={!isLoading() && !error() && totalPages() > 1}>
            <div class="p-4 bg-gray-50 border-t border-white/30 flex items-center justify-between text-sm text-gray-600">
              <div>페이지 {currentPage()} / {totalPages()} (총 {totalProducts()}개)</div>
              <div class="flex gap-2">
                <button class={`px-3 py-1.5 rounded border ${currentPage() <= 1 ? 'bg-gray-100 text-gray-400 cursor-not-allowed' : 'bg-white hover:bg-gray-50 text-gray-700'}`} disabled={currentPage() <= 1} onClick={() => loadProducts(Math.max(1, currentPage() - 1))}>이전</button>
                <button class={`px-3 py-1.5 rounded border ${currentPage() >= totalPages() ? 'bg-gray-100 text-gray-400 cursor-not-allowed' : 'bg-white hover:bg-gray-50 text-gray-700'}`} disabled={currentPage() >= totalPages()} onClick={() => loadProducts(Math.min(totalPages(), currentPage() + 1))}>다음</button>
              </div>
            </div>
          </Show>
        </div>

        {/* DB Management */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">데이터베이스 관리</h3>
          <div class="flex flex-wrap gap-2 items-center">
            <button class="px-4 py-2 rounded-lg text-white bg-indigo-600 hover:bg-indigo-700" onClick={exportData}>📤 데이터 내보내기</button>
            <button class="px-4 py-2 rounded-lg text-white bg-emerald-600 hover:bg-emerald-700" onClick={optimizeDatabase}>⚡ 데이터베이스 최적화</button>
            <button class="px-4 py-2 rounded-lg text-white bg-rose-600 hover:bg-rose-700" onClick={clearDatabase}>🗑️ 데이터베이스 초기화</button>
            <div class="h-6 w-px bg-gray-200 mx-2" />
            <button
              class={`px-4 py-2 rounded-lg text-white ${vendorSyncLoading() ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'}`}
              disabled={vendorSyncLoading()}
              onClick={syncVendors}
            >
              {vendorSyncLoading() ? '벤더 업데이트 중...' : 'CSA에서 벤더만 업데이트'}
            </button>
            <Show when={!!vendorSyncResult()}>
              <span class="text-sm text-gray-600 ml-2">
                완료: +{vendorSyncResult()!.inserted} 신규, {vendorSyncResult()!.updated} 수정, {vendorSyncResult()!.skipped} 유지 (총 {vendorSyncResult()!.final_count} / API {vendorSyncResult()!.api_total}, {vendorSyncResult()!.pages}페이지)
              </span>
            </Show>
          </div>
        </div>

        {/* Backup & Restore */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">백업 및 복원</h3>
          <div class="text-sm text-gray-600 mb-4">
            <div class="mb-1">백업 상태: 백업 기록을 확인하세요</div>
            <div>백업 기능은 데이터베이스 관리 도구를 통해 제공됩니다</div>
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="px-4 py-2 rounded-lg text-white bg-violet-600 hover:bg-violet-700"
              onClick={async () => {
                try {
                  const backupPath = await tauriApi.backupDatabase();
                  alert(`백업이 생성되었습니다: ${backupPath}`);
                } catch (err) {
                  alert(`백업 생성 실패: ${err}`);
                }
              }}
            >
              💾 백업 생성
            </button>
            <button class="px-4 py-2 rounded-lg text-white bg-amber-600 hover:bg-amber-700" onClick={() => alert('백업 복원 기능은 개발 중입니다.')}>📁 백업에서 복원</button>
          </div>
        </div>
      </div>
    </div>
  );
};
