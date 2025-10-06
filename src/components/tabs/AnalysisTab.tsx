/**
 * AnalysisTab - 데이터 분석 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createMemo, createSignal, onMount, For, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';

export const AnalysisTab: Component = () => {
  // product_details analytics data (from backend)
  const [pd, setPd] = createSignal<any | null>(null);
  
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string>('');

  // Global Filters
  const [startDate, setStartDate] = createSignal<string>('');
  const [endDate, setEndDate] = createSignal<string>('');
  const [selectedManufacturers, setSelectedManufacturers] = createSignal<string[]>([]);
  const [selectedSpecVersions, setSelectedSpecVersions] = createSignal<string[]>([]);
  const [selectedTransport, setSelectedTransport] = createSignal<string[]>([]);
  const [mfrInput, setMfrInput] = createSignal('');
  const [specInput, setSpecInput] = createSignal('');
  const [tiInput, setTiInput] = createSignal('');

  const mfrSuggestions = createMemo(() => {
    const all = (pd()?.distributions?.manufacturers ?? []) as Array<{ key?: string; count: number }>;
    const term = mfrInput().toLowerCase();
    const chosen = new Set(selectedManufacturers());
    return all
      .map((x) => x.key ?? 'Unknown')
      .filter((k) => k && !chosen.has(k) && (term ? k.toLowerCase().includes(term) : true))
      .slice(0, 8);
  });
  const specSuggestions = createMemo(() => {
    const all = (pd()?.distributions?.spec_versions ?? []) as Array<{ key?: string; count: number }>;
    const term = specInput().toLowerCase();
    const chosen = new Set(selectedSpecVersions());
    return all
      .map((x) => x.key ?? 'Unknown')
      .filter((k) => k && !chosen.has(k) && (term ? k.toLowerCase().includes(term) : true))
      .slice(0, 8);
  });
  const tiSuggestions = createMemo(() => {
    const all = (pd()?.distributions?.transport_interfaces ?? []) as Array<{ key?: string; count: number }>;
    const term = tiInput().toLowerCase();
    const chosen = new Set(selectedTransport());
    return all
      .map((x) => x.key ?? 'Unknown')
      .filter((k) => k && !chosen.has(k) && (term ? k.toLowerCase().includes(term) : true))
      .slice(0, 8);
  });

  // 실제 분석 데이터 로드
  const loadAnalysisData = async () => {
    try {
      setIsLoading(true);
      const data = await tauriApi.getProductDetailsAnalytics({
        startDate: startDate() || undefined,
        endDate: endDate() || undefined,
        manufacturers: selectedManufacturers().length ? selectedManufacturers() : undefined,
        specVersions: selectedSpecVersions().length ? selectedSpecVersions() : undefined,
        transportInterfaces: selectedTransport().length ? selectedTransport() : undefined,
      });
      setPd(data);
    } catch (err) {
      console.error('Failed to load analysis data:', err);
      setError(`분석 데이터 로드 실패: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  // 컴포넌트 마운트 시 데이터 로드
  onMount(() => {
    loadAnalysisData();
  });

  const generateReport = async () => {
    try {
      const reportPath = await tauriApi.exportCrawlingResults();
      alert(`분석 보고서가 생성되었습니다: ${reportPath}`);
    } catch (err) {
      alert(`보고서 생성 실패: ${err}`);
    }
  };

  const exportChart = () => {
    alert('차트 내보내기 기능은 개발 중입니다.');
  };

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <div class="flex items-start flex-col lg:flex-row gap-4 lg:items-center lg:justify-between">
            <h2 class="text-2xl md:text-3xl font-bold text-gray-800">📈 분석 (실제 데이터)</h2>
            <div class="w-full lg:w-auto">
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-3">
                <div>
                  <label class="block text-xs text-gray-600 mb-1">시작일</label>
                  <input type="date" class="px-3 py-2 rounded-md border border-gray-200 bg-white" value={startDate()} onInput={(e) => setStartDate(e.currentTarget.value)} />
                </div>
                <div>
                  <label class="block text-xs text-gray-600 mb-1">종료일</label>
                  <input type="date" class="px-3 py-2 rounded-md border border-gray-200 bg-white" value={endDate()} onInput={(e) => setEndDate(e.currentTarget.value)} />
                </div>
                <div class="min-w-[220px]">
                  <label class="block text-xs text-gray-600 mb-1">제조사</label>
                  <div class="bg-white border border-gray-200 rounded-md p-2">
                    <div class="flex flex-wrap gap-1 mb-2">
                      <For each={selectedManufacturers()}>{(m) => (
                        <span class="px-2 py-0.5 bg-indigo-100 text-indigo-800 rounded-full text-xs flex items-center gap-1">
                          {m}
                          <button class="text-indigo-700" onClick={() => setSelectedManufacturers(selectedManufacturers().filter((v) => v !== m))}>✕</button>
                        </span>
                      )}</For>
                    </div>
                    <input
                      type="text"
                      class="w-full px-2 py-1 border border-gray-200 rounded-md text-sm"
                      placeholder="제조사 검색..."
                      value={mfrInput()}
                      onInput={(e) => setMfrInput(e.currentTarget.value)}
                    />
                    <Show when={mfrInput().length > 0}>
                      <div class="mt-2 max-h-40 overflow-y-auto border border-gray-200 rounded-md">
                        <For each={mfrSuggestions()}>{(opt) => (
                          <button class="w-full text-left px-2 py-1 hover:bg-gray-50 text-sm" onClick={() => { setSelectedManufacturers([...selectedManufacturers(), opt]); setMfrInput(''); }}>{opt}</button>
                        )}</For>
                      </div>
                    </Show>
                  </div>
                </div>
                <div class="min-w-[200px]">
                  <label class="block text-xs text-gray-600 mb-1">Spec Version</label>
                  <div class="bg-white border border-gray-200 rounded-md p-2">
                    <div class="flex flex-wrap gap-1 mb-2">
                      <For each={selectedSpecVersions()}>{(m) => (
                        <span class="px-2 py-0.5 bg-amber-100 text-amber-800 rounded-full text-xs flex items-center gap-1">
                          {m}
                          <button class="text-amber-700" onClick={() => setSelectedSpecVersions(selectedSpecVersions().filter((v) => v !== m))}>✕</button>
                        </span>
                      )}</For>
                    </div>
                    <input
                      type="text"
                      class="w-full px-2 py-1 border border-gray-200 rounded-md text-sm"
                      placeholder="Spec Version 검색..."
                      value={specInput()}
                      onInput={(e) => setSpecInput(e.currentTarget.value)}
                    />
                    <Show when={specInput().length > 0}>
                      <div class="mt-2 max-h-40 overflow-y-auto border border-gray-200 rounded-md">
                        <For each={specSuggestions()}>{(opt) => (
                          <button class="w-full text-left px-2 py-1 hover:bg-gray-50 text-sm" onClick={() => { setSelectedSpecVersions([...selectedSpecVersions(), opt]); setSpecInput(''); }}>{opt}</button>
                        )}</For>
                      </div>
                    </Show>
                  </div>
                </div>
                <div class="min-w-[220px]">
                  <label class="block text-xs text-gray-600 mb-1">Transport Interface</label>
                  <div class="bg-white border border-gray-200 rounded-md p-2">
                    <div class="flex flex-wrap gap-1 mb-2">
                      <For each={selectedTransport()}>{(m) => (
                        <span class="px-2 py-0.5 bg-emerald-100 text-emerald-800 rounded-full text-xs flex items-center gap-1">
                          {m}
                          <button class="text-emerald-700" onClick={() => setSelectedTransport(selectedTransport().filter((v) => v !== m))}>✕</button>
                        </span>
                      )}</For>
                    </div>
                    <input
                      type="text"
                      class="w-full px-2 py-1 border border-gray-200 rounded-md text-sm"
                      placeholder="Transport 검색..."
                      value={tiInput()}
                      onInput={(e) => setTiInput(e.currentTarget.value)}
                    />
                    <Show when={tiInput().length > 0}>
                      <div class="mt-2 max-h-40 overflow-y-auto border border-gray-200 rounded-md">
                        <For each={tiSuggestions()}>{(opt) => (
                          <button class="w-full text-left px-2 py-1 hover:bg-gray-50 text-sm" onClick={() => { setSelectedTransport([...selectedTransport(), opt]); setTiInput(''); }}>{opt}</button>
                        )}</For>
                      </div>
                    </Show>
                  </div>
                </div>
                <div class="flex items-end">
                  <button class="px-3 py-2 rounded-md bg-indigo-600 text-white hover:bg-indigo-700" onClick={loadAnalysisData}>적용</button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <Show when={!!error()}>
          <div class="bg-rose-50 border border-rose-200 text-rose-700 rounded-2xl p-4">{error()}</div>
        </Show>

        <Show when={isLoading()}>
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-8 text-center text-gray-600">
            <div class="mb-2">분석 데이터를 로드하는 중...</div>
            <div class="w-6 h-6 border-2 border-gray-200 border-t-indigo-500 rounded-full animate-spin mx-auto"></div>
          </div>
        </Show>

        <Show when={!isLoading() && !error() && (!pd() || (pd()?.total_details ?? 0) === 0)}>
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-8 text-center text-gray-600">
            <div class="text-5xl mb-3">📊</div>
            <div class="text-lg font-medium mb-1">분석할 데이터가 없습니다</div>
            <div class="text-sm">크롤링을 실행하여 데이터를 수집한 후 분석을 확인할 수 있습니다.</div>
            <button class="mt-4 px-4 py-2 rounded-lg text-white bg-indigo-600 hover:bg-indigo-700" onClick={loadAnalysisData}>다시 시도</button>
          </div>
        </Show>

        <Show when={!isLoading() && !error() && (pd()?.total_details ?? 0) > 0}>
          {/* Key Metrics */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-lg font-semibold text-gray-800">주요 지표</h3>
              <button class="px-3 py-1.5 text-sm rounded-lg text-white bg-gray-700 hover:bg-gray-800" onClick={loadAnalysisData}>새로고침</button>
            </div>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
              <div class="bg-gradient-to-br from-blue-50 to-blue-100 rounded-2xl p-4 border border-blue-200/50 text-center">
                <div class="text-3xl font-bold text-blue-600 mb-1">{(pd()?.total_details ?? 0).toLocaleString()}</div>
                <div class="text-sm text-blue-700 font-medium">product_details 총 레코드</div>
              </div>
              <div class="bg-gradient-to-br from-emerald-50 to-emerald-100 rounded-2xl p-4 border border-emerald-200/50 text-center">
                {(() => {
                  const total = pd()?.total_details ?? 0;
                  const filled = (pd()?.completeness?.manufacturer_filled ?? 0) as number;
                  const pct = total > 0 ? ((filled / total) * 100).toFixed(1) : '0.0';
                  return (
                    <>
                      <div class="text-3xl font-bold text-emerald-600 mb-1">{pct}%</div>
                      <div class="text-sm text-emerald-700 font-medium">제조사 필드 충족률</div>
                    </>
                  );
                })()}
              </div>
              <div class="bg-gradient-to-br from-rose-50 to-rose-100 rounded-2xl p-4 border border-rose-200/50 text-center">
                {(() => {
                  const total = pd()?.total_details ?? 0;
                  const filled = (pd()?.completeness?.certificate_id_filled ?? 0) as number;
                  const pct = total > 0 ? ((filled / total) * 100).toFixed(1) : '0.0';
                  return (
                    <>
                      <div class="text-3xl font-bold text-rose-600 mb-1">{pct}%</div>
                      <div class="text-sm text-rose-700 font-medium">인증서ID 충족률</div>
                    </>
                  );
                })()}
              </div>
              <div class="bg-gradient-to-br from-amber-50 to-amber-100 rounded-2xl p-4 border border-amber-200/50 text-center">
                {(() => {
                  const total = pd()?.total_details ?? 0;
                  const filled = (pd()?.completeness?.spec_version_filled ?? 0) as number;
                  const pct = total > 0 ? ((filled / total) * 100).toFixed(1) : '0.0';
                  return (
                    <>
                      <div class="text-3xl font-bold text-amber-600 mb-1">{pct}%</div>
                      <div class="text-sm text-amber-700 font-medium">스펙버전 충족률</div>
                    </>
                  );
                })()}
              </div>
            </div>
          </div>

          {/* Manufacturer Distribution */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <h3 class="text-lg font-semibold text-gray-800 mb-4">제조사별 분포 (상위 20)</h3>
            <Show when={(pd()?.distributions?.manufacturers?.length ?? 0) === 0}>
              <div class="text-center text-gray-500 py-6">제조사 데이터가 없습니다.</div>
            </Show>
            <Show when={(pd()?.distributions?.manufacturers?.length ?? 0) > 0}>
              <div class="space-y-3">
                <For each={(pd()?.distributions?.manufacturers ?? []).slice(0, 20)}>
                  {(item: any) => {
                    const total = pd()?.total_details ?? 0;
                    const key = item.key ?? 'Unknown';
                    const count = item.count as number;
                    const pct = total > 0 ? ((count / total) * 100).toFixed(1) : '0';
                    return (
                      <div>
                        <div class="flex items-center justify-between mb-1">
                          <span class="font-medium text-gray-800">{key}</span>
                          <span class="text-sm text-gray-600">{count.toLocaleString()} ({pct}%)</span>
                        </div>
                        <div class="w-full bg-gray-200 rounded h-2">
                          <div class="h-2 rounded bg-indigo-500" style={{ width: `${pct}%` }}></div>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>

          {/* Certification Date Time Series (weekly) */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <h3 class="text-lg font-semibold text-gray-800 mb-4">인증일 기준 주차별 제품 수</h3>
            <Show when={(pd()?.time_series?.cert_by_date?.length ?? 0) === 0}>
              <div class="text-center text-gray-500 py-6">주차별 통계 데이터가 없습니다.</div>
            </Show>
            <Show when={(pd()?.time_series?.cert_by_date?.length ?? 0) > 0}>
              {(() => {
                const byWeek: Record<string, number> = {};
                // ISO week aggregation
                function isoWeekKey(dateStr: string): string | null {
                  const d = new Date(dateStr);
                  if (isNaN(d.getTime())) return null;
                  const target = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()));
                  const dayNr = (target.getUTCDay() + 6) % 7; // Monday=0
                  target.setUTCDate(target.getUTCDate() - dayNr + 3); // Thursday
                  const firstThursday = new Date(Date.UTC(target.getUTCFullYear(), 0, 4));
                  const diff = target.getTime() - firstThursday.getTime();
                  const week = 1 + Math.round(diff / (7 * 24 * 3600 * 1000));
                  const year = target.getUTCFullYear();
                  return `${year}-W${String(week).padStart(2, '0')}`;
                }
                for (const s of (pd()?.time_series?.cert_by_date ?? [])) {
                  const d = s.date as string | null;
                  if (!d) continue;
                  const key = isoWeekKey(d);
                  if (!key) continue;
                  byWeek[key] = (byWeek[key] ?? 0) + (s.count as number);
                }
                const arr = Object.entries(byWeek)
                  .sort((a, b) => a[0].localeCompare(b[0]))
                  .map(([wk, cnt]) => ({ week: wk, count: cnt }));
                const maxCount = Math.max(...arr.map((s: any) => s.count as number));
                return (
                  <div class="flex items-end gap-2 h-[250px] overflow-x-auto justify-start">
                    <For each={arr}>
                      {(stat: any) => {
                        const height = maxCount > 0 ? ((stat.count as number) / maxCount) * 200 : 0;
                        return (
                          <div class="flex flex-col items-center min-w-[48px]">
                            <div class="mb-2 text-[10px] font-medium text-gray-800">{stat.count}</div>
                            <div class="w-8 bg-gradient-to-t from-indigo-600 to-indigo-300 rounded-t" style={{ height: `${height}px`, 'min-height': '3px' }}></div>
                            <div class="mt-2 text-[10px] text-gray-500 -rotate-45 whitespace-nowrap">{stat.week}</div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                );
              })()}
            </Show>
          </div>

          {/* Device Type Distribution */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <h3 class="text-lg font-semibold text-gray-800 mb-4">디바이스 유형 분포 (상위 20)</h3>
            <Show when={(pd()?.distributions?.device_types?.length ?? 0) === 0}>
              <div class="text-center text-gray-500 py-6">디바이스 유형 데이터가 없습니다.</div>
            </Show>
            <Show when={(pd()?.distributions?.device_types?.length ?? 0) > 0}>
              <div class="space-y-3">
                <For each={(pd()?.distributions?.device_types ?? []).slice(0, 20)}>
                  {(item: any) => {
                    const total = pd()?.total_details ?? 0;
                    const key = item.key ?? 'Unknown';
                    const count = item.count as number;
                    const pct = total > 0 ? ((count / total) * 100).toFixed(1) : '0';
                    return (
                      <div>
                        <div class="flex items-center justify-between mb-1">
                          <span class="font-medium text-gray-800">{key}</span>
                          <span class="text-sm text-gray-600">{count.toLocaleString()} ({pct}%)</span>
                        </div>
                        <div class="w-full bg-gray-200 rounded h-2">
                          <div class="h-2 rounded bg-violet-500" style={{ width: `${pct}%` }}></div>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>

          {/* Transport Interface Distribution */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <h3 class="text-lg font-semibold text-gray-800 mb-4">Transport Interface별 분포</h3>
            <Show when={(pd()?.distributions?.transport_interfaces?.length ?? 0) === 0}>
              <div class="text-center text-gray-500 py-6">Transport Interface 데이터가 없습니다.</div>
            </Show>
            <Show when={(pd()?.distributions?.transport_interfaces?.length ?? 0) > 0}>
              <div class="space-y-3">
                <For each={(pd()?.distributions?.transport_interfaces ?? [])}>
                  {(item: any) => {
                    const total = pd()?.total_details ?? 0;
                    const key = item.key ?? 'Unknown';
                    const count = item.count as number;
                    const pct = total > 0 ? ((count / total) * 100).toFixed(1) : '0';
                    return (
                      <div>
                        <div class="flex items-center justify-between mb-1">
                          <span class="font-medium text-gray-800">{key}</span>
                          <span class="text-sm text-gray-600">{count.toLocaleString()} ({pct}%)</span>
                        </div>
                        <div class="w-full bg-gray-200 rounded h-2">
                          <div class="h-2 rounded bg-violet-500" style={{ width: `${pct}%` }}></div>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>



        </Show>
        {/* Spec / Transport charts */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">스펙/전송 인터페이스 분석</h3>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div class="p-4 bg-white rounded-xl border border-gray-200">
              <h4 class="text-base font-medium text-gray-800 mb-3">Spec Version 분포</h4>
              <Show when={(pd()?.distributions?.spec_versions?.length ?? 0) > 0}>
                <div class="space-y-2">
                  <For each={pd()?.distributions?.spec_versions ?? []}>
                    {(r: any) => {
                      const total = pd()?.total_details ?? 0;
                      const count = r.count as number;
                      const pct = total > 0 ? Math.round((count / total) * 100) : 0;
                      return (
                        <div>
                          <div class="flex justify-between text-sm text-gray-700 mb-1"><span>{r.key ?? 'Unknown'}</span><span>{count} ({pct}%)</span></div>
                          <div class="w-full bg-gray-200 rounded h-2">
                            <div class="h-2 rounded bg-amber-500" style={{ width: `${pct}%` }}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </Show>
              <Show when={(pd()?.distributions?.spec_versions?.length ?? 0) === 0}>
                <div class="text-sm text-gray-500">데이터가 없습니다.</div>
              </Show>
            </div>
            <div class="p-4 bg-white rounded-xl border border-gray-200">
              <h4 class="text-base font-medium text-gray-800 mb-3">Transport Interface 분포</h4>
              <Show when={(pd()?.distributions?.transport_interfaces?.length ?? 0) > 0}>
                <div class="space-y-2">
                  <For each={pd()?.distributions?.transport_interfaces ?? []}>
                    {(r: any) => {
                      const total = pd()?.total_details ?? 0;
                      const count = r.count as number;
                      const pct = total > 0 ? Math.round((count / total) * 100) : 0;
                      return (
                        <div>
                          <div class="flex justify-between text-sm text-gray-700 mb-1"><span>{r.key ?? 'Unknown'}</span><span>{count} ({pct}%)</span></div>
                          <div class="w-full bg-gray-200 rounded h-2">
                            <div class="h-2 rounded bg-emerald-500" style={{ width: `${pct}%` }}></div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </Show>
              <Show when={(pd()?.distributions?.transport_interfaces?.length ?? 0) === 0}>
                <div class="text-sm text-gray-500">데이터가 없습니다.</div>
              </Show>
            </div>
          </div>
        </div>

        {/* Tools */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
          <h3 class="text-lg font-semibold text-gray-800 mb-4">분석 도구</h3>
          <div class="flex flex-wrap gap-2">
            <button class="px-4 py-2 rounded-lg text-white bg-indigo-600 hover:bg-indigo-700" onClick={generateReport}>📊 분석 보고서 생성</button>
            <button class="px-4 py-2 rounded-lg text-white bg-emerald-600 hover:bg-emerald-700" onClick={exportChart}>📈 차트 내보내기</button>
            <button class="px-4 py-2 rounded-lg text-white bg-violet-600 hover:bg-violet-700" onClick={() => alert('데이터 필터링 옵션이 열렸습니다.')}>🔍 데이터 필터링</button>
          </div>
        </div>


      </div>
    </div>
  );
};
