import { Component, Show, For } from 'solid-js';
import NullCoordinatesPanel from './NullCoordinatesPanel.tsx';
import type { DiagnosticsResult } from '../../../types/diagnostics';

interface Props {
  diagResult: () => DiagnosticsResult | null;
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
  // 동기화 핸들러
  handleShallowSync?: () => Promise<void> | void;
  handleComplementCrawl?: () => Promise<void> | void;
}

const DiagnosticsPanel: Component<Props> = (p) => {
  return (
    <>
      <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 mb-8">
       
        {/* 동기화 및 진단 버튼 그룹 */}
        <div class="flex flex-wrap gap-2 mb-4 items-center">
          {p.handleShallowSync && (
            <button
              onClick={() => p.handleShallowSync?.()}
              disabled={p.isSyncing()}
              class="px-4 py-2 rounded-lg font-semibold text-white bg-gradient-to-r from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600 disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg transition text-sm"
              title="전체 페이지의 리스트만 크롤링하여 제품 URL과 좌표(page_id, index_in_page)를 DB에 업데이트합니다. 제품 상세 정보는 수집하지 않으므로 빠르게 완료됩니다. (예상 시간: 5-8분)"
            >
              📍 좌표 갱신
            </button>
          )}

          {p.handleComplementCrawl && (
            <button
              onClick={() => p.handleComplementCrawl?.()}
              disabled={p.isSyncing()}
              class="px-4 py-2 rounded-lg font-semibold text-white bg-gradient-to-r from-green-500 to-teal-500 hover:from-green-600 hover:to-teal-600 disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg transition text-sm"
              title="핵심 필드(certification_date, transport_interface, primary_device_type_ids) 중 하나라도 누락된 제품만 선별하여 재크롤링합니다. 리스트 크롤링을 건너뛰므로 매우 빠릅니다. (예상 시간: 2-5분)"
            >
              🔧 제품 보완
            </button>
          )}

          {/* 구분자 */}
          <div class="h-8 w-px bg-gray-300 mx-1" />

          {/* 진단 실행 버튼 */}
          <button
            class={`px-4 py-2 rounded-lg font-semibold text-white shadow-md hover:shadow-lg transition text-sm ${p.diagLoading() ? 'bg-gray-400 cursor-not-allowed' : 'bg-gradient-to-r from-indigo-500 to-purple-500 hover:from-indigo-600 hover:to-purple-600'}`}
            disabled={p.diagLoading()}
            onClick={p.runDiagnostics}
            title="로컬 DB의 page_id/index_in_page 정합성을 검사합니다."
          >
            {p.diagLoading() ? '🔍 진단 중...' : '🔍 진단 실행'}
          </button>

          {/* 진단 설명 */}
          <p class="text-xs text-gray-500 ml-2">
            로컬 DB의 page_id/index_in_page 정합성을 검사합니다. 실행을 눌러 결과를 확인하세요.
          </p>
        </div>
        
        <Show when={p.diagResult()}>
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
          
          {/* 제품 수 통계 - 상세 구분 */}
          <div class="grid grid-cols-2 gap-2">
            <div class="p-2 bg-blue-50 border border-blue-200 rounded">
              <div class="text-xs text-blue-600 font-semibold mb-1">🌐 사이트</div>
              <div class="flex flex-col gap-1 text-xs">
                <span>총 제품: <b class="text-blue-700">{p.diagResult()?.total_products_site?.toLocaleString() ?? '계산 중...'}</b></span>
                <span>총 페이지: <b class="text-blue-700">{p.diagResult()?.total_pages_site ?? '-'}</b></span>
                <span>마지막 페이지 아이템: <b class="text-blue-700">{p.diagResult()?.items_on_last_page ?? '-'}</b></span>
              </div>
            </div>
            
            <div class="p-2 bg-green-50 border border-green-200 rounded">
              <div class="text-xs text-green-600 font-semibold mb-1">💾 로컬 DB</div>
              <div class="flex flex-col gap-1 text-xs">
                <span>총 제품: <b class="text-green-700">{p.diagResult()?.total_products?.toLocaleString() ?? 0}</b></span>
                <span class="text-emerald-700">├ 좌표 있음: <b>{p.diagResult()?.total_products_with_coords?.toLocaleString() ?? 0}</b></span>
                <span class="text-orange-700">└ 좌표 없음: <b>{p.diagResult()?.total_products_without_coords?.toLocaleString() ?? 0}</b></span>
                <span class="text-gray-600 text-[10px]">DB 최대 page_id: {p.diagResult()?.max_page_id_db ?? '-'}</span>
              </div>
            </div>
          </div>
          
          <Show when={(p.diagResult()?.total_products_without_coords ?? 0) > 0}>
            <div class="p-2 bg-orange-50 border border-orange-300 rounded">
              <div class="text-orange-800 text-xs">
                ⚠️ <b>{p.diagResult()?.total_products_without_coords}</b>개 제품이 좌표(page_id, index_in_page) 정보가 없습니다.
                <span class="ml-2 text-orange-600">→ 아래 "좌표 없는 제품 관리" 섹션 참조</span>
              </div>
            </div>
          </Show>
          
          <div class="flex gap-4 text-[10px] text-gray-500 border-t pt-1">
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
              <For each={(p.diagResult()?.group_summaries ?? []).filter((g) => g.status !== 'ok')}>
                {(g) => (
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
          <Show when={(p.diagResult()?.missing_pages ?? []).length > 0}>
            <div class="bg-red-50 border border-red-200 rounded p-2">
              <b class="text-red-800">🚨 누락된 페이지 시퀀스</b> (총 {p.diagResult()?.total_missing_pages ?? 0}개 페이지)
              <ul class="list-disc ml-5 text-red-700">
                <For each={(p.diagResult()?.missing_pages ?? []).slice(0, 20)}>
                  {(gap) => (
                    <li>
                      {gap.gap_type === 'single' 
                        ? `page_id ${gap.start_page}${gap.start_physical_page != null ? ` (물리 ${gap.start_physical_page})` : ''} 누락`
                        : `page_id ${gap.start_page}~${gap.end_page}${gap.start_physical_page != null && gap.end_physical_page != null ? ` (물리 ${gap.end_physical_page}~${gap.start_physical_page})` : ''} 범위 누락 (${gap.missing_count}개 페이지)`
                      }
                    </li>
                  )}
                </For>
                <Show when={(p.diagResult()?.missing_pages ?? []).length > 20}>
                  <li class="text-gray-600">... 추가 {(p.diagResult()?.missing_pages ?? []).length - 20}개 갭</li>
                </Show>
              </ul>
            </div>
          </Show>
          <Show when={(p.diagResult()?.duplicate_positions ?? []).length > 0}>
            <div>
              <b>중복 위치 샘플</b>
              <ul class="list-disc ml-5">
                <For each={(p.diagResult()?.duplicate_positions ?? []).slice(0, 20)}>
                  {(d) => (
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
      
      {/* NULL 좌표 제품 관리 패널 */}
      <NullCoordinatesPanel 
        nullCoordsCount={() => p.diagResult()?.total_products_without_coords ?? 0}
        addLog={p.addLog}
        onRefresh={async () => { await p.runDiagnostics(); }}
      />
    </>
  );
};

export default DiagnosticsPanel;
