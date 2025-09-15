import { Component, Show, For } from 'solid-js';

interface Props {
  diagResult: () => any;
  diagLoading: () => boolean;
  cleanupLoading: () => boolean;
  runDiagnostics: () => void | Promise<void>;
  runUrlCleanup: () => void | Promise<void>;
  deriveRangesFromDiagnostics: () => string | null;
  setSyncRanges: (expr: string) => void;
  setSyncPulse: (v: boolean) => void;
  addLog: (msg: string) => void;
  isSyncing: () => boolean;
  startCoordSync: () => Promise<void>;
}

const DiagnosticsPanel: Component<Props> = (p) => {
  return (
    <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 mb-8">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-lg font-bold text-gray-800">Stage X: DB Pagination Diagnostics</h3>
        <div class="flex gap-2">
          <button
            class={`px-3 py-1.5 text-sm rounded-lg shadow ${p.diagLoading() ? 'bg-gray-200 text-gray-500' : 'bg-indigo-600 text-white hover:bg-indigo-700'}`}
            disabled={p.diagLoading()}
            onClick={p.runDiagnostics}
          >
            {p.diagLoading() ? '진단 중…' : '진단 실행'}
          </button>
          <button
            class={`px-3 py-1.5 text-sm rounded-lg shadow ${p.cleanupLoading() ? 'bg-gray-200 text-gray-500' : 'bg-rose-600 text-white hover:bg-rose-700'}`}
            disabled={p.cleanupLoading()}
            onClick={p.runUrlCleanup}
          >
            {p.cleanupLoading() ? '정리 중…' : 'URL 중복 제거'}
          </button>
          <button
            class={`px-3 py-1.5 text-sm rounded-lg shadow ${p.isSyncing() ? 'bg-gray-200 text-gray-500' : 'bg-blue-600 text-white hover:bg-blue-700'}`}
            disabled={p.isSyncing()}
            onClick={p.startCoordSync}
            title="products.url 기준으로 product_details에 page_id/index_in_page/id를 정합화합니다 (크롤링 없음)"
          >
            products→details 동기화
          </button>
        </div>
      </div>
      <Show when={p.diagResult()} fallback={<p class="text-xs text-gray-500">로컬 DB의 page_id/index_in_page 정합성을 검사합니다. 실행을 눌러 결과를 확인하세요.</p>}>
        <div class="text-xs text-gray-700 space-y-2">
          {(() => {
            const expr = p.deriveRangesFromDiagnostics();
            if (!expr) return null;
            return (
              <div class="p-2 rounded border border-amber-200 bg-amber-50 text-amber-900 flex items-center justify-between">
                <div>
                  <b>추천 Sync 범위</b>: <span class="font-mono">{expr}</span>
                </div>
                <div class="flex items-center gap-2">
                  <button
                    class="px-2 py-0.5 text-[11px] rounded bg-amber-600 text-white hover:bg-amber-700"
                    title="추천 범위를 Sync 입력에 적용"
                    onClick={() => {
                      p.setSyncRanges(expr);
                      p.setSyncPulse(true);
                      setTimeout(() => p.setSyncPulse(false), 400);
                      p.addLog(`🧭 추천 범위 적용 → ${expr}`);
                    }}
                  >
                    적용
                  </button>
                </div>
              </div>
            );
          })()}
          <div class="flex gap-4">
            <span>총 제품: <b>{p.diagResult()?.total_products ?? 0}</b></span>
            <span>DB 최대 page_id: <b>{p.diagResult()?.max_page_id_db ?? '-'}</b></span>
            <span>사이트 총 페이지: <b>{p.diagResult()?.total_pages_site ?? '-'}</b></span>
            <span>마지막 페이지 아이템: <b>{p.diagResult()?.items_on_last_page ?? '-'}</b></span>
          </div>
          <Show when={p.diagResult()?.prepass}>
            <div class="flex gap-4 text-teal-800 bg-teal-50 border border-teal-200 rounded p-2">
              <span>사전 정렬(details): <b>{p.diagResult()?.prepass?.details_aligned ?? 0}</b></span>
              <span>products.id 백필: <b>{p.diagResult()?.prepass?.products_id_backfilled ?? 0}</b></span>
            </div>
          </Show>
          <div>
            <b>이상 그룹</b>
            <ul class="list-disc ml-5">
              <For each={(p.diagResult()?.group_summaries ?? []).filter((g: any) => g.status !== 'ok')}>
                {(g: any) => (
                  <li>
                    page_id {g.page_id}
                    {g.current_page_number != null ? ` (물리 ${g.current_page_number})` : ''}
                    : status={g.status} count={g.count} distinct={g.distinct_indices}
                    {g.duplicate_indices?.length ? ` dup=${g.duplicate_indices.join(',')}` : ''}
                    {g.missing_indices?.length ? ` miss=${g.missing_indices.join(',')}` : ''}
                    {g.out_of_range_count ? ` oob=${g.out_of_range_count}` : ''}
                  </li>
                )}
              </For>
            </ul>
          </div>
          <Show when={(p.diagResult()?.duplicate_positions ?? []).length > 0}>
            <div>
              <b>중복 위치 샘플</b>
              <ul class="list-disc ml-5">
                <For each={(p.diagResult()?.duplicate_positions ?? []).slice(0, 20)}>
                  {(d: any) => (
                    <li>
                      page_id {d.page_id}
                      {d.current_page_number != null ? ` (물리 ${d.current_page_number})` : ''}, index {d.index_in_page}: {d.urls?.length ?? 0}개 URL
                    </li>
                  )}
                </For>
              </ul>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default DiagnosticsPanel;
