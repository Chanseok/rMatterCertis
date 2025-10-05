import { createSignal, Show, onMount, onCleanup } from "solid-js";
import { listen } from "@tauri-apps/api/event";

interface ComplementProgress {
  totalProducts: number;
  completedProducts: number;
  failedProducts: number;
  currentBatch: number;
  totalBatches: number;
  status: 'idle' | 'running' | 'completed' | 'failed';
  timestamp: number;
}

export default function ComplementCrawlProgressPanel() {
  const [isActive, setIsActive] = createSignal(false);
  const [progress, setProgress] = createSignal<ComplementProgress>({
    totalProducts: 0,
    completedProducts: 0,
    failedProducts: 0,
    currentBatch: 0,
    totalBatches: 0,
    status: 'idle',
    timestamp: Date.now(),
  });

  onMount(async () => {
    console.log('[ComplementCrawlProgress] Component mounted, setting up event listeners...');
    
    // actor-event 통합 이벤트 리스너
    const unlistenActorEvent = await listen<any>('actor-event', (event) => {
      const payload = event.payload;
      const variant = payload.variant;
      
      // 보완 크롤링 세션 시작 감지 (session_id에 'complement-crawl' 포함)
      if (variant === 'SessionStarted' && payload.session_id?.includes('complement-crawl')) {
        console.log('[ComplementCrawlProgress] ===== Complement crawl session started =====');
        console.log('[ComplementCrawlProgress] Full payload:', JSON.stringify(payload, null, 2));
        
        const totalProducts = payload.total_products || 0;
        const totalBatches = payload.total_batches || 0;
        
        setIsActive(true);
        setProgress({
          totalProducts,
          completedProducts: 0,
          failedProducts: 0,
          currentBatch: 0,
          totalBatches,
          status: 'running',
          timestamp: Date.now(),
        });
        
        console.log(`[ComplementCrawlProgress] Initialized: ${totalProducts} products, ${totalBatches} batches`);
      }
      
      // 진행 상황 업데이트 (배치 완료 시)
      if (variant === 'StageProgress' && payload.session_id?.includes('complement-crawl')) {
        console.log('[ComplementCrawlProgress] Batch progress:', payload);
        
        const currentBatch = payload.current_batch || 0;
        const completedProducts = payload.completed_products || 0;
        const failedProducts = payload.failed_products || 0;
        
        setProgress(prev => ({
          ...prev,
          currentBatch,
          completedProducts,
          failedProducts,
          timestamp: Date.now(),
        }));
        
        console.log(`[ComplementCrawlProgress] Batch ${currentBatch} completed: ${completedProducts} done, ${failedProducts} failed`);
      }
      
      // 세션 완료 감지
      if ((variant === 'SessionCompleted' || variant === 'SessionFailed') && 
          payload.session_id?.includes('complement-crawl')) {
        console.log('[ComplementCrawlProgress] Session ended');
        
        setProgress(prev => ({
          ...prev,
          status: variant === 'SessionCompleted' ? 'completed' : 'failed',
          timestamp: Date.now(),
        }));
        
        // 5초 후 패널 숨김
        setTimeout(() => {
          setIsActive(false);
        }, 5000);
      }
    });

    onCleanup(() => {
      console.log('[ComplementCrawlProgress] Cleaning up event listeners');
      unlistenActorEvent();
    });
  });

  const progressPercentage = () => {
    const p = progress();
    if (p.totalProducts === 0) return 0;
    return Math.round((p.completedProducts / p.totalProducts) * 100);
  };

  const batchProgressPercentage = () => {
    const p = progress();
    if (p.totalBatches === 0) return 0;
    return Math.round((p.currentBatch / p.totalBatches) * 100);
  };

  const statusColor = () => {
    const status = progress().status;
    switch (status) {
      case 'running': return 'text-blue-600';
      case 'completed': return 'text-green-600';
      case 'failed': return 'text-red-600';
      default: return 'text-gray-600';
    }
  };

  const statusIcon = () => {
    const status = progress().status;
    switch (status) {
      case 'running': return '🔄';
      case 'completed': return '✅';
      case 'failed': return '❌';
      default: return '⏸️';
    }
  };

  return (
    <Show when={isActive()}>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-lg p-6 mb-6 border-2 border-teal-400">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-bold flex items-center gap-2">
            <span>{statusIcon()}</span>
            <span class={statusColor()}>제품 보완 크롤링 진행 중</span>
          </h3>
          <div class="text-sm text-gray-500">
            {new Date(progress().timestamp).toLocaleTimeString('ko-KR')}
          </div>
        </div>

        {/* 전체 진행률 */}
        <div class="mb-4">
          <div class="flex justify-between items-center mb-2">
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
              전체 제품 진행률
            </span>
            <span class="text-sm font-bold text-teal-600">
              {progress().completedProducts} / {progress().totalProducts}
              <span class="ml-2 text-gray-500">({progressPercentage()}%)</span>
            </span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-4 overflow-hidden">
            <div 
              class="bg-gradient-to-r from-teal-400 to-teal-600 h-4 transition-all duration-500 flex items-center justify-end pr-2"
              style={{ width: `${progressPercentage()}%` }}
            >
              <span class="text-xs text-white font-semibold">
                {progressPercentage() > 10 ? `${progressPercentage()}%` : ''}
              </span>
            </div>
          </div>
        </div>

        {/* 배치 진행률 */}
        <div class="mb-4">
          <div class="flex justify-between items-center mb-2">
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
              배치 진행률 (동시성: 12개)
            </span>
            <span class="text-sm font-bold text-blue-600">
              {progress().currentBatch} / {progress().totalBatches}
              <span class="ml-2 text-gray-500">({batchProgressPercentage()}%)</span>
            </span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-3 overflow-hidden">
            <div 
              class="bg-gradient-to-r from-blue-400 to-blue-600 h-3 transition-all duration-500"
              style={{ width: `${batchProgressPercentage()}%` }}
            />
          </div>
        </div>

        {/* 통계 */}
        <div class="grid grid-cols-3 gap-4 mt-4">
          <div class="bg-teal-50 dark:bg-teal-900/20 rounded-lg p-3 text-center">
            <div class="text-xs text-gray-600 dark:text-gray-400 mb-1">완료</div>
            <div class="text-xl font-bold text-teal-600">
              {progress().completedProducts}
            </div>
          </div>
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-3 text-center">
            <div class="text-xs text-gray-600 dark:text-gray-400 mb-1">실패</div>
            <div class="text-xl font-bold text-red-600">
              {progress().failedProducts}
            </div>
          </div>
          <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-3 text-center">
            <div class="text-xs text-gray-600 dark:text-gray-400 mb-1">남은 제품</div>
            <div class="text-xl font-bold text-gray-600">
              {progress().totalProducts - progress().completedProducts - progress().failedProducts}
            </div>
          </div>
        </div>

        {/* 성공률 */}
        <Show when={progress().completedProducts + progress().failedProducts > 0}>
          <div class="mt-4 p-3 bg-gradient-to-r from-teal-50 to-blue-50 dark:from-teal-900/20 dark:to-blue-900/20 rounded-lg">
            <div class="text-sm text-gray-700 dark:text-gray-300 text-center">
              성공률: {' '}
              <span class="font-bold text-teal-600">
                {Math.round((progress().completedProducts / (progress().completedProducts + progress().failedProducts)) * 100)}%
              </span>
            </div>
          </div>
        </Show>
      </div>
    </Show>
  );
}
