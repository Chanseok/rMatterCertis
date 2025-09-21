import { createResource, createSignal, For, Show, createEffect } from 'solid-js';
// NOTE: 6B 재정비: 이 컴포넌트는 곧 `LocalDBTab`으로 통합 예정. 현재는 참조용/제거 대기 상태.
import { tauriApi } from '../services/tauri-api';
import CoordsRepairDebugPanel from './CoordsRepairDebugPanel';

export const LocalDbDashboard = () => {
  const [summary, { refetch: refetchSummary }] = createResource(async () => tauriApi.getDbSummary());
  const [analyticsRows, setAnalyticsRows] = createSignal<any[]>([]);
  const [analyticsTotal, setAnalyticsTotal] = createSignal(0);
  const [analyticsPage, setAnalyticsPage] = createSignal(0);
  const pageSize = 50;
  const [exportStatus, setExportStatus] = createSignal<string>('');
  const [importLog, setImportLog] = createSignal<string>('');
  const [importDataset, setImportDataset] = createSignal<'vendors' | 'device_types'>('vendors');
  const [importText, setImportText] = createSignal('');
  const [deleteFrom, setDeleteFrom] = createSignal('');
  const [deleteTo, setDeleteTo] = createSignal('');
  const [deletePreview, setDeletePreview] = createSignal<any>();
  const [deleteResult, setDeleteResult] = createSignal<any>();
  const [deviceTypesJson, setDeviceTypesJson] = createSignal('');
  const [deviceTypesMeta, setDeviceTypesMeta] = createSignal<any>();
  const [reseed, setReseed] = createSignal(true);
  const [dtSaveResult, setDtSaveResult] = createSignal<any>();
  const [vendorDryRun, setVendorDryRun] = createSignal<any>();
  const [vendorSyncResult, setVendorSyncResult] = createSignal<any>();

  const loadAnalytics = async (page: number) => {
    const res = await tauriApi.analyticsQuery({ offset: page * pageSize, limit: pageSize });
    setAnalyticsRows(res.rows || []);
    setAnalyticsTotal(res.total || 0);
    setAnalyticsPage(page);
  };

  loadAnalytics(0).catch(console.error);

  const loadDeviceTypes = async () => {
    try {
      const res = await tauriApi.getDeviceTypesJson();
      setDeviceTypesJson(res.json || '');
      setDeviceTypesMeta(res);
    } catch (e) { console.error(e); }
  };
  loadDeviceTypes();

  createEffect(() => {
    // subscribe vendor sync progress events
    // minimal: rely on global event audit or future explicit listener.
  });

  const doExport = async (ds: 'vendors' | 'device_types' | 'analytics') => {
    try { const path = await tauriApi.exportDataset(ds); setExportStatus(`Exported ${ds} -> ${path}`); }
    catch(e:any){ setExportStatus(`Export failed: ${e}`); }
  };
  const doImport = async () => {
    try {
      const res = await tauriApi.importDataset(importDataset(), importText());
      setImportLog(JSON.stringify(res, null, 2));
      refetchSummary();
    } catch(e:any){ setImportLog(`Import failed: ${e}`); }
  };
  const doPreviewDelete = async () => {
    try { const res = await tauriApi.previewDeleteRange(Number(deleteFrom()), Number(deleteTo())); setDeletePreview(res); }
    catch(e:any){ setDeletePreview({ error: String(e) }); }
  };
  const doDelete = async () => {
    if (!deletePreview() || deletePreview().error) return;
    try { const res = await tauriApi.deleteRange(Number(deleteFrom()), Number(deleteTo())); setDeleteResult(res); refetchSummary(); loadAnalytics(0); }
    catch(e:any){ setDeleteResult({ error: String(e) }); }
  };
  const doVendorDryRun = async () => { try { setVendorDryRun(await tauriApi.vendorSyncDryRun()); } catch(e:any){ setVendorDryRun({ error: String(e) }); } };
  const doVendorSync = async () => { try { setVendorSyncResult(await tauriApi.dashboardVendorSync({ dry_run: false })); refetchSummary(); } catch(e:any){ setVendorSyncResult({ error: String(e) }); } };
  const doSaveDeviceTypes = async () => {
    try { const res = await tauriApi.saveDeviceTypesJson(deviceTypesJson(), reseed()); setDtSaveResult(res); refetchSummary(); }
    catch(e:any){ setDtSaveResult({ error: String(e) }); }
  };

  return (
    <div style="padding:16px; font-family: system-ui, sans-serif;">
      <div style="margin-bottom:20px;">
        <CoordsRepairDebugPanel />
      </div>
      <h2 style="margin:0 0 12px;">Local DB Dashboard (Preview)</h2>
      <section style="margin-bottom:20px;">
        <h3>Summary</h3>
    <Show when={summary()} fallback={<div>Loading summary...</div>}>
      {(sAccessor) => { const s = sAccessor(); return (
            <div style="display:flex; gap:16px; flex-wrap:wrap;">
              {Object.entries(s).filter(([k]) => k !== 'top_device_categories').map(([k,v]) => (
                <div style="background:#222; color:#eee; padding:8px 12px; border-radius:6px; min-width:140px;">
                  <div style="font-size:11px; text-transform:uppercase; opacity:0.7;">{k}</div>
                  <div style="font-size:18px; font-weight:600;">{String(v)}</div>
                </div>
              ))}
              <div style="flex-basis:100%;" />
              <div style="background:#1b1b1b; padding:8px 12px; border-radius:6px;">
                <div style="font-size:11px; text-transform:uppercase; opacity:0.7;">Top Categories</div>
        <For each={s.top_device_categories}>
                  {(c) => <div>{c[0]}: {c[1]}</div>}
                </For>
              </div>
            </div>
      ); }}
        </Show>
        <button onClick={() => refetchSummary()} style="margin-top:10px;">Refresh Summary</button>
      </section>
      <section style="margin-bottom:28px;">
        <h3>Export / Import</h3>
        <div style="display:flex; gap:8px; flex-wrap:wrap; margin-bottom:8px;">
          <button onClick={() => doExport('vendors')}>Export Vendors</button>
          <button onClick={() => doExport('device_types')}>Export Device Types</button>
          <button onClick={() => doExport('analytics')}>Export Analytics</button>
          <span style="font-size:12px; opacity:0.7;">{exportStatus()}</span>
        </div>
        <div style="display:flex; flex-direction:column; gap:6px; max-width:680px;">
          <select value={importDataset()} onChange={e => setImportDataset(e.currentTarget.value as any)}>
            <option value="vendors">vendors</option>
            <option value="device_types">device_types</option>
          </select>
          <textarea placeholder="Paste CSV here" value={importText()} onInput={e => setImportText(e.currentTarget.value)} rows={6} style="font-family:monospace;" />
          <button onClick={doImport}>Import CSV</button>
          <pre style="background:#111; color:#9f9; padding:8px; max-height:160px; overflow:auto;">{importLog()}</pre>
        </div>
      </section>
      <section style="margin-bottom:28px;">
        <h3>Delete Range</h3>
        <div style="display:flex; gap:8px; align-items:center; flex-wrap:wrap;">
          <input placeholder="from" value={deleteFrom()} onInput={e=>setDeleteFrom(e.currentTarget.value)} style="width:80px;" />
          <input placeholder="to" value={deleteTo()} onInput={e=>setDeleteTo(e.currentTarget.value)} style="width:80px;" />
          <button onClick={doPreviewDelete}>Preview</button>
          <button disabled={!deletePreview() || deletePreview().error} onClick={doDelete}>Delete</button>
          <pre style="background:#131313; color:#ddd; padding:6px; margin:0; max-width:420px; overflow:auto;">{JSON.stringify(deletePreview() || {}, null, 2)}</pre>
          <pre style="background:#131313; color:#8ff; padding:6px; margin:0; max-width:420px; overflow:auto;">{JSON.stringify(deleteResult() || {}, null, 2)}</pre>
        </div>
      </section>
      <section style="margin-bottom:28px;">
        <h3>Vendor Sync</h3>
        <div style="display:flex; gap:8px; flex-wrap:wrap; align-items:flex-start;">
          <button onClick={doVendorDryRun}>Dry Run</button>
            <button onClick={doVendorSync}>Run Sync</button>
            <pre style="background:#101820; color:#9df; padding:6px; margin:0; max-width:300px; overflow:auto;">{JSON.stringify(vendorDryRun()||{}, null, 2)}</pre>
            <pre style="background:#101820; color:#9f9; padding:6px; margin:0; max-width:300px; overflow:auto;">{JSON.stringify(vendorSyncResult()||{}, null, 2)}</pre>
        </div>
      </section>
      <section style="margin-bottom:28px;">
        <h3>Device Types JSON Editor</h3>
        <div style="display:flex; flex-direction:column; gap:6px; max-width:840px;">
          <div style="font-size:12px; opacity:0.7;">File path: {deviceTypesMeta()?.path || 'N/A'} | file count: {deviceTypesMeta()?.count_in_file} | db count: {deviceTypesMeta()?.count_in_db}</div>
          <textarea value={deviceTypesJson()} onInput={e => setDeviceTypesJson(e.currentTarget.value)} rows={12} style="font-family:monospace;" />
          <label style="font-size:12px; display:flex; gap:4px; align-items:center;"><input type="checkbox" checked={reseed()} onChange={e => setReseed(e.currentTarget.checked)} /> Reseed after save</label>
          <div style="display:flex; gap:8px;">
            <button onClick={loadDeviceTypes}>Reload</button>
            <button onClick={doSaveDeviceTypes}>Save</button>
          </div>
          <pre style="background:#111; color:#f9f; padding:6px; margin:0; max-height:160px; overflow:auto;">{JSON.stringify(dtSaveResult()||{}, null, 2)}</pre>
        </div>
      </section>
      <section>
        <h3>Analytics (First Page)</h3>
        <div style="overflow:auto; max-height:300px; border:1px solid #333;">
          <table style="width:100%; border-collapse:collapse; font-size:12px;">
            <thead>
              <tr style="background:#333;">
                <th style="padding:4px; text-align:left;">URL</th>
                <th style="padding:4px; text-align:left;">Model</th>
                <th style="padding:4px; text-align:left;">Vendor</th>
                <th style="padding:4px; text-align:left;">Device Type</th>
                <th style="padding:4px; text-align:left;">Category</th>
                <th style="padding:4px; text-align:left;">Cert Date</th>
                <th style="padding:4px; text-align:left;">Created</th>
              </tr>
            </thead>
            <tbody>
              <For each={analyticsRows()}>
                {(r) => (
                  <tr>
                    <td style="padding:4px; max-width:240px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">{r.product_detail_url}</td>
                    <td style="padding:4px;">{r.model}</td>
                    <td style="padding:4px;">{r.vendor_name}</td>
                    <td style="padding:4px;">{r.device_type_name}</td>
                    <td style="padding:4px;">{r.device_category}</td>
                    <td style="padding:4px;">{r.certification_date}</td>
                    <td style="padding:4px;">{r.detail_created_at}</td>
                  </tr>
                )}
              </For>
              <Show when={analyticsRows().length === 0}>
                <tr><td colSpan={7} style="padding:8px; text-align:center; opacity:0.6;">No rows</td></tr>
              </Show>
            </tbody>
          </table>
        </div>
        <div style="margin-top:8px; display:flex; gap:8px; align-items:center;">
          <button disabled={analyticsPage()===0} onClick={() => loadAnalytics(analyticsPage()-1)}>Prev</button>
          <span>Page {analyticsPage()+1} / {Math.max(1, Math.ceil(analyticsTotal()/pageSize))}</span>
          <button disabled={(analyticsPage()+1)*pageSize >= analyticsTotal()} onClick={() => loadAnalytics(analyticsPage()+1)}>Next</button>
        </div>
      </section>
    </div>
  );
};
