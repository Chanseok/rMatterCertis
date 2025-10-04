import { Component, Show, For, createSignal, createEffect } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface ProductWithoutCoordinates {
  url: string;
  model: string | null;
  manufacturer: string | null;
  url_exists: boolean;
  checked_at: string | null;
}

interface NullCoordinatesReport {
  total_count: number;
  verified_exists: ProductWithoutCoordinates[];
  verified_missing: ProductWithoutCoordinates[];
  not_verified: ProductWithoutCoordinates[];
}

interface Props {
  nullCoordsCount: () => number;
  addLog: (msg: string) => void;
  onRefresh: () => Promise<void>;
}

const NullCoordinatesPanel: Component<Props> = (p) => {
  const [loading, setLoading] = createSignal(false);
  const [report, setReport] = createSignal<NullCoordinatesReport | null>(null);
  const [selectedTab, setSelectedTab] = createSignal<'all' | 'exists' | 'missing' | 'not_verified'>('all');
  const [selectedUrls, setSelectedUrls] = createSignal<Set<string>>(new Set());
  const [deleting, setDeleting] = createSignal(false);
  const [autoLoaded, setAutoLoaded] = createSignal(false);

  // 패널이 처음 표시될 때 자동으로 제품 목록 로드
  createEffect(() => {
    const count = p.nullCoordsCount();
    console.log('[NullCoordinatesPanel] createEffect triggered, count:', count, 'autoLoaded:', autoLoaded());
    
    if (count > 0 && !autoLoaded() && !loading()) {
      console.log('[NullCoordinatesPanel] Auto-loading products...');
      setAutoLoaded(true);
      loadProducts();
    }
  });

  const loadProducts = async (skipVerification: boolean = true) => {
    console.log('[NullCoordinatesPanel] loadProducts called, skipVerification:', skipVerification);
    setLoading(true);
    const count = p.nullCoordsCount();
    console.log('[NullCoordinatesPanel] nullCoordsCount:', count);
    
    if (skipVerification) {
      p.addLog(`� 좌표 없는 제품 ${count}개 조회 중... (URL 검증 생략)`);
    } else {
      p.addLog(`�🔍 좌표 없는 제품 ${count}개 조회 및 URL 검증 시작 (최대 50개까지 검증, 약 ${Math.min(count, 50) * 0.15}초 소요)...`);
    }
    
    try {
      console.log('[NullCoordinatesPanel] Invoking get_products_without_coordinates...');
      const result: NullCoordinatesReport = await invoke('get_products_without_coordinates', {
        skipVerification
      });
      console.log('[NullCoordinatesPanel] Result received:', result);
      console.log('[NullCoordinatesPanel] Result type:', typeof result, 'is null?', result === null);
      
      if (!result) {
        throw new Error('Received null result from backend');
      }
      
      setReport(result);
      console.log('[NullCoordinatesPanel] setReport called with:', result);
      console.log('[NullCoordinatesPanel] report() after set:', report());
      
      p.addLog(`✅ 총 ${result.total_count}개 제품 조회 완료`);
      if (!skipVerification) {
        p.addLog(`  - ✅ URL 존재: ${result.verified_exists.length}개`);
        p.addLog(`  - ❌ URL 없음: ${result.verified_missing.length}개`);
        p.addLog(`  - ⚠️ 미검증/확인실패: ${result.not_verified.length}개`);
      } else {
        p.addLog(`  (⚠️ URL 검증을 하려면 'URL 검증 실행' 버튼을 클릭하세요)`);
      }
    } catch (e: any) {
      console.error('[NullCoordinatesPanel] Error caught:', e);
      console.error('[NullCoordinatesPanel] Error type:', typeof e);
      console.error('[NullCoordinatesPanel] Error message:', e?.message);
      console.error('[NullCoordinatesPanel] Error stack:', e?.stack);
      p.addLog(`❌ 조회 실패: ${e?.message || e}`);
      setReport(null);
    } finally {
      console.log('[NullCoordinatesPanel] loadProducts finally, loading:', loading());
      setLoading(false);
      console.log('[NullCoordinatesPanel] loadProducts done, loading:', loading());
    }
  };

  const deleteSelected = async () => {
    const urls = Array.from(selectedUrls());
    if (urls.length === 0) {
      p.addLog('⚠️ 삭제할 제품을 선택해주세요');
      return;
    }

    if (!confirm(`선택한 ${urls.length}개 제품을 삭제하시겠습니까?\n\nproducts와 product_details 테이블에서 모두 삭제됩니다.\n이 작업은 되돌릴 수 없습니다.`)) {
      return;
    }

    setDeleting(true);
    p.addLog(`🗑️ ${urls.length}개 제품 삭제 시작...`);

    try {
      const deletedCount: number = await invoke('delete_products_without_coordinates', { urls });
      p.addLog(`✅ ${deletedCount}개 제품 삭제 완료 (products + product_details)`);
      
      // 선택 초기화 및 새로고침
      setSelectedUrls(new Set<string>());
      await loadProducts();
      await p.onRefresh(); // 진단 결과 새로고침
    } catch (e: any) {
      p.addLog(`❌ 삭제 실패: ${e?.message || e}`);
    } finally {
      setDeleting(false);
    }
  };

  const deleteAll = async (category: 'all' | 'exists' | 'missing' | 'not_verified') => {
    const r = report();
    if (!r) return;

    const products = category === 'all' ? [...r.verified_exists, ...r.verified_missing, ...r.not_verified] :
                     category === 'exists' ? r.verified_exists : 
                     category === 'missing' ? r.verified_missing : 
                     r.not_verified;
    
    const urls = products.map(p => p.url);
    if (urls.length === 0) {
      p.addLog('⚠️ 삭제할 제품이 없습니다');
      return;
    }

    const categoryName = category === 'all' ? '전체' :
                         category === 'exists' ? 'URL 존재' : 
                         category === 'missing' ? 'URL 없음' : 
                         '확인 실패';

    if (!confirm(`"${categoryName}" 카테고리의 ${urls.length}개 제품을 모두 삭제하시겠습니까?\n\nproducts와 product_details 테이블에서 모두 삭제됩니다.\n이 작업은 되돌릴 수 없습니다.`)) {
      return;
    }

    setDeleting(true);
    p.addLog(`🗑️ [${categoryName}] ${urls.length}개 제품 삭제 시작...`);

    try {
      const deletedCount: number = await invoke('delete_products_without_coordinates', { urls });
      p.addLog(`✅ ${deletedCount}개 제품 삭제 완료 (products + product_details)`);
      
      // 선택 초기화 및 새로고침
      setSelectedUrls(new Set<string>());
      await loadProducts();
      await p.onRefresh();
    } catch (e: any) {
      p.addLog(`❌ 삭제 실패: ${e?.message || e}`);
    } finally {
      setDeleting(false);
    }
  };

  const toggleSelection = (url: string) => {
    const newSet = new Set(selectedUrls());
    if (newSet.has(url)) {
      newSet.delete(url);
    } else {
      newSet.add(url);
    }
    setSelectedUrls(newSet);
  };

  const getCurrentTabProducts = (): ProductWithoutCoordinates[] => {
    const r = report();
    if (!r) return [];
    
    return selectedTab() === 'all' ? [...r.verified_exists, ...r.verified_missing, ...r.not_verified] :
           selectedTab() === 'exists' ? r.verified_exists :
           selectedTab() === 'missing' ? r.verified_missing :
           r.not_verified;
  };

  const selectAllInTab = () => {
    const products = getCurrentTabProducts();
    const newSet = new Set(selectedUrls());
    products.forEach(p => newSet.add(p.url));
    setSelectedUrls(newSet);
  };

  const deselectAllInTab = () => {
    const products = getCurrentTabProducts();
    const newSet = new Set(selectedUrls());
    products.forEach(p => newSet.delete(p.url));
    setSelectedUrls(newSet);
  };

  const ProductTable: Component<{ products: ProductWithoutCoordinates[] }> = (props) => (
    <div class="overflow-auto max-h-96">
      <table class="w-full text-xs border-collapse">
        <thead class="sticky top-0 bg-gray-100 border-b-2 border-gray-300">
          <tr>
            <th class="p-2 text-left w-10">
              <input
                type="checkbox"
                class="cursor-pointer"
                checked={props.products.length > 0 && props.products.every(p => selectedUrls().has(p.url))}
                onChange={() => {
                  if (props.products.every(p => selectedUrls().has(p.url))) {
                    deselectAllInTab();
                  } else {
                    selectAllInTab();
                  }
                }}
              />
            </th>
            <th class="p-2 text-left">제품명</th>
            <th class="p-2 text-left">제조사</th>
            <th class="p-2 text-left w-24">작업</th>
          </tr>
        </thead>
        <tbody>
          <For each={props.products}>
            {(product) => (
              <tr class="border-b border-gray-200 hover:bg-gray-50">
                <td class="p-2">
                  <input
                    type="checkbox"
                    class="cursor-pointer"
                    checked={selectedUrls().has(product.url)}
                    onChange={() => toggleSelection(product.url)}
                  />
                </td>
                <td class="p-2">
                  <a
                    href={product.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="text-blue-600 hover:text-blue-800 hover:underline font-medium"
                    title={product.url}
                  >
                    {product.model || '(제품명 없음)'}
                  </a>
                </td>
                <td class="p-2 text-gray-700">
                  {product.manufacturer || '(제조사 없음)'}
                </td>
                <td class="p-2">
                  <button
                    class="px-2 py-0.5 text-[10px] bg-red-600 text-white rounded hover:bg-red-700"
                    onClick={async () => {
                      if (confirm(`"${product.model || product.url}"를 삭제하시겠습니까?`)) {
                        setSelectedUrls(new Set([product.url]));
                        await deleteSelected();
                      }
                    }}
                  >
                    삭제
                  </button>
                </td>
              </tr>
            )}
          </For>
          <Show when={props.products.length === 0}>
            <tr>
              <td colspan="4" class="p-4 text-center text-gray-500">
                해당 카테고리에 제품이 없습니다
              </td>
            </tr>
          </Show>
        </tbody>
      </table>
    </div>
  );

  return (
    <Show when={p.nullCoordsCount() > 0}>
      <div class="bg-orange-50/50 backdrop-blur-sm rounded-2xl shadow-xl border border-orange-200 p-6 mb-8">
        <div class="flex items-center justify-between mb-3">
          <h3 class="text-lg font-bold text-orange-800">
            ⚠️ NULL 좌표 제품 관리 ({p.nullCoordsCount()}개)
          </h3>
          <div class="flex gap-2">
            <button
              class="px-3 py-2 text-sm rounded-lg font-semibold bg-blue-600 text-white hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
              onClick={() => loadProducts(false)}
              disabled={loading()}
            >
              {loading() ? '🔄 로딩 중...' : '🔍 URL 검증 실행'}
            </button>
            <button
              class="px-3 py-2 text-sm rounded-lg font-semibold bg-orange-600 text-white hover:bg-orange-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
              onClick={() => loadProducts(true)}
              disabled={loading()}
            >
              {loading() ? '🔄 로딩 중...' : '🔄 빠른 새로고침'}
            </button>
          </div>
        </div>

        <p class="text-xs text-orange-700 mb-4">
          이 제품들은 page_id 또는 index_in_page 정보가 없어서 정상적인 크롤링 범위에 포함되지 않습니다.
          제품명을 클릭하면 해당 페이지로 이동할 수 있습니다. 불필요한 제품은 삭제할 수 있습니다.
          <br/>
          <b>팁:</b> '빠른 새로고침'은 즉시 목록을 표시하고, 'URL 검증 실행'은 각 제품 URL이 존재하는지 확인합니다 (시간 소요).
        </p>

        <Show when={loading()}>
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-orange-600 mx-auto mb-4"></div>
              <p class="text-sm text-gray-600">제품 목록을 불러오는 중...</p>
              <p class="text-xs text-gray-500 mt-1">URL 검증 포함 최대 {Math.min(p.nullCoordsCount(), 50) * 0.15}초 소요</p>
            </div>
          </div>
        </Show>

        <Show when={!loading() && !report()}>
          <div class="text-center py-8 text-gray-600">
            <p class="mb-2">제품 목록을 로드하려면 '새로고침' 버튼을 클릭하세요</p>
            <p class="text-xs text-gray-500">(처음 로드 시 자동으로 실행됩니다)</p>
          </div>
        </Show>

        <Show when={!loading() && report()}>
          <div class="space-y-4">
            {/* 탭 메뉴 */}
            <div class="flex gap-2 border-b border-orange-300">
              <button
                class={`px-4 py-2 text-sm font-semibold transition-colors ${
                  selectedTab() === 'all'
                    ? 'border-b-2 border-blue-600 text-blue-700'
                    : 'text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => setSelectedTab('all')}
              >
                📋 전체 ({report()!.total_count})
              </button>
              <button
                class={`px-4 py-2 text-sm font-semibold transition-colors ${
                  selectedTab() === 'exists'
                    ? 'border-b-2 border-green-600 text-green-700'
                    : 'text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => setSelectedTab('exists')}
              >
                ✅ URL 존재 ({report()!.verified_exists.length})
              </button>
              <button
                class={`px-4 py-2 text-sm font-semibold transition-colors ${
                  selectedTab() === 'missing'
                    ? 'border-b-2 border-red-600 text-red-700'
                    : 'text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => setSelectedTab('missing')}
              >
                ❌ URL 없음 ({report()!.verified_missing.length})
              </button>
              <button
                class={`px-4 py-2 text-sm font-semibold transition-colors ${
                  selectedTab() === 'not_verified'
                    ? 'border-b-2 border-yellow-600 text-yellow-700'
                    : 'text-gray-600 hover:text-gray-800'
                }`}
                onClick={() => setSelectedTab('not_verified')}
              >
                ⚠️ 확인 실패 ({report()!.not_verified.length})
              </button>
            </div>

            {/* 액션 버튼 */}
            <div class="flex items-center justify-between p-2 bg-white rounded border border-orange-200">
              <div class="text-xs text-gray-600">
                선택된 제품: <b class="text-orange-700">{selectedUrls().size}개</b>
              </div>
              <div class="flex gap-2">
                <button
                  class="px-3 py-1 text-xs rounded bg-gray-500 text-white hover:bg-gray-600 disabled:bg-gray-300"
                  disabled={selectedUrls().size === 0}
                  onClick={() => setSelectedUrls(new Set<string>())}
                >
                  선택 해제
                </button>
                <button
                  class={`px-3 py-1 text-xs rounded ${deleting() ? 'bg-gray-300 text-gray-500' : 'bg-rose-600 text-white hover:bg-rose-700'}`}
                  disabled={selectedUrls().size === 0 || deleting()}
                  onClick={deleteSelected}
                >
                  {deleting() ? '삭제 중...' : '선택 삭제'}
                </button>
                <button
                  class={`px-3 py-1 text-xs rounded ${deleting() ? 'bg-gray-300 text-gray-500' : 'bg-red-700 text-white hover:bg-red-800'}`}
                  disabled={deleting() || getCurrentTabProducts().length === 0}
                  onClick={() => deleteAll(selectedTab())}
                >
                  {deleting() ? '삭제 중...' : '현재 탭 전체 삭제'}
                </button>
              </div>
            </div>

            {/* 제품 테이블 */}
            <Show when={selectedTab() === 'all'}>
              <div class="bg-blue-50 border border-blue-200 rounded p-3">
                <div class="text-xs text-blue-800 mb-2 font-semibold">
                  📋 전체 제품 ({report()!.total_count}개)
                </div>
                <p class="text-[11px] text-blue-700 mb-3">
                  좌표가 없는 모든 제품 목록입니다. URL 상태별로 탭을 전환하여 확인하세요.
                </p>
                <ProductTable products={getCurrentTabProducts()} />
              </div>
            </Show>

            <Show when={selectedTab() === 'exists'}>
              <div class="bg-green-50 border border-green-200 rounded p-3">
                <div class="text-xs text-green-800 mb-2 font-semibold">
                  ✅ URL이 실제로 존재하는 제품 ({report()!.verified_exists.length}개)
                </div>
                <p class="text-[11px] text-green-700 mb-3">
                  이 제품들은 사이트에 여전히 존재합니다. 재크롤링으로 좌표를 할당하거나, 불필요하면 삭제할 수 있습니다.
                </p>
                <ProductTable products={getCurrentTabProducts()} />
              </div>
            </Show>

            <Show when={selectedTab() === 'missing'}>
              <div class="bg-red-50 border border-red-200 rounded p-3">
                <div class="text-xs text-red-800 mb-2 font-semibold">
                  ❌ URL이 존재하지 않는 제품 ({report()!.verified_missing.length}개)
                </div>
                <p class="text-[11px] text-red-700 mb-3">
                  이 제품들은 사이트에서 삭제되었거나 접근할 수 없습니다. 삭제를 권장합니다.
                </p>
                <ProductTable products={getCurrentTabProducts()} />
              </div>
            </Show>

            <Show when={selectedTab() === 'not_verified'}>
              <div class="bg-yellow-50 border border-yellow-200 rounded p-3">
                <div class="text-xs text-yellow-800 mb-2 font-semibold">
                  ⚠️ URL 확인 실패 ({report()!.not_verified.length}개)
                </div>
                <p class="text-[11px] text-yellow-700 mb-3">
                  네트워크 오류 등으로 URL 접근 가능 여부를 확인할 수 없습니다. 제품명을 클릭하여 수동으로 확인하세요.
                </p>
                <ProductTable products={getCurrentTabProducts()} />
              </div>
            </Show>
          </div>
        </Show>

        <Show when={!report() && !loading()}>
          <div class="text-center py-8 text-gray-500">
            <p class="mb-2">제품 목록을 로드하려면 "새로고침" 버튼을 클릭하세요</p>
            <p class="text-xs">URL 검증을 포함하여 약 {Math.ceil(p.nullCoordsCount() * 0.1)}초 소요됩니다</p>
          </div>
        </Show>
      </div>
    </Show>
  );
};

export default NullCoordinatesPanel;
