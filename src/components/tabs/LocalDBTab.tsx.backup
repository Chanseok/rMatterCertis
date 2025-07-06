/**
 * LocalDBTab - 로컬 데이터베이스 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, For, Show, onMount } from 'solid-js';
import { createStore } from 'solid-js/store';

interface Product {
  id: number;
  manufacturer: string;
  model: string;
  device_type: string;
  certification_date: string;
  page_id: number;
}

export const LocalDBTab: Component = () => {
  const [products, setProducts] = createStore<Product[]>([]);
  const [currentPage, setCurrentPage] = createSignal(1);
  const [searchQuery, setSearchQuery] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);
  
  const itemsPerPage = 12;

  const filteredProducts = createMemo(() => {
    const query = searchQuery().toLowerCase();
    if (!query) return products;
    
    return products.filter(product => 
      product.manufacturer.toLowerCase().includes(query) ||
      product.model.toLowerCase().includes(query) ||
      product.device_type.toLowerCase().includes(query)
    );
  });

  const displayedProducts = createMemo(() => {
    const filtered = filteredProducts();
    const start = (currentPage() - 1) * itemsPerPage;
    return filtered.slice(start, start + itemsPerPage);
  });

  const totalFilteredPages = createMemo(() => 
    Math.ceil(filteredProducts().length / itemsPerPage)
  );

  const uniqueManufacturers = createMemo(() => 
    new Set(products.map(p => p.manufacturer)).size
  );

  const uniqueDeviceTypes = createMemo(() => 
    new Set(products.map(p => p.device_type)).size
  );

  const handleExport = async () => {
    try {
      setIsLoading(true);
      // Tauri 명령으로 엑셀 내보내기
      console.log('Exporting products to Excel...');
      // await invoke('export_to_excel', { data: products });
    } catch (error) {
      console.error('Failed to export:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const loadProducts = async () => {
    try {
      setIsLoading(true);
      // 실제로는 Tauri 명령으로 데이터를 가져와야 함
      // const response = await invoke('get_products', { page: currentPage(), limit: itemsPerPage });
      
      // 임시 데이터
      const mockProducts: Product[] = Array.from({ length: 50 }, (_, i) => ({
        id: i + 1,
        manufacturer: `Manufacturer ${Math.floor(i / 5) + 1}`,
        model: `Model ${i + 1}`,
        device_type: ['Smart Speaker', 'Light Bulb', 'Smart Switch', 'Sensor'][i % 4],
        certification_date: new Date(2023, Math.floor(Math.random() * 12), Math.floor(Math.random() * 28) + 1).toLocaleDateString(),
        page_id: Math.floor(Math.random() * 1000) + 1
      }));

      setProducts(mockProducts);
    } catch (error) {
      console.error('Failed to load products:', error);
    } finally {
      setIsLoading(false);
    }
  };

  onMount(() => {
    loadProducts();
  });

  return (
    <div class="space-y-6">
      {/* 데이터베이스 요약 */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 border border-gray-200 dark:border-gray-700">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">데이터베이스 현황</h3>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="text-center p-4 bg-purple-50 dark:bg-purple-900/20 rounded-lg">
            <div class="text-3xl font-bold text-purple-600 dark:text-purple-400">
              {products.length.toLocaleString()}
            </div>
            <div class="text-sm text-gray-600 dark:text-gray-400">총 제품 수</div>
          </div>
          <div class="text-center p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <div class="text-3xl font-bold text-blue-600 dark:text-blue-400">
              {uniqueManufacturers()}
            </div>
            <div class="text-sm text-gray-600 dark:text-gray-400">제조사 수</div>
          </div>
          <div class="text-center p-4 bg-green-50 dark:bg-green-900/20 rounded-lg">
            <div class="text-3xl font-bold text-green-600 dark:text-green-400">
              {uniqueDeviceTypes()}
            </div>
            <div class="text-sm text-gray-600 dark:text-gray-400">디바이스 유형 수</div>
          </div>
        </div>
      </div>

      {/* 검색 및 필터 */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 border border-gray-200 dark:border-gray-700">
        <div class="flex flex-col sm:flex-row gap-4">
          <div class="flex-1">
            <input
              type="text"
              placeholder="제조사, 모델명, 디바이스 유형으로 검색..."
              value={searchQuery()}
              onInput={(e) => {
                setSearchQuery(e.currentTarget.value);
                setCurrentPage(1); // 검색 시 첫 페이지로 이동
              }}
              class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-purple-500 dark:bg-gray-700 dark:text-white"
            />
          </div>
          <div class="flex gap-2">
            <button
              onClick={loadProducts}
              disabled={isLoading()}
              class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
            >
              {isLoading() ? '로딩...' : '새로고침'}
            </button>
            <button
              onClick={handleExport}
              disabled={isLoading()}
              class="px-6 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 disabled:opacity-50 transition-colors focus:outline-none focus:ring-2 focus:ring-purple-500 focus:ring-offset-2"
            >
              엑셀 내보내기
            </button>
          </div>
        </div>
      </div>

      {/* 제품 목록 */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-md border border-gray-200 dark:border-gray-700">
        <div class="p-6 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">제품 목록</h3>
          <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
            {searchQuery() ? `"${searchQuery()}" 검색 결과` : '전체 제품'} 
            ({filteredProducts().length.toLocaleString()}개)
          </p>
        </div>

        <Show when={!isLoading()} fallback={
          <div class="p-12 text-center">
            <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-purple-600 mx-auto"></div>
            <p class="mt-4 text-gray-600 dark:text-gray-400">데이터를 불러오는 중...</p>
          </div>
        }>
          <Show when={displayedProducts().length > 0} fallback={
            <div class="p-12 text-center">
              <div class="text-gray-400 text-6xl mb-4">📭</div>
              <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">
                {searchQuery() ? '검색 결과가 없습니다' : '데이터가 없습니다'}
              </h3>
              <p class="text-gray-600 dark:text-gray-400">
                {searchQuery() ? '다른 검색어를 시도해보세요' : '크롤링을 실행하여 데이터를 수집하세요'}
              </p>
            </div>
          }>
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead class="bg-gray-50 dark:bg-gray-700">
                  <tr>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">
                      제조사
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">
                      모델명
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">
                      디바이스 유형
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">
                      인증 날짜
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">
                      페이지 ID
                    </th>
                  </tr>
                </thead>
                <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                  <For each={displayedProducts()}>
                    {(product) => (
                      <tr class="hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors">
                        <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900 dark:text-white">
                          {product.manufacturer}
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                          {product.model}
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                          {product.device_type}
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                          {product.certification_date}
                        </td>
                        <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                          {product.page_id}
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>

            {/* 페이지네이션 */}
            <div class="bg-white dark:bg-gray-800 px-6 py-3 border-t border-gray-200 dark:border-gray-700">
              <div class="flex items-center justify-between">
                <div class="text-sm text-gray-700 dark:text-gray-300">
                  총 {filteredProducts().length}개 중 {(currentPage() - 1) * itemsPerPage + 1}-{Math.min(currentPage() * itemsPerPage, filteredProducts().length)}개 표시
                </div>
                <div class="flex items-center space-x-2">
                  <button
                    onClick={() => setCurrentPage(Math.max(1, currentPage() - 1))}
                    disabled={currentPage() === 1}
                    class="px-3 py-2 bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-300 dark:hover:bg-gray-500 transition-colors"
                  >
                    이전
                  </button>
                  <span class="px-3 py-2 text-gray-700 dark:text-gray-300 text-sm">
                    {currentPage()} / {totalFilteredPages()}
                  </span>
                  <button
                    onClick={() => setCurrentPage(Math.min(totalFilteredPages(), currentPage() + 1))}
                    disabled={currentPage() === totalFilteredPages()}
                    class="px-3 py-2 bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-300 dark:hover:bg-gray-500 transition-colors"
                  >
                    다음
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};
