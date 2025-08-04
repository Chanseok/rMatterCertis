/**
 * Realtime Dashboard Tab - Chart.js 기반 실시간 크롤링 대시보드 탭
 */

import { Component, createSignal, onMount } from 'solid-js';
import MainDashboard from '../dashboard/MainDashboard';

export const RealtimeDashboardTab: Component = () => {
  const [isLoading, setIsLoading] = createSignal(true);

  onMount(() => {
    // 초기화 시뮬레이션
    setTimeout(() => {
      setIsLoading(false);
    }, 800);
  });

  return (
    <div class="h-full flex flex-col">
      {/* 탭 헤더 */}
      <div class="bg-white border-b border-gray-200 px-6 py-4">
        <h1 class="text-xl font-semibold text-gray-900">🚀 실시간 크롤링 대시보드</h1>
        <p class="text-sm text-gray-600 mt-1">
          Chart.js 기반 실시간 성능 모니터링 및 크롤링 제어
        </p>
      </div>

      {/* 메인 콘텐츠 */}
      <div class="flex-1 overflow-auto">
        {isLoading() ? (
          <div class="flex items-center justify-center h-full">
            <div class="text-center">
              <div class="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
              <p class="mt-4 text-gray-600">실시간 대시보드 로딩 중...</p>
            </div>
          </div>
        ) : (
          <MainDashboard autoRefreshInterval={5000} />
        )}
      </div>
    </div>
  );
};

export default RealtimeDashboardTab;
