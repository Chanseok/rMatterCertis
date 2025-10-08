import { Show } from 'solid-js';
import { formatDuration, formatTime } from '../utils/timeUtils';
import type { CrawlingProgress } from '../types/crawling';

interface TimeEstimatesPanelProps {
  progress: CrawlingProgress | null;
}

export default function TimeEstimatesPanel(props: TimeEstimatesPanelProps) {
  return (
    <Show when={props.progress?.status === 'Running' && props.progress?.time_estimates}>
      <Show when={props.progress?.time_estimates}>
        {(estimates) => (
          <div class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4 shadow-sm">
            <h3 class="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4 flex items-center gap-2">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              예상 소요 시간
            </h3>

          <div class="space-y-4">
            {/* 배치 정보 */}
            <Show when={estimates().batch_info}>
              {(batchInfo) => (
                <div class="bg-indigo-50 dark:bg-indigo-900/20 rounded-lg p-3 border border-indigo-100 dark:border-indigo-800">
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <svg class="w-4 h-4 text-indigo-600 dark:text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                      </svg>
                      <span class="text-sm font-medium text-indigo-900 dark:text-indigo-100">현재 배치</span>
                    </div>
                    <span class="text-sm font-bold text-indigo-600 dark:text-indigo-400">
                      {batchInfo().current_batch} / {batchInfo().total_batches}
                    </span>
                  </div>
                </div>
              )}
            </Show>
            {/* ListPage 크롤링 통계 */}
            <Show when={estimates().list_page_stats}>
              {(listStats) => (
                <div class="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3 border border-blue-100 dark:border-blue-800">
                  <div class="flex items-center gap-2 mb-2">
                    <svg class="w-4 h-4 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                    <h4 class="text-sm font-medium text-blue-900 dark:text-blue-100">목록 페이지 수집</h4>
                  </div>
                  <div class="grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">완료/총 페이지:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {listStats().completed_pages} / {listStats().total_pages}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">남은 페이지:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {listStats().remaining_pages}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">페이지당 평균:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {formatDuration(listStats().avg_time_per_page_ms)}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">남은 시간:</span>
                      <span class="ml-1 font-semibold text-blue-600 dark:text-blue-400">
                        {formatDuration(listStats().estimated_remaining_ms)}
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </Show>

            {/* Detail 크롤링 통계 */}
            <Show when={estimates().detail_stats}>
              {(detailStats) => (
                <div class="bg-green-50 dark:bg-green-900/20 rounded-lg p-3 border border-green-100 dark:border-green-800">
                  <div class="flex items-center gap-2 mb-2">
                    <svg class="w-4 h-4 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                    </svg>
                    <h4 class="text-sm font-medium text-green-900 dark:text-green-100">상세 정보 수집</h4>
                  </div>
                  <div class="grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">완료/총 제품:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {detailStats().completed_products} / {detailStats().total_products}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">남은 제품:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {detailStats().remaining_products}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">10개당 평균:</span>
                      <span class="ml-1 font-semibold text-gray-900 dark:text-gray-100">
                        {formatDuration(detailStats().avg_time_per_10_products_ms)}
                      </span>
                    </div>
                    <div>
                      <span class="text-gray-600 dark:text-gray-400">남은 시간:</span>
                      <span class="ml-1 font-semibold text-green-600 dark:text-green-400">
                        {formatDuration(detailStats().estimated_remaining_ms)}
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </Show>

            {/* 세션 전체 예상 */}
            <div class="bg-gradient-to-r from-purple-50 to-pink-50 dark:from-purple-900/20 dark:to-pink-900/20 rounded-lg p-4 border-2 border-purple-200 dark:border-purple-700">
              <div class="flex items-center gap-2 mb-3">
                <svg class="w-5 h-5 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                <h4 class="text-base font-bold text-purple-900 dark:text-purple-100">세션 전체 예상</h4>
              </div>
              <div class="space-y-3">
                <div class="flex justify-between items-center bg-white/50 dark:bg-gray-800/50 rounded px-3 py-2">
                  <span class="text-sm font-medium text-gray-700 dark:text-gray-300">총 남은 시간:</span>
                  <span class="text-xl font-bold text-purple-600 dark:text-purple-400">
                    {formatDuration(estimates().total_estimated_remaining_ms)}
                  </span>
                </div>
                <div class="flex justify-between items-center text-sm">
                  <span class="text-gray-600 dark:text-gray-400">예상 완료 시각:</span>
                  <span class="font-semibold text-gray-900 dark:text-gray-100">
                    {formatTime(estimates().estimated_completion_time)}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </Show>
    </Show>
  );
}
