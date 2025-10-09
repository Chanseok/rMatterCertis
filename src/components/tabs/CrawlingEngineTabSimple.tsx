import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import SessionStatusCard from "./parts/SessionStatusCard";
import ControlPanel from "./parts/ControlPanel";
import StageStatsPanels from "./parts/StageStatsPanels";
import DiagnosticsPanel from "./parts/DiagnosticsPanel";
import HelpPanel from "./parts/HelpPanel";
import ListPageProgressPanel from "../ListPageProgressPanel";
import ComplementCrawlProgressPanel from "../ComplementCrawlProgressPanel";
import TimeEstimatesPanel from "../TimeEstimatesPanel";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
// Types are relaxed locally to avoid tight coupling during integration
import { tauriApi } from "../../services/tauri-api";
// Dev-only panels removed during cleanup
import { usePulse } from "../../hooks/usePulse";
import ValidationPanel from "./parts/ValidationPanel";
import DbSnapshotPanel from "./parts/DbSnapshotPanel";
import PersistPanel from "./parts/PersistPanel";
import { DetailTracker, ProductDetailEvent, ProductDetailPhase } from '../../services/detail-tracker';
import { getCrawlEventsStore } from '../../events/crawlEventsStore';
import { crawlerStore } from '../../stores/crawlerStore';
import type { DiagnosticsResult } from '../../types/diagnostics';

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [currentSessionId, setCurrentSessionId] = createSignal<string | null>(null);
  // Basic/Advanced toggle: default to basic view (advanced off)
  const [showAdvanced, setShowAdvanced] = createSignal(false);
  const [diagnosticsExpanded, setDiagnosticsExpanded] = createSignal(false); // Stage X 진단 패널 확장/축소
  const [helpPanelOpen, setHelpPanelOpen] = createSignal(false); // 도움말 패널
  const [planExpanded, setPlanExpanded] = createSignal(true); // 크롤링 플랜 확장/축소 (기본: 펼침)
  const [crawlingRange, setCrawlingRange] = createSignal<any | null>(null);
  const [statusMessage, setStatusMessage] =
    createSignal<string>("크롤링 준비 완료");
  const [logs, setLogs] = createSignal<string[]>([]);
  const [showConsole] = createSignal<boolean>(true);
  const [consoleExpanded, setConsoleExpanded] = createSignal<boolean>(false); // Actor 이벤트 콘솔 확장/축소 상태 (기본: 축소)
  // isValidating: 제거됨 (미사용)
  // Syncing flag (여러 단계 작업 중)
  const [isSyncing, setIsSyncing] = createSignal(false);
  // Shallow mode flag (좌표 갱신만 수행 중 - Stage 2 통계 업데이트 방지)
  const [isShallowMode, setIsShallowMode] = createSignal(false);
  const [syncRanges, setSyncRanges] = createSignal<string>("");
  // Stage 1: Page stats (runtime incremental)
  const [pageStats, setPageStats] = createSignal<{
    started: number;
    completed: number;
    failed: number;
    retried: number;
    totalEstimated: number; // estimated pages (from batches or preflight)
    inflight: number;
  }>({ started: 0, completed: 0, failed: 0, retried: 0, totalEstimated: 0, inflight: 0 });
  // Stage 2: Detail stats (runtime incremental; derived from lifecycle group/product events)
  const [detailStats, setDetailStats] = createSignal<{
    started: number;
    completed: number;
    failed: number;
    retried: number;
    inflight: number;
  }>({ started: 0, completed: 0, failed: 0, retried: 0, inflight: 0 });
  // Stage 2: 확정(또는 최신 누적) 상세 아이템 총계 (Progress 분모로 사용)
  const [detailPlannedTotal, _setDetailPlannedTotal] = createSignal<number | null>(null);
  
  // 🔍 DEBUG: detailPlannedTotal 설정 추적
  const setDetailPlannedTotal = (value: number | null) => {
    const prev = detailPlannedTotal();
    console.log('🔍 [TRACK] setDetailPlannedTotal called:', { prev, new: value });
    _setDetailPlannedTotal(value);
    
    // 글로벌 디버깅을 위한 창 객체 설정
    (window as any).__detailPlannedTotal = value;
  };
  // Batch info (Stage 1 batching)
  const [batchInfo, setBatchInfo] = createSignal<{ current: number; totalEstimated?: number; batchId?: any; startedAt?: number }>({ current: 0 });
  // Track pages already counted toward detail scheduling to prevent double counting
  const detailScheduledPages = new Set<number>();
  // Stage2 mapping cumulative tracker (batch-level) to compute delta of scheduled detail count
  const detailMappingBatchCounts = new Map<string, number>();
  // Stage2 legacy baseline tracking (to offset early legacy increments before first mapping delta per batch)
  const stage2LegacyBaseline = new Map<string, number>(); // batchId -> legacy started prior to first cumulative mapping event
  // Track cumulative authoritative group_size (or size) per batch for discrepancy checks
  const stage2GroupSizeSnapshot = new Map<string, number>();
  // Stage2 legacy started (no-batch path) total accumulator
  let stage2LegacyStartedTotal = 0;
  // Stage2 per-batch succ/fail cumulative snapshot to derive deltas
  const stage2BatchCum = new Map<string, { succ: number; fail: number }>();
  // Helper: recompute Stage 2 started from per-batch snapshots + legacy total
  const recomputeStage2Started = () => {
    const batchIds = new Set<string>([
      ...Array.from(detailMappingBatchCounts.keys()),
      ...Array.from(stage2GroupSizeSnapshot.keys()),
    ]);
    let startedFromBatches = 0;
    for (const id of batchIds) {
      const mapCount = detailMappingBatchCounts.get(id) || 0;
      const grpSize = stage2GroupSizeSnapshot.get(id) || 0;
      startedFromBatches += Math.max(mapCount, grpSize);
    }
    const totalStarted = startedFromBatches + (stage2LegacyStartedTotal || 0);
    setDetailStats((prev) => {
      const completed = prev.completed || 0;
      const failed = prev.failed || 0;
      const inflight = Math.max(0, totalStarted - (completed + failed));
      return { ...prev, started: totalStarted, inflight };
    });
  };
  const [lastActorEvent, setLastActorEvent] = createSignal<string>("");
  // Removed range FX related signals (legacy range panel removed)
  const [actorEventCount, setActorEventCount] = createSignal(0);
  // Legacy range animation helpers removed
  
  // 🚨 사이트 상태 경고 (페이지 수 감소 감지)
  const [siteHealthWarning, setSiteHealthWarning] = createSignal<{
    isWarning: boolean;
    currentPages: number;
    previousMaxPages: number;
    decreaseRatio: number;
  } | null>(null);
  
  // Lightweight Sync runtime view
  const [_syncLive, setSyncLive] = createSignal<{
    active: boolean;
    planned?: number | null;
    pagesProcessed: number;
    inserted: number;
    updated: number;
    skipped: number;
    failed: number;
    lastPage?: number | null;
    lastWarn?: string | null;
    durationMs?: number;
  }>({
    active: false,
    planned: null,
    pagesProcessed: 0,
    inserted: 0,
    updated: 0,
    skipped: 0,
    failed: 0,
  });
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
  const [downshiftInfo, setDownshiftInfo] = createSignal<null | {
    newLimit?: number;
    reason?: string;
  }>(null);
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
  // Animation toggles (validation pulse 제거됨)
  const [persistFlash, setPersistFlash] = createSignal(false);
  // Stage X: DB mismatch diagnostics
  const [diagLoading, setDiagLoading] = createSignal(false);
  const [diagResult, setDiagResult] = createSignal<DiagnosticsResult | null>(null);
  const [cleanupLoading, setCleanupLoading] = createSignal(false);
  const runDiagnostics = async () => {
    try {
      setDiagLoading(true);
      addLog("🧪 DB 진단 호출 시작");
      const res = await tauriApi.scanDbPaginationMismatches();
      addLog("✅ DB 진단 응답 수신");
      setDiagResult(res);
    } catch (e) {
      console.error("[Diagnostics] invoke failed", e);
      addLog(`❌ Diagnostics 실패: ${(e as any)?.message || e}`);
    } finally {
      setDiagLoading(false);
    }
  };
  // Build ranges from current diagnostics using physical pages and expand ±1 neighbors
  const deriveRangesFromDiagnostics = (): string | null => {
    const diag = diagResult();
    if (!diag) return null;
    const totalPages: number | undefined = Number.isFinite(
      diag.total_pages_site
    )
      ? Number(diag.total_pages_site)
      : undefined;
    // Select problematic groups more broadly: status!=ok OR any dup/miss/out-of-range hints
    const pages: number[] = (diag.group_summaries || [])
      .filter((g: any) => {
        const notOk = !!g.status && g.status !== "ok";
        const hasDup = (g.duplicate_indices?.length || 0) > 0;
        const hasMiss = (g.missing_indices?.length || 0) > 0;
        const oob = (g.out_of_range_count || 0) > 0;
        return notOk || hasDup || hasMiss || oob;
      })
      .map((g: any) => g.current_page_number)
      .filter((p: any) => typeof p === "number" && p > 0);
    
    // Add missing pages from page sequence gaps
    const missingPages: number[] = [];
    if (diag.missing_pages && Array.isArray(diag.missing_pages)) {
      for (const gap of diag.missing_pages) {
        if (gap.start_physical_page != null && gap.end_physical_page != null) {
          // Add all pages in the gap range
          for (let p = gap.end_physical_page; p <= gap.start_physical_page; p++) {
            if (p > 0) missingPages.push(p);
          }
        }
      }
    }
    
    // Combine existing problematic pages with missing pages
    const allPages = [...pages, ...missingPages];
    if (allPages.length === 0) return null;
    // Unique and neighbor expansion (±1) within site bounds
    const set = new Set<number>();
    for (const p of allPages) set.add(p);
    if (totalPages && totalPages > 1) {
      for (const p of Array.from(set)) {
        if (p - 1 >= 1) set.add(p - 1);
        if (p + 1 <= totalPages) set.add(p + 1);
      }
    }
    const uniq = Array.from(set).sort((a, b) => b - a);
    // Compress contiguous desc pages to ranges expr
    const parts: string[] = [];
    let start = uniq[0];
    let prev = uniq[0];
    for (const p of uniq.slice(1)) {
      if (p + 1 === prev) {
        prev = p;
        continue;
      }
      parts.push(start === prev ? `${start}` : `${start}-${prev}`);
      start = p;
      prev = p;
    }
    parts.push(start === prev ? `${start}` : `${start}-${prev}`);
    return parts.join(",");
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
      addLog("❌ URL 중복 제거 실패: " + (e as any)?.message);
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
    mode?: string;
    attempted: number;
    inserted: number;
    updated: number;
    succeeded: number;
    failed: number;
    duplicates: number;
    unchanged: number;
    failedTrue: number;
    durationMs: number;
    successRate: number;
    statusCounts: {
      insertedOnly: number;
      updatedOnly: number;
      mixed: number;
      allDuplicate: number;
      noop: number;
      failed: number;
      empty: number;
    };
    noEvents?: boolean;
    failureInferred?: boolean;
  }>({ mode: undefined, attempted: 0, inserted: 0, updated: 0, succeeded: 0, failed: 0, duplicates: 0, unchanged: 0, failedTrue: 0, durationMs: 0, successRate: 0, statusCounts: { insertedOnly: 0, updatedOnly: 0, mixed: 0, allDuplicate: 0, noop: 0, failed: 0, empty: 0 }, noEvents: undefined, failureInferred: undefined });
  // Persist normalization accumulator (Phase 1) - see docs/persist-metrics-normalization.md
  const persistAccumulator = (() => {
    let seenResult = false;
    let group = { attempted: 0, duplicates: 0, unchanged: 0, durationMs: 0 };
    let result = { attempted: 0, inserted: 0, updated: 0, duplicates: 0, unchanged: 0, durationMs: 0 };
    let statusCounts = { insertedOnly: 0, updatedOnly: 0, mixed: 0, allDuplicate: 0, noop: 0, failed: 0, empty: 0 };
    function applyGroup(e: { attempted: number; duplicates: number; unchanged: number; durationMs: number }) {
      if (seenResult) { group.durationMs += e.durationMs; return; }
      group.attempted += e.attempted;
      group.duplicates += e.duplicates;
      group.unchanged += e.unchanged;
      group.durationMs += e.durationMs;
    }
    function applyResult(e: { attempted: number; inserted: number; updated: number; duplicates: number; unchanged: number; durationMs: number; status?: string }) {
      seenResult = true;
      result.attempted += e.attempted;
      result.inserted += e.inserted;
      result.updated += e.updated;
      result.duplicates += e.duplicates;
      result.unchanged += e.unchanged;
      result.durationMs += e.durationMs;
      const s = e.status || "";
      if (s === 'persist_inserted') statusCounts.insertedOnly += 1; else if (s === 'persist_updated') statusCounts.updatedOnly += 1; else if (s === 'persist_mixed') statusCounts.mixed += 1; else if (s === 'persist_noop_all_duplicate') statusCounts.allDuplicate += 1; else if (s === 'persist_noop') statusCounts.noop += 1; else if (s === 'persist_failed') statusCounts.failed += 1; else if (s === 'persist_empty') statusCounts.empty += 1;
    }
    function applyEmpty() {
      seenResult = true;
      group = { attempted: 0, duplicates: 0, unchanged: 0, durationMs: 0 };
      result = { attempted: 0, inserted: 0, updated: 0, duplicates: 0, unchanged: 0, durationMs: 0 };
      statusCounts = { insertedOnly: 0, updatedOnly: 0, mixed: 0, allDuplicate: 0, noop: 0, failed: 0, empty: 1 };
    }
    function applyBatchFallback(e: { inserted: number; updated: number }) { if (seenResult) return; result.inserted += e.inserted; result.updated += e.updated; result.attempted += e.inserted + e.updated; }
    function snapshot() {
      const attempted = (seenResult ? result.attempted : group.attempted + result.attempted);
      const inserted = result.inserted;
      const updated = result.updated;
      const duplicates = seenResult ? result.duplicates : group.duplicates + result.duplicates;
      const unchanged = seenResult ? result.unchanged : group.unchanged + result.unchanged;
      const succeeded = inserted + updated;
      const failed = attempted - succeeded;
      const failedTrue = Math.max(0, attempted - succeeded - duplicates - unchanged);
      const durationMs = group.durationMs + result.durationMs;
      return { mode: seenResult ? 'mixed' : 'group-only', attempted, inserted, updated, duplicates, unchanged, succeeded, failed, failedTrue, durationMs, statusCounts: { ...statusCounts } };
    }
    return { applyGroup, applyResult, applyEmpty, applyBatchFallback, snapshot };
  })();
  // Stage 4: DB snapshot animation toggle
  const [dbFlash, setDbFlash] = createSignal(false);
  // Preflight diagnostics (site totals) to improve expected counts
  const [preflight, setPreflight] = createSignal<{ site_total_pages?: number; products_on_last_page?: number } | null>(null);
  
  // Global effects toggle
  const [effectsOn, setEffectsOn] = createSignal(true);
  // Stage2 discrepancy handling flags
  const autoBackfillStage2 = false; // toggle if we want to forcibly reconcile started upward
  // Per-page tracking for diagnostics: planned (scheduled) vs fetched (actual succeeded)
  const pagePlannedMap: Map<number, number> = new Map();
  const pageFetchedMap: Map<number, number> = new Map();
  // Persist diagnostics flags
  let persistEventsSeen = false;
  let persistFailureSynthesized = false;
  // Stage 2 diagnostics: detect if grouped fetch events are present to avoid double counting with stage-item detail events
  let detailGroupFetchEventsSeen = false;
  // Sync input pulse highlight
  const [syncPulse, setSyncPulse] = createSignal(false);
  // Track sync-start events to detect backend start and enable fallbacks
  let syncStartSeq = 0;
  onMount(async () => {
    // Initialize crawlerStore (이벤트 구독 포함)
    await crawlerStore.initialize();
    
    // Subscribe to structured crawl events store (dual emission path)
    try {
      const store = getCrawlEventsStore();
      const unsub = store.subscribe(snap => {
        const session = snap.activeSession;
        if (!session) return;
        // Map stage stats if present
        const listStage = session.stageStats['ListPageCrawling'];
        if (listStage) {
          setPageStats(prev => {
            const started = listStage.started ? Math.max(prev.started, listStage.completedItems + listStage.failedItems) : prev.started;
            const completed = listStage.completedItems;
            const failed = listStage.failedItems;
            const inflight = Math.max(0, started - (completed + failed));
            return { ...prev, started, completed, failed, inflight };
          });
        }
        const detailStage = session.stageStats['ProductDetailCrawling'];
        if (detailStage) {
          setDetailStats(prev => {
            const started = detailStage.started ? Math.max(prev.started, detailStage.completedItems + detailStage.failedItems + detailStage.retries) : prev.started;
            const completed = detailStage.completedItems;
            const failed = detailStage.failedItems;
            const inflight = Math.max(0, started - (completed + failed));
            return { ...prev, started, completed, failed, inflight };
          });
        }
      });
      onCleanup(() => { try { unsub(); } catch {} });
    } catch (e) {
      console.warn('[StructuredEvents] subscription failed', e);
    }
    try {
      const un1 = await tauriApi.subscribeToUnifiedActorEvents({
        variants: ['SyncStarted'],
        onEvent: () => {
          syncStartSeq++;
        },
      });
      onCleanup(() => {
        try { un1(); } catch {}
      });
    } catch {}
  });

  // Start button circular wave FX 제거됨 (미사용)

  // 크롤링 범위 계산
  const calculateCrawlingRange = async () => {
    addLog("📊 크롤링 범위 계산 중...");

    try {
      // 먼저 사이트 상태를 확인해서 실제 total_pages를 얻습니다
      addLog("🌐 사이트 상태 확인 중...");
      const siteStatusResponse = await invoke<any>(
        "check_advanced_site_status"
      );

      if (!siteStatusResponse?.data) {
        throw new Error("사이트 상태 확인 실패");
      }

      const siteStatus = siteStatusResponse.data;
      
      // 🐛 DEBUG: 사이트 상태 데이터 확인
      console.log('🔍 [DEBUG] siteStatus:', siteStatus);
      console.log('🔍 [DEBUG] is_page_count_decreased:', siteStatus.is_page_count_decreased);
      console.log('🔍 [DEBUG] previous_max_pages:', siteStatus.previous_max_pages);
      console.log('🔍 [DEBUG] total_pages:', siteStatus.total_pages);
      console.log('🔍 [DEBUG] page_decrease_ratio:', siteStatus.page_decrease_ratio);
      
      addLog(
        `✅ 사이트 상태 확인 완료: ${siteStatus.total_pages}페이지, 마지막 페이지 ${siteStatus.products_on_last_page}개 제품`
      );
      
      // 🚨 사이트 건강 상태 체크: 페이지 수 감소 감지
      if (siteStatus.is_page_count_decreased && siteStatus.previous_max_pages) {
        const decreaseRatio = siteStatus.page_decrease_ratio || 0;
        setSiteHealthWarning({
          isWarning: true,
          currentPages: siteStatus.total_pages,
          previousMaxPages: siteStatus.previous_max_pages,
          decreaseRatio: decreaseRatio,
        });
        addLog(
          `⚠️ 경고: 사이트 페이지 수 감소 감지 (${siteStatus.previous_max_pages} → ${siteStatus.total_pages}, -${(decreaseRatio * 100).toFixed(1)}%)`
        );
      } else {
        setSiteHealthWarning(null);
      }
      
      // Update preflight snapshot so readiness banner can show details immediately
      try {
        setPreflight({
          site_total_pages: Number(siteStatus.total_pages ?? 0) || undefined,
          products_on_last_page: Number(siteStatus.products_on_last_page ?? 0) || undefined,
        });
      } catch {}

      const request: any = {
        total_pages_on_site: siteStatus.total_pages,
        products_on_last_page: siteStatus.products_on_last_page,
      };

      addLog(
        `📋 크롤링 범위 계산 요청: ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품`
      );

      const response = await invoke<any>("calculate_crawling_range", {
        request,
      });
      setCrawlingRange(response);

      const startPage = response.range?.[0] || 0;
      const endPage = response.range?.[1] || 0;
      addLog(`📊 크롤링 범위 계산 완료: ${startPage} → ${endPage}`);
    } catch (error) {
      console.error("크롤링 범위 계산 실패:", error);
      addLog(`❌ 크롤링 범위 계산 실패: ${error}`);
    }
  };

  // 경량 설정 스타터 제거됨 (미사용)

  // 통합 Actor 기반 크롤링 (하이 설정)
  const startUnifiedAdvanced = async () => {
    if (isRunning()) return;

    setIsRunning(true);
    setStatusMessage("🎭 통합 파이프라인(하이) 시작 중...");
    addLog("🎭 통합 파이프라인 시작 (하이 설정)");

    try {
      const res = await tauriApi.startUnifiedCrawling({
        mode: "advanced",
        overrideConcurrency: 64,
        overrideBatchSize: 3,
        delayMs: 100,
      });
      
      console.log("🔍 Crawling started, response:", res);
      // 🔥 NEW: Save session ID for stop functionality
      if (res?.session_id) {
        console.log("✅ Setting session ID:", res.session_id);
        setCurrentSessionId(res.session_id);
        addLog(`✅ 통합 파이프라인(하이) 세션 시작: ${res.session_id}`);
      } else {
        console.warn("⚠️ No session_id in response:", res);
        addLog(`✅ 통합 파이프라인(하이) 세션 시작: ${JSON.stringify(res)}`);
      }
      
      setStatusMessage("🎭 통합 파이프라인 실행 중 (하이)");
    } catch (error) {
      console.error("통합 파이프라인(하이) 시작 실패:", error);
      addLog(`❌ 통합 파이프라인(하이) 시작 실패: ${error}`);
      setStatusMessage("크롤링 실패");
      setIsRunning(false);
      setCurrentSessionId(null);
    }
  };

  // 📍 좌표 갱신 (Shallow Sync): 리스트만 크롤링하여 좌표 업데이트
  const handleShallowSync = async () => {
    if (isRunning() || isSyncing()) {
      addLog("⚠️ 이미 크롤링이 진행 중입니다.");
      return;
    }

    setIsRunning(true);
    setIsSyncing(true);
    setIsShallowMode(true);
    setStatusMessage("📍 좌표 갱신 시작 중...");
    addLog("📍 좌표 갱신 시작 (리스트 크롤링, 상세 정보 제외)");

    try {
      const result = await invoke<any>("start_shallow_sync");
      
      // 세션 ID 저장 (중지 기능을 위해 필수)
      if (result.sessionId) {
        setCurrentSessionId(result.sessionId);
        console.log("📝 Shallow sync session ID saved:", result.sessionId);
      }
      
      // syncLive 초기화 (planned 값 설정)
      const pagesScanned = Number(result.pages_scanned || result.pagesScanned || 0);
      if (pagesScanned > 0) {
        setSyncLive({
          active: true,
          planned: pagesScanned,
          pagesProcessed: 0,
          inserted: 0,
          updated: 0,
          skipped: 0,
          failed: 0,
          lastPage: null,
          lastWarn: null,
          durationMs: undefined,
        });
        console.log(`🔍 [Shallow Sync] Initialized with planned=${pagesScanned} pages`);
      }
      
      addLog(`✅ 좌표 갱신 시작: ${pagesScanned}페이지, 세션 ID: ${result.sessionId}`);
      addLog(`💡 백그라운드에서 크롤링 진행 중... (중지 버튼으로 중단 가능)`);
      setStatusMessage(`📍 좌표 갱신 진행 중... (${pagesScanned}p)`);
      
      // Note: 크롤링은 백그라운드에서 계속 진행되며 이벤트로 완료를 알림
    } catch (error) {
      console.error("좌표 갱신 실패:", error);
      addLog(`❌ 좌표 갱신 실패: ${error}`);
      setStatusMessage("좌표 갱신 실패");
      setIsRunning(false);
      setIsSyncing(false);
      setIsShallowMode(false);
    }
  };

  // 🔧 제품 보완: 핵심 필드 누락 제품만 재크롤링
  const handleComplementCrawl = async () => {
    if (isRunning() || isSyncing()) {
      addLog("⚠️ 이미 크롤링이 진행 중입니다.");
      return;
    }

    setIsRunning(true);
    setIsSyncing(true);
    setStatusMessage("🔧 제품 보완 동기화 진행 중...");
    addLog("🔧 제품 보완 동기화 시작 (핵심 필드 누락 제품 재크롤링: cert_date, transport, device_type_ids)");
    addLog("💡 동시성: 12개 제품 병렬 처리");

    try {
      const result = await tauriApi.startComplementCrawl();
      
      // 🐛 디버깅: 결과 전체 출력
      console.log("🔍 startComplementCrawl result:", result);
      addLog(`🔍 DEBUG: result = ${JSON.stringify(result)}`);
      
      // 세션 ID 저장 (중지 버튼 지원)
      const sessionId = result.sessionId;
      console.log("🔍 Extracted sessionId:", sessionId, "| Type:", typeof sessionId);
      addLog(`🔍 DEBUG: sessionId = "${sessionId}" (${typeof sessionId})`);
      
      if (sessionId && sessionId.trim() !== "") { // 빈 문자열 체크 추가
        setCurrentSessionId(sessionId);
        addLog(`🆔 세션 ID 저장 성공: ${sessionId}`);
        addLog("💡 백그라운드에서 작업이 진행됩니다. 중지 버튼으로 중단 가능합니다.");
        console.log("✅ Session ID saved to state:", sessionId);
      } else {
        console.warn("⚠️ Session ID is empty or invalid:", sessionId);
        addLog(`⚠️ 세션 ID가 비어있거나 유효하지 않습니다: "${sessionId}"`);
      }
      
      if (result.urlsTargeted === 0) {
        addLog(`✨ 보완이 필요한 제품이 없습니다. (모든 제품에 핵심 필드 존재)`);
        setStatusMessage("✅ 제품 보완 동기화 완료 (보완 불필요)");
        setIsRunning(false);
        setIsSyncing(false);
      } else {
        addLog(`🔄 ${result.urlsTargeted}개 제품 병렬 재크롤링 시작`);
        // 성공 시: 상태는 "진행 중" 유지, actor-session-completed에서 종료
        setStatusMessage("🔧 제품 보완 동기화 완료 (백그라운드 작업 진행 중...)");
        addLog("💡 세션 완료 대기 중...");
      }
    } catch (error) {
      console.error("제품 보완 동기화 실패:", error);
      addLog(`❌ 제품 보완 동기화 실패: ${error}`);
      setStatusMessage("❌ 제품 보완 동기화 실패");
      setIsRunning(false);
      setIsSyncing(false);
    }
  };

  // 🛑 정지 핸들러
  const handleStop = async () => {
    console.log("🔍 handleStop CALLED! currentSessionId:", currentSessionId());
    const sessionId = currentSessionId();
    if (!sessionId) {
      console.warn("⚠️ No active session ID");
      addLog("⚠️ 활성 세션이 없습니다.");
      return;
    }
    
    console.log("🛑 Sending cancel request for session:", sessionId);
    addLog("🛑 작업 중지 요청...");
    setStatusMessage("작업 중지 중...");
    
    try {
      // Tauri에 정지 명령 전송 (camelCase 매개변수 사용)
      await invoke("cancel_real_crawling", { 
        sessionId: sessionId 
      });
      console.log("✅ Cancel command sent successfully");
      addLog("✅ 크롤링이 중지되었습니다.");
      setIsRunning(false);
      setCurrentSessionId(null);
      setStatusMessage("작업 중지됨");
    } catch (error) {
      console.error("❌ 정지 실패:", error);
      addLog(`❌ 정지 실패: ${error}`);
    }
  };

  // ... (legacy simple crawling entry removed)

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [`[${timestamp}] ${message}`, ...prev.slice(0, 19)]);
  };

  // Validation helper 제거됨 (미사용)

  // Sync starter 제거됨 (미사용)

  // 진단 기반 부분 Sync: 제거됨 (미사용)

  // 정밀 복구 실행 헬퍼: 제거됨 (미사용)

  const detailTracker = new DetailTracker();
  let detailTrackerActive = false; // becomes true once at least one keyed event ingested
  // Helper: recompute Stage1 retried count
  const recomputeStage1Retries = () => {
    let retried = 0;
    for (const v of pageAttempts.values()) {
      if (v > 1) retried += (v - 1);
    }
    setPageStats((prev) => ({ ...prev, retried }));
  };
  // Helper: integrate Stage2 retries when tracker active
  const applyDetailTrackerSnapshot = () => {
    if (!detailTrackerActive) return;
    const snap = detailTracker.snapshot();
    setDetailStats((prev) => {
      const started = snap.fetch.started;
      const completed = snap.fetch.succeeded;
      const failed = snap.fetch.failed;
      const retried = snap.retries;
      const inflight = Math.max(0, started - (completed + failed));
      return { ...prev, started, completed, failed, retried, inflight };
    });
  };

  onMount(() => {
    calculateCrawlingRange();

    const unsubs: Array<() => void> = [];

    // Listen settings-updated to recalc planned range
    try {
      listen("settings-updated", () => {
        addLog("🛠️ 설정 변경 감지 → 범위 재계산");
        // Optional transition snapshot for nicer UX
        try {
          const prev = crawlingRange();
          const _prevStart = (prev?.range?.[0] ?? 0) as number;
          void _prevStart; // kept for potential future use
        } catch {}
        calculateCrawlingRange();
      }).then((un) => unsubs.push(un));
    } catch (e) {
      console.warn(
        "[CrawlingEngineTabSimple] listen settings-updated failed",
        e
      );
    }

    // Listen to unified Actor session lifecycle to toggle buttons/status
    tauriApi
      .subscribeToUnifiedActorEvents({ onEvent: (payload) => {
  // Normalize event name: prefer event_name, fallback to variant -> kebab case
  let name = String(payload?.event_name || "");
  if (!name) {
    const variantStr = String(payload?.variant || "");
    if (variantStr) {
      const kebab = variantStr
        .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
        .replace(/([A-Z])([A-Z][a-z])/g, "$1-$2")
        .toLowerCase();
      name = `actor-${kebab}`;
    }
  }
  // Debug: track live event stream
  setActorEventCount((n) => n + 1);
  setLastActorEvent(name);
  
  // Debug: log all events to console
  if (name.includes("database-stats") || name.includes("product-lifecycle-group") || 
      name.includes("batch-completed") || name.includes("session-report") ||
      (name.includes("product-lifecycle") && payload?.status?.includes("persist"))) {
    console.log("[DEBUG] Event received:", name, payload);
  }
        // === Sync events → compact Sync panel ===
        if (name === "actor-sync-started") {
          try {
            const ranges: Array<[number, number]> = Array.isArray(
              payload?.ranges
            )
              ? payload.ranges
              : [];
            
            // Calculate planned pages from ranges
            let planned = ranges.reduce(
              (acc, [start, end]) => acc + Math.max(0, start - end + 1),
              0
            );
            
            // If no ranges or planned is 0, try to use total_pages or pages_scanned from payload
            if (!planned && payload) {
              planned = Number(payload.total_pages || payload.pages_scanned || 0);
            }
            
            console.log(`🔍 [Sync Started] ranges:`, ranges, `planned:`, planned, `payload:`, payload);
            
            setSyncLive({
              active: true,
              planned: planned || null,
              pagesProcessed: 0,
              inserted: 0,
              updated: 0,
              skipped: 0,
              failed: 0,
              lastPage: null,
              lastWarn: null,
              durationMs: undefined,
            });
            setStatusMessage("🔄 Sync 실행 중");
          } catch (e) {
            console.error('[Sync Started] Error processing payload:', e);
            setSyncLive({
              active: true,
              planned: null,
              pagesProcessed: 0,
              inserted: 0,
              updated: 0,
              skipped: 0,
              failed: 0,
              lastPage: null,
              lastWarn: null,
              durationMs: undefined,
            });
          }
        }
        if (name === "actor-sync-page-started") {
          const p = Number(payload?.physical_page ?? NaN);
          setSyncLive((prev) => ({
            ...prev,
            lastPage: Number.isFinite(p) ? p : prev.lastPage ?? null,
          }));
        }
        if (name === "actor-sync-page-completed") {
          const ins = Number(payload?.inserted ?? 0) || 0;
          const upd = Number(payload?.updated ?? 0) || 0;
          const skp = Number(payload?.skipped ?? 0) || 0;
          const fld = Number(payload?.failed ?? 0) || 0;
          setSyncLive((prev) => ({
            ...prev,
            pagesProcessed: (prev.pagesProcessed || 0) + 1,
            inserted: (prev.inserted || 0) + ins,
            updated: (prev.updated || 0) + upd,
            skipped: (prev.skipped || 0) + skp,
            failed: (prev.failed || 0) + fld,
          }));
        }
        if (name === "actor-sync-warning") {
          const code = String(payload?.code || "");
          const detail = String(payload?.detail || "");
          setSyncLive((prev) => ({
            ...prev,
            lastWarn: `${code}: ${detail}`.slice(0, 160),
          }));
        }
        if (name === "actor-sync-completed") {
          setSyncLive((prev) => ({
            ...prev,
            active: false,
            pagesProcessed:
              Number(payload?.pages_processed ?? prev.pagesProcessed) ||
              prev.pagesProcessed,
            inserted:
              Number(payload?.inserted ?? prev.inserted) || prev.inserted,
            updated: Number(payload?.updated ?? prev.updated) || prev.updated,
            skipped: Number(payload?.skipped ?? prev.skipped) || prev.skipped,
            failed: Number(payload?.failed ?? prev.failed) || prev.failed,
            durationMs:
              Number(payload?.duration_ms ?? prev.durationMs) ||
              prev.durationMs,
          }));
          setStatusMessage("Sync 완료");
        }
        if (name === "actor-session-started") {
          setIsRunning(true);
          // isSyncing 중이면 상태 메시지를 보호 (스마트 동기화 등 상위 작업 진행 중)
          if (!isSyncing()) {
            setStatusMessage("크롤링 실행 중 (세션 시작)");
          }
          addLog("🎬 세션 시작");
          // reset runtime stats
          setPageStats({
            started: 0,
            completed: 0,
            failed: 0,
            retried: 0,
            totalEstimated: 0,
            inflight: 0,
          });
          // Stage 2 상세 총계 초기화 (authoritative denominator)
          setDetailPlannedTotal(null);
          // Manual range override: if crawlingRange has explicit page list length, use it as totalEstimated baseline
          try {
            const info = crawlingRange()?.crawling_info;
            const manualPages = Array.isArray(info?.pages_explicit) ? info.pages_explicit.length : 0;
            if (manualPages > 0) {
              setPageStats((prev) => ({ ...prev, totalEstimated: manualPages }));
            }
          } catch {}
          setDetailStats({
            started: 0,
            completed: 0,
            failed: 0,
            retried: 0,
            inflight: 0,
          });
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
          // Clear Stage 2 per-batch state
          detailMappingBatchCounts.clear();
          stage2LegacyBaseline.clear();
          stage2GroupSizeSnapshot.clear();
          stage2BatchCum.clear();
          stage2LegacyStartedTotal = 0;
          setDownshiftInfo(null);
          setValidationStats({
            started: false,
            completed: false,
            targetPages: 0,
            pagesScanned: 0,
            divergences: 0,
            anomalies: 0,
            productsChecked: 0,
            lastPage: null,
            lastAssignedStart: null,
            lastAssignedEnd: null,
          });
          setDbSnapshot({});
          setPersistStats({
            mode: undefined,
            attempted: 0,
            inserted: 0,
            updated: 0,
            succeeded: 0,
            failed: 0,
            duplicates: 0,
            unchanged: 0,
            failedTrue: 0,
            durationMs: 0,
            successRate: 0,
            statusCounts: { insertedOnly: 0, updatedOnly: 0, mixed: 0, allDuplicate: 0, noop: 0, failed: 0, empty: 0 },
          });
        }
        if (name === "actor-session-completed") {
          // 🔥 Clear session ID when session completes
          setCurrentSessionId(null);
          
          // Shallow Mode는 첫 번째 세션 완료 시 해제 (이후 세션은 실제 Detail 크롤링 가능)
          if (isShallowMode()) {
            setIsShallowMode(false);
            console.log('[ShallowMode] Turned off after first session completion');
          }
          
          // 크롤링 완료 처리
          // isSyncing 모드(좌표 갱신, 제품 보완)는 세션 완료 시 종료
          if (isSyncing()) {
            setIsSyncing(false);
            setIsRunning(false);
            setStatusMessage("✅ 동기화 완료");
            addLog("✅ 동기화 작업 완료");
            
            // syncLive 완전 리셋 (SyncPanel 숨기기)
            setSyncLive({
              active: false,
              planned: null,
              pagesProcessed: 0,
              inserted: 0,
              updated: 0,
              skipped: 0,
              failed: 0,
              lastPage: null,
              lastWarn: null,
              durationMs: undefined,
            });
          } else if (!isSyncing()) {
            setIsRunning(false);
            setStatusMessage("크롤링 완료");
          }
          // isSyncing 중이어도 로그는 남김
          addLog("🏁 세션 완료");
          setBatchInfo((prev) => ({ ...prev }));
          // Play transition on session complete as well (helps visibility)
          try {
            // range snapshot removed
          } catch {}
          // Recompute crawling range so the UI reflects the newly planned range
          // Wait a bit to ensure DB writes are committed before recalculation
          setTimeout(() => {
            calculateCrawlingRange();
          }, 1000);
          // Mark persist no-events state if applicable
          if ((persistStats().attempted === 0 || persistStats().noEvents) ) {
            if (!(persistStats().failureInferred)) {
              setPersistStats((prev) => ({ ...prev, noEvents: true }));
              console.log('[DIAG][Persist][session-complete-no-events-marked]');
            }
          }
        }
        if (name === "actor-session-failed") {
          // 🔥 Clear session ID when session fails
          setCurrentSessionId(null);
          
          // Shallow Mode 리셋
          if (isShallowMode()) {
            setIsShallowMode(false);
            console.log('[ShallowMode] Turned off after session failure');
          }
          
          // 크롤링 실패 처리
          // isSyncing 모드(좌표 갱신, 제품 보완)도 세션 실패 시 종료
          if (isSyncing()) {
            setIsSyncing(false);
            setIsRunning(false);
            setStatusMessage("❌ 동기화 실패");
            addLog("❌ 동기화 작업 실패");
            
            // syncLive 완전 리셋
            setSyncLive({
              active: false,
              planned: null,
              pagesProcessed: 0,
              inserted: 0,
              updated: 0,
              skipped: 0,
              failed: 0,
              lastPage: null,
              lastWarn: null,
              durationMs: undefined,
            });
          } else {
            setIsRunning(false);
            setStatusMessage("크롤링 실패");
          }
          
          addLog(`❌ 세션 실패: ${JSON.stringify(payload)}`);
          setBatchInfo((prev) => ({ ...prev }));
        }
        if (
          name === "actor-session-timeout" ||
          name === "actor-shutdown-completed"
        ) {
          // 🔥 Clear session ID on timeout/shutdown
          setCurrentSessionId(null);
          
          // Shallow Mode 리셋
          if (isShallowMode()) {
            setIsShallowMode(false);
            console.log('[ShallowMode] Turned off after session timeout/shutdown');
          }
          
          // 크롤링 종료 처리
          // isSyncing 모드(좌표 갱신, 제품 보완)도 종료 시 리셋
          if (isSyncing()) {
            setIsSyncing(false);
            setIsRunning(false);
            setStatusMessage("🛑 동기화 중지됨");
            addLog("🛑 동기화 작업 중지됨");
            
            // syncLive 완전 리셋
            setSyncLive({
              active: false,
              planned: null,
              pagesProcessed: 0,
              inserted: 0,
              updated: 0,
              skipped: 0,
              failed: 0,
              lastPage: null,
              lastWarn: null,
              durationMs: undefined,
            });
          } else {
            setIsRunning(false);
            setStatusMessage("크롤링 종료");
          }
          
          addLog("🛑 세션 종료");
          setBatchInfo((prev) => ({ ...prev }));
          // Refresh planned range after abnormal end as well
          // Wait to ensure DB state is stable before recalculation
          setTimeout(() => {
            calculateCrawlingRange();
          }, 1000);
        }
        
        // 🔥 NEW: Handle cancellation event
        if (name === "crawling-cancelled") {
          setCurrentSessionId(null);
          setIsRunning(false);
          setIsSyncing(false);
          setIsShallowMode(false);
          setStatusMessage("사용자가 크롤링을 중단했습니다");
          addLog(`🛑 크롤링 취소됨: ${payload?.message || ""}`);
        }


        // Estimate totals from batch starts (pages in batch)
        if (name === "actor-batch-started") {
          const t = (payload?.pages_in_batch ??
            payload?.pages ??
            payload?.items_total ??
            payload?.pages_count ??
            0) as number;
          if (typeof t === "number" && t > 0) {
            setPageStats((prev) => ({
              ...prev,
              totalEstimated: prev.totalEstimated + t,
            }));
          }
          // Update batch info
          setBatchInfo((prev) => {
            const current = (prev.current || 0) + 1;
            let totalEstimated = prev.totalEstimated;
            const pagesTotal =
              (crawlingRange()?.crawling_info?.pages_to_crawl as number) || 0;
            const batchSizeGuess =
              Number(t) || Number(payload?.batch_size ?? 0) || 0;
            if (!totalEstimated && pagesTotal > 0 && batchSizeGuess > 0) {
              totalEstimated = Math.max(
                1,
                Math.ceil(pagesTotal / batchSizeGuess)
              );
            }
            return {
              current,
              totalEstimated,
              batchId: payload?.batch_id ?? prev.batchId,
            };
          });
        }
  if (name === "actor-batch-completed") {
          // Extract coordinate update stats for shallow mode
          const summary = (payload as any)?.summary;
          if (summary) {
            const productsUpdated = summary.products_updated ?? 0;
            const detailsUpdated = summary.details_updated ?? 0;
            const failed = summary.failed ?? 0;
            const totalUrls = summary.total_urls ?? 0;
            
            if (productsUpdated > 0 || detailsUpdated > 0) {
              addLog(`📊 좌표 갱신: ${productsUpdated}개 제품, ${detailsUpdated}개 상세 (실패: ${failed}, 총: ${totalUrls})`);
            }
          }
        }
        // === Fine-grained StageItem events (new) for real-time Stage 1/2 responsiveness ===
        if (name === "actor-stage-item-started" || name === "actor-stage-item-completed") {
          try {
            const stageTypeRaw = (payload as any)?.stage_type;
            const stageStr = typeof stageTypeRaw === 'string'
              ? stageTypeRaw.toLowerCase()
              : stageTypeRaw && typeof stageTypeRaw === 'object'
              ? (Object.keys(stageTypeRaw)[0] || '').toLowerCase()
              : '';
            const isList = stageStr.includes('list_page');
            const isDetail = stageStr.includes('product_detail');
            if (!isList && !isDetail) {
              // Only track Stage 1 & 2
              // (Validation & Persistence already have separate handlers)
              // Skip to avoid noise.
              // console.debug('[StageItem] Ignoring non list/detail item', stageStr);
            } else {
              // Extract item type structure - handle multiple possible serde shapes.
              const it = (payload as any)?.item_type || (payload as any)?.itemType;
              let pageNumber: number | undefined;
              if (it) {
                try {
                  // Shape A: { Page: { page_number: 123 } }
                  if (typeof it === 'object' && !Array.isArray(it) && it.Page) {
                    pageNumber = Number(it.Page.page_number ?? it.Page.pageNumber);
                  }
                  // Shape B: direct camelCase: { type: 'Page', page_number: 123 }
                  const typeStr = String(it.type || it.item_type || '').toLowerCase();
                  if (typeStr === 'page') {
                    pageNumber = Number(it.page_number ?? it.pageNumber ?? pageNumber);
                  }
                  // Shape C: flatten: { page_number: 123 }
                  if (pageNumber == null && (it.page_number != null || it.pageNumber != null)) {
                    pageNumber = Number(it.page_number ?? it.pageNumber);
                  }
                } catch {}
              }
              // Started event
              if (name === 'actor-stage-item-started') {
                if (isList && pageNumber != null && Number.isFinite(pageNumber)) {
                  if (!pageSeen.has(pageNumber)) {
                    pageSeen.add(pageNumber);
                    setPageStats((prev) => {
                      const started = pageSeen.size;
                      const inflight = Math.max(0, started - (prev.completed + prev.failed));
                      return { ...prev, started, inflight };
                    });
                    if (effectsOn()) triggerStage1Pulse();
                  }
                } else if (isDetail) {
                  if (detailTrackerActive) {
                    // Using new keyed tracker; ignore legacy per-item detail counting
                    return;
                  }
                  // Detail started events no longer increment 'started' to avoid double counting;
                  // rely on scheduling/mapping events for attempt counting.
                  // Optionally we could track inflight hints later.
                }
              } else if (name === 'actor-stage-item-completed') {
                const success = !!(payload as any)?.success;
                if (isList && pageNumber != null && Number.isFinite(pageNumber)) {
                  if (!pageSeen.has(pageNumber)) pageSeen.add(pageNumber);
                  if (success) {
                    if (!pageCompleted.has(pageNumber)) pageCompleted.add(pageNumber);
                  } else {
                    pageFailedFinal.add(pageNumber);
                  }
                  setPageStats((prev) => {
                    const started = pageSeen.size;
                    const completed = pageCompleted.size;
                    const failed = pageFailedFinal.size;
                    const inflight = Math.max(0, started - (completed + failed));
                    return { ...prev, started, completed, failed, inflight };
                  });
                  if (effectsOn()) triggerStage1Pulse();
                } else if (isDetail) {
                  // If grouped product-lifecycle-group fetch events are present we ignore per-detail stage-item
                  // events to avoid double counting (these appear to fire once per page as a summary).
                  if (detailGroupFetchEventsSeen) {
                    if ((window as any).__stage2DetailSkipped == null) (window as any).__stage2DetailSkipped = 0;
                    (window as any).__stage2DetailSkipped += 1;
                    // Optional verbose diagnostic (comment out if noisy)
                    // console.log('[DIAG][Stage2][detail-stage-item-skipped]');
                  } else {
                    setDetailStats((prev) => {
                      const completed = (prev.completed || 0) + (success ? 1 : 0);
                      const failed = (prev.failed || 0) + (success ? 0 : 1);
                      const inflight = Math.max(0, (prev.started) - (completed + failed));
                      return { ...prev, completed, failed, inflight };
                    });
                    if (effectsOn()) triggerStage2Pulse();
                  }
                }
              }
            }
          } catch (e) {
            console.warn('[CrawlingEngineTabSimple] stage-item event handling failed', e);
          }
        }
  // Stage 1 (list page) via consolidated 'actor-page-lifecycle' events
  // Map lifecycle to Stage 1 counters so the UI remains responsive.
        if (name === "actor-page-lifecycle") {
          const status = String(payload?.status || "").toLowerCase();
          const pageNum = Number(payload?.page_number ?? NaN);
          if (!Number.isFinite(pageNum)) return;
          // Stage 2 start accounting from mapping/schedule signals
          // CRITICAL: Skip during shallow mode (coordinate-only updates, no actual detail fetching)
          if (status === "detail_scheduled" || status === "detail_mapping_emitted") {
            console.log('[DIAG][ShallowMode] detail event:', { status, isShallowMode: isShallowMode(), pageNum });
            if (isShallowMode()) {
              console.log('[DIAG][ShallowMode] SKIPPING Stage 2 update (shallow mode active)');
              return; // Skip Stage 2 updates during shallow mode
            }
          }
          if ((status === "detail_scheduled" || status === "detail_mapping_emitted") && !isShallowMode()) {
            // NEW LOGIC: mapping events may arrive multiple times with cumulative url counts for the whole batch (not per page)
            // We switch to per-batch delta tracking instead of per-page single-shot to avoid undercount when subsequent
            // mapping updates add more URLs (previous logic ignored duplicates via Set).
            // Diagnostic: log raw metrics once per batch for verification
            const batchId = String(payload?.batch_id || payload?.batchId || "");
            if (batchId && !(window as any).__stage2MappingDiagLogged) {
              (window as any).__stage2MappingDiagLogged = new Set();
            }
            try {
              if (batchId && !(window as any).__stage2MappingDiagLogged.has(batchId)) {
                (window as any).__stage2MappingDiagLogged.add(batchId);
                console.log('[DIAG][Stage2][mapping-raw]', { batch: batchId, metrics: payload?.metrics });
              }
            } catch {}
            if (!batchId) {
              // Fallback to page-number legacy path if no batch id (rare)
              const legacyPageKey = pageNum;
              if (Number.isFinite(legacyPageKey) && !detailScheduledPages.has(legacyPageKey!)) {
                const topUrls = Number((payload as any)?.urls ?? 0) || 0;
                if (topUrls > 0) {
                  detailScheduledPages.add(legacyPageKey!);
                  pagePlannedMap.set(legacyPageKey!, topUrls);
                  stage2LegacyStartedTotal += topUrls;
                  recomputeStage2Started();
                  if (effectsOn()) triggerStage2Pulse();
                }
              }
            } else {
              // Extract cumulative url count (url_count preferred, then scheduled_details, then top-level urls)
              let urlCount = 0;
              let scheduled = 0;
              const m = payload?.metrics;
              const extract = (obj: any) => {
                if (!obj || typeof obj !== 'object') return;
                if (obj.url_count != null) urlCount = Number(obj.url_count) || urlCount;
                if (obj.scheduled_details != null) scheduled = Number(obj.scheduled_details) || scheduled;
              };
              if (m && typeof m === 'object' && !Array.isArray(m)) {
                const firstKey = Object.keys(m)[0];
                if (firstKey && typeof (m as any)[firstKey] === 'object') extract((m as any)[firstKey]);
                if (typeof (m as any).data === 'object') extract((m as any).data);
                extract(m);
              }
              let cumulative = urlCount > 0 ? urlCount : (scheduled > 0 ? scheduled : 0);
              if (cumulative === 0) {
                const topUrls = Number((payload as any)?.urls ?? 0) || 0;
                const topScheduled = Number((payload as any)?.scheduled ?? 0) || 0;
                cumulative = topUrls > 0 ? topUrls : (topScheduled > 0 ? topScheduled : 0);
              }
              if (cumulative > 0) {
                const prevCumulative = detailMappingBatchCounts.get(batchId) || 0;
                let delta = cumulative - prevCumulative;
                // 최초 또는 증가 시 상세 계획 총계 갱신 (분모로 활용)
                const currentPlanned = detailPlannedTotal();
                if (currentPlanned == null || cumulative > currentPlanned) {
                  setDetailPlannedTotal(cumulative);
                }
                // First snapshot baseline adjustment against legacy-only totals
                if (prevCumulative === 0) {
                  let legacyBase = stage2LegacyBaseline.get(batchId);
                  if (legacyBase == null) {
                    legacyBase = stage2LegacyStartedTotal || 0;
                    stage2LegacyBaseline.set(batchId, legacyBase);
                  }
                  if (legacyBase > 0 && delta > 0) {
                    const adjusted = Math.max(0, cumulative - legacyBase);
                    console.warn('[Stage2][adjust-initial-delta]', { batch: batchId, cumulative, rawDelta: delta, legacyBase, adjusted });
                    delta = adjusted;
                  }
                }
                if (cumulative >= prevCumulative) {
                  detailMappingBatchCounts.set(batchId, cumulative);
                  // Recompute authoritative started count from all batches
                  recomputeStage2Started();
                  if (delta > 0) {
                    if (effectsOn()) triggerStage2Pulse();
                    // Associate delta with a representative page for per-page diagnostics (optional)
                    if (Number.isFinite(pageNum)) {
                      const existing = pagePlannedMap.get(pageNum) || 0;
                      pagePlannedMap.set(pageNum, existing + delta);
                    }
                    console.log('[DIAG][Stage2][mapping-delta]', { batch: batchId, prev: prevCumulative, cumulative, delta });
                  }
                } else {
                  // Cumulative should never decrease; log anomaly
                  console.warn('[WARN][Stage2][mapping-cumulative-decrease]', { batch: batchId, prev: prevCumulative, cumulative });
                }
              } else {
                console.log('[DIAG][Stage2][mapping-no-count]', { batch: batchId });
              }
            }
          }
          if (status === "fetch_started") {
            const prevAttempts = pageAttempts.get(pageNum) ?? 0;
            pageAttempts.set(pageNum, prevAttempts + 1);
            if (prevAttempts >= 1) recomputeStage1Retries();
            if (!pageSeen.has(pageNum)) {
              pageSeen.add(pageNum);
              setPageStats((prev) => {
                const started = pageSeen.size;
                const inflight = Math.max(0, started - (prev.completed + prev.failed));
                return { ...prev, started, inflight };
              });
            }
            if (effectsOn()) triggerStage1Pulse();
          } else if (status === "fetch_completed" || status === "urls_extracted") {
            if (!pageCompleted.has(pageNum)) pageCompleted.add(pageNum);
            if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
            setPageStats((prev) => {
              const started = pageSeen.size;
              const completed = pageCompleted.size;
              const inflight = Math.max(0, started - (completed + prev.failed));
              return { ...prev, started, completed, inflight };
            });
            recomputeStage1Retries();
            if (effectsOn()) triggerStage1Pulse();
          } else if (status === "failed") {
            const prevAttempts = pageAttempts.get(pageNum) ?? 0;
            pageAttempts.set(pageNum, prevAttempts + 1);
            if (!pageSeen.has(pageNum)) pageSeen.add(pageNum);
            // lifecycle doesn't carry final_failure flag; treat as final for UI
            pageFailedFinal.add(pageNum);
            setPageStats((prev) => {
              const started = pageSeen.size;
              const failed = pageFailedFinal.size;
              const inflight = Math.max(0, started - (prev.completed + failed));
              return { ...prev, started, failed, inflight };
            });
            if (prevAttempts >= 1) recomputeStage1Retries();
            if (effectsOn()) triggerStage1Pulse();
          }
        }
        // Stage 2 (product detail) itemized - deduplicate by detail_id and track retries
        // Stage 2 via product lifecycle events
        if (
          name === "actor-product-lifecycle-group" &&
          payload?.phase === "fetch" && !detailTrackerActive
        ) {
          // Shallow Mode 체크: 좌표 전용 업데이트 시에는 Stage 2 스킵
          if (isShallowMode()) {
            console.log('[DIAG][ShallowMode] SKIPPING actor-product-lifecycle-group (shallow mode active)');
            // Early return - skip all Stage 2 processing
          } else {
            if (!detailGroupFetchEventsSeen) detailGroupFetchEventsSeen = true;
          const batchId = String(payload?.batch_id || "");
          const cumSucc = Number(payload?.succeeded ?? 0) || 0;
          const cumFail = Number(payload?.failed ?? 0) || 0;
          const groupSize = Number(payload?.group_size ?? payload?.size ?? payload?.started ?? 0) || 0;
          const partial = !!payload?.partial;

          // Track cumulative snapshot deltas (internal map)
          const prev = stage2BatchCum.get(batchId) || { succ: 0, fail: 0 };
          const dSucc = Math.max(0, cumSucc - prev.succ);
          const dFail = Math.max(0, cumFail - prev.fail);
          stage2BatchCum.set(batchId, { succ: cumSucc, fail: cumFail });
          if (dSucc > 0 || dFail > 0) {
            console.log('[DIAG][Stage2][delta]', { batchId, cumSucc, cumFail, prevSucc: prev.succ, prevFail: prev.fail, dSucc, dFail });
          } else if (cumSucc > 0 || cumFail > 0) {
            console.log('[DIAG][Stage2][delta-zero]', { batchId, cumSucc, cumFail, prevSucc: prev.succ, prevFail: prev.fail });
          }

          // Remember authoritative group size
          if (groupSize > 0) {
            stage2GroupSizeSnapshot.set(batchId, groupSize);
            // Recompute started to reflect latest group size/mapping counts
            recomputeStage2Started();
            // 중요: group_size 는 개별 batch snapshot 이며 전체 Stage 2 확정 total 이 아니다.
            // 과거 구현은 여기서 detailPlannedTotal 을 덮어써 조기 100% 진행률을 유발.
            // authoritative 분모는 'actor-stage-started(product_detail)' 의 items_count 에서만 설정.
            const currentStarted = detailStats().started || 0;
            if (currentStarted > groupSize && !partial) {
              console.warn('[Stage2][overcount-detected]', { batchId, currentStarted, groupSize, diff: currentStarted - groupSize });
            }
          }

          // Apply succeeded/failed deltas to completed/failed counters
          if (dSucc > 0 || dFail > 0) {
            setDetailStats((prevStats) => {
              const completed = (prevStats.completed || 0) + dSucc;
              const failedCt = (prevStats.failed || 0) + dFail;
              const startedNow = detailStats().started || prevStats.started || 0;
              const inflight = Math.max(0, startedNow - (completed + failedCt));
              // Per-page fetched diagnostics
              const pageNumber = Number(payload?.page_number ?? NaN);
              if (Number.isFinite(pageNumber)) {
                const prevFetched = pageFetchedMap.get(pageNumber) || 0;
                pageFetchedMap.set(pageNumber, prevFetched + dSucc + dFail);
                const planned = pagePlannedMap.get(pageNumber) || 0;
                const actual = pageFetchedMap.get(pageNumber) || 0;
                if (actual !== planned) {
                  console.log('[DIAG][Stage2][page-diff]', { page: pageNumber, planned, fetched: actual, delta: actual - planned });
                }
              }
              if (autoBackfillStage2) {
                const inferredAttempted = completed + failedCt;
                if (inferredAttempted > startedNow) {
                  return { ...prevStats, started: inferredAttempted, completed, failed: failedCt, inflight: Math.max(0, inferredAttempted - (completed + failedCt)) };
                }
              }
              return { ...prevStats, completed, failed: failedCt, inflight, started: startedNow };
            });
            if (effectsOn()) triggerStage2Pulse();
          }
          } // Close isShallowMode() else block
        }
        if (name === "actor-product-lifecycle" && !isShallowMode()) {
          const status = String(payload?.status || "").toLowerCase();
          if (status === "failed") {
            setDetailStats((prev) => {
              const started = detailStats().started || prev.started || 0; // use latest recomputed started
              const failed = (prev.failed || 0) + 1;
              const inflight = Math.max(0, started - (prev.completed + failed));
              return { ...prev, failed, inflight };
            });
            if (effectsOn()) triggerStage2Pulse();
          }
        }
        // Stage 3 (Validation) events
        if (name === "actor-validation-started") {
          const target = Number(payload?.scan_pages ?? 0) || 0;
          setValidationStats((prev) => ({
            ...(prev || {}),
            started: true,
            completed: false,
            targetPages: (prev?.targetPages || 0) + target,
            // keep running tallies across short validation bursts
            pagesScanned: prev?.pagesScanned || 0,
            divergences: prev?.divergences || 0,
            anomalies: prev?.anomalies || 0,
            productsChecked: prev?.productsChecked || 0,
            lastPage: prev?.lastPage ?? null,
            lastAssignedStart: prev?.lastAssignedStart ?? null,
            lastAssignedEnd: prev?.lastAssignedEnd ?? null,
          }));
        }
        if (name === "actor-validation-page-scanned") {
          setValidationStats((prev) => ({
            ...prev,
            pagesScanned: prev.pagesScanned + 1,
            productsChecked: prev.productsChecked + (Number(payload?.products_found ?? 0) || 0),
            lastPage: Number(payload?.physical_page ?? prev.lastPage ?? 0) || prev.lastPage,
            lastAssignedStart: Number(payload?.assigned_start_offset ?? prev.lastAssignedStart ?? 0) || prev.lastAssignedStart,
            lastAssignedEnd: Number(payload?.assigned_end_offset ?? prev.lastAssignedEnd ?? 0) || prev.lastAssignedEnd,
          }));
          // subtle validation pulse removed
        }
        if (name === "actor-validation-divergence") {
          setValidationStats((prev) => ({
            ...prev,
            divergences: prev.divergences + 1,
          }));
        }
        if (name === "actor-validation-anomaly") {
          setValidationStats((prev) => ({
            ...prev,
            anomalies: prev.anomalies + 1,
          }));
        }
        if (name === "actor-validation-completed") {
          setValidationStats((prev) => ({
            ...prev,
            completed: true,
            // Do not override pagesScanned; we increment on page-scanned events.
            productsChecked:
              Number(payload?.products_checked ?? prev.productsChecked) ||
              prev.productsChecked,
            divergences:
              Number(payload?.divergences ?? prev.divergences) ||
              prev.divergences,
            anomalies:
              Number(payload?.anomalies ?? prev.anomalies) || prev.anomalies,
          }));
        }

        // Fallback: If backend emits only generic stage events for Validation, reflect them here
        if (name === "actor-stage-started") {
          // 🔍 CRITICAL DEBUG: StageStarted 이벤트 수신됨
          console.log('🔍 [CRITICAL] StageStarted event received:', { name, payload });
          
          // stage_type may be serialized as nested enum object; normalize to string
          const stageTypeRaw = (payload as any)?.stage_type;
          const t = typeof stageTypeRaw === "string"
            ? stageTypeRaw.toLowerCase()
            : stageTypeRaw && typeof stageTypeRaw === "object"
            ? (Object.keys(stageTypeRaw)[0] || "").toLowerCase()
            : "";
          
          console.log('🔍 [CRITICAL] Stage type normalized:', { stageTypeRaw, t });
          
          // 🔍 EXTRA DEBUG: ProductDetail 검사 상세 로깅
          const isProductDetail = t.includes('productdetail') || t.includes('product_detail');
          console.log('🔍 [DEBUG-CONDITION] ProductDetail check:', { 
            rawStageType: stageTypeRaw,
            normalizedStageType: t, 
            includesProductDetail: t.includes('productdetail'), 
            includesProductUnder: t.includes('product_detail'),
            finalResult: isProductDetail,
            itemsCount: payload?.items_count,
            fullPayload: payload
          });
          
          // 🚨 CRITICAL: 모든 StageStarted 이벤트 로깅
          if (!isProductDetail) {
            console.log('🚨 [NON-PRODUCTDETAIL] Other stage detected:', {
              rawStageType: stageTypeRaw,
              normalizedStageType: t,
              isValidation: t.includes("validation"),
              isListPage: t.includes("listpage") || t.includes("list_page"),
              itemsCount: payload?.items_count
            });
          }
          
          if (t.includes("validation")) {
            const total = Number(payload?.items_count ?? 0) || 0;
            setValidationStats((prev) => ({
              ...prev,
              started: true,
              completed: false,
              targetPages: total || prev.targetPages,
            }));
          }
          // 🔧 FIX: ProductDetailCrawling 인식 수정
          if (isProductDetail) {
            console.log('🔍 [CRITICAL] ProductDetail stage detected!');
            const expanded = Number(payload?.items_count ?? 0) || 0;
            const prev = detailPlannedTotal();
            console.log('🔍 [CRITICAL] DetailPlannedTotal update:', { expanded, prev });
            
            if (expanded > 0) {
              // 🔧 FIX: 누적 처리 - 배치별로 누적
              const newTotal = (prev || 0) + expanded;
              console.log('✅ [SUCCESS] ACCUMULATING detailPlannedTotal:', { prev, expanded, newTotal });
              setDetailPlannedTotal(newTotal);
            } else {
              console.error('❌ [ERROR] StageStarted missing items_count:', payload);
            }
          }
        }
        if (name === "actor-stage-completed") {
          const stageTypeRaw = (payload as any)?.stage_type;
          const t = typeof stageTypeRaw === "string"
            ? stageTypeRaw.toLowerCase()
            : stageTypeRaw && typeof stageTypeRaw === "object"
            ? (Object.keys(stageTypeRaw)[0] || "").toLowerCase()
            : "";
          if (t.includes("validation")) {
            const processed =
              Number(payload?.result?.processed_items ?? 0) || 0;
            setValidationStats((prev) => ({
              ...prev,
              completed: true,
              pagesScanned: processed > 0 ? processed : prev.pagesScanned,
            }));
          }
          // Mark Stage 1 as complete when list_page_crawling completes
          if (t.includes("listpage") || t.includes("list_page")) {
            setPageStats((prev) => ({
              ...prev,
              completed: prev.started, // Mark all started as completed
              inflight: 0,
            }));
            if (effectsOn()) triggerStage1Pulse();
          }
          if (t.includes('data_saving')) {
            const ok = Number(payload?.result?.ok ?? 0) || 0;
            const fail = Number(payload?.result?.fail ?? 0) || 0;
            if (!persistEventsSeen && !persistFailureSynthesized && ok === 0 && fail > 0) {
              persistFailureSynthesized = true;
              console.log('[DIAG][Persist][failure-fallback]', { ok, fail });
              setPersistStats((prev) => ({
                ...prev,
                mode: prev.mode || 'group-only',
                attempted: prev.attempted > 0 ? prev.attempted : 1,
                failed: prev.failed > 0 ? prev.failed : 1,
                failedTrue: prev.failedTrue > 0 ? prev.failedTrue : 1,
                successRate: 0,
                noEvents: true,
                failureInferred: true,
              }));
            }
          }
        }

        // Stage 4 (DB) snapshots and session summary
        if (name === "actor-database-stats") {
          console.log("[DEBUG] DatabaseStats event received:", payload);
          // Extract total from the correct field name
          const totalFromPayload = Number(payload?.total_product_details ?? 0) || 0;
          
          // Extract page range information
          const minPageFromPayload = payload?.min_page ?? null;
          const maxPageFromPayload = payload?.max_page ?? null;
          
          setDbSnapshot((prev) => ({
            ...prev,
            total: totalFromPayload > 0 ? totalFromPayload : prev.total,
            minPage: minPageFromPayload !== null ? Number(minPageFromPayload) : prev.minPage,
            maxPage: maxPageFromPayload !== null ? Number(maxPageFromPayload) : prev.maxPage,
          }));
          if (effectsOn()) {
            setDbFlash(true);
            setTimeout(() => setDbFlash(false), 500);
          }
        }
  if (name === "actor-session-report") {
          setDbSnapshot((prev) => ({
            ...prev,
            inserted:
              Number(payload?.products_inserted ?? prev.inserted ?? 0) ||
              prev.inserted,
            updated:
              Number(payload?.products_updated ?? prev.updated ?? 0) ||
              prev.updated,
          }));
        }
        
        // Handle batch completed event to extract DB stats when persist events are missing
        if (name === "actor-batch-completed") {
          console.log("[DEBUG] BatchCompleted event received:", payload);
          // Try to extract products_inserted/updated from the payload
          const insertedFromBatch = Number(payload?.products_inserted ?? 0) || 0;
          const updatedFromBatch = Number(payload?.products_updated ?? 0) || 0;
          
          if (insertedFromBatch > 0 || updatedFromBatch > 0) {
            setDbSnapshot((prev) => ({
              ...prev,
              inserted: insertedFromBatch,
              updated: updatedFromBatch,
            }));
            
            persistAccumulator.applyBatchFallback({ inserted: insertedFromBatch, updated: updatedFromBatch });
            const snap = persistAccumulator.snapshot();
            setPersistStats((prev) => ({
              ...prev,
              ...snap,
              successRate: snap.attempted > 0 ? (snap.succeeded / snap.attempted) * 100 : 0,
            }));
            
            if (effectsOn()) {
              setDbFlash(true);
              setPersistFlash(true);
              setTimeout(() => {
                setDbFlash(false);
                setPersistFlash(false);
              }, 500);
            }
          }
        }
        if (name === "actor-preflight-diagnostics") {
          // Capture site totals for Stage 1 expected denominator
          const site_total_pages = Number(payload?.site_total_pages ?? 0) || undefined;
          setPreflight({ site_total_pages });
        }
        // Stage 5 (Persist) grouped lifecycle snapshot
        if (
          name === "actor-product-lifecycle-group" &&
          payload?.phase === "persist"
        ) {
          persistEventsSeen = true;
          console.log("[DEBUG] ProductLifecycleGroup persist event received:", payload);
          const groupSize = Number(payload?.group_size ?? 0) || 0;
          const succRaw = Number(payload?.succeeded ?? 0) || 0;
          const failRaw = Number(payload?.failed ?? 0) || 0;
          if (groupSize === 0 && succRaw === 0 && failRaw === 0 && !payload?.duration_ms && !payload?.duplicates) {
            addLog("ℹ️ Persist placeholder 이벤트 무시 (모든 값 0)");
            return;
          }
          const attempted = Number(payload?.group_size ?? 0) || 0;
          const succeeded = Number(payload?.succeeded ?? 0) || 0;
          const duplicates = Number(payload?.duplicates ?? 0) || 0;
          const unchanged = Math.max(0, attempted - (succeeded + duplicates));
          const durationMs = Number(payload?.duration_ms ?? 0) || 0;
          persistAccumulator.applyGroup({ attempted, duplicates, unchanged, durationMs });
          const snap = persistAccumulator.snapshot();
          setPersistStats((prev) => ({
            ...prev,
            ...snap,
            successRate: snap.attempted > 0 ? (snap.succeeded / snap.attempted) * 100 : 0,
            noEvents: false,
            failureInferred: false,
          }));


          // Also surface cumulative DB change counts when session report lags (treat succeeded as net changed rows)
          setDbSnapshot((prev) => ({
            ...prev,
            updated: (Number(prev.updated ?? 0) || 0) + succeeded,
          }));
          // flash Stage 5 panel
          if (effectsOn()) {
            setPersistFlash(true);
            setTimeout(() => setPersistFlash(false), 500);
          }
        }

        // Diagnostic: accept any persist-like ProductLifecycleGroup even if phase missing (schema drift)
        if (name === 'actor-product-lifecycle-group' && payload?.phase !== 'fetch' && payload?.phase !== 'persist') {
          // Heuristic: if group_size present and (succeeded or failed) and started fields exist, treat as persist fallback snapshot
          if (payload?.group_size != null && (payload?.succeeded != null || payload?.failed != null)) {
            console.log('[DIAG][Persist][phase-missing-or-unknown]', payload);
          }
        }

        // Expose inferred attempted for Stage 2 (completed + failed) to compare against started
        if (name === 'actor-product-lifecycle-group' && payload?.phase === 'fetch') {
          // After updating detailStats we can log discrepancy
          const ds = detailStats();
          const inferredAttempted = (ds.completed || 0) + (ds.failed || 0);
          if (ds.started > 0 && Math.abs(inferredAttempted - ds.started) >= 6) { // threshold to avoid noise
            console.log('[DIAG][Stage2][discrepancy]', { started: ds.started, inferredAttempted, completed: ds.completed, failed: ds.failed });
          }
        }

        // New keyed per-product detail events (authoritative, overrides legacy group/mapping logic)
        if (name === 'actor-product-detail-keyed') {
          try {
            const key = payload?.product_key || payload?.product_url;
            if (key) {
              const status: string = String(payload?.status || '');
              const phaseRaw: string = String(payload?.phase || 'fetch');
              // Map (phaseRaw,status) -> ProductDetailPhase
              let trackerPhase: ProductDetailPhase;
              if (phaseRaw === 'fetch') {
                trackerPhase = status === 'succeeded' ? 'fetch_succeeded' : (status === 'failed' ? 'fetch_failed' : 'fetch_started');
              } else if (phaseRaw === 'parse') {
                trackerPhase = status === 'succeeded' ? 'parse_succeeded' : (status === 'failed' ? 'parse_failed' : 'parse_started');
              } else if (phaseRaw === 'persist') {
                trackerPhase = status === 'succeeded' ? 'persist_succeeded' : (status === 'failed' ? 'persist_failed' : 'persist_started');
              } else {
                trackerPhase = 'fetch_started';
              }
              const isTerminal = status === 'succeeded' || status === 'failed';
              const pd: ProductDetailEvent = {
                key,
                attempt: Number(payload?.attempt || 1) || 1,
                phase: trackerPhase,
                ts: Date.now(),
                page: undefined,
                meta: undefined,
                final: isTerminal,
                error_code: status === 'failed' ? (payload?.error || 'failed') : undefined,
              };
              detailTracker.ingest(pd);
              if (!detailTrackerActive) {
                detailTrackerActive = true;
                console.log('[Stage2][Tracker] activated via actor-product-detail-keyed (switching to keyed counting)');
              }
              applyDetailTrackerSnapshot();
            }
          } catch (e) {
            console.warn('[Stage2][ProductDetailKeyed] handling failed', e);
          }
        }
        
        // Handle persist_empty case - when no products to persist
        if (name === "actor-product-lifecycle" && payload?.status === "persist_empty") {
          console.log("[DEBUG] ProductLifecycle persist_empty event received:", payload);
          persistAccumulator.applyEmpty();
          const snap = persistAccumulator.snapshot();
          setPersistStats((prev) => ({
            ...prev,
            ...snap,
            successRate: snap.attempted > 0 ? (snap.succeeded / snap.attempted) * 100 : 0,
          }));
          
          if (effectsOn()) {
            setPersistFlash(true);
            setTimeout(() => setPersistFlash(false), 500);
          }
        }

        // Fallback: parse generic ProductLifecycle persist_* with metrics.persist_result
        if (name === "actor-product-lifecycle" && typeof payload?.status === "string" && payload.status.startsWith("persist_")) {
          try {
            const m = payload?.metrics;
            let key: string | undefined;
            let value: string | undefined;
            // Shape A: { metrics: { Generic: { key, value } } }
            if (m && typeof m === 'object' && !Array.isArray(m)) {
              const k1 = Object.keys(m)[0];
              if (k1 && typeof (m as any)[k1] === 'object') {
                key = (m as any)[k1]?.key;
                value = (m as any)[k1]?.value;
              }
              // Shape B: { metrics: { type: 'Generic', data: { key, value } } }
              const t = String((m as any)?.type || '').toLowerCase();
              const d = (m as any)?.data;
              if (t === 'generic' && d && typeof d === 'object') {
                key = (d as any)?.key ?? key;
                value = (d as any)?.value ?? value;
              }
              // Shape C: direct: { metrics: { key, value } }
              if ((m as any)?.key != null || (m as any)?.value != null) {
                key = (m as any)?.key ?? key;
                value = (m as any)?.value ?? value;
              }
            }
            if ((key || '').toLowerCase() === 'persist_result' && typeof value === 'string') {
              // value format: attempted=..,inserted=..,updated=..,duplicates=..,unchanged=..
              const parts = Object.fromEntries(
                value.split(',').map((p) => {
                  const [k, v] = p.split('=');
                  return [k?.trim() || '', Number(v) || 0];
                })
              ) as Record<string, number>;
              const attempted = parts.attempted ?? 0;
              const inserted = parts.inserted ?? 0;
              const updated = parts.updated ?? 0;
              const duplicates = parts.duplicates ?? 0;
              const unchanged = parts.unchanged ?? Math.max(0, attempted - (inserted + updated + duplicates));
              const durationMs = Number(payload?.duration_ms ?? 0) || 0;
              console.info('[Stage5][PersistResult] raw parsed', { attempted, inserted, updated, duplicates, unchanged, durationMs, status: payload?.status });
              persistAccumulator.applyResult({ attempted, inserted, updated, duplicates, unchanged, durationMs, status: String(payload?.status || '') });
              const snap = persistAccumulator.snapshot();
              setPersistStats((prev) => ({
                ...prev,
                ...snap,
                successRate: snap.attempted > 0 ? (snap.succeeded / snap.attempted) * 100 : 0,
              }));
              if (effectsOn()) {
                setPersistFlash(true);
                setTimeout(() => setPersistFlash(false), 500);
              }
            }
          } catch (e) {
            console.warn('[CrawlingEngineTabSimple] persist_result parse failed', e);
          }
        }
  } })
      .then((un) => unsubs.push(un))
      .catch((e) =>
        console.warn(
          "[CrawlingEngineTabSimple] unified actor subscribe failed",
          e
        )
      );

  // Rely on actor-session lifecycle for completion/stop handling
  // Session completed handling already present above; stopped/failed/timeouts handled via actor-session-* cases

    onCleanup(() => {
      unsubs.forEach((u) => u());
    });

    // === New: Direct actor app-event listener for Stage 4/5 (DatabaseStats & DataSaving) ===
    try {
      listen("app-event", (evt) => {
        const ev: any = evt.payload;
        if (!ev || typeof ev !== "object" || !ev.type) return;
        switch (ev.type) {
          // NEW: keyed product detail event stream (future backend integration)
          case 'ProductDetailEvent': {
            try {
              const p: ProductDetailEvent = {
                key: ev.key,
                attempt: ev.attempt || 1,
                phase: ev.phase,
                ts: ev.ts,
                page: ev.page,
                meta: ev.meta,
                final: ev.final,
                error_code: ev.error_code,
              };
              detailTracker.ingest(p);
              if (!detailTrackerActive) {
                detailTrackerActive = true;
                console.log('[Stage2][Tracker] activated (switching UI to keyed counting)');
              }
              applyDetailTrackerSnapshot();
            } catch (e) {
              console.warn('[Stage2][Tracker] ingest failed', e);
            }
            break;
          }
          case "DatabaseStats": {
            // Normalize to existing state shape (dbSnapshot currently had optional keys)
            setDbSnapshot({
              total: ev.total_product_details,
              minPage: ev.min_page ?? null,
              maxPage: ev.max_page ?? null,
              inserted: ev.inserted, // may be undefined
              updated: ev.updated,
            });
            setDbFlash(true);
            setTimeout(() => setDbFlash(false), 300);
            break;
          }
          case "StageStarted": {
            if (ev.stage_type === "data_validation") {
              setValidationStats((s) => ({ ...s, started: true }));
            }
            if (ev.stage_type === "data_saving") {
              setPersistFlash(true);
              setTimeout(() => setPersistFlash(false), 250);
            }
            break;
          }
          case "StageCompleted": {
            if (ev.stage_type === "data_validation") {
              setValidationStats((s) => ({ ...s, completed: true }));
            }
            if (ev.stage_type === "data_saving") {
              const r: any = ev.result || {};
              const attempted = Number(r.processed_items || 0);
              const failedRaw = Number(r.failed_items || 0);
              setPersistStats((prev) => {
                const inserted = prev.inserted; // keep previously parsed totals
                const updated = prev.updated;
                const succeededTotal = inserted + updated;
                return {
                  ...prev,
                  attempted: Math.max(prev.attempted, attempted),
                  failed: Math.max(prev.failed, failedRaw),
                  succeeded: succeededTotal,
                };
              });
            }
            break;
          }
        }
      }).then((un) => unsubs.push(un));
    } catch (e) {
      console.warn("[CrawlingEngineTabSimple] failed to attach app-event listener", e);
    }
  });

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        <div class="flex items-center justify-end text-[11px] text-gray-500 select-none mb-2">
          <span class="px-2 py-1 rounded bg-white/70 border border-gray-200">
            events: {actorEventCount()} {lastActorEvent() ? `· last: ${lastActorEvent()}` : ""}
          </span>
        </div>
    
    <SessionStatusCard 
      isRunning={isRunning} 
      statusMessage={statusMessage} 
      batchInfo={batchInfo}
      siteHealthWarning={siteHealthWarning}
    />
    
    {/* ListPageCrawling 실시간 진행상황 패널 */}
    <ListPageProgressPanel />
    
    {/* 제품 보완 크롤링 실시간 진행상황 패널 */}
    <ComplementCrawlProgressPanel />
    
    {/* 크롤링 시간 추정 패널 */}
    <TimeEstimatesPanel progress={crawlerStore.progress()} />
    
    {/* 복원: 계산된 크롤링 범위 & 사전 분석 Premium Cards */}
    <Show when={!isRunning()}>
      <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 -mt-4 space-y-6">
        <div class="flex items-center justify-between cursor-pointer" onClick={() => setPlanExpanded(v => !v)}>
          <div class="flex items-center gap-2">
            <span class={`text-gray-500 transition-transform ${planExpanded() ? 'rotate-90' : ''}`}>▶</span>
            <h3 class="text-lg font-semibold bg-gradient-to-r from-blue-600 to-indigo-600 bg-clip-text text-transparent tracking-tight">📊 계산된 크롤링 플랜 개요</h3>
          </div>
          <div class="flex items-center gap-2 text-[11px] text-gray-500">
            <span class="px-2 py-1 rounded-full bg-gray-100 border border-gray-200">Site + LocalDB + Settings 분석</span>
            <button
              class="text-[11px] px-2.5 py-1 rounded border border-gray-300 text-gray-700 bg-white hover:bg-gray-50"
              onClick={(e) => {
                e.stopPropagation();
                setShowAdvanced(v => !v);
              }}
            >
              {showAdvanced() ? '고급 숨기기' : '고급 보기'}
            </button>
          </div>
        </div>
        <Show when={planExpanded()}>
          <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-6">
            <div class="group relative overflow-hidden rounded-xl border border-blue-200/60 bg-gradient-to-br from-blue-50 to-blue-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-blue-700 mb-1">시작 페이지</div>
              <div class="text-2xl font-bold text-blue-700 tabular-nums">{(() => { const r = crawlingRange(); return r?.range?.[0] ?? '-'; })()}</div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_70%_20%,rgba(255,255,255,.9),transparent_60%)]" />
            </div>
            <div class="group relative overflow-hidden rounded-xl border border-emerald-200/60 bg-gradient-to-br from-emerald-50 to-emerald-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-emerald-700 mb-1">종료 페이지</div>
              <div class="text-2xl font-bold text-emerald-700 tabular-nums">{(() => { const r = crawlingRange(); return r?.range?.[1] ?? '-'; })()}</div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_30%_80%,rgba(255,255,255,.85),transparent_65%)]" />
            </div>
            <div class="group relative overflow-hidden rounded-xl border border-purple-200/60 bg-gradient-to-br from-purple-50 to-purple-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-purple-700 mb-1">페이지 수</div>
              <div class="text-2xl font-bold text-purple-700 tabular-nums">{(() => { const v = Number(crawlingRange()?.crawling_info?.pages_to_crawl ?? 0); return v>0? v: '-'; })()}</div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,.9),transparent_60%)]" />
            </div>
            <div class="group relative overflow-hidden rounded-xl border border-indigo-200/60 bg-gradient-to-br from-indigo-50 to-indigo-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-indigo-700 mb-1">로컬DB 제품</div>
              <div class="text-2xl font-bold text-indigo-700 tabular-nums">
                {(() => { 
                  const local = Number(crawlingRange()?.local_db_info?.total_saved_products ?? 0); 
                  const pf = preflight();
                  const sitePages = Number(pf?.site_total_pages ?? 0);
                  const lastPageItems = Number(pf?.products_on_last_page ?? 0);
                  const site = sitePages > 0 && lastPageItems > 0 
                    ? (sitePages - 1) * 12 + lastPageItems 
                    : 0;
                  if (local > 0 && site > 0) {
                    return <>
                      {local.toLocaleString()}
                      <span class="text-sm font-normal text-indigo-500 ml-1">/ {site.toLocaleString()}</span>
                    </>;
                  }
                  return local > 0 ? local.toLocaleString() : '-'; 
                })()}
              </div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_80%_30%,rgba(255,255,255,.95),transparent_65%)]" />
            </div>
            <div class="group relative overflow-hidden rounded-xl border border-orange-200/60 bg-gradient-to-br from-orange-50 to-orange-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-orange-700 mb-1">커버리지</div>
              <div class="text-2xl font-bold text-orange-700 tabular-nums">{(() => { const p = Number(crawlingRange()?.progress?.progress_percentage ?? 0); return p>0? `${p.toFixed(1)}%` : '-'; })()}</div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_20%_40%,rgba(255,255,255,.9),transparent_65%)]" />
            </div>
            <div class="group relative overflow-hidden rounded-xl border border-teal-200/60 bg-gradient-to-br from-teal-50 to-teal-100 p-4 shadow hover:shadow-lg transition">
              <div class="text-[11px] font-medium text-teal-700 mb-1">예상 신규 세부</div>
              <div class="text-2xl font-bold text-teal-700 tabular-nums">{(() => { const info = crawlingRange()?.crawling_info; const est = Number(info?.estimated_new_products ?? 0); if(est>0) return est; const pages = Number(info?.pages_to_crawl ?? 0); return pages>0? pages*12 : '-'; })()}</div>
              <div class="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition bg-[radial-gradient(circle_at_70%_70%,rgba(255,255,255,.9),transparent_60%)]" />
            </div>
            <Show when={showAdvanced()}>
              <div class="group relative overflow-hidden rounded-xl border border-gray-200/60 bg-gradient-to-br from-gray-50 to-gray-100 p-4 shadow hover:shadow-lg transition sm:col-span-2 lg:col-span-3">
                <div class="flex items-center justify-between mb-2">
                  <div class="text-[11px] font-medium text-gray-600">사이트 메타</div>
                  <div class="text-[10px] text-gray-400">preflight</div>
                </div>
                <div class="flex flex-wrap gap-4 text-xs text-gray-700">
                  <div>총페이지: <span class="font-semibold">{(() => { const p = preflight(); const v = Number(p?.site_total_pages ?? 0); return v>0? v: '-'; })()}</span></div>
                  <div>마지막페이지제품: <span class="font-semibold">{(() => { const p = preflight(); const v = Number(p?.products_on_last_page ?? 0); return v>0? v: '-'; })()}</span></div>
                  <div>범위: <span class="font-semibold">{(() => { const r = crawlingRange(); const s=r?.range?.[0]; const e=r?.range?.[1]; return (s&&e)? `${s}→${e}`:'-'; })()}</span></div>
                  <div>설정 효과: <span class="font-semibold">{effectsOn() ? 'ON' : 'OFF'}</span></div>
                </div>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </Show>
    <StageStatsPanels
      crawlingRange={crawlingRange}
      preflight={preflight}
      pageStats={pageStats}
          detailStats={detailStats}
          detailTarget={detailPlannedTotal}
          stage1Pulse={stage1Pulse}
          stage2Pulse={stage2Pulse}
          downshiftInfo={downshiftInfo}
          effectsOn={effectsOn}
        />

        <ControlPanel
          isRunning={isRunning}
            isSyncing={isSyncing}
            effectsOn={effectsOn}
            syncPulse={syncPulse}
            syncRanges={syncRanges}
            startUnifiedAdvanced={startUnifiedAdvanced}
            calculateCrawlingRange={calculateCrawlingRange}
            deriveRangesFromDiagnostics={deriveRangesFromDiagnostics}
            setSyncRanges={setSyncRanges}
            setSyncPulse={setSyncPulse}
            setEffectsOn={setEffectsOn}
            setIsSyncing={setIsSyncing}
            setCrawlingRange={setCrawlingRange}
            setCurrentSessionId={setCurrentSessionId}
            addLog={addLog}
            tauriApi={tauriApi}
            handleShallowSync={handleShallowSync}
            handleComplementCrawl={handleComplementCrawl}
            onHelpClick={() => setHelpPanelOpen(true)}
            onStop={handleStop}
            siteHealthWarning={siteHealthWarning}
        />

        {/* 도움말 패널 */}
        <HelpPanel isOpen={helpPanelOpen} onClose={() => setHelpPanelOpen(false)} />

        {/* Stage1/Stage2 Runtime Monitor duplicated block removed; StageStatsPanels renders these above */}

        {/* Stage3/Stage4/Stage5 Mini Panels (modularized) */}
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
          <ValidationPanel
            stats={() => {
              const v = validationStats();
              return {
                targetPages: v.targetPages,
                pagesScanned: v.pagesScanned,
                divergences: v.divergences,
                anomalies: v.anomalies,
                lastPage: v.lastPage ?? null,
                lastAssignedStart: v.lastAssignedStart ?? null,
                lastAssignedEnd: v.lastAssignedEnd ?? null,
                started: !!v.started,
                completed: !!v.completed,
              };
            }}
            effectsOn={effectsOn}
          />
          <DbSnapshotPanel snapshot={dbSnapshot} flash={dbFlash} effectsOn={effectsOn} />
          <PersistPanel
            stats={() => {
              const p = persistStats();
              return {
                attempted: p.attempted,
                inserted: p.inserted,
                updated: p.updated,
                duplicates: p.duplicates,
                unchanged: p.unchanged,
                failedTrue: p.failedTrue,
                successRate: p.attempted > 0 ? (p.succeeded / Math.max(1, p.attempted)) * 100 : 0,
                mode: (p as any).mode, // optional mode if present
              };
            }}
            lastBatch={() => undefined}
            flash={persistFlash}
            effectsOn={effectsOn}
          />
        </div>

        {/* Stage X: DB Diagnostics (Collapsible) - Stage 5 이후로 이동 */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 overflow-hidden mb-8">
          <button
            class="w-full flex items-center justify-between p-4 hover:bg-gray-50 transition-colors"
            onClick={() => setDiagnosticsExpanded(!diagnosticsExpanded())}
          >
            <div class="flex items-center gap-2">
              <span class="text-lg">{diagnosticsExpanded() ? '▼' : '▶'}</span>
              <h3 class="text-lg font-bold text-gray-800">Stage X: DB 레코드 체크 및 동기화</h3>
              <Show when={diagResult()?.total_products_without_coords}>
                <span class="px-2 py-0.5 text-xs rounded-full bg-orange-100 text-orange-700 font-semibold">
                  NULL 좌표: {diagResult()?.total_products_without_coords}개
                </span>
              </Show>
            </div>
            <span class="text-xs text-gray-500">
              {diagnosticsExpanded() ? '접기' : '펼치기'}
            </span>
          </button>
          
          <Show when={diagnosticsExpanded()}>
            <div class="border-t border-gray-200">
              <DiagnosticsPanel
                diagResult={diagResult}
                diagLoading={diagLoading}
                cleanupLoading={cleanupLoading}
                runDiagnostics={runDiagnostics}
                runUrlCleanup={runUrlCleanup}
                deriveRangesFromDiagnostics={deriveRangesFromDiagnostics}
                setSyncRanges={setSyncRanges}
                setSyncPulse={setSyncPulse}
                addLog={addLog}
                isSyncing={isSyncing}
                startCoordSync={async () => {
                  try {
                    setIsSyncing(true);
                    addLog("🔁 products→details 좌표/ID 정합화 실행...");
                    const rep = await tauriApi.syncProductDetailsCoordinates();
                    addLog(`✅ 정합화 완료: products.id=${rep.updated_product_ids}, inserted=${rep.inserted_details}, updated_coords=${rep.updated_coordinates}, details.id=${rep.updated_ids} (p=${rep.total_products}, d=${rep.total_details})`);
                  } catch (e: any) {
                    addLog(`❌ 정합화 실패: ${e.message || e}`);
                  } finally {
                    setIsSyncing(false);
                  }
                }}
                handleShallowSync={handleShallowSync}
                handleComplementCrawl={handleComplementCrawl}
              />
            </div>
          </Show>
        </div>

        {/* 실시간 로그 */}
        <div class="bg-gray-900/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/10 p-6">
          <h3 class="text-sm font-semibold text-white/90 mb-3">📝 실시간 로그</h3>
          <div class="font-mono text-xs text-emerald-300 h-64 overflow-y-auto">
            <Show
              when={logs().length > 0}
              fallback={<div class="text-gray-400">로그 대기 중...</div>}
            >
              <For each={logs()}>{(log) => <div class="mb-1">{log}</div>}</For>
            </Show>
          </div>
        </div>

        {/* Actor 이벤트 콘솔 (개발용) */}
        <Show when={showConsole()}>
          <div class="mt-8 bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 overflow-hidden">
            <button
              class="w-full px-5 py-3 border-b border-white/30 bg-gradient-to-r from-gray-50 to-gray-100 text-sm text-gray-700 hover:from-gray-100 hover:to-white transition-colors flex items-center justify-between"
              onClick={() => setConsoleExpanded(!consoleExpanded())}
            >
              <span class="flex items-center gap-2">
                <span class={`transform transition-transform duration-200 ${
                  consoleExpanded() ? 'rotate-90' : 'rotate-0'
                }`}>
                  ▶
                </span>
                Actor 이벤트 콘솔
              </span>
              <span class="text-xs text-gray-500">
                {consoleExpanded() ? '숨기기' : '펼치기'}
              </span>
            </button>
            <Show when={consoleExpanded()}>
              <div class="animate-in slide-in-from-top duration-300">
                <div class="p-4">
                  {/* StageBatcherSettingsPanel removed */}
                </div>
                {/* EventConsole removed */}
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}
