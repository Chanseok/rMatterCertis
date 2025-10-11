/**
 * Header - 애플리케이션 헤더 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, onMount, onCleanup } from 'solid-js';
import { ZoomControls } from '../common/ZoomControls';

export const Header: Component = () => {
  // Live clock (updates every 30s)
  const formatTime = () =>
    new Date().toLocaleTimeString("ko-KR", {
      hour: "2-digit",
      minute: "2-digit",
    });
  const [clock, setClock] = createSignal<string>(formatTime());

  onMount(() => {
    const id = setInterval(() => setClock(formatTime()), 30_000);
    setClock(formatTime());
    onCleanup(() => clearInterval(id));
  });
  return (
    <header class="bg-white dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
      <div class="px-6 py-4">
        <div class="flex items-center justify-between">
          {/* 로고 및 타이틀 */}
          <div class="flex items-center space-x-3">
            <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <span class="text-white font-bold text-sm">MC</span>
            </div>
            <div>
              <h1 class="text-xl font-bold text-gray-900 dark:text-white">
                Matter Certification Crawler
              </h1>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                인증 정보 수집 및 관리 시스템
              </p>
            </div>
          </div>

          {/* 줌 컨트롤 및 시간 표시 */}
          <div class="flex items-center space-x-4">
            {/* 줌 컨트롤 */}
            <ZoomControls />
            
            {/* 현재 시간 표시 */}
            <div class="flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-blue-50 to-indigo-50 dark:from-blue-900/20 dark:to-indigo-900/20 rounded-full border border-blue-200 dark:border-blue-700 shadow-sm">
              <span class="text-xl" aria-hidden>🕐</span>
              <div class="flex flex-col">
                <span class="text-xs text-gray-500 dark:text-gray-400 font-medium leading-none">오늘</span>
                <span class="text-base font-semibold text-blue-700 dark:text-blue-300 tabular-nums leading-tight">
                  {clock()}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
};
