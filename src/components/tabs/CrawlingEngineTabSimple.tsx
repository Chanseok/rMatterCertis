import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import SyncPanel from "./parts/SyncPanel";
import SessionStatusCard from "./parts/SessionStatusCard";
import ControlPanel from "./parts/ControlPanel";
import StageStatsPanels from "./parts/StageStatsPanels";
import DiagnosticsPanel from "./parts/DiagnosticsPanel";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
// Types are relaxed locally to avoid tight coupling during integration
import { tauriApi } from "../../services/tauri-api";
// Dev-only panels removed during cleanup
import { usePulse } from "../../hooks/usePulse";
import CountUp from "../common/CountUp";
import ValidationPanel from "./parts/ValidationPanel";
import DbSnapshotPanel from "./parts/DbSnapshotPanel";
import PersistPanel from "./parts/PersistPanel";

export default function CrawlingEngineTabSimple() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingRange, setCrawlingRange] = createSignal<any | null>(null);
  const [statusMessage, setStatusMessage] =
    createSignal<string>("크롤링 준비 완료");
  const [logs, setLogs] = createSignal<string[]>([]);
  const [showConsole] = createSignal<boolean>(true);
  const [consoleExpanded, setConsoleExpanded] = createSignal<boolean>(false); // Actor 이벤트 콘솔 확장/축소 상태 (기본: 축소)
  // isValidating: 제거됨 (미사용)
  const [isSyncing, setIsSyncing] = createSignal(false);
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
  // Batch info (Stage 1 batching)
  const [batchInfo, setBatchInfo] = createSignal<{ current: number; totalEstimated?: number; batchId?: any; startedAt?: number }>({ current: 0 });
  // Track pages already counted toward detail scheduling to prevent double counting
  const detailScheduledPages = new Set<number>();
  const [lastActorEvent, setLastActorEvent] = createSignal<string>("");
  // Removed range FX related signals (legacy range panel removed)
  const [actorEventCount, setActorEventCount] = createSignal(0);
  // Legacy range animation helpers removed
  // Lightweight Sync runtime view
  const [syncLive, setSyncLive] = createSignal<{
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
  const [diagResult, setDiagResult] = createSignal<any | null>(null);
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
  const [preflight, setPreflight] = createSignal<{ site_total_pages?: number } | null>(null);
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
  // Sync input pulse highlight
  const [syncPulse, setSyncPulse] = createSignal(false);
  // Track sync-start events to detect backend start and enable fallbacks
  let syncStartSeq = 0;
  onMount(async () => {
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
      addLog(
        `✅ 사이트 상태 확인 완료: ${siteStatus.total_pages}페이지, 마지막 페이지 ${siteStatus.products_on_last_page}개 제품`
      );

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
      addLog(`✅ 통합 파이프라인(하이) 세션 시작: ${JSON.stringify(res)}`);
      setStatusMessage("🎭 통합 파이프라인 실행 중 (하이)");
    } catch (error) {
      console.error("통합 파이프라인(하이) 시작 실패:", error);
      addLog(`❌ 통합 파이프라인(하이) 시작 실패: ${error}`);
      setStatusMessage("크롤링 실패");
      setIsRunning(false);
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
            const planned = ranges.reduce(
              (acc, [start, end]) => acc + Math.max(0, start - end + 1),
              0
            );
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
          } catch {
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
          setStatusMessage("크롤링 실행 중 (세션 시작)");
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
          setIsRunning(false);
          setStatusMessage("크롤링 완료");
          addLog("🏁 세션 완료");
          setBatchInfo((prev) => ({ ...prev }));
          // Play transition on session complete as well (helps visibility)
          try {
            // range snapshot removed
          } catch {}
          // Recompute crawling range so the UI reflects the newly planned range
          calculateCrawlingRange();
          // Mark persist no-events state if applicable
          if ((persistStats().attempted === 0 || persistStats().noEvents) ) {
            if (!(persistStats().failureInferred)) {
              setPersistStats((prev) => ({ ...prev, noEvents: true }));
              console.log('[DIAG][Persist][session-complete-no-events-marked]');
            }
          }
        }
        if (name === "actor-session-failed") {
          setIsRunning(false);
          setStatusMessage("크롤링 실패");
          addLog(`❌ 세션 실패: ${JSON.stringify(payload)}`);
          setBatchInfo((prev) => ({ ...prev }));
        }
        if (
          name === "actor-session-timeout" ||
          name === "actor-shutdown-completed"
        ) {
          setIsRunning(false);
          setStatusMessage("크롤링 종료");
          addLog("🛑 세션 종료");
          setBatchInfo((prev) => ({ ...prev }));
          // Refresh planned range after abnormal end as well
          calculateCrawlingRange();
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
          // Keep current count; nothing to do for now.
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
          if (status === "detail_scheduled" || status === "detail_mapping_emitted") {
            const pageNumKey = pageNum; // use page number as key; if unavailable skip
            if (!Number.isFinite(pageNumKey)) return;
            if (detailScheduledPages.has(pageNumKey)) {
              // Already accounted for this page's details
            } else {
              const m = payload?.metrics;
              let scheduled = 0;
              // Priority order: url_count > scheduled_details
              let urlCount = 0;
              if (m && typeof m === 'object' && !Array.isArray(m)) {
                // Unified extract function
                const extract = (obj: any) => {
                  if (!obj || typeof obj !== 'object') return;
                  if (obj.url_count != null) urlCount = Number(obj.url_count) || urlCount;
                  if (obj.scheduled_details != null) scheduled = Number(obj.scheduled_details) || scheduled;
                };
                // Variant A: first key object
                const firstKey = Object.keys(m)[0];
                if (firstKey && typeof (m as any)[firstKey] === 'object') extract((m as any)[firstKey]);
                // Variant B: type/data pattern
                if (typeof (m as any).data === 'object') extract((m as any).data);
                // Variant C: direct
                extract(m);
              }
              let perPage = urlCount > 0 ? urlCount : (scheduled > 0 ? scheduled : 0);
              if (perPage === 0) {
                // Fallback: some PageLifecycle events expose top-level urls / scheduled counts (see logs)
                const topUrls = Number((payload as any)?.urls ?? 0) || 0;
                const topScheduled = Number((payload as any)?.scheduled ?? 0) || 0;
                const fallback = topUrls > 0 ? topUrls : (topScheduled > 0 ? topScheduled : 0);
                if (fallback > 0) {
                  perPage = fallback;
                  console.log('[DIAG][Stage2][fallback-top-level]', { page: pageNumKey, topUrls, topScheduled });
                }
              }
              if (perPage > 0) {
                detailScheduledPages.add(pageNumKey);
                // Record planned per-page (first mapping only)
                if (!pagePlannedMap.has(pageNumKey)) {
                  pagePlannedMap.set(pageNumKey, perPage);
                } else {
                  // If a second mapping arrives with different count, log it
                  const prevPlanned = pagePlannedMap.get(pageNumKey)!;
                  if (prevPlanned !== perPage) {
                    console.log('[DIAG][Stage2][mapping-ignored]', { page: pageNumKey, prevPlanned, newPlanned: perPage });
                  }
                }
                setDetailStats((prev) => {
                  const started = (prev.started || 0) + perPage;
                  const inflight = Math.max(0, started - ((prev.completed || 0) + (prev.failed || 0)));
                  return { ...prev, started, inflight };
                });
                if (effectsOn()) triggerStage2Pulse();
              }
            }
          }
          if (status === "fetch_started") {
            const prevAttempts = pageAttempts.get(pageNum) ?? 0;
            pageAttempts.set(pageNum, prevAttempts + 1);
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
            if (effectsOn()) triggerStage1Pulse();
          }
        }
        // Stage 2 (product detail) itemized - deduplicate by detail_id and track retries
        // Stage 2 via product lifecycle events
        if (
          name === "actor-product-lifecycle-group" &&
          payload?.phase === "fetch"
        ) {
          // Backend emits cumulative succeeded/failed per batch; convert to delta to avoid triangular overcount.
          const batchId = String(payload?.batch_id || "");
          const cumSucc = Number(payload?.succeeded ?? 0) || 0;
          const cumFail = Number(payload?.failed ?? 0) || 0;
          if (!(window as any).__detailGroupPrev) {
            (window as any).__detailGroupPrev = new Map<string, { s: number; f: number }>();
          }
            const prevMap: Map<string, { s: number; f: number }> = (window as any).__detailGroupPrev;
            const prev = prevMap.get(batchId) || { s: 0, f: 0 };
            const dSucc = Math.max(0, cumSucc - prev.s);
            const dFail = Math.max(0, cumFail - prev.f);
            // Update snapshot
            prevMap.set(batchId, { s: cumSucc, f: cumFail });
            if (dSucc > 0 || dFail > 0) {
              console.log('[DIAG][Stage2][delta]', { batchId, cumSucc, cumFail, prevSucc: prev.s, prevFail: prev.f, dSucc, dFail });
            } else if (cumSucc > 0 || cumFail > 0) {
              // No delta but cumulative advanced earlier; helpful to detect missed batches
              console.log('[DIAG][Stage2][delta-zero]', { batchId, cumSucc, cumFail, prevSucc: prev.s, prevFail: prev.f });
            }
            if (dSucc > 0 || dFail > 0) {
              setDetailStats((prevStats) => {
                const completed = (prevStats.completed || 0) + dSucc;
                const failedCt = (prevStats.failed || 0) + dFail;
                const started = (prevStats.started || 0); // keep raw started (from scheduling)
                const inflight = Math.max(0, started - (completed + failedCt));
                // Track per-page fetched counts (page_number may be present in payload)
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
                // Optional auto backfill
                if (autoBackfillStage2) {
                  const inferredAttempted = completed + failedCt;
                  if (inferredAttempted > started) {
                    return { ...prevStats, started: inferredAttempted, completed, failed: failedCt, inflight: Math.max(0, inferredAttempted - (completed + failedCt)) };
                  }
                }
                return { ...prevStats, completed, failed: failedCt, inflight };
              });
              if (effectsOn()) triggerStage2Pulse();
            }
        }
        if (name === "actor-product-lifecycle") {
          const status = String(payload?.status || "").toLowerCase();
          if (status === "failed") {
            setDetailStats((prev) => {
              const started = prev.started || 0; // cannot infer per-product start
              const failed = (prev.failed || 0) + 1;
              const inflight = Math.max(0, started - (prev.completed + failed));
              return { ...prev, failed, inflight };
            });
            if (effectsOn()) triggerStage2Pulse();
          }
        }
        if (name === "actor-detail-concurrency-downshifted") {
          setDownshiftInfo({
            newLimit: payload?.new_limit,
            reason: payload?.reason,
          });
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
            // Optional: we can accumulate products_found into productsChecked
            productsChecked:
              prev.productsChecked +
              (Number(payload?.products_found ?? 0) || 0),
            lastPage:
              Number(payload?.physical_page ?? prev.lastPage ?? 0) ||
              prev.lastPage,
            lastAssignedStart:
              Number(
                payload?.assigned_start_offset ?? prev.lastAssignedStart ?? 0
              ) || prev.lastAssignedStart,
            lastAssignedEnd:
              Number(
                payload?.assigned_end_offset ?? prev.lastAssignedEnd ?? 0
              ) || prev.lastAssignedEnd,
          }));
          // trigger subtle pulse animation
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
          // stage_type may be serialized as nested enum object; normalize to string
          const stageTypeRaw = (payload as any)?.stage_type;
          const t = typeof stageTypeRaw === "string"
            ? stageTypeRaw.toLowerCase()
            : stageTypeRaw && typeof stageTypeRaw === "object"
            ? (Object.keys(stageTypeRaw)[0] || "").toLowerCase()
            : "";
          if (t.includes("validation")) {
            const total = Number(payload?.items_count ?? 0) || 0;
            setValidationStats((prev) => ({
              ...prev,
              started: true,
              completed: false,
              targetPages: total || prev.targetPages,
            }));
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
    <SyncPanel syncLive={syncLive} />
    <SessionStatusCard isRunning={isRunning} statusMessage={statusMessage} batchInfo={batchInfo} />
        <StageStatsPanels
          crawlingRange={crawlingRange}
          preflight={preflight}
          pageStats={pageStats}
          detailStats={detailStats}
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
            addLog={addLog}
            tauriApi={tauriApi}
        />

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
        />

        {/* Stage1/Stage2 Runtime Monitor */}
        <div
          class={`grid grid-cols-1 md:grid-cols-2 gap-4 mb-8 ${
            stage1Pulse() ? "pulse-once" : ""
          }`}
        >
          <div
            class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${
              stage1Pulse() ? "pulse-once" : ""
            }`}
          >
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">
                Stage 1: 제품 목록 수집
              </h3>
              <span class="text-xs text-gray-500">
                {(() => {
                  const cr = crawlingRange();
                  const pre = preflight();
                  const siteTotal = Number(pre?.site_total_pages ?? 0) || 0;
                  const planned = (cr?.crawling_info?.pages_to_crawl ??
                    ((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1 || 0)) as number;
                  const batchEst = pageStats().totalEstimated || 0;
                  // Prefer planned total pages when available; fallback to batch-estimated, then site total.
                  const est = planned > 0 ? planned : (batchEst > 0 ? batchEst : siteTotal);
                  return est > 0 ? `예상 ${est}p` : "";
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">
                  <CountUp value={pageStats().started} />
                </div>
                <div class="text-xs text-gray-600">시작</div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">
                  <CountUp value={pageStats().completed} />
                </div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">
                  <CountUp value={pageStats().inflight} />
                </div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">
                  <CountUp value={pageStats().failed} />
                </div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600">
                  <CountUp value={pageStats().retried} />
                </div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div
                class="progress-fill rounded-full"
                style={{
                  width: `${(() => {
                    const cr = crawlingRange();
                    const pre = preflight();
                    const siteTotal = Number(pre?.site_total_pages ?? 0) || 0;
                    const planned = (cr?.crawling_info?.pages_to_crawl ??
                      ((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1 || 0)) as number;
                    const batchEst = pageStats().totalEstimated || 0;
                    const denom = planned > 0 ? planned : (batchEst > 0 ? batchEst : siteTotal);
                    return denom > 0
                      ? Math.min(100, (pageStats().completed / denom) * 100)
                      : 0;
                  })()}%`,
                }}
              ></div>
            </div>
          </div>

          <div
            class={`bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 ${
              stage2Pulse() ? "pulse-once" : ""
            }`}
          >
            <div class="flex items-center justify-between mb-2">
              <h3 class="text-md font-semibold text-gray-800">
                Stage 2: 세부 정보 수집
              </h3>
              <Show when={!!downshiftInfo()}>
                <span
                  class="text-[10px] px-2 py-1 bg-yellow-100 text-yellow-700 rounded shake-x"
                  title={downshiftInfo()?.reason || ""}
                >
                  ↓ 제한 {downshiftInfo()?.newLimit ?? "-"}
                </span>
              </Show>
              <span class="text-xs text-gray-500">
                {(() => {
                  const cr = crawlingRange();
                  const plannedPages = (cr?.crawling_info?.pages_to_crawl ??
                    ((cr?.range?.[0] ?? 0) - (cr?.range?.[1] ?? 0) + 1 || 0)) as number;
                  const plannedProducts = plannedPages > 0 ? plannedPages * 12 : 0;
                  const est = (cr?.crawling_info?.estimated_new_products ?? 0) as number;
                  const observed = Math.max(detailStats().started || 0, detailStats().completed || 0);
                  // Prefer planned products when available; else prefer observed; else backend estimate
                  const val = plannedProducts > 0 ? plannedProducts : (observed > 0 ? observed : (est > 0 ? est : 0));
                  return val > 0 ? `예상 ${val}` : "";
                })()}
              </span>
            </div>
            <div class="grid grid-cols-5 gap-2 text-center">
              <div class="bg-blue-50 rounded p-2">
                <div class="text-xl font-bold text-blue-600">
                  <CountUp value={detailStats().started} />
                </div>
                <div class="text-xs text-gray-600">
                  시작
                  <Show when={(detailStats().completed + detailStats().failed) > (detailStats().started || 0)}>
                    <span class="ml-1 inline-block text-[10px] px-1 py-0.5 rounded bg-indigo-100 text-indigo-700" title="completed+failed 로 추론한 값이 시작 수보다 큼">
                      추론 {(detailStats().completed + detailStats().failed)}
                    </span>
                  </Show>
                </div>
              </div>
              <div class="bg-emerald-50 rounded p-2">
                <div class="text-xl font-bold text-emerald-600">
                  <CountUp value={detailStats().completed} />
                </div>
                <div class="text-xs text-gray-600">완료</div>
              </div>
              <div class="bg-amber-50 rounded p-2">
                <div class="text-xl font-bold text-amber-600">
                  <CountUp value={detailStats().inflight} />
                </div>
                <div class="text-xs text-gray-600">진행중</div>
              </div>
              <div class="bg-rose-50 rounded p-2">
                <div class="text-xl font-bold text-rose-600">
                  <CountUp value={detailStats().failed} />
                </div>
                <div class="text-xs text-gray-600">실패</div>
              </div>
              <div class="bg-violet-50 rounded p-2">
                <div class="text-xl font-bold text-violet-600">
                  <CountUp value={detailStats().retried} />
                </div>
                <div class="text-xs text-gray-600">재시도</div>
              </div>
            </div>
            <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
              <div
                class="progress-fill rounded-full"
                style={{
                  width: `${(() => {
                    const est = (crawlingRange()?.crawling_info?.estimated_new_products ?? 0) as number;
                    const observed = Math.max(detailStats().started || 0, detailStats().completed || 0);
                    // Prefer observed when available; fallback to estimate only if no observed yet
                    const denom = observed > 0 ? observed : (est > 0 ? est : 0);
                    return denom > 0
                      ? Math.min(100, (detailStats().completed / denom) * 100)
                      : 0;
                  })()}%`,
                }}
              ></div>
            </div>
          </div>
        </div>

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
