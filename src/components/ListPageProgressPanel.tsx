import { createSignal, For, Show, onMount, onCleanup, createEffect } from "solid-js";
import { listen } from "@tauri-apps/api/event";

interface PageProgress {
  pageNumber: number;
  pageId: string;
  collectedUrls: number;
  expectedUrls: number;
  status: 'pending' | 'success' | 'partial' | 'failed' | 'processing';
  retryCount: number;
  error?: string;
  timestamp: number;
}

export default function ListPageProgressPanel() {
  const [isActive, setIsActive] = createSignal(false);
  const [totalPages, setTotalPages] = createSignal(0);
  const [pages, setPages] = createSignal<Map<number, PageProgress>>(new Map());
  const [sessionActive, setSessionActive] = createSignal(false); // 전체 세션 활성 상태
  const [lastUpdatedPage, setLastUpdatedPage] = createSignal<number | null>(null);
  let scrollContainerRef: HTMLDivElement | undefined;

  // 페이지 번호 추출 헬퍼 함수 - "page_123" → 123
  const extractPageNumber = (itemId: string): number | null => {
    const match = itemId.match(/^page_(\d+)$/);
    return match ? parseInt(match[1], 10) : null;
  };

  // 페이지 업데이트 시 자동 스크롤
  createEffect(() => {
    const updatedPage = lastUpdatedPage();
    if (updatedPage !== null && scrollContainerRef) {
      const pageElement = scrollContainerRef.querySelector(`[data-page="${updatedPage}"]`);
      if (pageElement) {
        pageElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
    }
  });

  onMount(async () => {
    console.log('[ListPageProgressPanel] Component mounted, setting up event listeners...');
    
    // actor-event 통합 이벤트 리스너
    const unlistenActorEvent = await listen<any>('actor-event', (event) => {
      console.log('[ListPageProgress] 🎯 actor-event received, variant:', event.payload?.variant);
      const payload = event.payload;
      const variant = payload.variant;
      
      // SessionStarted - 세션 시작 감지 및 전체 페이지 범위 초기화
      if (variant === 'SessionStarted') {
        console.log('[ListPageProgress] ===== Session started =====');
        console.log('[ListPageProgress] Full payload:', JSON.stringify(payload, null, 2));
        
        const plannedPages = payload.planned_pages || [];
        console.log('[ListPageProgress] planned_pages from SessionStarted:', plannedPages);
        console.log('[ListPageProgress] planned_pages count:', plannedPages.length);
        
        setSessionActive(true);
        setIsActive(true); // 즉시 패널 표시
        
        // 전체 크롤링 대상 페이지를 pending 상태로 초기화
        const initialPages = new Map<number, PageProgress>();
        plannedPages.forEach((pageNum: number) => {
          console.log('[ListPageProgress] Initializing pending page:', pageNum);
          initialPages.set(pageNum, {
            pageNumber: pageNum,
            pageId: `page_${pageNum}`,
            collectedUrls: 0,
            expectedUrls: 12,
            status: 'pending',
            retryCount: 0,
            timestamp: Date.now(),
          });
        });
        
        setPages(initialPages);
        setTotalPages(plannedPages.length);
        console.log('[ListPageProgress] Initialized pages:', initialPages.size);
      }
      
      // SessionCompleted - 세션 완료 감지
      if (variant === 'SessionCompleted' || variant === 'SessionFailed') {
        console.log('[ListPageProgress] Session ended');
        // 5초 후 패널 숨김
        setTimeout(() => {
          setSessionActive(false);
          setIsActive(false);
        }, 5000);
      }
      
      // ListPageBatchStarted 처리 (fallback: SessionStarted에서 초기화되지 않은 경우)
      if (variant === 'ListPageBatchStarted') {
        console.log('[ListPageProgress] Batch started (batch-level)');
        const pageNumbers = payload.page_numbers || [];
        
        // SessionStarted에서 이미 초기화되었으면 스킵
        const currentPages = pages();
        if (currentPages.size === 0 && pageNumbers.length > 0) {
          console.log('[ListPageProgress] Fallback: Initializing pages from batch event');
          setIsActive(true);
          setSessionActive(true);
          
          setPages((prev) => {
            const updated = new Map(prev);
            pageNumbers.forEach((pageNum: number) => {
              if (!updated.has(pageNum)) {
                updated.set(pageNum, {
                  pageNumber: pageNum,
                  pageId: `page_${pageNum}`,
                  collectedUrls: 0,
                  expectedUrls: 12,
                  status: 'pending',
                  retryCount: 0,
                  timestamp: Date.now(),
                });
              }
            });
            return updated;
          });
          
          setTotalPages((prev: number) => prev + pageNumbers.length);
        }
      }
      
      // ListPageProgress 처리 - 최종 통계 업데이트 (collected_urls 정보 반영)
      if (variant === 'ListPageProgress') {
        console.log('[ListPageProgress] 📊 Final page statistics:', payload);
        const { page_number, page_id, collected_urls, expected_urls, status, retry_count, error } = payload;
        
        setPages((prev) => {
          const updated = new Map(prev);
          
          // 최종 통계로 업데이트
          updated.set(page_number, {
            pageNumber: page_number,
            pageId: page_id,
            collectedUrls: collected_urls, // 최종 수집 URL 개수
            expectedUrls: expected_urls,   // 예상 URL 개수
            status: status as PageProgress['status'], // 최종 상태 (success/partial/failed)
            retryCount: retry_count,
            error,
            timestamp: Date.now(),
          });
          return updated;
        });
        
        setLastUpdatedPage(page_number);
      }
      
      // StageItemStarted - 개별 페이지 크롤링 시작 (processing 상태)
      if (variant === 'StageItemStarted') {
        console.log('[ListPageProgress] 🔄 StageItemStarted:', payload);
        const { stage_type, item_id, retry_count } = payload;
        
        // ListPageCrawling 스테이지만 처리
        if (stage_type === 'ListPageCrawling') {
          const pageNum = extractPageNumber(item_id);
          if (pageNum !== null) {
            console.log(`[ListPageProgress] Page ${pageNum} started (retry: ${retry_count || 0})`);
            
            setPages((prev) => {
              const updated = new Map(prev);
              const existing = updated.get(pageNum);
              if (existing) {
                updated.set(pageNum, {
                  ...existing,
                  status: 'processing', // pending → processing
                  retryCount: retry_count || 0,
                  timestamp: Date.now(),
                });
              }
              return updated;
            });
            
            setLastUpdatedPage(pageNum);
          }
        }
      }
      
      // StageItemCompleted - 개별 페이지 크롤링 완료 (success/failed 상태)
      if (variant === 'StageItemCompleted') {
        console.log('[ListPageProgress] ✅ StageItemCompleted:', payload);
        const { stage_type, item_id, success, retry_count, error } = payload;
        
        // ListPageCrawling 스테이지만 처리
        if (stage_type === 'ListPageCrawling') {
          const pageNum = extractPageNumber(item_id);
          if (pageNum !== null) {
            console.log(`[ListPageProgress] Page ${pageNum} completed (success: ${success}, retry: ${retry_count || 0})`);
            
            setPages((prev) => {
              const updated = new Map(prev);
              const existing = updated.get(pageNum);
              if (existing) {
                updated.set(pageNum, {
                  ...existing,
                  status: success ? 'success' : 'failed', // processing → success/failed
                  retryCount: retry_count || 0,
                  error: error || existing.error,
                  timestamp: Date.now(),
                });
              }
              return updated;
            });
            
            setLastUpdatedPage(pageNum);
          }
        }
      }

      
      // ListPageBatchCompleted 처리
      if (variant === 'ListPageBatchCompleted') {
        console.log('[ListPageProgress] Batch completed:', payload);
        // 배치 완료는 로그만 남기고 패널은 계속 표시 (세션 전체가 끝날 때까지)
      }
    });

    onCleanup(() => {
      console.log('[ListPageProgressPanel] Cleanup: unregistering event listener');
      unlistenActorEvent();
    });
  });

  const progressPercentage = () => {
    const pagesArray = Array.from(pages().values());
    const completed = pagesArray.filter(p => p.status !== 'pending' && p.status !== 'processing').length;
    return totalPages() > 0 ? (completed / totalPages()) * 100 : 0;
  };

  const getStatusIcon = (status: PageProgress['status']) => {
    switch (status) {
      case 'success': return '🚩'; // 완료 깃발
      case 'partial': return '⚠'; // 부분 수집 경고
      case 'failed': return '✗'; // 실패
      case 'processing': return '⟳'; // 진행 중 (회전 애니메이션 대상)
      case 'pending': return ''; // 빈 공터 (원형만 표시)
      default: return '';
    }
  };

  const getStatusColor = (status: PageProgress['status'], retryCount: number) => {
    switch (status) {
      case 'success': return 'bg-emerald-500 border-emerald-600 shadow-emerald-200';
      case 'partial': return 'bg-amber-500 border-amber-600 shadow-amber-200';
      case 'failed': return 'bg-rose-600 border-rose-700 shadow-rose-200';
      case 'processing': 
        // 재시도 중이면 주황색 + 빠른 애니메이션
        if (retryCount > 0) {
          return 'bg-orange-500 border-orange-600 shadow-orange-300';
        }
        return 'bg-blue-500 border-blue-600 shadow-blue-300';
      case 'pending': return 'bg-gray-100 border-gray-300 border-dashed'; // 빈 공터 느낌
      default: return 'bg-gray-100 border-gray-300 border-dashed';
    }
  };

  const getStatusLabel = (status: PageProgress['status'], retryCount: number) => {
    if (status === 'processing' && retryCount > 0) {
      return `재시도 중 (${retryCount}회)`;
    }
    switch (status) {
      case 'success': return '완료';
      case 'partial': return '부분 수집';
      case 'failed': return '실패';
      case 'processing': return '진행 중';
      case 'pending': return '대기';
      default: return '대기';
    }
  };

  const stats = () => {
    const pagesArray = Array.from(pages().values());
    return {
      successful: pagesArray.filter(p => p.status === 'success').length,
      partial: pagesArray.filter(p => p.status === 'partial').length,
      failed: pagesArray.filter(p => p.status === 'failed').length,
      pending: pagesArray.filter(p => p.status === 'pending').length,
      total: pagesArray.length,
    };
  };

  return (
    <Show when={isActive() && sessionActive()}>
      <div class="bg-white rounded-lg shadow-lg p-4 border border-blue-200">
        <div class="mb-3">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-base font-semibold text-gray-800">
              📄 ListPage 크롤링 진행상황
            </h3>
            <div class="flex items-center gap-3 text-xs">
              <span class="text-gray-600">Total: {stats().total}</span>
              <span class="text-green-600">✓ {stats().successful}</span>
              <span class="text-yellow-600">! {stats().partial}</span>
              <span class="text-red-600">✗ {stats().failed}</span>
              <span class="text-gray-400">· {stats().pending}</span>
            </div>
          </div>
          
          {/* 전체 진행률 바 */}
          <div class="relative w-full h-4 bg-gray-200 rounded-full overflow-hidden">
            <div 
              class="absolute top-0 left-0 h-full bg-gradient-to-r from-blue-500 to-blue-600 transition-all duration-300 ease-out"
              style={{ width: `${progressPercentage()}%` }}
            />
            <div class="absolute inset-0 flex items-center justify-center text-[10px] font-semibold text-gray-700">
              {progressPercentage().toFixed(1)}%
            </div>
          </div>
        </div>

        {/* 페이지 그리드 - 원형 디자인으로 빈 공터 → 깃발 꽂기 */}
        <div 
          ref={scrollContainerRef}
          class="grid grid-cols-20 gap-2 max-h-64 overflow-y-auto p-2 bg-gray-50 rounded border border-gray-200"
          style="grid-template-columns: repeat(20, minmax(0, 1fr))"
        >
          <For each={Array.from(pages().values()).sort((a, b) => b.pageNumber - a.pageNumber)}>
            {(page) => (
              <div
                data-page={page.pageNumber}
                class="flex flex-col items-center gap-0.5"
              >
                {/* 상태 표시 원/사각형 */}
                <div
                  class={`
                    relative group cursor-pointer transition-all duration-200 
                    ${page.status === 'pending' ? 'rounded-full' : 'rounded-sm'}
                    border-2 ${getStatusColor(page.status, page.retryCount)} 
                    hover:scale-125 hover:z-10
                  `}
                  classList={{
                    'animate-pulse': page.status === 'processing',
                  }}
                  style={{
                    "width": "24px",
                    "height": "24px",
                    "animation": page.status === 'processing' ? 'pulse 1s cubic-bezier(0.4, 0, 0.6, 1) infinite' : 'none',
                  }}
                  title={`Page ${page.pageNumber}${page.retryCount > 0 ? ` (재시도 ${page.retryCount}회)` : ''} - ${getStatusLabel(page.status, page.retryCount)}`}
                >
                  {/* 메인 아이콘 - 진행 중이면 회전 애니메이션 */}
                  <div 
                    class="absolute inset-0 flex items-center justify-center text-white font-bold drop-shadow select-none"
                    style={{
                      "font-size": page.status === 'success' ? "14px" : "12px",
                      "animation": page.status === 'processing' ? 'spin 1s linear infinite' : 'none',
                    }}
                  >
                    {getStatusIcon(page.status)}
                  </div>
                  
                  {/* 재시도 뱃지 - 우측 상단 */}
                  <Show when={page.retryCount > 0}>
                    <div class="absolute -top-1 -right-1 bg-orange-700 text-white rounded-full w-3 h-3 flex items-center justify-center text-[7px] font-bold shadow-sm ring-1 ring-white">
                      {page.retryCount}
                    </div>
                  </Show>
                  
                  {/* 상세 툴팁 */}
                  <div class="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2 hidden group-hover:block z-20 w-60">
                    <div class="bg-gray-900 text-white text-[10px] rounded-lg p-2.5 shadow-2xl border border-gray-700">
                      <div class="font-semibold mb-1.5 pb-1 border-b border-gray-700 flex items-center gap-1">
                        📄 <span>페이지 {page.pageNumber}</span>
                      </div>
                      
                      <div class="space-y-1">
                        <div class="flex justify-between items-center">
                          <span class="text-gray-400">상태:</span>
                          <span class={`font-semibold ${
                            page.status === 'success' ? 'text-emerald-400' :
                            page.status === 'partial' ? 'text-amber-400' :
                            page.status === 'failed' ? 'text-rose-400' :
                            page.status === 'processing' ? (page.retryCount > 0 ? 'text-orange-400' : 'text-blue-400') :
                            'text-gray-400'
                          }`}>
                            {getStatusLabel(page.status, page.retryCount)}
                          </span>
                        </div>
                        
                        <div class="flex justify-between">
                          <span class="text-gray-400">페이지 ID:</span>
                          <span class="font-mono">{page.pageId}</span>
                        </div>
                        
                        <Show when={page.collectedUrls !== undefined}>
                          <div class="flex justify-between">
                            <span class="text-gray-400">수집 URL:</span>
                            <span class={page.collectedUrls === page.expectedUrls ? 'text-emerald-400' : 'text-amber-400'}>
                              {page.collectedUrls} / {page.expectedUrls ?? '?'}
                            </span>
                          </div>
                        </Show>
                        
                        <Show when={page.retryCount > 0}>
                          <div class="flex justify-between pt-1 border-t border-gray-700">
                            <span class="text-gray-400">재시도:</span>
                            <span class="text-orange-400 font-semibold">{page.retryCount}회</span>
                          </div>
                        </Show>
                        
                        <Show when={page.error}>
                          <div class="pt-1 border-t border-gray-700">
                            <div class="text-gray-400 mb-0.5">⚠️ 오류:</div>
                            <div class="text-rose-400 break-words leading-tight">{page.error}</div>
                          </div>
                        </Show>
                      </div>
                    </div>
                  </div>
                </div>

                {/* 페이지 번호 라벨 */}
                <div class="text-[9px] text-gray-600 font-medium leading-none">
                  {page.pageNumber}
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </Show>
  );
}
