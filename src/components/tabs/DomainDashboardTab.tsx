/**
 * Domain Dashboard Tab - 새로운 타입 시스템을 활용한 도메인 상태 탭
 */

import { Component, createSignal, onMount } from 'solid-js';
import { DomainStatusDashboard } from '../dashboard/DomainStatusDashboard';
import { modernCrawlerStore } from '../../stores/modernCrawlerStore';

export const DomainDashboardTab: Component = () => {
  const [isLoading, setIsLoading] = createSignal(true);

  onMount(() => {
    // 초기 데이터 로딩 시뮬레이션
    setTimeout(() => {
      setIsLoading(false);
    }, 1000);
  });

  return (
    <div class="h-full flex flex-col">
      {/* 탭 헤더 */}
      <div class="bg-white border-b border-gray-200 px-6 py-4">
        <h1 class="text-xl font-semibold text-gray-900">도메인 대시보드</h1>
        <p class="text-sm text-gray-600 mt-1">
          자동 생성된 타입 시스템을 활용한 실시간 도메인 상태 모니터링
        </p>
      </div>

      {/* 메인 콘텐츠 */}
      <div class="flex-1 overflow-auto">
        <DomainStatusDashboard
          siteStatus={modernCrawlerStore.state.siteStatus}
          databaseAnalysis={modernCrawlerStore.state.databaseAnalysis}
          processingStrategy={modernCrawlerStore.state.processingStrategy}
          recentProducts={modernCrawlerStore.state.recentProducts}
        />
      </div>
    </div>
  );
};
