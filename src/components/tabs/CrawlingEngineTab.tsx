/**
 * CrawlingEngineTab - Advanced Crawling Engine 통합 탭
 * Phase 4A의 5단계 파이프라인을 UI에서 제어하고 모니터링
 */

import { Component, createSignal, createEffect, onMount, onCleanup, Show, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../../services/tauri-api';
import type { 
  CrawlingProgressInfo, 
  SiteStatusInfo, 
  ProductInfo, 
  CrawlingSession, 
  DatabaseStats,
  ApiResponse,
  CrawlingRangeRequest,
  CrawlingRangeResponse
} from '../../types/advanced-engine';
// Session animation/status panel (Actor system shared component)
// Removed Actor Session Status panel from this tab
import { useActorVisualizationStream } from '../../hooks/useActorVisualizationStream';

export const CrawlingEngineTab: Component = () => {
  // 기본 설정값을 반환하는 더미 함수 (백엔드가 설정 파일을 직접 읽음)
  // const userConfig = () => ({
  //   user: {
  //     crawling: {
  //       page_range_limit: 6,
  //       crawling_mode: 'incremental',
  //       auto_adjust_range: true,
  //       workers: {
  //         list_page_max_concurrent: 5,
  //         product_detail_max_concurrent: 10
  //       },
  //       product_list_retry_count: 2,
  //       product_detail_retry_count: 2,
  //       error_threshold_percent: 10,
  //       gap_detection_threshold: 5,
  //       binary_search_max_depth: 10,
  //       enable_data_validation: true,
  //       auto_add_to_local_db: true
  //     },
  //     batch: {
  //       batch_size: 12,
  //       batch_delay_ms: 1000,
  //       enable_batch_processing: true
  //     },
  //     max_concurrent_requests: 3,
  //     request_delay_ms: 1000
  //   },
  //   advanced: {
  //     request_timeout_seconds: 30,
  //     retry_delay_ms: 2000
  //   }
  // });

  // 더미 함수 - 실제로는 백엔드가 설정 파일을 자동으로 읽음
  // const loadUserConfig = () => {
  //   addLog('ℹ️ 백엔드가 설정 파일을 자동으로 읽어 사용합니다');
  // };
  
  // const [showAdvancedSettings, setShowAdvancedSettings] = createSignal(false);
  const [siteStatus, setSiteStatus] = createSignal<SiteStatusInfo | null>(null);
  const [progress, setProgress] = createSignal<CrawlingProgressInfo | null>(null);
  const [recentProducts, setRecentProducts] = createSignal<ProductInfo[]>([]);
  const [logs, setLogs] = createSignal<string[]>([]);
  const [isRunning, setIsRunning] = createSignal(false);
  const [isPaused, setIsPaused] = createSignal(false);
  const [currentSessionId, setCurrentSessionId] = createSignal<string | null>(null);
  const [dbStats, setDbStats] = createSignal<DatabaseStats | null>(null);
  const [crawlingRange, setCrawlingRange] = createSignal<CrawlingRangeResponse | null>(null);
  const [showSiteStatus, setShowSiteStatus] = createSignal(true);
  const [batchSize, setBatchSize] = createSignal(3); // 기본값 3, 실제 설정에서 로드됨
  // Optional overrides / inputs
  const [batchSizeOverride, setBatchSizeOverride] = createSignal<number | null>(null);
  const [repairBuffer, setRepairBuffer] = createSignal<number>(2);
  // Validation state
  const [isValidating, setIsValidating] = createSignal(false);
  const [validationStats, setValidationStats] = createSignal<{pages_scanned:number;products_checked:number;divergences:number;anomalies:number;duration_ms:number;session_id?:string}|null>(null);
  const [validationDetails, setValidationDetails] = createSignal<any|null>(null); // full summary
  const [validationEvents, setValidationEvents] = createSignal<any[]>([]);
  // Validation custom range as a single expression: e.g., "498-489" or "498~489" (oldest -> newer)
  const [valRangeExpr, setValRangeExpr] = createSignal<string>('');
  // Track if user manually edited (to avoid auto overwrite)
  let userTouchedValidationRange = false;
  // Remember the last resolved validation window and expression so Sync button can reuse it
  const [lastValidationRange, setLastValidationRange] = createSignal<{start:number; end:number} | null>(null);
  const [lastValidationExpr, setLastValidationExpr] = createSignal<string>('');

  // Sync state
  const [isSyncing, setIsSyncing] = createSignal(false);
  const [syncEvents, setSyncEvents] = createSignal<any[]>([]);
  const [syncStats, setSyncStats] = createSignal<{pages_processed:number;inserted:number;updated:number;skipped:number;failed:number;duration_ms?:number;session_id?:string}|null>(null);
  // Planned pages for progress (sum of ranges from actor-sync-started)
  const [plannedPages, setPlannedPages] = createSignal<number | null>(null);
  // Track whether a sync start event was seen recently (for fallback)
  let lastSyncStartSeq = 0;
  const [diagnosisResult, setDiagnosisResult] = createSignal<any | null>(null);
  const [autoReDiagnose, setAutoReDiagnose] = createSignal(false);
  // UI notices (e.g., range corrections)
  const [rangeNotice, setRangeNotice] = createSignal<string | null>(null);
  // Config: optional cap for validation/sync span
  const [validationPageLimit, setValidationPageLimit] = createSignal<number | null>(null);
  // Shared actor/concurrency events
  const { events: actorEvents } = useActorVisualizationStream(600);
  // Multi-range validation control
  const [isMultiRangeRun, setIsMultiRangeRun] = createSignal(false);
  // Persistence gate
  let settingsRestored = false;

  // === Sync stage cards (Stage 1, 3, 5) ===
  const [stage1State, setStage1State] = createSignal<{currentPage?: number; pagesStarted: number; mismatchWarnings: number; lastWarning?: string}>({ pagesStarted: 0, mismatchWarnings: 0 });
  const [stage3State, setStage3State] = createSignal<{detailWarnings: number; lastDetailWarning?: string}>({ detailWarnings: 0 });
  const [stage3Success, setStage3Success] = createSignal<{persisted:number; skipped:number}>({ persisted: 0, skipped: 0 });
  const [stage3LastStatus, setStage3LastStatus] = createSignal<string | undefined>(undefined);
  const [stage5StateExtra, setStage5StateExtra] = createSignal<{globalIdBackfillAffected?: number; lastDbWarning?: string; lastPerPage?: {page:number; placeholders:number; core:number; pid:number; prodId:number}}>( {} as any );

  // Log helper
  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-19), `[${timestamp}] ${message}`]);
  };

  // 설정 로드
  const loadConfig = async () => {
    try {
      const backendConfig = await tauriApi.getComprehensiveCrawlerConfig();
      setBatchSize(backendConfig.batch_size);
      addLog(`📋 설정 로드 완료: batch_size=${backendConfig.batch_size}`);
      // Load full app settings to discover optional validation_page_limit
      try {
        const appCfg: any = await invoke('get_app_settings');
        const limit = appCfg?.user?.crawling?.validation_page_limit;
        if (typeof limit === 'number' && limit > 0) {
          setValidationPageLimit(limit);
          addLog(`📋 validation_page_limit 감지됨: ${limit} 페이지`);
        }
      } catch (e) {
        console.warn('get_app_settings failed (non-fatal):', e);
      }
    } catch (error) {
      addLog(`❌ 설정 로드 실패: ${error}`);
    }
  };

  // 크롤링 범위 계산
  const calculateCrawlingRange = async () => {
    try {
      addLog('🔍 크롤링 범위 계산 함수 시작...');
      
      const siteInfo = siteStatus();
      if (!siteInfo) {
        addLog('❌ 크롤링 범위 계산 실패: 사이트 상태 정보 없음');
        console.warn('siteStatus is null:', siteInfo);
        return;
      }

      addLog(`🔍 사이트 정보 확인됨: ${siteInfo.total_pages}페이지, 마지막 페이지 ${siteInfo.products_on_last_page}개 제품`);

      const request: CrawlingRangeRequest = {
        total_pages_on_site: siteInfo.total_pages,
        products_on_last_page: siteInfo.products_on_last_page
      };

      addLog(`🔍 크롤링 범위 계산 중... (총 ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품)`);
      
      console.log('Calling calculate_crawling_range with request:', request);
      
      const response = await invoke<CrawlingRangeResponse>('calculate_crawling_range', { request });
      
      console.log('Response from calculate_crawling_range:', response);
      
      if (response?.success && response?.range) {
        setCrawlingRange(response);
        const [start_page, end_page] = response.range;
        const total_pages_to_crawl = start_page - end_page + 1;
        addLog(`✅ 계산된 크롤링 범위: ${start_page} → ${end_page} (${total_pages_to_crawl} 페이지)`);
        console.log('Successfully set crawling range:', response);
      } else {
        addLog(`❌ 크롤링 범위 계산 실패: ${response?.message || '알 수 없는 오류'}`);
        console.error('Failed to calculate crawling range:', response);
      }
    } catch (error) {
      addLog(`❌ 크롤링 범위 계산 오류: ${error}`);
      console.error('크롤링 범위 계산 오류:', error);
    }
  };

  // Initialize and load data
  onMount(async () => {
    addLog('🎯 Advanced Crawling Engine 탭 로드됨');
    // Restore persisted UI settings
    try {
      const rbRaw = localStorage.getItem('mc_repair_buffer');
      if (rbRaw !== null) {
        const v = parseInt(rbRaw, 10);
        if (!Number.isNaN(v) && v >= 0) setRepairBuffer(v);
      }
      const boRaw = localStorage.getItem('mc_batch_override');
      if (boRaw !== null) {
        if (boRaw === '') {
          setBatchSizeOverride(null);
        } else {
          const v = parseInt(boRaw, 10);
          setBatchSizeOverride(Number.isNaN(v) ? null : Math.max(1, v));
        }
      }
      const arRaw = localStorage.getItem('mc_auto_rediag');
      if (arRaw !== null) setAutoReDiagnose(arRaw === 'true');
    } catch (e) {
      console.warn('Setting restore failed (non-fatal):', e);
    } finally {
      settingsRestored = true;
    }
    
    await checkSiteStatus(); // 이 함수 내에서 이미 calculateCrawlingRange() 호출됨
    await loadRecentProducts();
    await loadDatabaseStats();
    
    // Tauri 이벤트 리스너 등록
    const unlistenProgress = await listen('crawling-progress', (event) => {
      const progressData = event.payload as CrawlingProgressInfo;
      setProgress(progressData);
      addLog(`🔄 진행률: ${progressData.progress_percentage.toFixed(1)}% - ${progressData.current_message}`);
    });
    
    const unlistenCompleted = await listen('crawling-completed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setIsPaused(false);
      setCurrentSessionId(null);
      addLog(`✅ 크롤링 완료: 세션 ${sessionData.session_id}`);
      loadRecentProducts(); // 완료 후 제품 목록 새로고침
    });
    
    const unlistenFailed = await listen('crawling-failed', (event) => {
      const sessionData = event.payload as CrawlingSession;
      setIsRunning(false);
      setIsPaused(false);
      setCurrentSessionId(null);
      addLog(`❌ 크롤링 실패: 세션 ${sessionData.session_id}`);
    });

    // Validation event listeners
  const vStarted = await listen('actor-validation-started', (e) => {
      const p = e.payload as any;
      setIsValidating(true);
      setValidationStats(null);
      setValidationEvents(evts => [...evts, p]);
      addLog(`🧪 Validation 시작: session=${p.session_id} scan_pages=${p.scan_pages}`);
    });
    const vPage = await listen('actor-validation-page-scanned', (e) => {
      const p = e.payload as any;
      setValidationEvents(evts => [...evts.slice(-199), p]);
      addLog(`🧪 페이지 스캔: physical=${p.physical_page} products=${p.products_found}`);
    });
    const vDiv = await listen('actor-validation-divergence', (e) => {
      const p = e.payload as any;
      setValidationEvents(evts => [...evts.slice(-199), p]);
      addLog(`⚠️ 불일치 발견: ${p.kind} (${p.detail?.substring(0,80)})`);
    });
    const vAnom = await listen('actor-validation-anomaly', (e) => {
      const p = e.payload as any;
      setValidationEvents(evts => [...evts.slice(-199), p]);
      addLog(`⚠️ 이상 징후: ${p.code}`);
    });
    const vDone = await listen('actor-validation-completed', (e) => {
      const p = e.payload as any;
      setIsValidating(false);
      // If multi-range run is active, don't override aggregated stats here
      if (!isMultiRangeRun()) {
        setValidationStats({
          pages_scanned: p.pages_scanned,
          products_checked: p.products_checked,
          divergences: p.divergences,
          anomalies: p.anomalies,
          duration_ms: p.duration_ms,
          session_id: p.session_id
        });
      }
      setValidationDetails(p); // store full enriched payload
      setValidationEvents(evts => [...evts.slice(-199), p]);
      addLog(`🧪 Validation 완료: pages=${p.pages_scanned} divergences=${p.divergences} anomalies=${p.anomalies}`);
    });

    // Sync event listeners
    const sStarted = await listen('actor-sync-started', (e) => {
      const p = e.payload as any;
      setIsSyncing(true);
      setSyncStats({ pages_processed: 0, inserted: 0, updated: 0, skipped: 0, failed: 0, session_id: p.session_id });
      setSyncEvents(evts => [...evts.slice(-199), p]);
      addLog(`🔄 Sync 시작: session=${p.session_id} ranges=${JSON.stringify(p.ranges)}`);
      lastSyncStartSeq++;
      // Derive total planned pages from ranges
      try {
        const ranges: Array<[number, number]> = Array.isArray(p.ranges) ? p.ranges : [];
        const total = ranges.reduce((acc, [start, end]) => acc + Math.max(0, (start - end + 1)), 0);
        setPlannedPages(total > 0 ? total : null);
      } catch { setPlannedPages(null); }
      // Reset stage counters
      setStage1State({ pagesStarted: 0, mismatchWarnings: 0 });
      setStage3State({ detailWarnings: 0 });
      setStage3Success({ persisted: 0, skipped: 0 });
      setStage3LastStatus(undefined);
      setStage5StateExtra({});
    });
    const sPage = await listen('actor-sync-page-started', (e) => {
      const p = e.payload as any;
      setSyncEvents(evts => [...evts.slice(-199), p]);
      addLog(`🔄 Sync 페이지 시작: physical=${p.physical_page}`);
      setStage1State(prev => ({
        ...prev,
        currentPage: p.physical_page,
        pagesStarted: (prev.pagesStarted || 0) + 1,
      }));
    });
    const sProg = await listen('actor-sync-upsert-progress', (e) => {
      const p = e.payload as any;
      setSyncEvents(evts => [...evts.slice(-199), p]);
    });
    const sPageDone = await listen('actor-sync-page-completed', (e) => {
      const p = e.payload as any;
      setSyncEvents(evts => [...evts.slice(-199), p]);
      // Update aggregate stats incrementally
      setSyncStats(prev => prev ? {
        ...prev,
        pages_processed: (prev.pages_processed || 0) + 1,
        inserted: (prev.inserted || 0) + (p.inserted || 0),
        updated: (prev.updated || 0) + (p.updated || 0),
        skipped: (prev.skipped || 0) + (p.skipped || 0),
        failed: (prev.failed || 0) + (p.failed || 0),
      } : prev);
    });
    const sWarn = await listen('actor-sync-warning', (e) => {
      const p = e.payload as any;
      setSyncEvents(evts => [...evts.slice(-199), p]);
      addLog(`⚠️ Sync 경고: ${p.code} ${p.detail}`);
  try {
        const code: string = p.code || '';
        // Stage 1: 페이지 접근/리스트 단계 관련 경고 매핑
        if (code === 'count_mismatch' || code === 'page_incomplete_after_retries' || code === 'tx_begin_failed') {
          setStage1State(prev => ({
            ...prev,
            mismatchWarnings: (prev.mismatchWarnings || 0) + 1,
            lastWarning: `${code}: ${p.detail || ''}`.slice(0, 160),
          }));
        }
        // Stage 3: 상세 수집 관련 경고 매핑
        if (code.startsWith('details_')) {
          setStage3State(prev => ({
            ...prev,
            detailWarnings: (prev.detailWarnings || 0) + 1,
            lastDetailWarning: `${code}: ${p.detail || ''}`.slice(0, 160),
          }));
        }
        // Stage 5: DB-only/글로벌 스윕 등 저장단계 관련 경고 매핑
        if (code === 'global_products_id_backfill_sweep') {
          const m = /affected_rows=(\d+)/.exec(String(p.detail || ''));
          const affected = m ? parseInt(m[1] || '0', 10) : undefined;
          setStage5StateExtra(prev => ({ ...prev, globalIdBackfillAffected: affected }));
        }
        if (code === 'db_only_backfill_metrics') {
          try {
            const obj = JSON.parse(String(p.detail || '{}')) as any;
            const page = Number(obj.page || 0);
            const pid = Number(obj.pid || 0);
            const placeholders = Number(obj.placeholders || 0);
            const core = Number(obj.product_core_backfilled || obj.core || 0);
            const prodId = Number(obj.products_id_backfilled || obj.prodId || 0);
            setStage5StateExtra(prev => ({ ...prev, lastPerPage: { page, pid, placeholders, core, prodId } }));
          } catch {}
        }
        if (code.endsWith('_failed') || code.startsWith('db_only_')) {
          setStage5StateExtra(prev => ({ ...prev, lastDbWarning: `${code}: ${p.detail || ''}`.slice(0, 160) }));
        }
      } catch {}
    });
    // Product lifecycle events for details success/skip counters
    const sPlc = await listen('actor-product-lifecycle', (e) => {
      const p = e.payload as any;
      const status = String(p.status || '');
      if (status === 'details_persisted') {
        setStage3Success(prev => ({ ...prev, persisted: (prev.persisted || 0) + 1 }));
        setStage3LastStatus(`${status}: ${p.product_ref || ''}`.slice(0, 160));
      } else if (status === 'details_skipped_exists') {
        setStage3Success(prev => ({ ...prev, skipped: (prev.skipped || 0) + 1 }));
        setStage3LastStatus(`${status}: ${p.product_ref || ''}`.slice(0, 160));
      }
    });
    const sDone = await listen('actor-sync-completed', (e) => {
      const p = e.payload as any;
      setIsSyncing(false);
      setSyncStats({
        pages_processed: p.pages_processed,
        inserted: p.inserted,
        updated: p.updated,
        skipped: p.skipped,
        failed: p.failed,
        duration_ms: p.duration_ms,
        session_id: p.session_id,
      });
      setSyncEvents(evts => [...evts.slice(-199), p]);
      addLog(`🔄 Sync 완료: pages=${p.pages_processed} ins=${p.inserted} upd=${p.updated} skip=${p.skipped} fail=${p.failed}`);
      // Optional auto re-diagnosis
      if (autoReDiagnose()) {
        addLog('🧪 Sync 완료 후 재진단 실행...');
        tauriApi.diagnoseAndRepairData(false)
          .then((res) => {
            setDiagnosisResult(res);
            addLog(`🧪 재진단 결과: ${JSON.stringify(res)}`);
          })
          .catch((e) => addLog(`❌ 재진단 실패: ${e}`));
      }
    });
    
    // 컴포넌트 언마운트 시 리스너 해제
    onCleanup(() => {
      unlistenProgress();
      unlistenCompleted();
      unlistenFailed();
      vStarted(); vPage(); vDiv(); vAnom(); vDone();
  sStarted(); sPage(); sProg(); sPageDone(); sWarn(); sPlc(); sDone();
    });
  });

  // Persist settings when changed (after initial restore)
  createEffect(() => {
    if (!settingsRestored) return;
    const v = repairBuffer();
    try { localStorage.setItem('mc_repair_buffer', String(Math.max(0, v))); } catch {}
  });
  createEffect(() => {
    if (!settingsRestored) return;
    const v = batchSizeOverride();
    try {
      if (v == null || Number.isNaN(v)) localStorage.setItem('mc_batch_override', '');
      else localStorage.setItem('mc_batch_override', String(Math.max(1, v)));
    } catch {}
  });
  createEffect(() => {
    if (!settingsRestored) return;
    const v = autoReDiagnose();
    try { localStorage.setItem('mc_auto_rediag', String(!!v)); } catch {}
  });

  // Detect user edits
  const onUserEditRangeExpr = (v: string) => { setValRangeExpr(v); userTouchedValidationRange = true; };

  // Small parser for single range expression like "498-489" or "498~489"
  const parseRangeExpr = (expr: string): {start:number; end:number} | null => {
    const s = (expr || '').trim();
    if (!s) return null;
    // Normalize whitespace and unicode separators: en/em dashes, minus, fullwidth, wave variants
    const norm0 = s.replace(/\s+/g, '');
    const norm = norm0
      .replace(/[–—−﹣－]/g, '-')   // dash variants -> '-'
      .replace(/[~〜～]/g, '~');   // tilde variants -> '~'
    const sep = norm.includes('~') ? '~' : '-';
    const parts = norm.split(sep);
    if (parts.length !== 2) return null;
    const a = parseInt(parts[0], 10);
    const b = parseInt(parts[1], 10);
    if (Number.isNaN(a) || Number.isNaN(b)) return null;
    // Oldest (larger) should be first
    const start = Math.max(a, b);
    const end = Math.min(a, b);
    return { start, end };
  };

  // Parse multi-range expression like: "498-492,489,487-485"
  const parseMultiRangeExpr = (expr: string): Array<{start:number; end:number}> => {
    const raw = (expr || '').trim();
    if (!raw) return [];
    const tokens = raw.split(',').map(t => t.trim()).filter(Boolean);
    const out: Array<{start:number; end:number}> = [];
    for (const t of tokens) {
      const tt = t
        .replace(/\s+/g, '')
        .replace(/[–—−﹣－]/g, '-')
        .replace(/[~〜～]/g, '~');
      const single = parseRangeExpr(tt) || (/^\d+$/.test(tt) ? { start: parseInt(tt, 10), end: parseInt(tt, 10) } : null);
      if (single) out.push(single);
    }
    return out;
  };

  // Clamp ranges to site bounds (1..total_pages) and serialize back to expr
  const clampRangesToSite = (expr: string, siteTotalPages?: number): {
    expr: string;
    changed: boolean;
    details: Array<{ start: number; end: number; before: { start: number; end: number } }>
  } => {
    const total = typeof siteTotalPages === 'number' && siteTotalPages > 0
      ? siteTotalPages
      : siteStatus()?.total_pages;
    const ranges = parseMultiRangeExpr(expr);
  if (!ranges.length) return { expr, changed: false, details: [] };
    let changed = false;
    const clamped = ranges.map(r => {
      const before = { ...r };
      let s = r.start;
      let e = r.end;
      if (typeof total === 'number') {
        if (s > total) { s = total; changed = true; }
        if (e > total) { e = total; changed = true; }
      }
      if (s < 1) { s = 1; changed = true; }
      if (e < 1) { e = 1; changed = true; }
      // Ensure start >= end after clamping
      if (s < e) { const t = s; s = e; e = t; changed = true; }
      // Normalize to inclusive range string
      return { start: s, end: e, before } as { start:number; end:number; before:{start:number;end:number} };
    });
    const exprClamped = clamped.map(r => r.start === r.end ? `${r.start}` : `${r.start}-${r.end}`).join(',');
  return { expr: exprClamped, changed, details: clamped };
  };

  // Clamp ranges to configured validationPageLimit (max span per contiguous range)
  const clampRangesToLimit = (expr: string, limit?: number | null): {
    expr: string;
    changed: boolean;
    details: Array<{ start: number; end: number; before: { start: number; end: number } }>
  } => {
    const ranges = parseMultiRangeExpr(expr);
    if (!ranges.length || !limit || limit <= 0) return { expr, changed: false, details: [] };
    let changed = false;
    const adjusted = ranges.map(r => {
      const before = { ...r };
      const span = Math.max(1, r.start - r.end + 1);
      if (span > limit) {
        // new_end moves towards start to reduce span, but never goes beyond current end (stay within user's newer bound)
        const computedEnd = r.start - limit + 1;
        const newEnd = Math.max(r.end, computedEnd);
        changed = true;
        return { start: r.start, end: newEnd, before };
      }
      return { start: r.start, end: r.end, before };
    });
    const exprAdjusted = adjusted.map(r => r.start === r.end ? `${r.start}` : `${r.start}-${r.end}`).join(',');
    return { expr: exprAdjusted, changed, details: adjusted };
  };

  // Auto-populate default validation range when site status & crawling range become available
  createEffect(() => {
    const site = siteStatus();
    const cr = crawlingRange();
    if (!site || !cr?.range || userTouchedValidationRange) return;
    if (valRangeExpr() !== '') return; // already filled (e.g., restored)
    const totalPages = site.total_pages;
    const crawlStart = cr.range[0];
    let endDefault = crawlStart + 1; // just before crawl window
    if (endDefault > totalPages) endDefault = totalPages;
    if (endDefault < 1) endDefault = 1;
    const startDefault = totalPages;
    if (startDefault >= endDefault) {
      setValRangeExpr(`${startDefault}-${endDefault}`);
      addLog(`🧪 기본 Validation 범위 자동 설정: physical ${startDefault} → ${endDefault}`);
    }
  });

  const loadDatabaseStats = async () => {
    try {
      const response = await invoke<ApiResponse<DatabaseStats>>('get_database_stats');
      
      if (response.success && response.data) {
        setDbStats(response.data);
        addLog(`📊 데이터베이스: 총 ${response.data.total_products}개 제품`);
      } else {
        addLog(`❌ DB 통계 로드 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ DB 통계 로드 오류: ${error}`);
    }
  };

  // API functions
  const checkSiteStatus = async () => {
    try {
      addLog('🌐 사이트 상태 확인 중...');
      const response = await invoke<ApiResponse<SiteStatusInfo>>('check_advanced_site_status');
      
      if (response.success && response.data) {
        setSiteStatus(response.data);
        addLog(`✅ 사이트 상태: ${response.data.total_pages}페이지, ${response.data.estimated_total_products}개 제품 예상`);
        
        // 사이트 상태 업데이트 후 크롤링 범위 재계산
        addLog('🔍 사이트 상태 확인 완료, 크롤링 범위 계산 시작...');
        console.log('About to call calculateCrawlingRange from checkSiteStatus');
        await calculateCrawlingRange();
      } else {
        addLog(`❌ 사이트 상태 확인 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 사이트 상태 확인 오류: ${error}`);
    }
  };

  const loadRecentProducts = async () => {
    try {
      addLog('📋 최근 제품 로드 중...');
      const response = await invoke<ApiResponse<{ products: ProductInfo[] }>>('get_recent_products', { page: 1, limit: 10 });
      
      if (response.success && response.data) {
        setRecentProducts(response.data.products);
        addLog(`📋 최근 제품 ${response.data.products.length}개 로드됨`);
      } else {
        addLog(`❌ 제품 로드 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 제품 로드 오류: ${error}`);
    }
  };

  const startCrawling = async () => {
    if (isRunning()) return;
    
    try {
      setIsRunning(true);
      
      addLog(`🚀 Actor System Crawling 시작 - 실시간 이벤트 모니터링`);
      
      // ✅ Actor 시스템 방식: 실시간 이벤트가 있는 크롤링
  const sessionId = await invoke<string>('start_crawling_session');
  setCurrentSessionId(sessionId);
  addLog(`✅ Actor 시스템 크롤링 세션 시작: ${sessionId}`);
  // Notify session status panel to refresh immediately
  window.dispatchEvent(new CustomEvent('actorSessionRefresh', { detail: { sessionId } }));
  setTimeout(() => window.dispatchEvent(new CustomEvent('actorSessionRefresh', { detail: { sessionId } })), 800);
      
    } catch (error) {
      setIsRunning(false);
      addLog(`❌ Actor 시스템 크롤링 시작 실패: ${error}`);
      console.error('Actor 시스템 크롤링 시작 오류:', error);
    }
  };

  // (removed) 가짜 Actor 시스템 크롤링 핸들러는 UI에서 제외되었습니다

  // 진짜 Actor 시스템 설정 기반 크롤링
  const startRealActorSystemWithCalculatedRange = async () => {
    if (isRunning()) return;
    
    setIsRunning(true);
    addLog('🎭 진짜 Actor 시스템 크롤링 시작 (CrawlingPlanner 설정 기반)');

    try {
      // 사이트 상태 정보가 필요하므로 먼저 확인
      const siteInfo = siteStatus();
      if (!siteInfo) {
        addLog('❌ 사이트 상태 정보 없음. 먼저 사이트 상태를 확인해주세요.');
        setIsRunning(false);
        return;
      }

      // 배치 플랜을 계산해서 설정값을 가져옵니다
      const request: CrawlingRangeRequest = {
        total_pages_on_site: siteInfo.total_pages,
        products_on_last_page: siteInfo.products_on_last_page
      };
      
      addLog(`🔍 배치 플랜 계산 중... (총 ${request.total_pages_on_site}페이지, 마지막 페이지 ${request.products_on_last_page}개 제품)`);
      
      const crawlingRange = await invoke('calculate_crawling_range', { request }) as CrawlingRangeResponse;
      const configBasedBatchSize = crawlingRange?.batch_plan?.batch_size || 9; // 기본값 9
      
      addLog(`📋 설정 기반 배치 크기: ${configBasedBatchSize}`);
      
      const result: any = await invoke('start_actor_system_crawling', {
        request: {
          // 🧠 CrawlingPlanner 설정을 기반으로 한 값들 사용
          start_page: 0,     // By Design: 프론트엔드에서 범위 지정하지 않음
          end_page: 0,       // By Design: 프론트엔드에서 범위 지정하지 않음  
          concurrency: 64,
          batch_size: configBasedBatchSize, // 설정파일에서 읽은 값 사용
          delay_ms: 100
        }
      });
      addLog(`✅ 진짜 Actor 시스템 크롤링 세션 시작: ${JSON.stringify(result)}`);
      addLog('🎭 진짜 Actor 시스템이 활성화되었습니다. CrawlingPlanner 설정 기반으로 SessionActor가 실행됩니다.');
      if (result?.session_id) {
        window.dispatchEvent(new CustomEvent('actorSessionRefresh', { detail: { sessionId: result.session_id } }));
        setTimeout(() => window.dispatchEvent(new CustomEvent('actorSessionRefresh', { detail: { sessionId: result.session_id } })), 800);
      } else {
        window.dispatchEvent(new CustomEvent('actorSessionRefresh'));
        setTimeout(() => window.dispatchEvent(new CustomEvent('actorSessionRefresh')), 800);
      }
      
    } catch (error) {
      console.error('진짜 Actor 시스템 크롤링 시작 실패:', error);
      addLog(`❌ 진짜 Actor 시스템 크롤링 시작 실패: ${error}`);
      setIsRunning(false);
    }
  };

  const pauseCrawling = async () => {
    if (!currentSessionId()) {
      addLog('❌ 활성 세션이 없습니다');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('pause_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsPaused(true);
        addLog(`⏸️ 크롤링 일시 중지: ${currentSessionId()}`);
      } else {
        addLog(`❌ 일시 중지 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 일시 중지 오류: ${error}`);
    }
  };

  const resumeCrawling = async () => {
    if (!currentSessionId()) {
      addLog('❌ 활성 세션이 없습니다');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('resume_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsPaused(false);
        addLog(`▶️ 크롤링 재개: ${currentSessionId()}`);
      } else {
        addLog(`❌ 재개 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 재개 오류: ${error}`);
    }
  };

  const stopCrawling = async () => {
    if (!currentSessionId()) {
      setIsRunning(false);
      setIsPaused(false);
      addLog('⏹️ 크롤링 중단됨');
      return;
    }

    try {
      const response = await invoke<ApiResponse<any>>('stop_crawling_session', {
        session_id: currentSessionId()
      });
      
      if (response.success) {
        setIsRunning(false);
        setIsPaused(false);
        setCurrentSessionId(null);
        addLog(`⏹️ 크롤링 완전 중단: ${currentSessionId()}`);
      } else {
        addLog(`❌ 중단 실패: ${response.error?.message || 'Unknown error'}`);
      }
    } catch (error) {
      addLog(`❌ 중단 오류: ${error}`);
    }
  };

  // Validation invocation
  const runValidation = async () => {
    if (isValidating()) {
      addLog('⏳ Validation 이미 실행 중');
      return;
    }
    setIsValidating(true);
    addLog('🧪 Validation 요청 중...');
    try {
      const expr = valRangeExpr();
      const ranges = parseMultiRangeExpr(expr);
      let aggregated = { pages_scanned: 0, products_checked: 0, divergences: 0, anomalies: 0, duration_ms: 0 };
      if (ranges.length >= 1) {
        // Multi-range (or single parsed as list size 1)
        setLastValidationExpr(expr);
        setIsMultiRangeRun(ranges.length > 1);
        for (let i = 0; i < ranges.length; i++) {
          let r = ranges[i];
          let span = Math.max(1, r.start - r.end + 1);
          // Apply optional FE-side cap to match backend behavior and inform user
          const vLimit = validationPageLimit();
          if (vLimit && vLimit > 0 && span > vLimit) {
            const computedEnd = r.start - vLimit + 1;
            const newEnd = Math.max(r.end, computedEnd);
            addLog(`ℹ️ 설정된 최대 범위(${vLimit}p)를 초과하여 Validation 범위를 보정합니다: ${r.start}-${r.end} → ${r.start}-${newEnd}`);
            setRangeNotice(`최대 Validation 범위 ${vLimit}페이지를 초과하여 보정했습니다: ${r.start}-${r.end} → ${r.start}-${newEnd}`);
            r = { start: r.start, end: newEnd };
            span = Math.max(1, r.start - r.end + 1);
            // Reflect corrected expr for UX transparency
            if (ranges.length === 1) setValRangeExpr(`${r.start}-${r.end}`);
          }
          // Normalize expr fallback per-range if empty
          const exprForThis = (expr && expr.trim()) ? expr : `${r.start}-${r.end}`;
          // Send both snake_case (Rust) and camelCase (defensive) keys
          const args: any = {
            start_physical_page: r.start,
            end_physical_page: r.end,
            scan_pages: span,
            ranges_expr: exprForThis,
            // defensive aliases (ignored by Rust, helpful if any layer maps keys unexpectedly)
            startPhysicalPage: r.start,
            endPhysicalPage: r.end,
            scanPages: span,
            rangesExpr: exprForThis,
          };
          // FE guard: enforce presence before invoking
          if (
            !("start_physical_page" in args) ||
            !("end_physical_page" in args) ||
            !("scan_pages" in args)
          ) {
            console.error('[validation][guard] Missing required args', args);
            addLog('❌ Validation 호출 차단: 필수 인자 누락(start/end/scan_pages)');
            setIsValidating(false);
            return;
          }
          addLog(`🧪 Validation 실행 (${i+1}/${ranges.length}): physical ${r.start} → ${r.end} (scan_pages=${span})`);
          console.info('[validation] expr:', exprForThis, 'ranges:', ranges, 'args:', args);
          const summary = await invoke<any>('start_validation', args);
          if (summary) {
            aggregated.pages_scanned += summary.pages_scanned || 0;
            aggregated.products_checked += summary.products_checked || 0;
            aggregated.divergences += summary.divergences || 0;
            aggregated.anomalies += summary.anomalies || 0;
            aggregated.duration_ms += summary.duration_ms || 0;
            setValidationDetails(summary);
            if (typeof summary.resolved_start_oldest === 'number' && typeof summary.resolved_end_newest === 'number') {
              setLastValidationRange({ start: summary.resolved_start_oldest, end: summary.resolved_end_newest });
              setLastValidationExpr(`${summary.resolved_start_oldest}-${summary.resolved_end_newest}`);
              addLog(`🧪 적용 범위(백엔드 확정): ${summary.resolved_start_oldest} → ${summary.resolved_end_newest}`);
            }
            setValidationStats({
              pages_scanned: aggregated.pages_scanned,
              products_checked: aggregated.products_checked,
              divergences: aggregated.divergences,
              anomalies: aggregated.anomalies,
              duration_ms: aggregated.duration_ms,
              session_id: summary.session_id,
            });
          }
        }
        setIsMultiRangeRun(false);
        const last = ranges[ranges.length - 1];
        setLastValidationRange({ start: last.start, end: last.end });
        addLog(`🧪 다중 범위 Validation 완료: ${ranges.length}개 범위 합산`);
      } else {
        addLog('⚠️ 잘못된 범위 표현식입니다. 예시: "488-479" 또는 "488~479" 형식으로 입력해주세요.');
        setIsValidating(false);
        return;
      }
      } catch (err) {
        setIsValidating(false);
        addLog(`❌ Validation 실패: ${err}`);
        console.error('Validation error', err);
      }
  };

  // Trigger Sync using the last validation range (with robust fallbacks)
  const runSyncForLastValidationRange = async (dryRun = false) => {
    // Prefer current input, then last validation expr/range; if absent, fall back to recommended crawlingRange or a small tail window
    let rangesExpr = (valRangeExpr() && valRangeExpr().trim()) ? valRangeExpr().trim() : (lastValidationExpr() || '');
    if (!rangesExpr) {
      const rng = lastValidationRange();
      if (rng) {
        rangesExpr = `${rng.start}-${rng.end}`;
        addLog(`ℹ️ Validation 범위 재사용: ${rangesExpr}`);
      } else if (crawlingRange()?.range && Array.isArray(crawlingRange()!.range)) {
        const [s, e] = crawlingRange()!.range as [number, number];
        if (typeof s === 'number' && typeof e === 'number') {
          rangesExpr = `${s}-${e}`;
          addLog(`ℹ️ 권장 크롤링 범위 사용: ${rangesExpr}`);
        }
      }
    }
    // Final fallback: tail window from siteStatus
    if (!rangesExpr) {
      const site = siteStatus();
      if (site && typeof site.total_pages === 'number' && site.total_pages > 0) {
        const total = site.total_pages;
        const limit = Math.max(1, validationPageLimit() || 3);
        const start = total;
        const end = Math.max(1, start - limit + 1);
        rangesExpr = `${start}-${end}`;
        addLog(`ℹ️ 기본 꼬리 범위 사용: ${rangesExpr}`);
      } else {
        addLog('⚠️ Sync 불가: 범위를 결정할 수 없습니다. (사이트 상태/Validation 없음)');
        return;
      }
    }
    // Clamp to site bounds and inform user if corrected
    const clamp = clampRangesToSite(rangesExpr);
    if (clamp.changed) {
      const total = siteStatus()?.total_pages;
      const fixes = (clamp.details || [])
        .filter((d: { before: { start: number; end: number }; start: number; end: number }) => d.before.start !== d.start || d.before.end !== d.end)
        .map((d: { before: { start: number; end: number }; start: number; end: number }) => `${d.before.start === d.before.end ? d.before.start : `${d.before.start}-${d.before.end}`}` +
                  ` → ${d.start === d.end ? d.start : `${d.start}-${d.end}`}`);
      const msg1 = `입력 범위를 사이트 최대 페이지${typeof total==='number' ? `(${total})` : ''} 기준으로 보정했습니다: ${fixes.join(', ')} (최종: ${clamp.expr})`;
      addLog(`ℹ️ ${msg1}`);
      setRangeNotice(msg1);
      setValRangeExpr(clamp.expr); // reflect correction in UI
      rangesExpr = clamp.expr;
    } else {
      setRangeNotice(null);
    }
    // Then clamp to configured validationPageLimit (max span)
    const vLimit = validationPageLimit();
    if (vLimit && vLimit > 0) {
      const byLimit = clampRangesToLimit(rangesExpr, vLimit);
      if (byLimit.changed) {
        const fixes = (byLimit.details || [])
          .filter((d: { before: { start: number; end: number }; start: number; end: number }) => d.before.end !== d.end)
          .map((d: { before: { start: number; end: number }; start: number; end: number }) => `${d.before.start}-${d.before.end} → ${d.start}-${d.end}`);
        const msg2 = `최대 Validation/Sync 범위 ${vLimit}페이지를 초과하여 보정했습니다: ${fixes.join(', ')} (최종: ${byLimit.expr})`;
        addLog(`ℹ️ ${msg2}`);
        setRangeNotice(prev => prev ? `${prev} | ${msg2}` : msg2);
        setValRangeExpr(byLimit.expr);
        rangesExpr = byLimit.expr;
      }
    }
    try {
  addLog(`🔄 Sync 요청: ranges=${rangesExpr} dryRun=${dryRun}`);
  try { await invoke('ui_debug_log', { message: `[AdvancedTab] sync_button_click ranges=${rangesExpr} dryRun=${dryRun}` }); } catch {}
      // Optimistic UI: show syncing state immediately; backend events will update stats
      setIsSyncing(true);
      setSyncStats({ pages_processed: 0, inserted: 0, updated: 0, skipped: 0, failed: 0, session_id: undefined });
      // Pre-compute a local planned pages estimate so the tiny progress bar can start moving even if start event is delayed
      try {
        const localRanges = parseMultiRangeExpr(rangesExpr);
        const totalLocal = localRanges.reduce((acc, r) => acc + Math.max(1, r.start - r.end + 1), 0);
        if (totalLocal > 0) setPlannedPages(totalLocal);
      } catch {}
      // Fire-and-forget: backend emits actor-sync-* events; don't block UI waiting for result
  invoke<any>('start_partial_sync', { ranges: rangesExpr, dry_run: dryRun })
        .then((summary) => {
          addLog(`✅ Sync 완료 (invoke 반환): ${summary ? JSON.stringify(summary) : 'OK'}`);
        })
        .catch((err) => {
          addLog(`❌ Sync invoke 오류: ${err}`);
          setIsSyncing(false);
        });
  addLog('📨 Sync 요청을 백엔드로 전달함 (이벤트 대기)');
  try { await invoke('ui_debug_log', { message: `[AdvancedTab] start_partial_sync invoked ranges=${rangesExpr}` }); } catch {}
      // Fallback: if no start event arrives shortly, trigger explicit page sync
  const startSeqAtRequest = lastSyncStartSeq;
  setTimeout(async () => {
        // If a start event has arrived, do nothing
        if (lastSyncStartSeq !== startSeqAtRequest) return;
        if (!isSyncing()) return;
        try {
          const parsed = parseMultiRangeExpr(rangesExpr);
          const pages: number[] = [];
          for (const r of parsed) {
            for (let p = r.start; p >= r.end; p--) pages.push(p);
          }
          // Deduplicate while keeping order
          const seen = new Set<number>();
          const uniquePages = pages.filter((p) => (seen.has(p) ? false : (seen.add(p), true)));
      if (uniquePages.length > 0) {
            addLog(`⛑️ Sync 시작 이벤트가 지연되어 대체 경로 실행: start_sync_pages pages=[${uniquePages.join(', ')}]`);
    try { await invoke('ui_debug_log', { message: `[AdvancedTab] fallback_start_sync_pages pages=[${uniquePages.join(',')}]` }); } catch {}
            await tauriApi.startSyncPages(uniquePages, dryRun);
            addLog('⛑️ 대체 경로 요청 완료 (이벤트 대기)');
          }
        } catch (err) {
          addLog(`❌ 대체 start_sync_pages 실패: ${err}`);
        }
    }, 800);
    } catch (e:any) {
      addLog(`❌ Sync 시작 실패: ${e}`);
      console.error('start_partial_sync error', e);
      setIsSyncing(false);
    }
  };

  const stageNames = [
    'Stage 0: 사이트 상태 확인',
    'Stage 1: 데이터베이스 분석', 
    'Stage 2: 제품 목록 수집',
    'Stage 3: 제품 상세정보 수집',
    'Stage 4: 데이터 처리 파이프라인',
    'Stage 5: 데이터베이스 저장'
  ];

  // 데이터 정합성 체크 (page_id / index_in_page) 실행
  const runConsistencyCheck = async () => {
    addLog('🧪 정합성 체크 실행 중...');
    try {
      const json = await invoke<string>('check_page_index_consistency');
      addLog('✅ 정합성 체크 완료 (콘솔 상세 출력)');
      console.log('[ConsistencyReport]', json);
      try {
        const report = JSON.parse(json);
        if (report && typeof report.invalid === 'number') {
          if (report.invalid > 0) {
            addLog(`⚠️ 불일치 ${report.invalid}건 (샘플 ${report.sample_inconsistencies?.length || 0})`);
          } else {
            addLog('🧪 불일치 없음 (OK)');
          }
        }
      } catch (_) { /* ignore parse error */ }
    } catch (e:any) {
      addLog(`❌ 정합성 체크 실패: ${e}`);
      console.error('Consistency check failed', e);
    }
  };

  return (
    <div class="min-h-screen bg-gray-50 p-6">
      <div class="max-w-7xl mx-auto space-y-6">
        <div class="mb-8">
          <h1 class="text-3xl font-bold text-gray-900 mb-2">
            🔬 Advanced Crawling Engine
          </h1>
          <p class="text-gray-600">
            Phase 4A 5단계 파이프라인 제어 및 모니터링
          </p>
        </div>

  <div class="grid grid-cols-1 xl:grid-cols-3 gap-6">
          <div class="space-y-6">
            {/* Sync Stage Cards */}
            <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
              {/* Stage 1: 페이지 접근/목록 */}
              <div class="rounded-lg border p-4 bg-indigo-50 border-indigo-200">
                <div class="text-xs font-semibold text-indigo-800 mb-1">Stage 1 · 목록 페이지 접근</div>
                <div class="text-[11px] text-indigo-900">현재 페이지: <b>{stage1State()?.currentPage ?? '-'}</b></div>
                <div class="text-[11px] text-indigo-900">시작된 페이지 수: <b>{stage1State()?.pagesStarted || 0}</b></div>
                {/* Tiny progress bar approximation based on page starts vs total (if known) */}
                <Show when={syncStats()?.pages_processed !== undefined}>
                  <div class="mt-1 w-full bg-indigo-100 rounded h-1.5">
                    {(() => {
                      const processed = syncStats()?.pages_processed || 0;
                      const total = plannedPages() ?? (crawlingRange()?.range ? (crawlingRange()!.range![0] - crawlingRange()!.range![1] + 1) : processed || 1);
                      const pct = Math.min(100, (100 * processed) / Math.max(1, total));
                      return <div class="h-1.5 bg-indigo-500 rounded transition-all" style={`width: ${pct}%`}></div>;
                    })()}
                  </div>
                </Show>
                <Show when={(stage1State()?.mismatchWarnings || 0) > 0}>
                  <div class="mt-1 text-[11px] text-amber-800 bg-amber-50 border border-amber-200 rounded px-2 py-1">
                    경고: {stage1State()?.mismatchWarnings}건<br/>
                    <span class="line-clamp-2">{stage1State()?.lastWarning}</span>
                  </div>
                </Show>
                <p class="mt-2 text-[11px] text-indigo-700">사이트 목록에서 제품 URL을 수집하고 예상 개수와 일치하는지 점검합니다.</p>
              </div>
              {/* Stage 3: 상세 정보 수집 */}
              <div class="rounded-lg border p-4 bg-emerald-50 border-emerald-200">
                <div class="text-xs font-semibold text-emerald-800 mb-1">Stage 3 · 상세 정보 추출</div>
                <div class="text-[11px] text-emerald-900">상세 경고 수: <b>{stage3State()?.detailWarnings || 0}</b></div>
                <div class="text-[11px] text-emerald-900 mt-0.5">상세 저장: 
                  <b class="text-emerald-700"> {stage3Success().persisted}</b>
                  <span class="mx-1">/</span>
                  <b class="text-gray-700">{stage3Success().skipped}</b>
                </div>
                <Show when={stage3LastStatus()}>
                  <div class="mt-1 text-[11px] text-emerald-800 bg-emerald-50 border border-emerald-200 rounded px-2 py-1">
                    최근: <span class="line-clamp-2">{stage3LastStatus()}</span>
                  </div>
                </Show>
                <Show when={(stage3State()?.detailWarnings || 0) > 0}>
                  <div class="mt-1 text-[11px] text-amber-800 bg-amber-50 border border-amber-200 rounded px-2 py-1">
                    최근: <span class="line-clamp-2">{stage3State()?.lastDetailWarning}</span>
                  </div>
                </Show>
                <p class="mt-2 text-[11px] text-emerald-700">제품 상세 페이지에서 주요 필드를 추출하고 누락값은 재시도로 보정합니다.</p>
              </div>
              {/* Stage 5: DB 저장/백필 */}
              <div class="rounded-lg border p-4 bg-purple-50 border-purple-200">
                <div class="text-xs font-semibold text-purple-800 mb-1">Stage 5 · DB 저장 및 백필</div>
                <div class="text-[11px] text-purple-900">글로벌 제품 ID 백필: <b>{stage5StateExtra()?.globalIdBackfillAffected ?? 0}</b></div>
                <Show when={stage5StateExtra()?.lastPerPage}>
                  <div class="mt-1 text-[11px] text-purple-900 bg-purple-100 border border-purple-200 rounded px-2 py-1">
                    <div>최근 페이지 p{stage5StateExtra()!.lastPerPage!.page} (pid {stage5StateExtra()!.lastPerPage!.pid})</div>
                    <div class="flex gap-2">
                      <span>placeholder: <b>{stage5StateExtra()!.lastPerPage!.placeholders}</b></span>
                      <span>core: <b>{stage5StateExtra()!.lastPerPage!.core}</b></span>
                      <span>id: <b>{stage5StateExtra()!.lastPerPage!.prodId}</b></span>
                    </div>
                  </div>
                </Show>
                <Show when={stage5StateExtra()?.lastDbWarning}>
                  <div class="mt-1 text-[11px] text-rose-800 bg-rose-50 border border-rose-200 rounded px-2 py-1">
                    최근 경고: <span class="line-clamp-2">{stage5StateExtra()?.lastDbWarning}</span>
                  </div>
                </Show>
                <p class="mt-2 text-[11px] text-purple-700">페이지 단위 자리표시자/코어 백필과 세션 종료 후 글로벌 제품 ID 스윕 결과를 요약합니다.</p>
              </div>
            </div>
            {/* Site Status */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <div class="flex items-center space-x-2">
                  <h2 class="text-lg font-semibold text-gray-900">🌐 사이트 상태</h2>
                  <button
                    onClick={() => setShowSiteStatus(!showSiteStatus())}
                    class="text-gray-500 hover:text-gray-700 transition-colors"
                  >
                    {showSiteStatus() ? '🔽' : '▶️'}
                  </button>
                </div>
                <div class="flex items-center gap-2">
                  <button
                    onClick={runConsistencyCheck}
                    class="px-3 py-1.5 text-sm bg-amber-100 text-amber-700 rounded-md hover:bg-amber-200"
                    title="DB 제품 page_id / index_in_page 값이 사이트 구조와 맞는지 검사"
                  >
                    🧪 정합성 체크
                  </button>
                  <button
                    onClick={checkSiteStatus}
                    class="px-3 py-1.5 text-sm bg-blue-100 text-blue-700 rounded-md hover:bg-blue-200"
                  >
                    새로고침
                  </button>
                </div>
              </div>
              
              <Show when={showSiteStatus()}>
                <Show
                  when={siteStatus()}
                  fallback={<p class="text-gray-500">사이트 상태를 확인 중...</p>}
                >
                  <div class="space-y-4">
                    {/* 기본 사이트 정보 */}
                    <div class="grid grid-cols-2 gap-4">
                      <div class="space-y-2 text-sm">
                        <div class="flex justify-between">
                          <span class="text-gray-600">접근 가능:</span>
                          <span class={siteStatus()?.is_accessible ? "text-green-600" : "text-red-600"}>
                            {siteStatus()?.is_accessible ? "✅ 가능" : "❌ 불가능"}
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">전체 페이지:</span>
                          <span class="font-medium">{siteStatus()?.total_pages || 0}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">예상 제품 수:</span>
                          <span class="font-medium">{siteStatus()?.estimated_total_products || 0}</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">마지막 페이지 제품:</span>
                          <span class="font-medium">{siteStatus()?.products_on_last_page || 0}</span>
                        </div>
                      </div>
                      
                      <div class="space-y-2 text-sm">
                        <div class="flex justify-between">
                          <span class="text-gray-600">상태 점수:</span>
                          <span class={`font-medium ${
                            (siteStatus()?.health_score || 0) > 0.8 ? 'text-green-600' : 
                            (siteStatus()?.health_score || 0) > 0.5 ? 'text-yellow-600' : 'text-red-600'
                          }`}>
                            {((siteStatus()?.health_score || 0) * 100).toFixed(1)}%
                          </span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">응답 시간:</span>
                          <span class="font-medium">{siteStatus()?.response_time_ms || 0}ms</span>
                        </div>
                        <div class="flex justify-between">
                          <span class="text-gray-600">마지막 확인:</span>
                          <span class="font-medium text-xs">방금 전</span>
                        </div>
                      </div>
                    </div>

                    {/* 크롤링 범위 정보 */}
                    <Show when={crawlingRange()?.success}>
                      <div class="border-t pt-4">
                        <h3 class="font-medium text-gray-900 mb-2">📊 권장 크롤링 범위</h3>
                        <div class="bg-blue-50 border border-blue-200 rounded-md p-3">
                          <div class="flex items-center justify-between">
                            <span class="text-sm text-blue-700">
                              페이지 {crawlingRange()?.range?.[0]} → {crawlingRange()?.range?.[1]} 
                              ({(crawlingRange()?.range?.[0] || 0) - (crawlingRange()?.range?.[1] || 0) + 1}페이지)
                            </span>
                            <span class="text-xs text-blue-600 font-mono">
                              {crawlingRange()?.crawling_info?.strategy || 'auto'}
                            </span>
                          </div>
                          <p class="text-xs text-blue-600 mt-1">
                            {crawlingRange()?.message || '자동 계산된 최적 범위'}
                          </p>
                        </div>
                      </div>
                    </Show>

                    {/* 데이터베이스 현황 */}
                    <Show when={dbStats()}>
                      <div class="border-t pt-4">
                        <h3 class="font-medium text-gray-900 mb-2">💾 로컬 데이터베이스</h3>
                        <div class="grid grid-cols-2 gap-4 text-sm">
                          <div class="flex justify-between">
                            <span class="text-gray-600">저장된 제품:</span>
                            <span class="font-medium">{dbStats()?.total_products || 0}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">오늘 추가:</span>
                            <span class="font-medium">{dbStats()?.products_added_today || 0}</span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">마지막 업데이트:</span>
                            <span class="font-medium text-xs">
                              {dbStats()?.last_updated ? 
                                new Date(dbStats()!.last_updated!).toLocaleDateString() : 
                                '데이터 없음'
                              }
                            </span>
                          </div>
                          <div class="flex justify-between">
                            <span class="text-gray-600">DB 크기:</span>
                            <span class="font-medium">
                              {dbStats()?.database_size_bytes ? 
                                `${(dbStats()!.database_size_bytes / 1024 / 1024).toFixed(1)}MB` : 
                                '0MB'
                              }
                            </span>
                          </div>
                        </div>
                      </div>
                    </Show>
                  </div>
                </Show>
              </Show>
            </div>

            {/* Validation Panel */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">🧪 페이지/인덱스 Validation</h2>
                <button
                  onClick={runValidation}
                  class={`px-3 py-1.5 text-sm rounded-md font-medium transition-colors ${isValidating() ? 'bg-gray-200 text-gray-500 cursor-not-allowed' : 'bg-emerald-100 text-emerald-700 hover:bg-emerald-200'}`}
                  disabled={isValidating()}
                  title="사이트 실제 페이지를 oldest→newer 순서로 스캔하여 DB page_id/index_in_page 정합성 검증"
                >
                  {isValidating() ? '⏳ 실행 중...' : '🧪 Validation 실행'}
                </button>
              </div>
              <div class="mb-3 grid grid-cols-5 gap-2 items-end">
                <div class="col-span-4">
                  <label class="block text-[11px] text-gray-600 mb-1">페이지 범위 (oldest→newer, 쉼표로 다중 지정) — 예: 498-489,487~485,480</label>
                  <input
                    type="text"
                    placeholder="예: 498-489,487-485,480"
                    value={valRangeExpr()}
                    onInput={e => onUserEditRangeExpr(e.currentTarget.value)}
                    class="w-full px-2 py-1 rounded border text-xs focus:ring-emerald-500 focus:border-emerald-500"
                  />
                </div>
                <div class="col-span-1 flex flex-col gap-1 text-[10px] text-gray-500 leading-tight">
                  <span class="mt-[18px]">빈칸=자동</span>
                  <span class="">(oldest → 크롤링 시작 직전)</span>
                  <button
                    class="text-amber-600 underline"
                    onClick={() => { setValRangeExpr(''); userTouchedValidationRange = false; }}
                  >초기화</button>
                </div>
              </div>

              <div class="flex items-center gap-2 mb-2 flex-wrap">
                <button
                  onClick={() => runSyncForLastValidationRange(false)}
                  class={`px-3 py-1.5 text-sm rounded-md transition-colors ${isSyncing() ? 'bg-gray-200 text-gray-500 cursor-not-allowed' : 'bg-blue-100 text-blue-700 hover:bg-blue-200'}`}
                  disabled={isSyncing()}
                  title="Validation 범위를 기준으로 partial sync 실행"
                >
                  {isSyncing() ? '⏳ Sync 실행 중...' : '🔄 이 범위 Sync 실행'}
                </button>
                <button
                  onClick={() => runSyncForLastValidationRange(true)}
                  class={`px-3 py-1.5 text-sm rounded-md transition-colors ${isSyncing() ? 'bg-gray-200 text-gray-500 cursor-not-allowed' : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}`}
                  disabled={isSyncing()}
                  title="DB 변경 없이 진행 상황만 보기"
                >
                  Dry-run
                </button>
                <div class="flex items-center gap-1 text-xs text-gray-600">
                  <label class="ml-2">Repair buffer</label>
                  <input
                    type="number"
                    min="0"
                    value={repairBuffer()}
                    onInput={e => {
                      const v = parseInt(e.currentTarget.value, 10);
                      setRepairBuffer(Number.isNaN(v) ? 0 : Math.max(0, v));
                    }}
                    class="w-16 px-2 py-1 rounded border"
                  />
                </div>
                <button
                  onClick={async () => {
                    setIsSyncing(true);
                    const buf = Math.max(0, repairBuffer());
                    addLog(`🩺 Repair Sync 시작(버퍼=${buf})...`);
                    try {
                      const res = await tauriApi.startRepairSync(buf, false);
                      addLog(`✅ Repair Sync 완료: ${JSON.stringify(res)}`);
                    } catch (e) { addLog(`❌ Repair Sync 실패: ${e}`); }
                    finally { setIsSyncing(false); }
                  }}
                  class={`px-3 py-1.5 text-sm rounded-md transition-colors ${isSyncing() ? 'bg-gray-200 text-gray-500 cursor-not-allowed' : 'bg-rose-100 text-rose-700 hover:bg-rose-200'}`}
                  disabled={isSyncing()}
                  title="DB 이상치(cnt!=12) 주변 윈도우를 자동 계산해 부분 동기화 실행"
                >
                  🩺 Repair Sync
                </button>
                <button
                  onClick={async () => {
                    addLog('🧹 DB 진단 실행...');
                    try {
                      const res = await tauriApi.diagnoseAndRepairData(false, false);
                      setDiagnosisResult(res);
                      addLog(`🧪 진단 결과: ${JSON.stringify(res)}`);
                    } catch (e) { addLog(`❌ 진단 실패: ${e}`); }
                  }}
                  class="px-3 py-1.5 text-sm rounded-md bg-amber-100 text-amber-700 hover:bg-amber-200"
                  title="products/product_details 미스매치 및 이상치 수집(삭제 없음)"
                >
                  🧪 DB 진단
                </button>
                <button
                  onClick={async () => {
                    addLog('🔁 products→details 좌표/ID 동기화 실행...');
                    try {
                      const rep = await tauriApi.syncProductDetailsCoordinates();
                      addLog(`✅ 동기화 완료: products.id=${rep.updated_product_ids}, inserted=${rep.inserted_details}, updated_coords=${rep.updated_coordinates}, details.id=${rep.updated_ids} (p=${rep.total_products}, d=${rep.total_details})`);
                    } catch (e) {
                      addLog(`❌ 동기화 실패: ${e}`);
                    }
                  }}
                  class="px-3 py-1.5 text-sm rounded-md bg-blue-100 text-blue-700 hover:bg-blue-200"
                  title="products.url 기준으로 product_details에 page_id/index_in_page/id를 정합화합니다 (크롤링 없음)"
                >
                  🔁 products→details 동기화
                </button>
                <button
                  onClick={async () => {
                    addLog('🧪 Orphan 상세 동기화 실행...');
                    try {
                      const res = await tauriApi.diagnoseAndRepairData(false, true);
                      setDiagnosisResult(res);
                      addLog(`✅ Orphan 상세 동기화 완료: ${JSON.stringify(res)}`);
                    } catch (e) { addLog(`❌ Orphan 동기화 실패: ${e}`); }
                  }}
                  class="px-3 py-1.5 text-sm rounded-md bg-emerald-100 text-emerald-700 hover:bg-emerald-200"
                  title="products만 있고 details 없는 URL들에 대해 상세를 조회해 product_details를 채웁니다"
                >
                  🔁 Orphan 상세 동기화
                </button>
                {/* Move auto re-diagnose toggle to end so it doesn't push buttons off-screen */}
                <label class="flex items-center gap-1 text-xs text-gray-700 ml-auto select-none">
                  <input type="checkbox" checked={autoReDiagnose()} onInput={e=>setAutoReDiagnose((e.currentTarget as HTMLInputElement).checked)} />
                  자동 재진단
                </label>
                <button
                  onClick={async () => {
                    if (!confirm('정말로 이상치/미스매치 레코드를 삭제할까요? 되돌릴 수 없습니다.')) return;
                    addLog('🧹 DB 진단+삭제 실행...');
                    try {
                        const res = await tauriApi.diagnoseAndRepairData(true, false);
                        addLog(`✅ 삭제 완료: ${JSON.stringify(res)}`);
                    } catch (e) { addLog(`❌ 삭제 실패: ${e}`); }
                  }}
                  class="px-3 py-1.5 text-sm rounded-md bg-red-100 text-red-700 hover:bg-red-200"
                  title="미스매치/이상치 레코드 삭제 수행(주의)"
                >
                  🧹 DB 진단+삭제
                </button>
                <div class="flex items-center gap-1 text-xs text-gray-600">
                  <label>NULL cert 재시도 수</label>
                  <input id="retryLimit" type="number" min="1" placeholder="200" class="w-20 px-2 py-1 rounded border" />
                  <button
                    class="px-2 py-1 rounded bg-blue-100 text-blue-700 hover:bg-blue-200"
                    onClick={async () => {
                      const el = document.getElementById('retryLimit') as HTMLInputElement | null;
                      const v = el ? parseInt(el.value || '0', 10) : 0;
                      const limit = Number.isNaN(v) || v <= 0 ? undefined : v;
                      addLog(`🔁 certificate_id NULL 상세 재시도 실행(limit=${limit ?? 200})...`);
                      try {
                        const res = await tauriApi.retryFailedDetails(limit, false);
                        addLog(`✅ 재시도 결과: ${JSON.stringify(res)}`);
                      } catch (e) { addLog(`❌ 재시도 실패: ${e}`); }
                    }}
                  >
                    🔁 NULL cert 재시도
                  </button>
                </div>
                <div class="flex items-center gap-1 text-xs text-gray-600">
                  <label class="ml-2">Batch override</label>
                  <input
                    type="number"
                    min="1"
                    value={batchSizeOverride() ?? ''}
                    placeholder={`${batchSize()}`}
                    onInput={e => {
                      const v = parseInt(e.currentTarget.value, 10);
                      setBatchSizeOverride(Number.isNaN(v) ? null : Math.max(1, v));
                    }}
                    class="w-20 px-2 py-1 rounded border"
                    title="비워두면 설정의 batch_size를 사용합니다"
                  />
                </div>
                <button
                  onClick={async () => {
                    // Use same range derivation as partial button
                    let rangesExpr = (valRangeExpr() && valRangeExpr().trim()) ? valRangeExpr().trim() : (lastValidationExpr() || '');
                    if (!rangesExpr) {
                      const rng = lastValidationRange();
                      if (!rng) { addLog('⚠️ Batched Sync 불가: 최근 Validation 범위 정보가 없습니다. 먼저 Validation을 실행하세요.'); return; }
                      rangesExpr = `${rng.start}-${rng.end}`;
                    }
                    setIsSyncing(true);
                    addLog(`📦 Batched Sync 시작: ${rangesExpr}`);
                    try {
                      const override = batchSizeOverride() ?? undefined;
                      const res = await tauriApi.startBatchedSync(rangesExpr, override);
                      addLog(`✅ Batched Sync 완료: ${JSON.stringify(res)}`);
                    } catch (e) {
                      addLog(`❌ Batched Sync 실패: ${e}`);
                    } finally { setIsSyncing(false); }
                  }}
                  class={`px-3 py-1.5 text-sm rounded-md transition-colors ${isSyncing() ? 'bg-gray-200 text-gray-500 cursor-not-allowed' : 'bg-indigo-100 text-indigo-700 hover:bg-indigo-200'}`}
                  disabled={isSyncing()}
                  title="연속 페이지를 배치로 묶어 순차 실행 (Partial과 동일 Flow)"
                >
                  📦 Batched Sync
                </button>
              </div>
              <Show when={rangeNotice()}>
                <div class="mb-3 text-xs bg-amber-50 border border-amber-200 text-amber-800 rounded p-2">
                  {rangeNotice()}
                </div>
              </Show>

              <Show when={syncStats()}>
                <div class="text-xs text-gray-700 bg-gray-50 border border-gray-200 rounded p-2 mb-2">
                  <div class="flex gap-3">
                    <span>pages: <b>{syncStats()!.pages_processed || 0}</b></span>
                    <span>ins: <b class="text-emerald-700">{syncStats()!.inserted || 0}</b></span>
                    <span>upd: <b class="text-blue-700">{syncStats()!.updated || 0}</b></span>
                    <span>skip: <b class="text-gray-600">{syncStats()!.skipped || 0}</b></span>
                    <span>fail: <b class="text-red-700">{syncStats()!.failed || 0}</b></span>
                    <Show when={typeof syncStats()!.duration_ms !== 'undefined'}>
                      <span>ms: <b>{syncStats()!.duration_ms}</b></span>
                    </Show>
                  </div>
                </div>
              </Show>
              <Show when={validationStats()} fallback={
                <p class="text-sm text-gray-600">
                  {isValidating() ? '실시간 이벤트 수신 중...' : '아직 실행된 Validation 없음'}
                </p>
              }>
        <div class="grid grid-cols-2 gap-4 text-sm">
                  <div class="flex justify-between"><span class="text-gray-600">스캔 페이지:</span><span class="font-medium">{validationStats()?.pages_scanned}</span></div>
                  <div class="flex justify-between"><span class="text-gray-600">검증 제품:</span><span class="font-medium">{validationStats()?.products_checked}</span></div>
                  <div class="flex justify-between"><span class="text-gray-600">불일치:</span><span class="font-medium text-red-600">{validationStats()?.divergences}</span></div>
                  <div class="flex justify-between"><span class="text-gray-600">이상 징후:</span><span class="font-medium text-amber-600">{validationStats()?.anomalies}</span></div>
                  <div class="flex justify-between col-span-2"><span class="text-gray-600">소요 시간:</span><span class="font-medium">{(validationStats()!.duration_ms/1000).toFixed(2)}s</span></div>
                  <Show when={validationDetails()}>
          <div class="flex justify-between col-span-2"><span class="text-gray-600">적용 범위:</span><span class="font-medium">{validationDetails()?.resolved_start_oldest} → {validationDetails()?.resolved_end_newest}</span></div>
          <div class="flex justify-between col-span-2"><span class="text-gray-600">시도/성공 페이지:</span><span class="font-medium">{validationDetails()?.pages_attempted || 0} / {validationStats()?.pages_scanned || 0}</span></div>
          <div class="flex justify-between col-span-2"><span class="text-gray-600">사이트 메타:</span><span class="font-medium">pages={validationDetails()?.total_pages_site} last_items={validationDetails()?.items_on_last_page}</span></div>
                    <div class="flex justify-between col-span-2"><span class="text-gray-600">gap ranges:</span><span class="font-medium">{validationDetails()?.gap_ranges?.length || 0}</span></div>
                    <div class="flex justify-between col-span-2"><span class="text-gray-600">cross-page dup URLs:</span><span class="font-medium">{validationDetails()?.cross_page_duplicate_urls || 0}</span></div>
                  </Show>
                </div>
                <p class="mt-2 text-xs text-gray-500 font-mono break-all">session: {validationStats()?.session_id}</p>
                <Show when={validationDetails()}>
                  <div class="mt-3 border-t pt-3 space-y-3">
                    <div>
                      <h3 class="text-sm font-semibold text-gray-800 mb-1">📌 불일치 샘플 (최대 8)</h3>
                      <div class="space-y-1 text-[11px] font-mono bg-gray-50 p-2 rounded border border-gray-200 max-h-40 overflow-auto">
                        <For each={(validationDetails()?.divergence_samples || []).slice(0,8)}>{(d:any) =>
                          <div class="truncate">
                            p{d.physical_page} {d.kind} url={d.url.split('/').filter(Boolean).slice(-2,-1)} db=({d.db_page_id ?? '-'}, {d.db_index_in_page ?? '-'}) exp=({d.expected_page_id},{d.expected_index_in_page})
                          </div>
                        }</For>
                        <Show when={(validationDetails()?.divergence_samples || []).length > 8}>
                          <div class="text-gray-500">… {(validationDetails()?.divergence_samples.length || 0)-8} more</div>
                        </Show>
                      </div>
                    </div>
                    <div>
                      <h3 class="text-sm font-semibold text-gray-800 mb-1">🗂 페이지별 요약</h3>
                      <div class="space-y-1 text-[11px] font-mono bg-gray-50 p-2 rounded border border-gray-200 max-h-60 overflow-auto">
                        <For each={validationDetails()?.per_page || []}>{(r:any) =>
                          <div class="truncate">
                            p{r.physical_page}: prod={r.products_found} div={r.divergences} (miss={r.mismatch_missing} coord={r.mismatch_coord}) anom={r.anomalies}{r.mismatch_shift_pattern !== null ? ` shift=${r.mismatch_shift_pattern}`:''}
                          </div>
                        }</For>
                        <Show when={(validationDetails()?.gap_ranges || []).length > 0}>
                          <div class="mt-2 pt-2 border-t border-gray-200 text-[11px]">
                            <span class="font-semibold">Gaps:</span>
                            <For each={validationDetails()?.gap_ranges || []}>{(g:any) => <div>offset {g.start_offset}..{g.end_offset} (size={g.size})</div>}</For>
                          </div>
                        </Show>
                      </div>
                    </div>
                  </div>
                </Show>
              </Show>
              <Show when={diagnosisResult()}>
                <div class="mt-3 text-xs">
                  <h3 class="font-semibold text-gray-800 mb-1">🧪 DB 진단 결과</h3>
                  {/* Compact summary cards */}
                  <div class="grid grid-cols-2 md:grid-cols-4 gap-2 mb-2">
                    {(() => {
                      const d: any = diagnosisResult();
                      const diag = d?.data?.diagnostics ?? d?.diagnostics;
                      if (!diag) return null;
                      const cards = [
                        { label: 'Orphans', value: diag.orphans_products_without_details, color: 'bg-rose-50 text-rose-700 border-rose-200' },
                        { label: 'Nullish core', value: diag.products_with_nullish_core_fields, color: 'bg-amber-50 text-amber-700 border-amber-200' },
                        { label: 'Out-of-range page_id', value: diag.out_of_range_page_id, color: 'bg-indigo-50 text-indigo-700 border-indigo-200' },
                        { label: 'Invalid indices/page_id', value: diag.invalid_indices_or_page_id, color: 'bg-gray-50 text-gray-700 border-gray-200' },
                      ];
                      return cards.map((c) => (
                        <div class={`border rounded px-2 py-1 ${c.color}`}>
                          <div class="text-[10px]">{c.label}</div>
                          <div class="text-sm font-semibold">{c.value ?? 0}</div>
                        </div>
                      ));
                    })()}
                  </div>
                  {/* Actions summary + quick re-run */}
                  <div class="flex items-center gap-2 mb-2">
                    {(() => {
                      const d: any = diagnosisResult();
                      const act = d?.data?.actions ?? d?.actions;
                      if (!act) return null;
                      return (
                        <div class="text-[11px] text-gray-600">삭제옵션: {String(act.delete_mismatches ?? false)} / 삭제행: <b class="text-gray-800">{act.deleted_rows ?? 0}</b></div>
                      );
                    })()}
                    <button
                      class="ml-auto px-2 py-1 rounded bg-emerald-100 text-emerald-700 hover:bg-emerald-200"
                      onClick={async () => {
                        addLog('🧪 재진단 실행...');
                        try {
                          const res = await tauriApi.diagnoseAndRepairData(false);
                          setDiagnosisResult(res);
                          addLog(`🧪 재진단 결과: ${JSON.stringify(res)}`);
                        } catch (e) { addLog(`❌ 재진단 실패: ${e}`); }
                      }}
                    >재진단</button>
                  </div>
                  <div class="bg-gray-50 border border-gray-200 rounded p-2 max-h-56 overflow-auto font-mono whitespace-pre-wrap break-words">
                    {JSON.stringify(diagnosisResult(), null, 2)}
                  </div>
                </div>
              </Show>
              <Show when={validationEvents().length > 0}>
                <div class="mt-4 max-h-40 overflow-auto bg-gray-50 border border-gray-200 rounded p-2 text-xs font-mono space-y-0.5">
                  <For each={validationEvents().slice(-50)}>{(e:any) => <div class="truncate">{e.event_name}:{e.physical_page ?? ''}:{e.kind ?? e.code ?? ''}</div>}</For>
                </div>
              </Show>
            </div>

            {/* Actor System Controls */}
            <div class="bg-gradient-to-r from-purple-50 to-indigo-50 rounded-lg shadow-sm border border-purple-200 p-6 mb-6">
              <h2 class="text-lg font-semibold text-purple-900 mb-4">🎭 Actor 시스템 크롤링</h2>
              <div class="space-y-4">
                
                {/* Calculated Range Display */}
                <Show when={crawlingRange()?.range}>
                  <div class="bg-purple-100 border border-purple-300 rounded-md p-3">
                    <div class="text-sm text-purple-800">
                      <strong>📊 CrawlingPlanner 계산 결과:</strong><br/>
                      크롤링 범위: <span class="font-mono font-bold">{crawlingRange()?.range?.[0]} → {crawlingRange()?.range?.[1]}</span> 
                      ({(crawlingRange()?.range?.[0] || 0) - (crawlingRange()?.range?.[1] || 0) + 1} 페이지)<br/>
                      <span class="text-xs">• 설정, 사이트 상태, DB 상태를 종합하여 자동 계산됨</span>
                      
                      {/* Batch Execution Plan */}
                      <div class="mt-3 pt-3 border-t border-purple-200">
                        <strong>📦 배치 실행 계획 (batch_size={crawlingRange()?.batch_plan?.batch_size || 'N/A'}):</strong><br/>
                        <div class="mt-1 space-y-1">
                          {(() => {
                            const batchPlan = crawlingRange()?.batch_plan;
                            if (!batchPlan || !batchPlan.batches.length) return null;
                            
                            return batchPlan.batches.map((batch: any) => (
                              <div class="text-xs font-mono bg-purple-50 px-2 py-1 rounded">
                                <span class="text-purple-700">Batch {batch.batch_id + 1}:</span> 
                                <span class="text-purple-900"> [{batch.pages.join(', ')}]</span>
                                <span class="text-purple-600"> ({batch.pages.length}페이지, ~{batch.estimated_products}제품)</span>
                              </div>
                            ));
                          })()}
                        </div>
                        
                        {/* 추가 배치 계획 정보 */}
                        {crawlingRange()?.batch_plan && (
                          <div class="mt-2 text-xs text-purple-600">
                            <div>• 총 배치 수: {crawlingRange()!.batch_plan.total_batches}개</div>
                            <div>• 동시 실행 제한: {crawlingRange()!.batch_plan.concurrency_limit}</div>
                            <div>• 실행 전략: {crawlingRange()!.batch_plan.execution_strategy}</div>
                            <div>• 예상 소요 시간: {Math.floor(crawlingRange()!.batch_plan.estimated_duration_seconds / 60)}분</div>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </Show>

                {/* Debug: Batch Plan Calculation Button */}
                <button
                  onClick={calculateCrawlingRange}
                  class="w-full py-2 px-4 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 font-medium text-sm"
                >
                  🔍 크롤링 범위 및 배치 플랜 계산
                  <span class="text-xs block mt-1">설정파일 batch_size=9로 배치 플랜을 생성합니다</span>
                </button>

                {/* Real Actor System Main Button */}
                <button
                  onClick={startRealActorSystemWithCalculatedRange}
                  class="w-full py-3 px-4 bg-purple-600 text-white rounded-md hover:bg-purple-700 font-medium disabled:bg-gray-400 disabled:cursor-not-allowed"
                  disabled={isRunning()}
                >
                  🎭 진짜 Actor 시스템으로 크롤링 시작 (설정 기반)
                  <span class="text-xs block mt-1">CrawlingPlanner가 자동으로 범위와 배치를 계산합니다</span>
                </button>
                
              </div>
            </div>

            {/* Crawling Controls */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 class="text-lg font-semibold text-gray-900 mb-4">🎮 크롤링 제어</h2>
              <div class="space-y-3">
                <Show 
                  when={!isRunning()}
                  fallback={
                    <div class="bg-blue-100 border border-blue-300 rounded-md p-2 text-center">
                      <span class="text-sm text-blue-800 font-medium">
                        {isPaused() ? '⏸️ 일시 중지됨' : '⏳ 크롤링 실행 중...'}
                      </span>
                    </div>
                  }
                >
                  <button
                    onClick={startCrawling}
                    class="w-full py-2.5 px-4 bg-blue-600 text-white rounded-md hover:bg-blue-700 font-medium"
                  >
                    🚀 크롤링 시작
                  </button>
                </Show>

                <Show when={isRunning()}>
                  <div class="grid grid-cols-2 gap-2">
                    <Show 
                      when={!isPaused()}
                      fallback={
                        <button
                          onClick={resumeCrawling}
                          class="py-2 px-3 bg-green-600 text-white rounded-md hover:bg-green-700 font-medium text-sm"
                        >
                          ▶️ 재개
                        </button>
                      }
                    >
                      <button
                        onClick={pauseCrawling}
                        class="py-2 px-3 bg-yellow-600 text-white rounded-md hover:bg-yellow-700 font-medium text-sm"
                      >
                        ⏸️ 일시 중지
                      </button>
                    </Show>
                    <button
                      onClick={stopCrawling}
                      class="py-2 px-3 bg-red-600 text-white rounded-md hover:bg-red-700 font-medium text-sm"
                    >
                      ⏹️ 완전 정지
                    </button>
                  </div>
                </Show>
              </div>
            </div>

            {/* Progress */}
            <Show when={progress()}>
              <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 class="text-lg font-semibold text-gray-900 mb-4">📊 진행 상황</h2>
                <div class="space-y-4">
                  <div>
                    <div class="flex justify-between items-center mb-2">
                      <span class="text-sm font-medium text-gray-700">
                        {stageNames[progress()?.stage || 0]}
                      </span>
                      <span class="text-sm text-gray-500">
                        {Math.round(progress()?.progress_percentage || 0)}%
                      </span>
                    </div>
                    <div class="w-full bg-gray-200 rounded-full h-2">
                      <div
                        class={`h-2 rounded-full transition-all duration-300 ${
                          isPaused() ? 'bg-yellow-500' : 'bg-blue-600'
                        }`}
                        style={`width: ${progress()?.progress_percentage || 0}%`}
                      />
                    </div>
                  </div>
                  <div class={`rounded-md p-3 ${
                    isPaused() ? 'bg-yellow-50 border border-yellow-200' : 'bg-gray-50'
                  }`}>
                    <p class={`text-sm ${
                      isPaused() ? 'text-yellow-800' : 'text-gray-700'
                    }`}>
                      {isPaused() ? '⏸️ 일시 중지됨' : `💬 ${progress()?.current_message}`}
                    </p>
                  </div>
                </div>
              </div>
            </Show>
          </div>

          <div class="space-y-6">
            {/* Recent Products */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">📦 최근 수집된 제품</h2>
                <button
                  onClick={loadRecentProducts}
                  class="px-3 py-1.5 text-sm bg-green-100 text-green-700 rounded-md hover:bg-green-200"
                >
                  새로고침
                </button>
              </div>
              <div class="space-y-3 max-h-80 overflow-y-auto">
                <Show
                  when={recentProducts().length > 0}
                  fallback={<p class="text-gray-500 text-sm">아직 수집된 제품이 없습니다.</p>}
                >
                  <For each={recentProducts()}>
                    {(product) => (
                      <div class="border border-gray-200 rounded-md p-3 bg-gray-50">
                        <h3 class="font-medium text-gray-900 text-sm">{product.name}</h3>
                        <p class="text-xs text-gray-600">{product.company}</p>
                        <p class="text-xs text-blue-600 font-mono">{product.certification_number}</p>
                      </div>
                    )}
                  </For>
                </Show>
              </div>
            </div>

            {/* Live Logs + Actor Events */}
            <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6 space-y-6">
              <div>
                <h2 class="text-lg font-semibold text-gray-900 mb-4">📝 실시간 로그</h2>
                <div class="bg-gray-900 rounded-md p-4 h-60 overflow-y-auto font-mono text-sm">
                  <Show
                    when={logs().length > 0}
                    fallback={<p class="text-gray-400">로그 대기 중...</p>}
                  >
                    <For each={logs()}>
                      {(log) => (
                        <div class="text-green-400 mb-1">{log}</div>
                      )}
                    </For>
                  </Show>
                </div>
              </div>
              <div>
                <h2 class="text-lg font-semibold text-gray-900 mb-2">🎯 Actor / Concurrency Events</h2>
                <div class="border border-gray-200 rounded-md bg-gray-50 h-60 overflow-y-auto p-2">
                  <Show when={actorEvents().length} fallback={<div class="text-xs text-gray-500">아직 이벤트 없음</div>}>
                    <ol class="text-[11px] font-mono space-y-0.5">
                      <For each={actorEvents().slice(-120)}>{ev => (
                        <li class="flex gap-2">
                          <span class="text-gray-400">#{ev.seq}</span>
                          <span class="px-1 rounded bg-gray-100 text-gray-700">{ev.rawName}</span>
                          <Show when={ev.batchId}><span class="text-emerald-600">{ev.batchId}</span></Show>
                          <Show when={ev.page !== undefined}><span class="text-indigo-600">p{ev.page}</span></Show>
                          <Show when={ev.progressPct !== undefined}><span class="text-amber-600">{ev.progressPct?.toFixed(1)}%</span></Show>
                          <Show when={ev.activeDetails !== undefined}><span class="text-pink-600">d{ev.activeDetails}</span></Show>
                        </li>
                      )}</For>
                    </ol>
                  </Show>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
