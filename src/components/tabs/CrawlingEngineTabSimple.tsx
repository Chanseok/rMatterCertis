import { createSignal, Show, onMount, onCleanup, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
// Types are relaxed locally to avoid tight coupling during integration
import { tauriApi } from '../../services/tauri-api';
import EventConsole from '../dev/EventConsole';
import { usePulse } from '../../hooks/usePulse';
import CountUp from '../common/CountUp';

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingRange, setCrawlingRange] = createSignal<any | null>(null);
  const [statusMessage, setStatusMessage] = createSignal<string>('크롤링 준비 완료');
  const [logs, setLogs] = createSignal<string[]>([]);
  const [showConsole, setShowConsole] = createSignal<boolean>(true);
  const [isValidating, setIsValidating] = createSignal(false);
  const [isSyncing, setIsSyncing] = createSignal(false);
  const [syncRanges, setSyncRanges] = createSignal<string>('');
  // Lightweight Sync runtime view
  const [syncLive, setSyncLive] = createSignal<{ active: boolean; planned?: number | null; pagesProcessed: number; inserted: number; updated: number; skipped: number; failed: number; lastPage?: number | null; lastWarn?: string | null; durationMs?: number }>(
    { active: false, planned: null, pagesProcessed: 0, inserted: 0, updated: 0, skipped: 0, failed: 0 }
  );
  const [validationPages, setValidationPages] = createSignal<number | ''>('');
  // Auto re-plan from backend after a session completes
  const [nextPlan, setNextPlan] = createSignal<any | null>(null);

  // Dramatic transition for Calculated Crawling Range
  const [rangeFxKey, setRangeFxKey] = createSignal(0);
  const [rangeFxActive, setRangeFxActive] = createSignal(false);
  const [confettiPieces, setConfettiPieces] = createSignal<Array<{x:number;y:number;color:string;rx:number;ry:number;rot:number;cw?:number;ch?:number}>>([]);
  const [rangePrevSnapshot, setRangePrevSnapshot] = createSignal<{
    start: number; end: number; total: number; coverText: string;
  } | null>(null);
  let rangePanelRef: HTMLDivElement | undefined;

  // Split text into animated particles (shatter)
  const renderShatterText = (text: string) => text.split('').map((ch) => {
    const mag = 140 + Math.random() * 160; // stronger spread
    const theta = (Math.random() * Math.PI * 1.3) - (Math.PI * 0.65);
    const dx = Math.cos(theta) * mag;
    const dy = Math.sin(theta) * mag - 20; // upward bias
    const rot = (Math.random() - 0.5) * 200;
    const style = { '--dx': `${dx}px`, '--dy': `${dy}px`, '--rot': `${rot}deg` } as any;
    return <span class="shatter-char" style={style} aria-hidden="true">{ch}</span>;
  });

  // Drum-roll in for new text
  const renderDrumText = (text: string) => text.split('').map((ch, i) => (
    <span class="drum-in" style={{ '--delay': `${i * 35}ms` } as any}>{ch}</span>
  ));

  // Lightweight CSS confetti
  const triggerConfetti = (n = 48) => {
    if (!rangePanelRef) return;
    const colors = ['#60A5FA', '#34D399', '#FBBF24', '#F472B6', '#A78BFA', '#22D3EE'];
    const pieces = Array.from({ length: n }, () => {
      const angle = Math.random() * Math.PI * 2;
      const dist = 90 + Math.random() * 160; // farther burst
      const cw = 4 + Math.random() * 8; // width 4~12
      const ch = 6 + Math.random() * 14; // height 6~20
      return { x: 0, y: 0, color: colors[Math.floor(Math.random() * colors.length)], rx: Math.cos(angle) * dist, ry: Math.sin(angle) * dist, rot: (Math.random() - 0.5) * 220, cw, ch };
    });
    setConfettiPieces(pieces);
    setTimeout(() => setConfettiPieces([]), 950);
  };

  const playRangeTransition = () => {
    setRangeFxActive(true);
    setRangeFxKey(k => k + 1);
    triggerConfetti();
    setTimeout(() => setRangeFxActive(false), 720);
  };

  // Optimistically apply a planner result to the Calculated Crawling Range panel
  const applyPlanToCalculatedRange = (plan: any) => {
    try {
      const phases = (plan?.phases || []) as any[];
      const pages: number[] = phases.flatMap((p: any) => Array.isArray(p?.pages) ? (p.pages as number[]) : []);
      const uniq = Array.from(new Set(pages)).filter(n => Number.isFinite(n)).sort((a,b)=>b-a);
      if (uniq.length === 0) return;
      const start = uniq[0];
      const end = uniq[uniq.length - 1];
      setCrawlingRange(prev => ({
        ...(prev || {}),
        range: [start, end],
        crawling_info: {
          ...((prev as any)?.crawling_info || {}),
          pages_to_crawl: uniq.length,
        },
      }));
    } catch {}
  };
  // Batch progress (best-effort estimation)
  const [batchInfo, setBatchInfo] = createSignal<{ current: number; totalEstimated?: number; batchId?: string; pagesInBatch?: number }>({ current: 0 });
  // Lightweight runtime monitor for Stage 1 (list pages) and Stage 2 (detail)
  const [pageStats, setPageStats] = createSignal({ started: 0, completed: 0, failed: 0, retried: 0, totalEstimated: 0, inflight: 0 });
  const [detailStats, setDetailStats] = createSignal({ started: 0, completed: 0, failed: 0, retried: 0, inflight: 0 });
  // Stage 1 unique tracking (per page) to avoid double counting and track retries
  const pageSeen = new Set<number>();
  const pageCompleted = new Set<number>();
  const pageFailedFinal = new Set<number>();
  const pageAttempts = new Map<number, number>();
  // Stage 2 grouped accounting (no per-detail IDs; rely on product lifecycle group snapshots)
  // We still keep simple counters for retries/failures inferred from per-product lifecycle events.
  const detailSeen = new Set<string>(); // deprecated: kept for compatibility; not used in new flow
  const detailCompleted = new Set<string>(); // deprecated
  const detailFailedFinal = new Set<string>(); // deprecated
  const detailAttempts = new Map<string, number>(); // deprecated
  const [downshiftInfo, setDownshiftInfo] = createSignal<null | { newLimit?: number; reason?: string }>(null);
  // UI pulses for counters
  const [stage1Pulse, triggerStage1Pulse] = usePulse(300);
  const [stage2Pulse, triggerStage2Pulse] = usePulse(300);
  // Stage 3: Validation stats (lightweight)
  const [validationStats, setValidationStats] = createSignal({
    started: false,
    completed: false,
    targetPages: 0,
    pagesScanned: 0,
    divergences: 0,
    anomalies: 0,
    productsChecked: 0,
    lastPage: null as number | null,
    lastAssignedStart: null as number | null,
    lastAssignedEnd: null as number | null,
  });
  // Animation toggles
  const [validationPulse, setValidationPulse] = createSignal(false);
  const [persistFlash, setPersistFlash] = createSignal(false);
  // Stage X: DB mismatch diagnostics
  const [diagLoading, setDiagLoading] = createSignal(false);
  const [diagResult, setDiagResult] = createSignal<any | null>(null);
  const [cleanupLoading, setCleanupLoading] = createSignal(false);
  const runDiagnostics = async () => {
    try {
      setDiagLoading(true);
  addLog('🧪 DB 진단 호출 시작');
  const res = await tauriApi.scanDbPaginationMismatches();
  addLog('✅ DB 진단 응답 수신');
      setDiagResult(res);
    } catch (e) {
  console.error('[Diagnostics] invoke failed', e);
  addLog(`❌ Diagnostics 실패: ${(e as any)?.message || e}`);
    } finally {
      setDiagLoading(false);
    }
  };
  // Build ranges from current diagnostics using physical pages and expand ±1 neighbors
  const deriveRangesFromDiagnostics = (): string | null => {
    const diag = diagResult();
    if (!diag) return null;
    const totalPages: number | undefined = Number.isFinite(diag.total_pages_site) ? Number(diag.total_pages_site) : undefined;
    // Select problematic groups more broadly: status!=ok OR any dup/miss/out-of-range hints
    const pages: number[] = (diag.group_summaries || [])
      .filter((g: any) => {
        const notOk = !!g.status && g.status !== 'ok';
        const hasDup = (g.duplicate_indices?.length || 0) > 0;
        const hasMiss = (g.missing_indices?.length || 0) > 0;
        const oob = (g.out_of_range_count || 0) > 0;
        return notOk || hasDup || hasMiss || oob;
      })
      .map((g: any) => g.current_page_number)
      .filter((p: any) => typeof p === 'number' && p > 0);
    if (pages.length === 0) return null;
    // Unique and neighbor expansion (±1) within site bounds
    const set = new Set<number>();
    for (const p of pages) set.add(p);
    if (totalPages && totalPages > 1) {
      for (const p of Array.from(set)) {
        if (p - 1 >= 1) set.add(p - 1);
        if (p + 1 <= totalPages) set.add(p + 1);
      }
    }
    const uniq = Array.from(set).sort((a,b)=>b-a);
    // Compress contiguous desc pages to ranges expr
    const parts: string[] = [];
    let start = uniq[0];
    let prev = uniq[0];
    for (const p of uniq.slice(1)) {
      if (p + 1 === prev) { prev = p; continue; }
      parts.push(start === prev ? `${start}` : `${start}-${prev}`);
      start = p; prev = p;
    }
    parts.push(start === prev ? `${start}` : `${start}-${prev}`);
    return parts.join(',');
  };
  const runUrlCleanup = async () => {
    try {
      setCleanupLoading(true);
      const res = await tauriApi.cleanupDuplicateUrls();
      addLog(
        `🧹 중복 제거 완료: URL기준 products ${res.products_removed}, details ${res.product_details_removed} 삭제 | 슬롯기준(page_id,index) products ${res.slot_products_removed}, details ${res.slot_product_details_removed} 삭제 | 남은 URL중복 products ${res.remaining_duplicates_products}, details ${res.remaining_duplicates_product_details} | 남은 슬롯중복 products ${res.remaining_slot_duplicates_products}, details ${res.remaining_slot_duplicates_product_details}`
      );
      // Refresh diagnostics after cleanup for convenience
      await runDiagnostics();
    } catch (e) {
      addLog('❌ URL 중복 제거 실패: ' + (e as any)?.message);
    } finally {
      setCleanupLoading(false);
    }
  };
  // Stage 4: DB snapshot (latest observed)
  const [dbSnapshot, setDbSnapshot] = createSignal<{
    total?: number;
    minPage?: number | null;
    maxPage?: number | null;
    inserted?: number;
    updated?: number;
  }>({});
  // Stage 5: Persist (grouped snapshot)
  const [persistStats, setPersistStats] = createSignal<{
    attempted: number;
    succeeded: number;
    failed: number;
    duplicates: number;
    durationMs: number;
  }>({ attempted: 0, succeeded: 0, failed: 0, duplicates: 0, durationMs: 0 });
  // Stage 4: DB snapshot animation toggle
  const [dbFlash, setDbFlash] = createSignal(false);
  // Global effects toggle
  const [effectsOn, setEffectsOn] = createSignal(true);
  // Sync input pulse highlight
  const [syncPulse, setSyncPulse] = createSignal(false);
  // Track sync-start events to detect backend start and enable fallbacks
  let syncStartSeq = 0;
  onMount(async () => {
    try {
      const un1 = await listen('actor-sync-started', () => { syncStartSeq++; });
      onCleanup(() => { try { (un1 as any)(); } catch {} });
    } catch {}
  });

  // Start button circular wave FX (restored)
  const [waveBursts, setWaveBursts] = createSignal<Array<{ id:number; x:number; y:number; kind:'up'|'down'|'ring' }>>([]);
  let waveIdSeq = 1;
  const triggerStartWave = (evt?: MouseEvent | PointerEvent) => {
    // Compute click point in viewport; fallback to the center of the pressed button, else screen center
    let x: number | undefined = (evt as any)?.clientX;
    let y: number | undefined = (evt as any)?.clientY;
    if ((x == null || y == null) && (evt as any)?.currentTarget) {
      try {
        const el = (evt as any).currentTarget as HTMLElement;
        const rect = el.getBoundingClientRect();
        x = rect.left + rect.width / 2;
        y = rect.top + rect.height / 2;
      } catch {}
    }
    if (x == null || y == null) {
      x = window.innerWidth / 2;
      y = window.innerHeight / 2;
    }
    // Compute scale to fill the viewport from the click point
    const dx = Math.max(x, window.innerWidth - x);
    const dy = Math.max(y, window.innerHeight - y);
    const radius = Math.hypot(dx, dy);
    const baseRadius = 12; // starting diameter ~24px, so radius ~12
    const fillScale = Math.max(35, (radius / baseRadius));
    const idUp = waveIdSeq++;
    const idDown = waveIdSeq++;
    const idRing = waveIdSeq++;
    setWaveBursts(prev => [...prev, { id:idUp, x, y, kind:'up' }, { id:idDown, x, y, kind:'down' }, { id:idRing, x, y, kind:'ring' }]);
    // Auto cleanup after animations
    setTimeout(() => setWaveBursts(prev => prev.filter(w => w.id !== idUp && w.id !== idDown && w.id !== idRing)), 1000);
  };

  // 크롤링 범위 계산
  const calculateCrawlingRange = async () => {
    addLog('📊 크롤링 범위 계산 중...');
    
    try {
      // 먼저 사이트 상태를 확인해서 실제 total_pages를 얻습니다
      addLog('🌐 사이트 상태 확인 중...');
      const siteStatusResponse = await invoke<any>('check_advanced_site_status');
      
      if (!siteStatusResponse?.data) {
        throw new Error('사이트 상태 확인 실패');
      }
      
      const siteStatus = siteStatusResponse.data;
      addLog(`✅ 사이트 상태 확인 완료: ${siteStatus.total_pages}페이지, 마지막 페이지 ${siteStatus.products_on_last_page}개 제품`);
      
  const request: any = {
        total_pages_on_site: siteStatus.total_pages,
        products_on_last_page: siteStatus.products_on_last_page,
      };
      
      addLog(`📋 크롤링 범위 계산 요청: ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품`);
      
  const response = await invoke<any>('calculate_crawling_range', { request });
      setCrawlingRange(response);
      
      const startPage = response.range?.[0] || 0;
      const endPage = response.range?.[1] || 0;
      addLog(`📊 크롤링 범위 계산 완료: ${startPage} → ${endPage}`);
    } catch (error) {
      console.error('크롤링 범위 계산 실패:', error);
      addLog(`❌ 크롤링 범위 계산 실패: ${error}`);
    }
  };  
  
  // 통합 Actor 기반 크롤링 (경량 설정)
  const startLightUnified = async () => {
    if (isRunning()) return;

    setIsRunning(true);
    setStatusMessage('🎭 통합 파이프라인(라이트) 시작 중...');
    addLog('🎭 통합 파이프라인 시작 (라이트 설정)');

    try {
      const res = await tauriApi.startUnifiedCrawling({
        mode: 'advanced',
        overrideConcurrency: 8,
        overrideBatchSize: 3,
        delayMs: 100,
      });
      addLog(`✅ 통합 파이프라인(라이트) 세션 시작: ${JSON.stringify(res)}`);
      setStatusMessage('🎭 통합 파이프라인 실행 중 (라이트)');
    } catch (error) {
      console.error('통합 파이프라인(라이트) 시작 실패:', error);
      addLog(`❌ 통합 파이프라인(라이트) 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
      setIsRunning(false);
    }
  };

  // 통합 Actor 기반 크롤링 (하이 설정)
  const startUnifiedAdvanced = async () => {
    if (isRunning()) return;

    setIsRunning(true);
    setStatusMessage('🎭 통합 파이프라인(하이) 시작 중...');
    addLog('🎭 통합 파이프라인 시작 (하이 설정)');

    try {
      const res = await tauriApi.startUnifiedCrawling({
        mode: 'advanced',
        overrideConcurrency: 64,
        overrideBatchSize: 3,
        delayMs: 100,
      });
      addLog(`✅ 통합 파이프라인(하이) 세션 시작: ${JSON.stringify(res)}`);
      setStatusMessage('🎭 통합 파이프라인 실행 중 (하이)');
    } catch (error) {
      console.error('통합 파이프라인(하이) 시작 실패:', error);
      addLog(`❌ 통합 파이프라인(하이) 시작 실패: ${error}`);
      setStatusMessage('크롤링 실패');
      setIsRunning(false);
    }
  };

  // ... (legacy simple crawling entry removed)

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [`[${timestamp}] ${message}`, ...prev.slice(0, 19)]);
  };

  // Validation run
  const startValidationRun = async () => {
    if (isValidating()) return;
    setIsValidating(true);
    addLog('🧪 Validation 시작');
    try {
      const res = await tauriApi.startValidation({
        scanPages: typeof validationPages() === 'number' ? (validationPages() as number) : undefined,
      });
      addLog(`✅ Validation 요청 완료: ${JSON.stringify(res)}`);
    } catch (e) {
      console.error(e);
      addLog(`❌ Validation 실패: ${e}`);
    } finally {
      setIsValidating(false);
    }
  };

  // Sync run
  const startSyncRun = async () => {
    if (isSyncing()) return;
    setIsSyncing(true);
    const ranges = syncRanges().trim();
    addLog(`🔄 Sync 시작 ${ranges ? `(범위: ${ranges})` : '(자동 범위)'}`);
    try {
      const res = ranges
        ? await tauriApi.startPartialSync(ranges)
        : await tauriApi.startRepairSync();
      addLog(`✅ Sync 완료: ${JSON.stringify(res)}`);
    } catch (e) {
      addLog(`❌ Sync 실패: ${e}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const syncMissingPagesFromDiagnostics = async () => {
    if (isSyncing()) return;
    const diag = diagResult();
    if (!diag) {
      addLog('⚠️ 먼저 진단을 실행하세요.');
      return;
    }
    // Collect physical pages where group status indicates holes/sparse and we have current_page_number
    const pages: number[] = (diag.group_summaries || [])
      .filter((g: any) => g.status && g.status !== 'ok' && (g.missing_indices?.length || 0) > 0)
      .map((g: any) => g.current_page_number)
      .filter((p: any) => typeof p === 'number' && p > 0);
    const uniquePages = Array.from(new Set(pages));
    if (uniquePages.length === 0) {
      addLog('ℹ️ 누락 항목이 있는 물리 페이지가 없습니다.');
      return;
    }
    setIsSyncing(true);
    addLog(`🔁 진단 선택 페이지만 Sync (기본 엔진): [${uniquePages.join(', ')}]`);
    try {
      const res = await tauriApi.startBasicSyncPages(uniquePages);
      addLog(`✅ 부분 Sync 완료: ${JSON.stringify(res)}`);
      // Re-run diagnostics to show before/after
      await runDiagnostics();
    } catch (e) {
      addLog(`❌ 부분 Sync 실패: ${e}`);
    } finally {
      setIsSyncing(false);
    }
  };

  // 정밀 복구 실행: 현재 진단 결과에서 각 페이지의 누락 슬롯(index)만 정확히 채움
  const runPreciseDiagnosticRepair = async () => {
    const diag = diagResult();
    if (!diag) {
      addLog('⚠️ 먼저 진단을 실행하세요.');
      return;
    }
    // group_summaries에서 status!=ok 이고 missing_indices가 존재하는 항목을 모아 payload 구성
    const groups: Array<{ physical_page: number; miss_indices: number[] }> = [];
    for (const g of (diag.group_summaries || [])) {
      const miss = (g.missing_indices || []).filter((n: any) => Number.isInteger(n) && n >= 0 && n < 12);
      const phys = g.current_page_number;
      if (!phys || miss.length === 0) continue;
      groups.push({ physical_page: phys as number, miss_indices: miss.map((x: number) => Number(x)) });
    }
    if (groups.length === 0) {
      addLog('ℹ️ 정밀 복구 대상이 없습니다. (누락 슬롯 없음)');
      return;
    }
    setIsSyncing(true);
    addLog(`🧩 정밀 복구 실행: ${groups.length}개 페이지 (슬롯 지정)`);
    try {
      // 스냅샷은 생략(백엔드가 알아서 최신 사이트 메타 조회), 필요 시 diag의 total_pages_site/items_on_last_page를 넣을 수 있음
      const res = await tauriApi.startDiagnosticSync(groups);
      addLog(`✅ 정밀 복구 완료: ${JSON.stringify(res)}`);
      await runDiagnostics();
    } catch (e) {
      addLog(`❌ 정밀 복구 실패: ${e}`);
    } finally {
      setIsSyncing(false);
    }
  };

  onMount(() => {
    calculateCrawlingRange();

    const unsubs: Array<() => void> = [];

    // Listen settings-updated to recalc planned range
    try {
      listen('settings-updated', () => {
        addLog('🛠️ 설정 변경 감지 → 범위 재계산');
        // Optional transition snapshot for nicer UX
        try {
          const prev = crawlingRange();
          const prevStart = (prev?.range?.[0] ?? 0) as number;
          const prevEnd = (prev?.range?.[1] ?? 0) as number;
          const prevTotal = (prev?.progress?.total_products ?? 0) as number;
          const prevCover = `${(prev?.progress?.progress_percentage?.toFixed?.(1) ?? '0.0')}%`;
          setRangePrevSnapshot({ start: prevStart, end: prevEnd, total: prevTotal, coverText: String(prevCover) });
        } catch {}
        calculateCrawlingRange();
      }).then((un) => unsubs.push(un));
    } catch (e) {
      console.warn('[CrawlingEngineTabSimple] listen settings-updated failed', e);
    }

    // Listen to unified Actor session lifecycle to toggle buttons/status
    tauriApi
      .subscribeToActorBridgeEvents((name, payload) => {
        // === Sync events → compact Sync panel ===
        if (name === 'actor-sync-started') {
          try {
            const ranges: Array<[number, number]> = Array.isArray(payload?.ranges) ? payload.ranges : [];
            const planned = ranges.reduce((acc, [start, end]) => acc + Math.max(0, (start - end + 1)), 0);
            setSyncLive({ active: true, planned: planned || null, pagesProcessed: 0, inserted: 0, updated: 0, skipped: 0, failed: 0, lastPage: null, lastWarn: null, durationMs: undefined });
            setStatusMessage('🔄 Sync 실행 중');
          } catch {
            setSyncLive({ active: true, planned: null, pagesProcessed: 0, inserted: 0, updated: 0, skipped: 0, failed: 0, lastPage: null, lastWarn: null, durationMs: undefined });
          }
        }
        if (name === 'actor-sync-page-started') {
          const p = Number(payload?.physical_page ?? NaN);
          setSyncLive(prev => ({ ...prev, lastPage: Number.isFinite(p) ? p : prev.lastPage ?? null }));
        }
        if (name === 'actor-sync-page-completed') {
          const ins = Number(payload?.inserted ?? 0) || 0;
          const upd = Number(payload?.updated ?? 0) || 0;
          const skp = Number(payload?.skipped ?? 0) || 0;
          const fld = Number(payload?.failed ?? 0) || 0;
          setSyncLive(prev => ({
            ...prev,
            pagesProcessed: (prev.pagesProcessed || 0) + 1,
            inserted: (prev.inserted || 0) + ins,
            updated: (prev.updated || 0) + upd,
            skipped: (prev.skipped || 0) + skp,
            failed: (prev.failed || 0) + fld,
          }));
        }
        if (name === 'actor-sync-warning') {
          const code = String(payload?.code || '');
          const detail = String(payload?.detail || '');
          setSyncLive(prev => ({ ...prev, lastWarn: `${code}: ${detail}`.slice(0, 160) }));
        }
        if (name === 'actor-sync-completed') {
          setSyncLive(prev => ({
            ...prev,
            active: false,
            pagesProcessed: Number(payload?.pages_processed ?? prev.pagesProcessed) || prev.pagesProcessed,
            inserted: Number(payload?.inserted ?? prev.inserted) || prev.inserted,
            updated: Number(payload?.updated ?? prev.updated) || prev.updated,
            skipped: Number(payload?.skipped ?? prev.skipped) || prev.skipped,
            failed: Number(payload?.failed ?? prev.failed) || prev.failed,
            durationMs: Number(payload?.duration_ms ?? prev.durationMs) || prev.durationMs,
          }));
          setStatusMessage('Sync 완료');
        }
        if (name === 'actor-session-started') {
          setIsRunning(true);
          setStatusMessage('크롤링 실행 중 (세션 시작)');
          addLog('🎬 세션 시작');
          // reset runtime stats
          setPageStats({ started: 0, completed: 0, failed: 0, retried: 0, totalEstimated: 0, inflight: 0 });
          setDetailStats({ started: 0, completed: 0, failed: 0, retried: 0, inflight: 0 });
          setBatchInfo({ current: 0 });
          // clear Stage 1 tracking
          pageSeen.clear();
          pageCompleted.clear();
          pageFailedFinal.clear();
          pageAttempts.clear();
          // clear unique tracking as a new session begins
          detailSeen.clear();
          detailCompleted.clear();
          detailFailedFinal.clear();
          detailAttempts.clear();
          setDownshiftInfo(null);
          setValidationStats({ started: false, completed: false, targetPages: 0, pagesScanned: 0, divergences: 0, anomalies: 0, productsChecked: 0, lastPage: null, lastAssignedStart: null, lastAssignedEnd: null });
          setDbSnapshot({});
          setPersistStats({ attempted: 0, succeeded: 0, failed: 0, duplicates: 0, durationMs: 0 });
        }
        if (name === 'actor-session-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 완료');
          addLog('🏁 세션 완료');
          setBatchInfo(prev => ({ ...prev }));
          // Play transition on session complete as well (helps visibility)
          try {
            const prev = crawlingRange();
            const prevStart = (prev?.range?.[0] ?? 0) as number;
            const prevEnd = (prev?.range?.[1] ?? 0) as number;
            const prevTotal = (prev?.progress?.total_products ?? 0) as number;
            const prevCover = `${(prev?.progress?.progress_percentage?.toFixed?.(1) ?? '0.0')}%`;
            setRangePrevSnapshot({ start: prevStart, end: prevEnd, total: prevTotal, coverText: String(prevCover) });
            if (effectsOn()) playRangeTransition();
          } catch {}
          // Recompute crawling range so the UI reflects the newly planned range
          calculateCrawlingRange();
        }
        if (name === 'actor-session-failed') {
          setIsRunning(false);
          setStatusMessage('크롤링 실패');
          addLog(`❌ 세션 실패: ${JSON.stringify(payload)}`);
          setBatchInfo(prev => ({ ...prev }));
        }
        if (name === 'actor-session-timeout' || name === 'actor-shutdown-completed') {
          setIsRunning(false);
          setStatusMessage('크롤링 종료');
          addLog('🛑 세션 종료');
          setBatchInfo(prev => ({ ...prev }));
          // Refresh planned range after abnormal end as well
          calculateCrawlingRange();
        }

        // Post-session auto re-plan (NextPlanReady)
        if (name === 'actor-next-plan-ready') {
          try {
            const plan = (payload && payload.plan) || payload;
            // Take snapshot before values change
            const prev = crawlingRange();
            const prevStart = (prev?.range?.[0] ?? 0) as number;
            const prevEnd = (prev?.range?.[1] ?? 0) as number;
            const prevTotal = (prev?.progress?.total_products ?? 0) as number;
            const prevCover = `${(prev?.progress?.progress_percentage?.toFixed?.(1) ?? '0.0')}%`;
            setRangePrevSnapshot({ start: prevStart, end: prevEnd, total: prevTotal, coverText: String(prevCover) });
            setNextPlan(plan);
            addLog('🧭 다음 실행 계획 수신');
            // Optimistically reflect into the Calculated Range panel
            applyPlanToCalculatedRange(plan);
            if (effectsOn()) playRangeTransition();
            // Update the calculated crawling range panel using backend planner
            calculateCrawlingRange();
          } catch (e) {
            console.warn('[CrawlingEngineTabSimple] next-plan parse failed', e);
          }
        }

        // Estimate totals from batch starts (pages in batch)
        if (name === 'actor-batch-started') {
          const t = (payload?.pages_in_batch ?? payload?.pages ?? payload?.items_total ?? payload?.pages_count ?? 0) as number;
          if (typeof t === 'number' && t > 0) {
            setPageStats(prev => ({ ...prev, totalEstimated: prev.totalEstimated + t }));
          }
          // Update batch info
          setBatchInfo(prev => {
            const current = (prev.current || 0) + 1;
            let totalEstimated = prev.totalEstimated;
            const pagesTotal = (crawlingRange()?.crawling_info?.pages_to_crawl as number) || 0;
            const batchSizeGuess = Number(t) || Number(payload?.batch_size ?? 0) || 0;
            if (!totalEstimated && pagesTotal > 0 && batchSizeGuess > 0) {
              totalEstimated = Math.max(1, Math.ceil(pagesTotal / batchSizeGuess));
            }
            return { current, totalEstimated, batchId: payload?.batch_id ?? prev.batchId, pagesInBatch: t || prev.pagesInBatch };
          });
        }
        if (name === 'actor-batch-completed') {
          // Keep current count; nothing to do for now.
        }
        // Stage 1 (list page) itemized with de-duplication and retry tracking
  if (name === 'actor-page-task-started') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          const prevAttempts = pageAttempts.get(pageNum) ?? 0;
          pageAttempts.set(pageNum, prevAttempts + 1);
          if (!pageSeen.has(pageNum)) {
            pageSeen.add(pageNum);
            setPageStats(prev => {
              const started = pageSeen.size; // unique pages
              const inflight = Math.max(0, started - (prev.completed + prev.failed));
              return { ...prev, started, inflight };
            });
          }
          if (effectsOn()) triggerStage1Pulse();
        }
  if (name === 'actor-page-task-completed') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          if (!pageCompleted.has(pageNum)) pageCompleted.add(pageNum);
          if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
          setPageStats(prev => {
            const started = pageSeen.size;
            const completed = pageCompleted.size;
            const inflight = Math.max(0, started - (completed + prev.failed));
            return { ...prev, started, completed, inflight };
          });
          if (effectsOn()) triggerStage1Pulse();
        }
  if (name === 'actor-page-task-failed') {
          const pageNum = Number(payload?.page ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          const final = Boolean(payload?.final_failure);
          const prevAttempts = pageAttempts.get(pageNum) ?? 0;
          pageAttempts.set(pageNum, prevAttempts + 1);
          if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
          if (final) {
            pageFailedFinal.add(pageNum);
          } else {
      setPageStats(prev => ({ ...prev, retried: prev.retried + 1 }));
          }
          setPageStats(prev => {
            const started = pageSeen.size;
            const failed = pageFailedFinal.size;
            const inflight = Math.max(0, started - (prev.completed + failed));
            return { ...prev, started, failed, inflight };
          });
          if (effectsOn()) triggerStage1Pulse();
        }
        // Stage 2 (product detail) itemized - deduplicate by detail_id and track retries
  // Stage 2 via product lifecycle events
        if (name === 'actor-product-lifecycle-group' && (payload?.phase === 'fetch')) {
          const group = Number(payload?.group_size ?? payload?.started ?? 0) || 0;
          const succeeded = Number(payload?.succeeded ?? 0) || group; // default: success when not provided
          const failed = Number(payload?.failed ?? 0) || 0;
          setDetailStats(prev => {
            const started = (prev.started || 0) + group;
            const completed = (prev.completed || 0) + succeeded;
            const failedCt = (prev.failed || 0) + failed;
            const inflight = Math.max(0, started - (completed + failedCt));
            return { ...prev, started, completed, failed: failedCt, inflight };
          });
          if (effectsOn()) triggerStage2Pulse();
        }
        if (name === 'actor-product-lifecycle') {
          const status = String(payload?.status || '').toLowerCase();
          if (status === 'failed') {
            setDetailStats(prev => {
              const started = prev.started || 0; // cannot infer per-product start
              const failed = (prev.failed || 0) + 1;
              const inflight = Math.max(0, started - (prev.completed + failed));
              return { ...prev, failed, inflight };
            });
            if (effectsOn()) triggerStage2Pulse();
          }
        }
        if (name === 'actor-detail-concurrency-downshifted') {
          setDownshiftInfo({ newLimit: payload?.new_limit, reason: payload?.reason });
        }

  // Stage 3 (Validation) events
        if (name === 'actor-validation-started') {
          const target = Number(payload?.scan_pages ?? 0) || 0;
          setValidationStats({ started: true, completed: false, targetPages: target, pagesScanned: 0, divergences: 0, anomalies: 0, productsChecked: 0, lastPage: null, lastAssignedStart: null, lastAssignedEnd: null });
        }
        if (name === 'actor-validation-page-scanned') {
          setValidationStats(prev => ({
            ...prev,
            pagesScanned: prev.pagesScanned + 1,
            // Optional: we can accumulate products_found into productsChecked
            productsChecked: prev.productsChecked + (Number(payload?.products_found ?? 0) || 0),
            lastPage: Number(payload?.physical_page ?? prev.lastPage ?? 0) || prev.lastPage,
            lastAssignedStart: Number(payload?.assigned_start_offset ?? prev.lastAssignedStart ?? 0) || prev.lastAssignedStart,
            lastAssignedEnd: Number(payload?.assigned_end_offset ?? prev.lastAssignedEnd ?? 0) || prev.lastAssignedEnd,
          }));
          // trigger subtle pulse animation
          if (effectsOn()) {
            setValidationPulse(true);
            setTimeout(() => setValidationPulse(false), 300);
          }
        }
        if (name === 'actor-validation-divergence') {
          setValidationStats(prev => ({ ...prev, divergences: prev.divergences + 1 }));
        }
        if (name === 'actor-validation-anomaly') {
          setValidationStats(prev => ({ ...prev, anomalies: prev.anomalies + 1 }));
        }
        if (name === 'actor-validation-completed') {
          setValidationStats(prev => ({
            ...prev,
            completed: true,
            pagesScanned: Number(payload?.pages_scanned ?? prev.pagesScanned) || prev.pagesScanned,
            productsChecked: Number(payload?.products_checked ?? prev.productsChecked) || prev.productsChecked,
            divergences: Number(payload?.divergences ?? prev.divergences) || prev.divergences,
            anomalies: Number(payload?.anomalies ?? prev.anomalies) || prev.anomalies,
          }));
        }

        // Fallback: If backend emits only generic stage events for Validation, reflect them here
        if (name === 'actor-stage-started') {
          const t = String(payload?.stage_type || '').toLowerCase();
          if (t.includes('validation')) {
            const total = Number(payload?.items_count ?? 0) || 0;
            setValidationStats(prev => ({
              ...prev,
              started: true,
              completed: false,
              targetPages: total || prev.targetPages,
            }));
          }
        }
        if (name === 'actor-stage-completed') {
          const t = String(payload?.stage_type || '').toLowerCase();
          if (t.includes('validation')) {
            const processed = Number(payload?.result?.processed_items ?? 0) || 0;
            setValidationStats(prev => ({
              ...prev,
              completed: true,
              pagesScanned: processed > 0 ? processed : prev.pagesScanned,
            }));
          }
        }

        // Stage 4 (DB) snapshots and session summary
        if (name === 'actor-database-stats') {
          setDbSnapshot(prev => ({
            ...prev,
            total: Number(payload?.total_product_details ?? prev.total ?? 0) || prev.total,
            minPage: payload?.min_page ?? prev.minPage ?? null,
            maxPage: payload?.max_page ?? prev.maxPage ?? null,
          }));
          if (effectsOn()) {
            setDbFlash(true);
            setTimeout(() => setDbFlash(false), 500);
          }
        }
        if (name === 'actor-session-report') {
          setDbSnapshot(prev => ({
            ...prev,
            inserted: Number(payload?.products_inserted ?? prev.inserted ?? 0) || prev.inserted,
            updated: Number(payload?.products_updated ?? prev.updated ?? 0) || prev.updated,
          }));
        }
        // Stage 5 (Persist) grouped lifecycle snapshot
        if (name === 'actor-product-lifecycle-group' && (payload?.phase === 'persist')) {
          const attempted = Number(payload?.group_size ?? 0) || 0;
          const succeeded = Number(payload?.succeeded ?? 0) || 0;
          const failed = Number(payload?.failed ?? 0) || 0;
          const duplicates = Number(payload?.duplicates ?? 0) || 0;
          const durationMs = Number(payload?.duration_ms ?? 0) || 0;
          setPersistStats({ attempted, succeeded, failed, duplicates, durationMs });
          // flash Stage 5 panel
          if (effectsOn()) {
            setPersistFlash(true);
            setTimeout(() => setPersistFlash(false), 500);
          }
        }
      })
      .then((un) => unsubs.push(un))
      .catch((e) => console.warn('[CrawlingEngineTabSimple] actor bridge subscribe failed', e));

    // Legacy completion/stopped fallbacks
    tauriApi
      .subscribeToCompletion(() => {
        setIsRunning(false);
        setStatusMessage('크롤링 완료');
        addLog('🏁 완료 이벤트 수신');
      })
      .then((un) => unsubs.push(un))
      .catch(() => {});

    tauriApi
      .subscribeToCrawlingStopped(() => {
        setIsRunning(false);
        setStatusMessage('크롤링 중지됨');
        addLog('⏹️ 중지 이벤트 수신');
      })
      .then((un) => unsubs.push(un))
      .catch(() => {});

    onCleanup(() => {
      unsubs.forEach((u) => u());
    });
  });

  return (
    <div class="w-full max-w-6xl mx-auto">
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6 mb-6">
        <h1 class="text-2xl font-bold text-gray-900 mb-2">🤖 스마트 크롤링 엔진</h1>
        <p class="text-gray-600 text-sm mb-4">
          설정 파일 기반 자동 크롤링 시스템 - 별도 설정 전송 없이 즉시 시작
        </p>

        {/* Sync Runtime (compact) */}
        <Show when={syncLive().active || syncLive().pagesProcessed > 0}>
          <div class="mb-4 p-3 rounded-lg border border-teal-200 bg-teal-50">
            <div class="flex items-center justify-between mb-1">
              <div class="text-sm font-semibold text-teal-900">Stage S: DB Sync</div>
              <div class="text-[11px] text-teal-700">{syncLive().planned ? `계획 ${syncLive().planned}p` : '계획 미정'}</div>
            </div>
            <div class="mt-1 w-full bg-teal-100 rounded h-2">
              {(() => {
                const processed = syncLive().pagesProcessed || 0;
                const total = syncLive().planned || processed || 1;
                const pct = Math.min(100, (processed / Math.max(1, total)) * 100);
                return <div class="h-2 bg-teal-500 rounded transition-all" style={{ width: `${pct}%` }} />;
              })()}
            </div>
            <div class="mt-2 flex gap-3 text-xs text-teal-900">
              <span>pages: <b>{syncLive().pagesProcessed}</b></span>
              <span>ins: <b class="text-emerald-700">{syncLive().inserted}</b></span>
              <span>upd: <b class="text-indigo-700">{syncLive().updated}</b></span>
              <span>skip: <b class="text-gray-700">{syncLive().skipped}</b></span>
              <span>fail: <b class="text-rose-700">{syncLive().failed}</b></span>
              <Show when={typeof syncLive().durationMs !== 'undefined'}>
                <span>ms: <b>{syncLive().durationMs}</b></span>
              </Show>
            </div>
            <Show when={syncLive().lastWarn}>
              <div class="mt-1 text-[11px] text-rose-800 bg-rose-50 border border-rose-200 rounded px-2 py-1">
                최근 경고: <span class="line-clamp-2">{syncLive().lastWarn}</span>
              </div>
            </Show>
          </div>
        </Show>

        {/* 상태 표시 */}
        <div class="mb-6">
          <div class={`px-4 py-3 rounded-lg border ${isRunning() 
            ? 'bg-blue-50 border-blue-200 text-blue-700' 
            : 'bg-green-50 border-green-200 text-green-700'
          }`}>
            <div class="flex items-center space-x-2">
              <span>{isRunning() ? '🔄' : '✅'}</span>
              <span class="font-medium">{statusMessage()}</span>
              <Show when={isRunning() && (batchInfo().current > 0)}>
                <span class="text-xs ml-2 px-2 py-0.5 rounded bg-blue-100 text-blue-700">
                  배치 {batchInfo().current}{batchInfo().totalEstimated ? `/${batchInfo().totalEstimated}` : ''}
                </span>
              </Show>
              <Show when={isRunning() && batchInfo().batchId}>
                <span class="text-[10px] ml-1 text-gray-500">({batchInfo().batchId})</span>
              </Show>
            </div>
          </div>
          {/* Next plan preview panel */}
          <Show when={nextPlan()}>
            <div class="mt-3 p-3 rounded-lg border border-indigo-200 bg-indigo-50 transition-opacity duration-300 opacity-100">
              <div class="flex items-start justify-between gap-3">
                <div>
                  <div class="text-sm font-semibold text-indigo-900">🧭 다음 실행 계획 준비됨</div>
                  <div class="text-xs text-indigo-800 mt-1">
                    {(() => {
                      try {
                        const plan: any = nextPlan();
                        const phases = (plan?.phases || []) as any[];
                        const pages: number[] = phases.flatMap((p: any) => Array.isArray(p?.pages) ? (p.pages as number[]) : []);
                        const uniq = Array.from(new Set(pages)).sort((a,b)=>b-a);
                        const sample = uniq.slice(0, Math.min(24, uniq.length));
                        return (
                          <span>
                            단계 {phases.length}개 • 페이지 {uniq.length}개
                            <span class="block mt-0.5 font-mono text-[11px] text-indigo-900">
                              {sample.join(', ')}{uniq.length>sample.length ? ' …' : ''}
                            </span>
                          </span>
                        );
                      } catch { return <span>요약 표시 오류</span>; }
                    })()}
                  </div>
                </div>
                <div class="shrink-0 flex flex-col items-end gap-1">
                  <button
                    class="px-2.5 py-1 text-xs rounded bg-indigo-600 text-white hover:bg-indigo-700"
                    title="이 계획의 페이지를 Sync 범위 입력에 적용"
                    onClick={() => {
                      try {
                        const plan: any = nextPlan();
                        const phases = (plan?.phases || []) as any[];
                        const pages: number[] = phases.flatMap((p: any) => Array.isArray(p?.pages) ? (p.pages as number[]) : []);
                        const uniq = Array.from(new Set(pages)).sort((a,b)=>b-a);
                        let parts: string[] = [];
                        if (uniq.length) {
                          let start = uniq[0];
                          let prev = uniq[0];
                          for (const pg of uniq.slice(1)) {
                            if (pg + 1 === prev) { prev = pg; continue; }
                            parts.push(start === prev ? `${start}` : `${start}-${prev}`);
                            start = pg; prev = pg;
                          }
                          parts.push(start === prev ? `${start}` : `${start}-${prev}`);
                        }
                        const expr = parts.join(',');
                        if (expr) {
                          setSyncRanges(expr);
                          addLog(`🧭 다음 계획 적용 → Sync 범위: ${expr}`);
                          setSyncPulse(true);
                          setTimeout(() => setSyncPulse(false), 400);
                        }
                      } catch (e) {
                        console.warn('apply next plan failed', e);
                      }
                    }}
                  >계획 적용 → Sync</button>
                  <button
                    class="px-2.5 py-1 text-xs rounded bg-gray-200 text-gray-700 hover:bg-gray-300"
                    onClick={() => setNextPlan(null)}
                  >숨기기</button>
                </div>
              </div>
            </div>
          </Show>
        </div>

  {/* Stage1/Stage2 Runtime Monitor */}
  <div class={`grid grid-cols-1 md:grid-cols-2 gap-4 mb-6 ${stage1Pulse() ? 'pulse-once' : ''}`}>
    <div class={`bg-white rounded-lg border p-4 ${stage1Pulse() ? 'pulse-once' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 1: 제품 목록 수집</h3>
              <span class="text-xs text-gray-500">
                {(() => {
                  const cr = crawlingRange();
                  const fallback = (cr?.crawling_info?.pages_to_crawl ?? (((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1) || 0)) as number;
                  const est = pageStats().totalEstimated || fallback || 0;
                  return est > 0 ? `예상 ${est}p` : '';
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600"><CountUp value={pageStats().started} /></div>
                <div class="text-xs text-gray-600">시작</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600"><CountUp value={pageStats().completed} /></div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600"><CountUp value={pageStats().inflight} /></div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600"><CountUp value={pageStats().failed} /></div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600"><CountUp value={pageStats().retried} /></div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="progress-fill rounded-full" style={{ width: `${(() => {
                const cr = crawlingRange();
                const fallback = (cr?.crawling_info?.pages_to_crawl ?? (((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1) || 0)) as number;
                const denom = pageStats().totalEstimated || fallback || 0;
                return denom > 0 ? Math.min(100, (pageStats().completed / denom) * 100) : 0;
              })()}%` }}></div>
            </div>
          </div>

          <div class={`bg-white rounded-lg border p-4 ${stage2Pulse() ? 'pulse-once' : ''}`}> 
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 2: 세부 정보 수집</h3>
              <Show when={!!downshiftInfo()}>
                <span class="text-[10px] px-2 py-1 bg-yellow-100 text-yellow-700 rounded shake-x" title={downshiftInfo()?.reason || ''}>↓ 제한 {downshiftInfo()?.newLimit ?? '-'}
                </span>
              </Show>
              <span class="text-xs text-gray-500">
                {(() => {
                  const est = (crawlingRange()?.crawling_info?.estimated_new_products ?? 0) as number;
                  return est > 0 ? `예상 ${est}` : '';
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600"><CountUp value={detailStats().started} /></div>
                <div class="text-xs text-gray-600">시작</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600"><CountUp value={detailStats().completed} /></div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600"><CountUp value={detailStats().inflight} /></div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600"><CountUp value={detailStats().failed} /></div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600"><CountUp value={detailStats().retried} /></div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="progress-fill rounded-full" style={{ width: `${(() => {
                const denom = (crawlingRange()?.crawling_info?.estimated_new_products as number) || detailStats().started || 0;
                return denom > 0 ? Math.min(100, (detailStats().completed / denom) * 100) : 0;
              })()}%` }}></div>
            </div>
          </div>
        </div>

        {/* Stage X: DB Pagination Diagnostics */}
        <div class="bg-white rounded-lg border p-4 mb-6">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-md font-semibold text-gray-800">Stage X: DB Pagination Diagnostics</h3>
            <div class="flex gap-2">
              <button class={`px-3 py-1 text-sm rounded ${diagLoading() ? 'bg-gray-200 text-gray-500' : 'bg-indigo-600 text-white hover:bg-indigo-700'}`} disabled={diagLoading()} onClick={runDiagnostics}>
                {diagLoading() ? '진단 중…' : '진단 실행'}
              </button>
              <button class={`px-3 py-1 text-sm rounded ${cleanupLoading() ? 'bg-gray-200 text-gray-500' : 'bg-rose-600 text-white hover:bg-rose-700'}`} disabled={cleanupLoading()} onClick={runUrlCleanup}>
                {cleanupLoading() ? '정리 중…' : 'URL 중복 제거'}
              </button>
              <button
                class={`px-3 py-1 text-sm rounded ${isSyncing() ? 'bg-gray-200 text-gray-500' : 'bg-blue-600 text-white hover:bg-blue-700'}`}
                disabled={isSyncing()}
                onClick={async () => {
                  try {
                    setIsSyncing(true);
                    addLog('🔁 products→details 좌표/ID 정합화 실행...');
                    const rep = await tauriApi.syncProductDetailsCoordinates();
                    addLog(`✅ 정합화 완료: products.id=${rep.updated_product_ids}, inserted=${rep.inserted_details}, updated_coords=${rep.updated_coordinates}, details.id=${rep.updated_ids} (p=${rep.total_products}, d=${rep.total_details})`);
                  } catch (e:any) {
                    addLog(`❌ 정합화 실패: ${e.message || e}`);
                  } finally {
                    setIsSyncing(false);
                  }
                }}
                title="products.url 기준으로 product_details에 page_id/index_in_page/id를 정합화합니다 (크롤링 없음)"
              >
                products→details 동기화
              </button>
              <button class={`px-3 py-1 text-sm rounded ${isSyncing() ? 'bg-gray-200 text-gray-500' : 'bg-teal-600 text-white hover:bg-teal-700'}`} disabled={isSyncing()} onClick={syncMissingPagesFromDiagnostics}>
                {isSyncing() ? '동기화 중…' : '누락 페이지만 동기화'}
              </button>
                <button class={`px-3 py-1 text-sm rounded ${isSyncing() ? 'bg-gray-200 text-gray-500' : 'bg-emerald-600 text-white hover:bg-emerald-700'}`} disabled={isSyncing()} onClick={runPreciseDiagnosticRepair}>
                  {isSyncing() ? '동기화 중…' : '정밀 복구 실행'}
                </button>
            </div>
          </div>
          <Show when={diagResult()} fallback={<p class="text-xs text-gray-500">로컬 DB의 page_id/index_in_page 정합성을 검사합니다. 실행을 눌러 결과를 확인하세요.</p>}>
            <div class="text-xs text-gray-700 space-y-2">
              {(() => {
                const expr = deriveRangesFromDiagnostics();
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
                        onClick={() => { setSyncRanges(expr); setSyncPulse(true); setTimeout(() => setSyncPulse(false), 400); addLog(`🧭 추천 범위 적용 → ${expr}`); }}
                      >적용</button>
                    </div>
                  </div>
                );
              })()}
              <div class="flex gap-4">
                <span>총 제품: <b>{diagResult()?.total_products ?? 0}</b></span>
                <span>DB 최대 page_id: <b>{diagResult()?.max_page_id_db ?? '-'}</b></span>
                <span>사이트 총 페이지: <b>{diagResult()?.total_pages_site ?? '-'}</b></span>
                <span>마지막 페이지 아이템: <b>{diagResult()?.items_on_last_page ?? '-'}</b></span>
              </div>
              <Show when={diagResult()?.prepass}>
                <div class="flex gap-4 text-teal-800 bg-teal-50 border border-teal-200 rounded p-2">
                  <span>사전 정렬(details): <b>{diagResult()?.prepass?.details_aligned ?? 0}</b></span>
                  <span>products.id 백필: <b>{diagResult()?.prepass?.products_id_backfilled ?? 0}</b></span>
                </div>
              </Show>
              <div>
                <b>이상 그룹</b>
                <ul class="list-disc ml-5">
                  <For each={(diagResult()?.group_summaries ?? []).filter((g: any) => g.status !== 'ok')}>
                    {(g: any) => (
                      <li>
                        page_id {g.page_id}{g.current_page_number != null ? ` (물리 ${g.current_page_number})` : ''}: status={g.status} count={g.count} distinct={g.distinct_indices}
                        {g.duplicate_indices?.length ? ` dup=${g.duplicate_indices.join(',')}` : ''}
                        {g.missing_indices?.length ? ` miss=${g.missing_indices.join(',')}` : ''}
                        {g.out_of_range_count ? ` oob=${g.out_of_range_count}` : ''}
                      </li>
                    )}
                  </For>
                </ul>
              </div>
              <Show when={(diagResult()?.duplicate_positions ?? []).length > 0}>
                <div>
                  <b>중복 위치 샘플</b>
                  <ul class="list-disc ml-5">
                    <For each={(diagResult()?.duplicate_positions ?? []).slice(0, 20)}>
                      {(d: any) => (
                        <li>
                          page_id {d.page_id}{d.current_page_number != null ? ` (물리 ${d.current_page_number})` : ''}, index {d.index_in_page}: {d.urls?.length ?? 0}개 URL
                        </li>
                      )}
                    </For>
                  </ul>
                </div>
              </Show>
            </div>
          </Show>
        </div>

  {/* Stage3/Stage4/Stage5 Mini Panels */}
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          {/* Stage 3: Validation */}
          <div class="bg-white rounded-lg border p-4">
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 3: Validation</h3>
              <span class="text-xs text-gray-500">
                {validationStats().started ? (validationStats().completed ? '완료' : '진행 중') : '대기'}
              </span>
            </div>
            <div class="grid grid-cols-4 gap-2 text-center">
              <div class="bg-indigo-50 rounded p-2">
                <div class="text-xl font-bold text-indigo-600">{effectsOn() ? <CountUp value={validationStats().targetPages} /> : validationStats().targetPages}</div>
                <div class="text-xs text-gray-600">대상 페이지</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{effectsOn() ? <CountUp value={validationStats().pagesScanned} /> : validationStats().pagesScanned}</div>
                <div class="text-xs text-gray-600">스캔</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{effectsOn() ? <CountUp value={validationStats().divergences} /> : validationStats().divergences}</div>
                <div class="text-xs text-gray-600">불일치</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{effectsOn() ? <CountUp value={validationStats().anomalies} /> : validationStats().anomalies}</div>
                <div class="text-xs text-gray-600">이상</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div class="h-2 rounded-full bg-indigo-500 transition-all" style={{ width: `${(() => {
                const t = validationStats().targetPages || 0;
                const s = validationStats().pagesScanned || 0;
                return t > 0 ? Math.min(100, (s / t) * 100) : 0;
              })()}%` }}></div>
            </div>
            <Show when={validationStats().lastPage != null}>
              <div class="mt-2 text-[11px] text-gray-500">
                최근 스캔: 페이지 {validationStats().lastPage} (오프셋 {validationStats().lastAssignedStart ?? '-'}–{validationStats().lastAssignedEnd ?? '-'})
              </div>
            </Show>
          </div>
          {/* Stage 4: DB Snapshot */}
          <div class={`bg-white rounded-lg border p-4 ${dbFlash() && effectsOn() ? 'flash-db' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 4: DB 저장 스냅샷</h3>
              <span class="text-xs text-gray-500">최근 보고 기준</span>
            </div>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-2 text-center">
              <div class="bg-sky-50 rounded p-2">
                <div class="text-xl font-bold text-sky-600">{effectsOn() && typeof dbSnapshot().total === 'number' ? <CountUp value={dbSnapshot().total as number} /> : (dbSnapshot().total ?? '-')}</div>
                <div class="text-xs text-gray-600">총 상세 수</div>
              </div>
              <div class="bg-purple-50 rounded p-2">
                <div class="text-xl font-bold text-purple-600">{effectsOn() && typeof dbSnapshot().minPage === 'number' ? <CountUp value={dbSnapshot().minPage as number} /> : (dbSnapshot().minPage ?? '-')}</div>
                <div class="text-xs text-gray-600">DB 최소 페이지</div>
              </div>
              <div class="bg-purple-50 rounded p-2">
                <div class="text-xl font-bold text-purple-600">{effectsOn() && typeof dbSnapshot().maxPage === 'number' ? <CountUp value={dbSnapshot().maxPage as number} /> : (dbSnapshot().maxPage ?? '-')}</div>
                <div class="text-xs text-gray-600">DB 최대 페이지</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">
                  {effectsOn() ? <CountUp value={dbSnapshot().inserted ?? 0} /> : (dbSnapshot().inserted ?? 0)}
                  /
                  {effectsOn() ? <CountUp value={dbSnapshot().updated ?? 0} /> : (dbSnapshot().updated ?? 0)}
                </div>
                <div class="text-xs text-gray-600">삽입/업데이트(세션)</div>
              </div>
            </div>
          </div>

          {/* Stage 5: Persist 요약 */}
          <div class={`bg-white rounded-lg border p-4 ${persistFlash() && effectsOn() ? 'flash-save' : ''}`}>
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">Stage 5: 저장 요약</h3>
              <span class="text-xs text-gray-500">그룹 이벤트</span>
            </div>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">{effectsOn() ? <CountUp value={persistStats().attempted} /> : persistStats().attempted}</div>
                <div class="text-xs text-gray-600">시도</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">{effectsOn() ? <CountUp value={persistStats().succeeded} /> : persistStats().succeeded}</div>
                <div class="text-xs text-gray-600">성공</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">{effectsOn() ? <CountUp value={persistStats().failed} /> : persistStats().failed}</div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">{effectsOn() ? <CountUp value={persistStats().duplicates} /> : persistStats().duplicates}</div>
                <div class="text-xs text-gray-600">중복</div>
              </div>
            </div>
            <div class="mt-2 text-xs text-gray-500">소요 시간: {persistStats().durationMs}ms</div>
          </div>
        </div>

        {/* 크롤링 범위 정보 */}
        <Show when={crawlingRange()}>
          <div ref={el => (rangePanelRef = el!)} class={`bg-gray-50 rounded-lg p-4 mb-6 ${rangeFxActive() ? 'range-transition-ring' : ''}`}>
            <div class="flex items-center justify-between mb-3">
              <h3 class="text-lg font-semibold text-gray-900">📊 계산된 크롤링 범위</h3>
              <button
                class="text-xs px-2.5 py-1 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-40"
                onClick={() => {
                  const prev = crawlingRange();
                  const prevStart = (prev?.range?.[0] ?? 0) as number;
                  const prevEnd = (prev?.range?.[1] ?? 0) as number;
                  const prevTotal = (prev?.progress?.total_products ?? 0) as number;
                  const prevCover = `${(prev?.progress?.progress_percentage?.toFixed?.(1) ?? '0.0')}%`;
                  setRangePrevSnapshot({ start: prevStart, end: prevEnd, total: prevTotal, coverText: String(prevCover) });
                  if (effectsOn()) playRangeTransition();
                }}
                disabled={!effectsOn()}
                title={effectsOn() ? '계산된 범위 효과 미리보기' : '효과가 꺼져 있습니다'}
              >효과 미리보기</button>
            </div>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              <div class="text-center">
                <div class="text-2xl font-bold text-blue-600">
                  <Show when={rangeFxActive()} fallback={<span class="drum-line">{renderDrumText(String(crawlingRange()?.range?.[0] || 0))}</span>}>
                    <span class="shatter-line">{renderShatterText(String((rangePrevSnapshot()?.start ?? (crawlingRange()?.range?.[0] || 0))))}</span>
                  </Show>
                </div>
                <div class="text-sm text-gray-600">시작 페이지</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-green-600">
                  <Show when={rangeFxActive()} fallback={<span class="drum-line">{renderDrumText(String(crawlingRange()?.range?.[1] || 0))}</span>}>
                    <span class="shatter-line">{renderShatterText(String((rangePrevSnapshot()?.end ?? (crawlingRange()?.range?.[1] || 0))))}</span>
                  </Show>
                </div>
                <div class="text-sm text-gray-600">종료 페이지</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-purple-600">
                  {crawlingRange()?.progress?.total_products || 0}
                </div>
                <div class="text-sm text-gray-600">총 제품 수</div>
              </div>
              <div class="text-center">
                <div class="text-2xl font-bold text-orange-600">
                  <Show when={rangeFxActive()} fallback={<span class="drum-line">{renderDrumText(`${(crawlingRange()?.progress?.progress_percentage.toFixed(1) || 0)}%`)}</span>}>
                    <span class="shatter-line">{renderShatterText(String(rangePrevSnapshot()?.coverText ?? `${(crawlingRange()?.progress?.progress_percentage.toFixed(1) || 0)}%`))}</span>
                  </Show>
                </div>
                <div class="text-sm text-gray-600">커버리지</div>
              </div>
            </div>

            {/* Confetti overlay */}
            <Show when={confettiPieces().length > 0}>
              <div class="relative">
                <div class="pointer-events-none absolute inset-0 overflow-visible" aria-hidden="true">
          <For each={confettiPieces()}>{(p) => (
                    <span
                      class="confetti-piece"
            style={{ left: '50%', top: '0', background: p.color, '--cx': `${p.rx}px`, '--cy': `${p.ry}px`, '--crot': `${p.rot}deg`, '--cw': `${p.cw}px`, '--ch': `${p.ch}px` } as any}
                    />
                  )}</For>
                </div>
              </div>
            </Show>

            {/* 사이트 정보 섹션 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">🌐 사이트 정보</h4>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-blue-600">{crawlingRange()?.site_info?.total_pages || 0}</div>
                  <div class="text-xs text-gray-600">사이트 총 페이지 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-green-600">{crawlingRange()?.site_info?.products_on_last_page || 0}</div>
                  <div class="text-xs text-gray-600">마지막 페이지 제품 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-purple-600">{crawlingRange()?.site_info?.estimated_total_products || 0}</div>
                  <div class="text-xs text-gray-600">추정 총 제품 수</div>
                </div>
              </div>
            </div>

            {/* 로컬 DB 정보 섹션 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">💾 로컬 DB 정보</h4>
              <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-4">
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-indigo-600">{crawlingRange()?.local_db_info?.total_saved_products || 0}</div>
                  <div class="text-xs text-gray-600">수집한 제품 수</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-teal-600">{crawlingRange()?.local_db_info?.last_crawled_page || 'N/A'}</div>
                  <div class="text-xs text-gray-600">마지막 크롤링 페이지</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-pink-600">{crawlingRange()?.local_db_info?.coverage_percentage?.toFixed(1) || 0}%</div>
                  <div class="text-xs text-gray-600">DB 커버리지</div>
                </div>
                <div class="text-center bg-white rounded p-3 border">
                  <div class="text-xl font-bold text-cyan-600">{crawlingRange()?.crawling_info?.pages_to_crawl || 0}</div>
                  <div class="text-xs text-gray-600">크롤링할 페이지 수</div>
                </div>
              </div>
            </div>

            {/* 크롤링 전략 정보 */}
            <div class="border-t pt-4">
              <h4 class="text-md font-medium text-gray-800 mb-3">🎯 크롤링 전략</h4>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="bg-white rounded p-3 border">
                  <div class="text-sm text-gray-600">전략</div>
                  <div class="text-lg font-semibold text-gray-800 capitalize">{crawlingRange()?.crawling_info?.strategy || 'unknown'}</div>
                </div>
                <div class="bg-white rounded p-3 border">
                  <div class="text-sm text-gray-600">예상 신규 제품</div>
                  <div class="text-lg font-semibold text-gray-800">{crawlingRange()?.crawling_info?.estimated_new_products || 0}</div>
                </div>
              </div>
            </div>
          </div>
        </Show>

  {/* 제어 버튼 */}
  <div class="flex flex-wrap gap-4 mb-6 items-end">
          {/* Legacy simple crawling button removed */}
          
          <button
            onClick={(e) => { triggerStartWave(e as unknown as MouseEvent); startUnifiedAdvanced(); }}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ripple ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-purple-600 hover:bg-purple-700'
            }`}
          >
            {isRunning() ? '통합 파이프라인 실행 중...' : '🎭 통합 파이프라인 (하이)'}
          </button>
          
          <button
            onClick={(e) => { triggerStartWave(e as unknown as MouseEvent); startLightUnified(); }}
            disabled={isRunning()}
            class={`px-6 py-3 rounded-lg font-medium text-white ripple ${
              isRunning() 
                ? 'bg-gray-400 cursor-not-allowed' 
                : 'bg-orange-600 hover:bg-orange-700'
            }`}
          >
            {isRunning() ? '통합 파이프라인 실행 중...' : '🎭 통합 파이프라인 (라이트)'}
          </button>
          
          <button
            onClick={calculateCrawlingRange}
            disabled={isRunning()}
            class="px-6 py-3 rounded-lg font-medium text-blue-600 border border-blue-600 hover:bg-blue-50 disabled:opacity-50 ripple"
          >
            📊 범위 다시 계산
          </button>

          {/* Surface ripple preview button */}
          <button
            onClick={(e) => triggerStartWave(e as unknown as MouseEvent)}
            disabled={isRunning()}
            class={`px-3 py-2 rounded-lg font-medium border text-gray-700 hover:bg-gray-50 ${
              isRunning() ? 'opacity-50 cursor-not-allowed' : 'border-gray-300'
            }`}
            title="원형 파동 효과 미리보기"
          >파도 미리보기</button>

          {/* Global start-wave overlay (circular) */}
          <div class="start-wave-root" aria-hidden="true">
            <For each={waveBursts()}>{(w) => (
              <>
                <div
                  class={`start-wave-circle ${w.kind === 'up' ? 'wave-up' : w.kind === 'down' ? 'wave-down' : 'wave-ring'}`}
                  style={{ '--x': `${w.x}px`, '--y': `${w.y}px`, '--scale': `${(() => {
                    const dx = Math.max(w.x, window.innerWidth - w.x);
                    const dy = Math.max(w.y, window.innerHeight - w.y);
                    const radius = Math.hypot(dx, dy);
                    const baseRadius = 12;
                    return Math.max(35, (radius / baseRadius));
                  })()}` } as any}
                />
                {w.kind === 'up' && <div class="start-wave-wash" style={{ '--x': `${w.x}px`, '--y': `${w.y}px` } as any} />}
              </>
            )}</For>
          </div>
          {/* Validation Controls */}
          <div class="flex items-center gap-2">
            <input
              type="number"
              min="1"
              class="w-28 px-3 py-2 border rounded-md text-sm"
              placeholder="검증 페이지 수"
              value={validationPages() as any}
              onInput={(e) => {
                const v = (e.currentTarget.value || '').trim();
                setValidationPages(v === '' ? '' : Number(v));
              }}
            />
            <button
              onClick={startValidationRun}
              disabled={isValidating()}
              class={`px-4 py-2 rounded-lg font-medium text-white ripple ${
                isValidating() ? 'bg-gray-400 cursor-not-allowed' : 'bg-emerald-600 hover:bg-emerald-700'
              }`}
            >
              {isValidating() ? '검증 실행 중...' : '🧪 Validation 실행'}
            </button>
          </div>
          {/* Sync Controls */}
          <div class="flex items-center gap-2">
            <input
              type="text"
              class={`w-64 px-3 py-2 border rounded-md text-sm ${syncPulse() && effectsOn() ? 'flash-db' : ''}`}
              placeholder="Sync 범위 (예: 498-492,489,487-485)"
              value={syncRanges()}
              onInput={(e) => setSyncRanges(e.currentTarget.value)}
            />
            <button
              onClick={startSyncRun}
              disabled={isSyncing()}
              class={`px-4 py-2 rounded-lg font-medium text-white ripple ${
                isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-teal-600 hover:bg-teal-700'
              }`}
            >
              {isSyncing() ? 'Sync 실행 중...' : '🔄 Sync 실행'}
            </button>
            <button
              onClick={async () => {
                if (isSyncing()) return;
                let ranges = (syncRanges() || '').trim();
                if (!ranges) {
                  const auto = deriveRangesFromDiagnostics();
                  if (auto) {
                    setSyncRanges(auto);
                    addLog(`🔁 Diagnostics 기반 범위 자동설정: ${auto}`);
                    ranges = auto;
                  } else {
                    addLog('⚠️ 먼저 Sync 범위를 입력하거나, 진단을 실행해 주세요. 예: 498-492,489');
                    return;
                  }
                }
                setIsSyncing(true);
                addLog(`🔄 Partial 모드(이 범위) Sync 실행: ${ranges}`);
                try {
                  try { await invoke('ui_debug_log', { message: `[SimpleTab] sync_button_click ranges=${ranges}` }); } catch {}
                  const res = await tauriApi.startPartialSync(ranges);
                  addLog(`✅ Partial Sync 완료: ${JSON.stringify(res)}`);
                } catch (e) {
                  addLog(`❌ Partial Sync 실패: ${e}`);
                } finally { setIsSyncing(false); }
              }}
              disabled={isSyncing()}
              class={`px-4 py-2 rounded-lg font-medium text-white ripple ${
                isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'
              }`}
              title="Partial 모드로 이 범위만 실행"
            >
              이 범위 Sync 실행
            </button>
            <button
              onClick={async () => {
                if (isSyncing()) return;
                let ranges = (syncRanges() || '').trim();
                if (!ranges) {
                  const auto = deriveRangesFromDiagnostics();
                  if (auto) {
                    setSyncRanges(auto);
                    addLog(`🔁 Diagnostics 기반 범위 자동설정: ${auto}`);
                    ranges = auto;
                  } else {
                    addLog('⚠️ 먼저 Sync 범위를 입력하거나, 진단을 실행해 주세요. 예: 498-492,489');
                    return;
                  }
                }
                // Parse ranges into explicit pages
                const norm = ranges
                  .replace(/\s+/g, '')
                  .replace(/[–—−﹣－]/g, '-')
                  .replace(/[〜～]/g, '~');
                const tokens = norm.split(',').map(t => t.trim()).filter(Boolean);
                const pages: number[] = [];
                for (const tk of tokens) {
                  if (tk.includes('-') || tk.includes('~')) {
                    const sep = tk.includes('~') ? '~' : '-';
                    const [a,b] = tk.split(sep);
                    let s = parseInt(a, 10), e = parseInt(b, 10);
                    if (!Number.isFinite(s) || !Number.isFinite(e)) continue;
                    if (e > s) { const tmp = s; s = e; e = tmp; }
                    for (let p = s; p >= e; p--) pages.push(p);
                  } else {
                    const v = parseInt(tk, 10);
                    if (Number.isFinite(v)) pages.push(v);
                  }
                }
                const seen = new Set<number>();
                const uniquePages = pages.filter(p => seen.has(p) ? false : (seen.add(p), true));
                if (uniquePages.length === 0) {
                  addLog('⚠️ 유효한 페이지가 없습니다. 예: 498-492,489');
                  return;
                }
                setIsSyncing(true);
                addLog(`🧑‍💻 수동 크롤링(Actor) 실행: [${uniquePages.join(', ')}]`);
                try {
                  const res = await tauriApi.startManualCrawlPagesActor(uniquePages, true);
                  addLog(`✅ 수동 크롤링 세션 시작: ${JSON.stringify(res)}`);
                  if (res?.session_id) {
                    addLog(`🆔 세션 ID: ${res.session_id}`);
                  }
                } catch (e) {
                  addLog(`❌ 수동 크롤링(Actor) 실패: ${e}`);
                } finally { setIsSyncing(false); }
              }}
              disabled={isSyncing()}
              class={`px-4 py-2 rounded-lg font-medium text-white ripple ${
                isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-purple-600 hover:bg-purple-700'
              }`}
              title="기본 엔진으로 명시적 페이지 배열을 실행"
            >
              수동 크롤링
            </button>
            <button
              onClick={async () => {
                if (isSyncing()) return;
                let ranges = (syncRanges() || '').trim();
                if (!ranges) {
                  const auto = deriveRangesFromDiagnostics();
                  if (auto) {
                    setSyncRanges(auto);
                    addLog(`🔁 Diagnostics 기반 범위 자동설정: ${auto}`);
                    ranges = auto;
                  } else {
                    addLog('⚠️ 먼저 Sync 범위를 입력하거나, 진단을 실행해 주세요. 예: 498-492,489');
                    return;
                  }
                }
                setIsSyncing(true);
                addLog(`📦 순차 실행(연속 페이지 배치): ${ranges}`);
                try {
                  const res = await tauriApi.startBatchedSync(ranges);
                  addLog(`✅ 순차 실행 완료: ${JSON.stringify(res)}`);
                } catch (e) {
                  addLog(`❌ 순차 실행 실패: ${e}`);
                } finally { setIsSyncing(false); }
              }}
              disabled={isSyncing()}
              class={`px-4 py-2 rounded-lg font-medium text-white ripple ${
                isSyncing() ? 'bg-gray-400 cursor-not-allowed' : 'bg-indigo-600 hover:bg-indigo-700'
              }`}
              title="연속 페이지를 배치로 묶어 순차 실행 (Partial과 동일 Flow)"
            >
              순차 실행
            </button>
          </div>
          <button
            onClick={() => setShowConsole(!showConsole())}
            class="px-6 py-3 rounded-lg font-medium text-gray-700 border border-gray-300 hover:bg-gray-50"
          >
            {showConsole() ? '🧪 이벤트 콘솔 숨기기' : '🧪 이벤트 콘솔 보기'}
          </button>
          {/* Effects toggle */}
          <label class="flex items-center gap-2 text-sm text-gray-600 select-none">
            <input type="checkbox" checked={effectsOn()} onInput={(e) => setEffectsOn(e.currentTarget.checked)} />
            애니메이션 효과
          </label>
        </div>

        {/* 실시간 로그 */}
        <div class="bg-black rounded-lg p-4">
          <h3 class="text-sm font-semibold text-white mb-2">📝 실시간 로그</h3>
          <div class="font-mono text-xs text-green-400 h-64 overflow-y-auto">
            <Show 
              when={logs().length > 0} 
              fallback={<div class="text-gray-500">로그 대기 중...</div>}
            >
              <For each={logs()}>
                {(log) => (
                  <div class="mb-1">{log}</div>
                )}
              </For>
            </Show>
          </div>
        </div>

        {/* Actor 이벤트 콘솔 (개발용) */}
        <Show when={showConsole()}>
          <div class="mt-6 border rounded-lg">
            <div class="px-4 py-2 border-b bg-gray-50 text-sm text-gray-700">Actor 이벤트 콘솔</div>
            <EventConsole />
          </div>
        </Show>
      </div>
    </div>
  );
}
