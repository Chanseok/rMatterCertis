/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, For, onMount, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { localDbDashboardStore, initializeLocalDbDashboard } from '../../stores/localDbDashboardStore';
import { listen } from '@tauri-apps/api/event';
import type { VendorSyncResult } from '../../types/domain';

export const LocalDBTab: Component = () => {
  // Phase 6B: 통합된 Summary / Analytics / Maintenance UI
  const s = localDbDashboardStore.summary;
  const analytics = localDbDashboardStore.analytics;
  const ui = localDbDashboardStore.ui;

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

  // Analytics 테이블 페이지 계산
  const totalPages = () => Math.max(1, Math.ceil(analytics.total / analytics.limit));

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
  let selectedDataset: 'vendors' | 'device_types' = 'vendors';
  const handleImport = () => {
    const ta = document.getElementById('import-text') as HTMLTextAreaElement | null;
    if (!ta) return;
    localDbDashboardStore.importDataset(selectedDataset, ta.value);
  };
  const handleDeletePreview = () => {
    const fromEl = document.getElementById('del-from') as HTMLInputElement | null;
    const toEl = document.getElementById('del-to') as HTMLInputElement | null;
    if (!fromEl || !toEl) return;
    const from = Number(fromEl.value); const to = Number(toEl.value);
    if (Number.isNaN(from) || Number.isNaN(to)) return alert('숫자를 입력하세요');
    localDbDashboardStore.previewDeleteRange(from, to);
  };
  const handleDeleteExecute = () => {
    const fromEl = document.getElementById('del-from') as HTMLInputElement | null;
    const toEl = document.getElementById('del-to') as HTMLInputElement | null;
    if (!fromEl || !toEl) return;
    const from = Number(fromEl.value); const to = Number(toEl.value);
    localDbDashboardStore.executeDeleteRange(from, to);
  };

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h2 class="text-2xl md:text-3xl font-bold text-gray-800">🗄️ 로컬DB (통합 대시보드)</h2>
          <p class="text-sm text-gray-500 mt-1">Phase 6B: 단일 탭 재구성 / DSL 필터 자리 확보</p>
        </div>

        {/* Summary */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-lg font-semibold text-gray-800">요약 지표</h3>
            <button class="px-3 py-1.5 rounded bg-indigo-600 text-white text-sm" onClick={() => localDbDashboardStore.loadSummary()}>새로고침</button>
          </div>
          <Show when={!ui.loadingSummary && s()} fallback={<div class="text-sm text-gray-500">요약 로딩 중...</div>}>
            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
              <For each={Object.entries(s()!).filter(([k]) => k !== 'top_device_categories')}>
                {([k,v]) => (
                  <div class="rounded-xl border bg-gradient-to-br from-gray-50 to-white p-4 shadow-sm">
                    <div class="text-[10px] uppercase tracking-wide text-gray-500">{k}</div>
                    <div class="text-xl font-bold text-gray-800">{String(v)}</div>
                  </div>
                )}
              </For>
            </div>
            <div class="mt-4">
              <div class="text-xs font-semibold text-gray-600 mb-1">Top Categories</div>
              <div class="flex flex-wrap gap-2">
                <For each={s()!.top_device_categories}>{(c) => <span class="px-2 py-0.5 bg-blue-100 text-blue-700 rounded text-xs">{c[0]}: {c[1]}</span>}</For>
                <Show when={s()!.top_device_categories.length===0}><span class="text-xs text-gray-400">없음</span></Show>
              </div>
            </div>
          </Show>
          <Show when={ui.summaryError}><div class="text-sm text-rose-600 mt-2">{ui.summaryError}</div></Show>
        </div>

        {/* Analytics + DSL Filter Placeholder */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 space-y-4">
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold text-gray-800">Analytics</h3>
            <div class="flex gap-2">
              <input class="px-3 py-1.5 border rounded text-sm" placeholder="필터 DSL (예: vendor:Philips date>=2024-01-01)" value={analytics.filterDraft} onInput={e => localDbDashboardStore.setAnalytics({ ...analytics, filterDraft: e.currentTarget.value })} />
              <button class="px-3 py-1.5 rounded bg-indigo-600 text-white text-sm" onClick={() => localDbDashboardStore.applyFilter()} disabled={analytics.loading}>적용</button>
              <button class="px-3 py-1.5 rounded bg-gray-200 text-gray-700 text-sm" onClick={() => localDbDashboardStore.resetFilter()} disabled={analytics.loading || (!analytics.filterApplied && !analytics.filterDraft)}>초기화</button>
            </div>
          </div>
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
                  <th class="p-2 text-left">URL</th>
                  <th class="p-2 text-left">Model</th>
                  <th class="p-2 text-left">Vendor</th>
                  <th class="p-2 text-left">Device Type</th>
                  <th class="p-2 text-left">Category</th>
                  <th class="p-2 text-left">Cert Date</th>
                  <th class="p-2 text-left">Created</th>
                </tr>
              </thead>
              <tbody>
                <Show when={!analytics.loading && analytics.rows.length === 0}>
                  <tr><td class="p-4 text-center text-gray-400" colSpan={7}>행 없음</td></tr>
                </Show>
                <For each={analytics.rows}>{r => (
                  <tr class="border-t border-gray-100">
                    <td class="p-2 max-w-[180px] truncate" title={r.product_detail_url}>{r.product_detail_url}</td>
                    <td class="p-2">{r.model}</td>
                    <td class="p-2">{r.vendor_name}</td>
                    <td class="p-2">{r.device_type_name}</td>
                    <td class="p-2">{r.device_category}</td>
                    <td class="p-2">{r.certification_date}</td>
                    <td class="p-2">{r.detail_created_at}</td>
                  </tr>
                )}</For>
              </tbody>
            </table>
            <Show when={analytics.loading}>
              <div class="p-6 text-center text-gray-500 text-sm">로딩 중...</div>
            </Show>
          </div>
          <div class="flex items-center gap-3 text-sm">
            <button class="px-2 py-1 rounded border" disabled={analytics.offset === 0 || analytics.loading} onClick={() => localDbDashboardStore.loadAnalytics(Math.max(0, analytics.offset - analytics.limit))}>이전</button>
            <div>페이지 {Math.floor(analytics.offset / analytics.limit) + 1} / {totalPages()}</div>
            <button class="px-2 py-1 rounded border" disabled={analytics.loading || analytics.offset + analytics.limit >= analytics.total} onClick={() => localDbDashboardStore.loadAnalytics(analytics.offset + analytics.limit)}>다음</button>
            <div class="ml-auto text-xs text-gray-400">총 {analytics.total} 행 / limit {analytics.limit}</div>
          </div>
        </div>

        {/* Maintenance (Export / Import / Delete Range) */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-indigo-200 p-6 space-y-4">
          <h3 class="text-lg font-semibold text-gray-800">유지보수 (Export / Import / Delete Range)</h3>
          <div class="space-y-2">
            <div class="flex flex-wrap gap-2 items-center">
              <button class="px-3 py-1.5 rounded bg-indigo-600 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('vendors')}>Export Vendors</button>
              <button class="px-3 py-1.5 rounded bg-indigo-600 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('device_types')}>Export Device Types</button>
              <button class="px-3 py-1.5 rounded bg-indigo-600 text-white text-xs" onClick={() => localDbDashboardStore.exportDataset('analytics')}>Export Analytics</button>
              <span class="text-xs text-gray-500">{localDbDashboardStore.ui.exportStatus}</span>
            </div>
            <div class="grid md:grid-cols-2 gap-4">
              <div class="space-y-2">
                <div class="text-xs font-medium text-gray-600">CSV Import</div>
                <select id="import-dataset" class="px-2 py-1 border rounded text-xs" onChange={e => (selectedDataset = e.currentTarget.value as any)}>
                  <option value="vendors">vendors</option>
                  <option value="device_types">device_types</option>
                </select>
                <textarea id="import-text" class="w-full h-28 text-xs font-mono border rounded p-2" placeholder="CSV 붙여넣기" />
                <div class="flex gap-2">
                  <button class="px-3 py-1.5 rounded bg-emerald-600 text-white text-xs" onClick={() => handleImport()}>Import</button>
                  <span class="text-xs text-gray-500">{localDbDashboardStore.ui.importStatus}</span>
                </div>
                <pre class="bg-gray-900 text-[10px] text-green-300 p-2 rounded max-h-40 overflow-auto">{localDbDashboardStore.ui.importLog}</pre>
              </div>
              <div class="space-y-2">
                <div class="text-xs font-medium text-gray-600">Delete Range (페이지)</div>
                <div class="flex gap-2 items-center flex-wrap text-xs">
                  <input id="del-from" type="number" placeholder="from" class="w-24 px-2 py-1 border rounded" />
                  <input id="del-to" type="number" placeholder="to" class="w-24 px-2 py-1 border rounded" />
                  <button class="px-3 py-1.5 rounded bg-amber-600 text-white" onClick={() => handleDeletePreview()}>Preview</button>
                  <button class="px-3 py-1.5 rounded bg-rose-600 text-white disabled:opacity-40" disabled={!localDbDashboardStore.ui.deletePreview || localDbDashboardStore.ui.deletePreview.error} onClick={() => handleDeleteExecute()}>Delete</button>
                </div>
                <pre class="bg-gray-100 text-[10px] text-gray-700 p-2 rounded max-h-32 overflow-auto">{JSON.stringify(localDbDashboardStore.ui.deletePreview || {}, null, 2)}</pre>
                <pre class="bg-gray-100 text-[10px] text-blue-700 p-2 rounded max-h-32 overflow-auto">{JSON.stringify(localDbDashboardStore.ui.deleteResult || {}, null, 2)}</pre>
              </div>
            </div>
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

        {/* Device Types Editor */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-fuchsia-200 p-6 space-y-3">
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold text-gray-800">Device Types JSON Editor</h3>
            <div class="flex items-center gap-3 text-xs text-gray-500">
              <span>파일 개수: {localDbDashboardStore.ui.deviceTypesMeta?.count_in_file}</span>
              <span>DB: {localDbDashboardStore.ui.deviceTypesMeta?.count_in_db}</span>
            </div>
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
      </div>
    </div>
  );
};
