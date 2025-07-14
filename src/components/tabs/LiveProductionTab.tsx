/**
 * LiveProductionTab.tsx
 * @description Live Production Line UI를 위한 탭 컴포넌트
 */

import { Component } from 'solid-js';
import { CrawlingProcessDashboard } from '../CrawlingProcessDashboard';

export const LiveProductionTab: Component = () => {
  return (
    <div class="h-full flex flex-col">
      <div class="flex-shrink-0 border-b border-gray-200 p-4">
        <h2 class="text-xl font-semibold text-gray-800">Live Production Line</h2>
        <p class="text-sm text-gray-600 mt-1">
          실시간 크롤링 프로세스를 3D 그래프로 시각화합니다
        </p>
      </div>
      
      <div class="flex-1 overflow-hidden">
        <CrawlingProcessDashboard />
      </div>
    </div>
  );
};
