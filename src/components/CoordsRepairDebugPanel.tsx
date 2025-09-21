import { createSignal, Show, For, createMemo, onCleanup, onMount } from 'solid-js';
import { TauriApiService } from '../services/tauri-api';

const api = new TauriApiService();

interface PageZeroEntry { url: string; created_at?: string; detail_has_coords: boolean }

export function CoordsRepairDebugPanel() {
  const [repairResult, setRepairResult] = createSignal<string>('');
  const [pageZero, setPageZero] = createSignal<PageZeroEntry[]>([]);
  const [pageZeroFilterNoDetailCoords, setPageZeroFilterNoDetailCoords] = createSignal(false);
  const [recrawlPagesInput, setRecrawlPagesInput] = createSignal('');
  const [recrawlResult, setRecrawlResult] = createSignal<string>('');
  const [loading, setLoading] = createSignal<string | null>(null);
  const [limit, setLimit] = createSignal<number | undefined>(50);
  const [lastDiagMismatch, setLastDiagMismatch] = createSignal<number | undefined>(undefined);
  const [currentDiagMismatch, setCurrentDiagMismatch] = createSignal<number | undefined>(undefined);
  const [lockErrors, setLockErrors] = createSignal<number>(0); // TODO: hook into global event stream
  const [breakdown, setBreakdown] = createSignal<any | null>(null);
  const [rehydrateResult, setRehydrateResult] = createSignal<string>('');
  const [rehydratePagesInput, setRehydratePagesInput] = createSignal('');
  const [rehydrateDryRun, setRehydrateDryRun] = createSignal(true);

  const filteredPageZero = createMemo(() => {
    const rows = pageZero();
    return pageZeroFilterNoDetailCoords() ? rows.filter(r => !r.detail_has_coords) : rows;
  });

  // Poll lock error counter every 5s (lightweight; backend simple read)
  let lockTimer: number | undefined;
  onMount(() => {
    const poll = async () => {
      try { setLockErrors(await api.getLockErrorCount()); } catch {}
    };
    poll();
    lockTimer = window.setInterval(poll, 5000);
  });
  onCleanup(() => { if (lockTimer) window.clearInterval(lockTimer); });

  async function resetLockCounter() {
    try { await api.resetLockErrorCount(); setLockErrors(0); } catch {}
  }

  async function runRepair() {
    try {
      setLoading('repair');
      const res = await api.repairProductCoordinates(limit());
      setRepairResult(`fixed=${res.fixed}`);
      // Auto run diagnostics to show pre→post diff
      await runDiagnostics(true);
    } catch (e: any) {
      setRepairResult(`error: ${e}`);
    } finally { setLoading(null); }
  }

  async function loadPageZero() {
    try {
      setLoading('pageZero');
      const rows = await api.listPageZeroUrls(200);
      setPageZero(rows);
    } catch (e) {
      console.error(e);
    } finally { setLoading(null); }
  }

  async function recrawlPages() {
    const raw = recrawlPagesInput().trim();
    if (!raw) { setRecrawlResult('no pages'); return; }
    const pages = raw.split(/[,\s]+/).map(v => parseInt(v, 10)).filter(n => !isNaN(n));
    if (!pages.length) { setRecrawlResult('invalid input'); return; }
    try {
      setLoading('recrawl');
      const res = await api.recrawlPhysicalPages(pages);
      setRecrawlResult(`accepted=${res.accepted} count=${res.count}`);
    } catch (e: any) {
      setRecrawlResult(`error: ${e}`);
    } finally { setLoading(null); }
  }

  async function runDiagnostics(skipLoading?: boolean) {
    try {
      if (!skipLoading) setLoading('diagnostics');
      const res: any = await api.scanDbPaginationMismatches();
  // Also fetch breakdown (A)
  try { setBreakdown(await api.coordMismatchBreakdown()); } catch { /* ignore */ }
      const mismatch: number | undefined = res?.coord_mismatch ?? undefined;
      setLastDiagMismatch(currentDiagMismatch());
      setCurrentDiagMismatch(mismatch);
      const parts: string[] = [];
      if (mismatch !== undefined) {
        if (lastDiagMismatch() !== undefined) {
          parts.push(`coord_mismatch=${lastDiagMismatch()}→${mismatch}`);
        } else {
          parts.push(`coord_mismatch=${mismatch}`);
        }
      } else {
        parts.push('coord_mismatch=NA');
      }
      if (res?.details_missing_coords !== undefined) parts.push(`details_missing=${res.details_missing_coords}`);
      if (res?.products_missing_coords !== undefined) parts.push(`products_missing=${res.products_missing_coords}`);
      setRepairResult(prev => `${prev ? prev + ' | ' : ''}diag(${parts.join(' ')})`);
    } catch (e) {
      console.error(e);
      setRepairResult(prev => `${prev ? prev + ' | ' : ''}diag_error`);
    } finally { if (!skipLoading) setLoading(null); }
  }

  async function runRehydrate() {
    // pages list optional
    const raw = rehydratePagesInput().trim();
    let pages: number[] | undefined = undefined;
    if (raw) {
      pages = raw.split(/[\s,]+/).map(v => parseInt(v,10)).filter(n => !isNaN(n));
      if (!pages.length) pages = undefined;
    }
    try {
      setLoading('rehydrate');
  const res: any = await api.rehydrateListPages({ pages, dryRun: rehydrateDryRun(), limit: undefined });
  setRehydrateResult(`rehydrate ${rehydrateDryRun() ? 'DRY' : 'RUN'} auto=${res.auto_selected} targeted=${res.pages_targeted} processed=${res.pages_processed} filled=${res.products_filled} would_fill=${res.would_fill} already=${res.products_already_had_coords} still_null_before=${res.products_with_null_coords_before} still_null_after=${res.products_still_null_after} mismatches=${res.mismatches_detected} failed=${res.pages_failed} http_err=${res.http_errors} ${res.elapsed_ms}ms note=${res.note}`);
      // auto run diagnostics after a non-dry run
      if (!rehydrateDryRun()) await runDiagnostics(true);
    } catch (e: any) {
      setRehydrateResult(`rehydrate_error: ${e}`);
    } finally { setLoading(null); }
  }

  return (
    <div class="p-4 rounded-xl text-sm space-y-3 bg-gradient-to-br from-gray-900 via-gray-850 to-gray-800 border border-gray-700 shadow-inner">
      <h3 class="font-semibold text-gray-100 flex items-center gap-2">
        <span class="text-lg">🛠️</span>
        <span>Coordinate Repair & Diagnostics</span>
        <Show when={lockErrors() > 0}>
          <button class="ml-2 inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-rose-700/60 text-rose-200 text-[10px] border border-rose-500/50 hover:bg-rose-600/60 transition" title="Detected database lock errors (session) - click to reset" onClick={resetLockCounter}>
            LOCK {lockErrors()}
          </button>
        </Show>
        <Show when={loading()}><span class="ml-2 animate-pulse text-xs text-amber-400">{loading()}...</span></Show>
      </h3>
      <div class="flex items-center gap-2">
        <label class="text-gray-300 text-xs uppercase tracking-wide">Limit</label>
        <input type="number" class="w-24 bg-gray-800/70 focus:bg-gray-800 text-gray-100 p-1.5 rounded border border-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500" value={limit() ?? ''} onInput={e => setLimit(e.currentTarget.value ? parseInt(e.currentTarget.value,10) : undefined)} />
        <button class="px-3 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium disabled:opacity-40" disabled={loading()==='repair'} onClick={runRepair}>Repair</button>
        <button class="px-3 py-1.5 rounded bg-teal-600 hover:bg-teal-500 text-white text-xs font-medium disabled:opacity-40" disabled={loading()==='diagnostics'} onClick={() => runDiagnostics(false)}>Diag</button>
        <button class="px-3 py-1.5 rounded bg-amber-600 hover:bg-amber-500 text-white text-xs font-medium disabled:opacity-40" disabled={loading()==='pageZero'} onClick={loadPageZero}>List page_id=0</button>
        <div class="flex items-center gap-1 ml-2">
          <input placeholder="rehydrate pages" class="w-40 bg-gray-800/70 focus:bg-gray-800 text-gray-100 p-1.5 rounded border border-gray-600 focus:outline-none focus:ring-2 focus:ring-emerald-500" value={rehydratePagesInput()} onInput={e => setRehydratePagesInput(e.currentTarget.value)} />
          <label class="flex items-center gap-1 text-[11px] text-gray-300 select-none cursor-pointer">
            <input type="checkbox" class="accent-emerald-500" checked={rehydrateDryRun()} onInput={e => setRehydrateDryRun(e.currentTarget.checked)} />
            <span>dry-run</span>
          </label>
          <button class="px-3 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-medium disabled:opacity-40" disabled={loading()==='rehydrate'} onClick={runRehydrate}>{rehydrateDryRun() ? 'Rehydrate Dry' : 'Rehydrate'}</button>
        </div>
        <label class="flex items-center gap-1 text-[11px] text-gray-300 ml-2 select-none cursor-pointer">
          <input type="checkbox" class="accent-indigo-500" checked={pageZeroFilterNoDetailCoords()} onInput={e => setPageZeroFilterNoDetailCoords(e.currentTarget.checked)} />
          <span>detail=N 만</span>
        </label>
      </div>
      <div class="flex items-center gap-2">
        <input placeholder="pages (예: 500,501 502)" class="flex-1 bg-gray-800/70 focus:bg-gray-800 text-gray-100 p-1.5 rounded border border-gray-600 focus:outline-none focus:ring-2 focus:ring-purple-500" value={recrawlPagesInput()} onInput={e => setRecrawlPagesInput(e.currentTarget.value)} />
        <button class="px-3 py-1.5 rounded bg-fuchsia-600 hover:bg-fuchsia-500 text-white text-xs font-medium disabled:opacity-40" disabled={loading()==='recrawl'} onClick={recrawlPages}>Recrawl Pages</button>
      </div>
      <div class="grid gap-1 text-[11px] font-mono text-gray-300">
        <Show when={repairResult()}>
          <div class="bg-gray-800/60 rounded px-2 py-1 border border-gray-700">{repairResult()}</div>
        </Show>
        <Show when={breakdown()}>
          <div class="bg-gray-800/40 rounded px-2 py-1 border border-gray-700 flex flex-wrap gap-x-3 gap-y-1">
            <span class="text-gray-400">BD:</span>
            <span>both_null={breakdown()!.both_null}</span>
            <span>prod_null_det_full={breakdown()!.products_null_details_filled}</span>
            <span>det_null_prod_full={breakdown()!.details_null_products_filled}</span>
            <span>value_mismatch={breakdown()!.value_mismatch}</span>
            <span>p0_rows={breakdown()!.page0_total_rows}</span>
            <span>p0_distinct_idx={breakdown()!.page0_distinct_indices}</span>
          </div>
        </Show>
        <Show when={recrawlResult()}>
          <div class="bg-gray-800/60 rounded px-2 py-1 border border-gray-700">{recrawlResult()}</div>
        </Show>
        <Show when={rehydrateResult()}>
          <div class="bg-gray-800/60 rounded px-2 py-1 border border-gray-700">{rehydrateResult()}</div>
        </Show>
      </div>
      <Show when={filteredPageZero().length}>
        <div class="max-h-60 overflow-auto border-t border-gray-700 pt-2">
          <table class="w-full text-xs text-gray-200">
            <thead>
              <tr class="text-left text-gray-400 bg-gray-800/70 sticky top-0 backdrop-blur">
                <th class="py-1 pr-2 font-medium">URL</th>
                <th class="py-1 pr-2 font-medium">created</th>
                <th class="py-1 pr-2 font-medium">detail</th>
              </tr>
            </thead>
            <tbody>
              <For each={filteredPageZero()}>{row => (
                <tr class="border-t border-gray-800 hover:bg-gray-800/60">
                  <td class="truncate max-w-[320px] underline decoration-dotted decoration-gray-600 hover:decoration-indigo-400">{row.url}</td>
                  <td>{row.created_at?.split('T')[0] ?? ''}</td>
                  <td class={row.detail_has_coords ? 'text-emerald-400' : 'text-rose-400'}>{row.detail_has_coords ? 'Y' : 'N'}</td>
                </tr>
              )}</For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
}

export default CoordsRepairDebugPanel;
