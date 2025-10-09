import { Component, Show, For, createSignal } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';

interface OrphanProduct {
  url: string;
  manufacturer: string | null;
  model: string | null;
  page_id: number | null;
  index_in_page: number | null;
}

interface TableInconsistencyReport {
  products_count: number;
  details_count: number;
  only_in_products: OrphanProduct[];
  only_in_details: OrphanProduct[];
}

interface Props {
  addLog: (msg: string) => void;
  onRefresh: () => Promise<void>;
}

const TableConsistencyPanel: Component<Props> = (p) => {
  const [loading, setLoading] = createSignal(false);
  const [report, setReport] = createSignal<TableInconsistencyReport | null>(null);
  const [selectedTab, setSelectedTab] = createSignal<'products' | 'details'>('products');
  const [selectedUrls, setSelectedUrls] = createSignal<Set<string>>(new Set());
  const [deleting, setDeleting] = createSignal(false);

  const checkConsistency = async () => {
    setLoading(true);
    p.addLog('🔍 테이블 일관성 체크 시작...');

    try {
      const result: TableInconsistencyReport = await invoke('check_table_consistency');
      setReport(result);
      
      const diff = Math.abs(result.products_count - result.details_count);
      if (diff === 0 && result.only_in_products.length === 0 && result.only_in_details.length === 0) {
        p.addLog('✅ 테이블 일관성 정상: products와 product_details가 완벽하게 일치합니다.');
      } else {
        p.addLog(`⚠️ 테이블 불일치 감지:`);
        p.addLog(`  - products: ${result.products_count}개`);
        p.addLog(`  - product_details: ${result.details_count}개`);
        p.addLog(`  - products에만 있는 제품: ${result.only_in_products.length}개`);
        p.addLog(`  - product_details에만 있는 제품: ${result.only_in_details.length}개`);
      }
    } catch (e: any) {
      p.addLog(`❌ 일관성 체크 실패: ${e?.message || e}`);
      setReport(null);
    } finally {
      setLoading(false);
    }
  };

  const deleteSelected = async () => {
    const urls = Array.from(selectedUrls());
    if (urls.length === 0) {
      p.addLog('⚠️ 삭제할 제품을 선택해주세요');
      return;
    }

    const table = selectedTab() === 'products' ? 'products' : 'product_details';
    const tableName = table === 'products' ? 'products 테이블' : 'product_details 테이블';

    const confirmed = await ask(
      `선택한 ${urls.length}개 제품을 ${tableName}에서 삭제하시겠습니까?\n\n⚠️ 주의: ${tableName}에서만 삭제됩니다.\n이 작업은 되돌릴 수 없습니다.`,
      {
        title: 'rMatterCertis',
        kind: 'warning'
      }
    );

    if (!confirmed) {
      return;
    }

    setDeleting(true);
    p.addLog(`🗑️ ${tableName}에서 ${urls.length}개 제품 삭제 시작...`);

    try {
      const deletedCount: number = await invoke('delete_orphan_records', { urls, table });
      p.addLog(`✅ ${deletedCount}개 제품 삭제 완료 (${tableName})`);

      // 선택 초기화 및 새로고침
      setSelectedUrls(new Set<string>());
      await checkConsistency();
      await p.onRefresh();
    } catch (e: any) {
      p.addLog(`❌ 삭제 실패: ${e?.message || e}`);
    } finally {
      setDeleting(false);
    }
  };

  const deleteAll = async (category: 'products' | 'details') => {
    const r = report();
    if (!r) return;

    const products = category === 'products' ? r.only_in_products : r.only_in_details;
    const urls = products.map(p => p.url);

    if (urls.length === 0) {
      p.addLog('⚠️ 삭제할 제품이 없습니다');
      return;
    }

    const table = category === 'products' ? 'products' : 'product_details';
    const tableName = table === 'products' ? 'products 테이블' : 'product_details 테이블';

    const confirmed = await ask(
      `${tableName}에만 있는 ${urls.length}개 고아 레코드를 모두 삭제하시겠습니까?\n\n⚠️ 주의: ${tableName}에서만 삭제됩니다.\n이 작업은 되돌릴 수 없습니다.`,
      {
        title: 'rMatterCertis',
        kind: 'warning'
      }
    );

    if (!confirmed) {
      return;
    }

    setDeleting(true);
    p.addLog(`🗑️ ${tableName}에서 ${urls.length}개 고아 레코드 삭제 시작...`);

    try {
      const deletedCount: number = await invoke('delete_orphan_records', { urls, table });
      p.addLog(`✅ ${deletedCount}개 제품 삭제 완료 (${tableName})`);

      // 선택 초기화 및 새로고침
      setSelectedUrls(new Set<string>());
      await checkConsistency();
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

  const selectAll = () => {
    const r = report();
    if (!r) return;

    const products = selectedTab() === 'products' ? r.only_in_products : r.only_in_details;
    const urls = products.map(p => p.url);
    setSelectedUrls(new Set(urls));
  };

  const deselectAll = () => {
    setSelectedUrls(new Set<string>());
  };

  return (
    <div class="mt-4 p-4 bg-gradient-to-br from-yellow-50 to-orange-50 border border-yellow-200 rounded-lg">
      <div class="flex items-center justify-between mb-3">
        <h3 class="text-sm font-bold text-yellow-900 flex items-center gap-2">
          ⚖️ 테이블 일관성 체크
        </h3>
        <button
          onClick={checkConsistency}
          disabled={loading()}
          class="px-3 py-1 text-xs rounded bg-yellow-600 text-white hover:bg-yellow-700 disabled:opacity-50 disabled:cursor-not-allowed transition"
        >
          {loading() ? '🔍 체크 중...' : '🔍 일관성 체크'}
        </button>
      </div>

      <Show when={report()}>
        {(r) => (
          <div class="space-y-3">
            {/* 통계 요약 */}
            <div class="grid grid-cols-2 gap-2 text-xs">
              <div class="p-2 bg-blue-50 border border-blue-200 rounded">
                <div class="font-semibold text-blue-900">products 테이블</div>
                <div class="text-blue-700">{r().products_count.toLocaleString()}개</div>
              </div>
              <div class="p-2 bg-purple-50 border border-purple-200 rounded">
                <div class="font-semibold text-purple-900">product_details 테이블</div>
                <div class="text-purple-700">{r().details_count.toLocaleString()}개</div>
              </div>
            </div>

            {/* 불일치 경고 */}
            <Show when={r().products_count !== r().details_count || r().only_in_products.length > 0 || r().only_in_details.length > 0}>
              <div class="p-3 bg-red-50 border border-red-200 rounded">
                <div class="font-semibold text-red-900 mb-2">⚠️ 테이블 불일치 감지</div>
                <div class="text-xs text-red-700 space-y-1">
                  <div>• products에만 있는 제품: <span class="font-bold">{r().only_in_products.length}개</span></div>
                  <div>• product_details에만 있는 제품: <span class="font-bold">{r().only_in_details.length}개</span></div>
                  <div class="mt-2 text-red-600">
                    💡 이런 "고아 레코드"는 크롤링 중단, 삭제 트랜잭션 실패 등으로 발생할 수 있습니다.
                  </div>
                </div>
              </div>
            </Show>

            {/* 탭 선택 */}
            <Show when={r().only_in_products.length > 0 || r().only_in_details.length > 0}>
              <div class="flex gap-2 border-b border-yellow-300">
                <button
                  class={`px-3 py-2 text-xs font-semibold transition ${
                    selectedTab() === 'products'
                      ? 'border-b-2 border-yellow-600 text-yellow-900'
                      : 'text-yellow-700 hover:text-yellow-900'
                  }`}
                  onClick={() => {
                    setSelectedTab('products');
                    deselectAll();
                  }}
                >
                  products에만 있음 ({r().only_in_products.length})
                </button>
                <button
                  class={`px-3 py-2 text-xs font-semibold transition ${
                    selectedTab() === 'details'
                      ? 'border-b-2 border-yellow-600 text-yellow-900'
                      : 'text-yellow-700 hover:text-yellow-900'
                  }`}
                  onClick={() => {
                    setSelectedTab('details');
                    deselectAll();
                  }}
                >
                  product_details에만 있음 ({r().only_in_details.length})
                </button>
              </div>

              {/* 액션 버튼 */}
              <div class="flex gap-2 items-center">
                <button
                  onClick={selectAll}
                  class="px-2 py-1 text-xs rounded bg-gray-500 text-white hover:bg-gray-600 transition"
                >
                  전체 선택
                </button>
                <button
                  onClick={deselectAll}
                  class="px-2 py-1 text-xs rounded bg-gray-400 text-white hover:bg-gray-500 transition"
                >
                  선택 해제
                </button>
                <button
                  onClick={deleteSelected}
                  disabled={selectedUrls().size === 0 || deleting()}
                  class="px-2 py-1 text-xs rounded bg-red-600 text-white hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition"
                >
                  선택 삭제 ({selectedUrls().size})
                </button>
                <button
                  onClick={() => deleteAll(selectedTab())}
                  disabled={deleting()}
                  class="px-2 py-1 text-xs rounded bg-red-700 text-white hover:bg-red-800 disabled:opacity-50 disabled:cursor-not-allowed transition"
                >
                  전체 삭제
                </button>
              </div>

              {/* 제품 목록 */}
              <div class="max-h-80 overflow-y-auto border border-yellow-200 rounded bg-white">
                <table class="min-w-full text-xs">
                  <thead class="bg-yellow-100 sticky top-0">
                    <tr>
                      <th class="px-2 py-1 text-left">
                        <input
                          type="checkbox"
                          checked={selectedUrls().size > 0 && selectedUrls().size === (selectedTab() === 'products' ? r().only_in_products.length : r().only_in_details.length)}
                          onChange={(e) => e.currentTarget.checked ? selectAll() : deselectAll()}
                          class="cursor-pointer"
                        />
                      </th>
                      <th class="px-2 py-1 text-left font-semibold">제조사</th>
                      <th class="px-2 py-1 text-left font-semibold">모델</th>
                      <th class="px-2 py-1 text-left font-semibold">좌표</th>
                      <th class="px-2 py-1 text-left font-semibold">URL</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={selectedTab() === 'products' ? r().only_in_products : r().only_in_details}>
                      {(product) => (
                        <tr class="border-t border-yellow-100 hover:bg-yellow-50 transition">
                          <td class="px-2 py-1">
                            <input
                              type="checkbox"
                              checked={selectedUrls().has(product.url)}
                              onChange={() => toggleSelection(product.url)}
                              class="cursor-pointer"
                            />
                          </td>
                          <td class="px-2 py-1 text-gray-700">{product.manufacturer || '-'}</td>
                          <td class="px-2 py-1 text-gray-900 font-medium">{product.model || '-'}</td>
                          <td class="px-2 py-1 text-gray-600">
                            {product.page_id != null && product.index_in_page != null 
                              ? `p${product.page_id}i${product.index_in_page.toString().padStart(2, '0')}`
                              : '-'}
                          </td>
                          <td class="px-2 py-1">
                            <a
                              href={product.url}
                              target="_blank"
                              rel="noopener noreferrer"
                              class="text-blue-600 hover:text-blue-800 underline truncate block max-w-xs"
                              title={product.url}
                            >
                              {product.url.split('/').pop()}
                            </a>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
};

export default TableConsistencyPanel;
