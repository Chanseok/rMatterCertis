/**
 * LiveProductionTab.tsx
 * @description Live Production Line UI를 위한 탭 컴포넌트
 */

import { Component, createSignal, onMount, onCleanup } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import CrawlingProgressDashboard from '../visualization/CrawlingProgressDashboard';

export const LiveProductionTab: Component = () => {
  // 크롤링 상태 관리
  const [isCrawlingActive, setIsCrawlingActive] = createSignal(false);
  const [crawlingStatus, setCrawlingStatus] = createSignal('대기 중');
  const [activeBatches, setActiveBatches] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(0);
  const [totalProducts, setTotalProducts] = createSignal(0);
  const [errorCount, setErrorCount] = createSignal(0);

  // 크롤링 시작
  const startCrawling = async () => {
    try {
      setIsCrawlingActive(true);
      setCrawlingStatus('크롤링 시작 중...');
      
      // Tauri 백엔드 크롤링 시작 명령
      await invoke('start_crawling', {
        config: {
          batchSize: 50,
          maxConcurrency: 10,
          retryCount: 3,
          delayMs: 1000
        }
      });
      
      setCrawlingStatus('크롤링 실행 중');
    } catch (error) {
      console.error('크롤링 시작 실패:', error);
      setCrawlingStatus('크롤링 시작 실패');
      setIsCrawlingActive(false);
    }
  };

  // 크롤링 중지
  const stopCrawling = async () => {
    try {
      setCrawlingStatus('크롤링 중지 중...');
      
      // Tauri 백엔드 크롤링 중지 명령
      await invoke('stop_crawling');
      
      setIsCrawlingActive(false);
      setCrawlingStatus('크롤링 중지됨');
    } catch (error) {
      console.error('크롤링 중지 실패:', error);
      setCrawlingStatus('크롤링 중지 실패');
    }
  };

  // 크롤링 상태 리셋
  const resetCrawling = () => {
    setIsCrawlingActive(false);
    setCrawlingStatus('대기 중');
    setActiveBatches(0);
    setTotalPages(0);
    setTotalProducts(0);
    setErrorCount(0);
  };

  // 실시간 이벤트 리스너 설정
  const setupEventListeners = async () => {
    try {
      // 배치 생성 이벤트
      await listen('batch-created', (event: any) => {
        console.log('배치 생성됨:', event.payload);
        setActiveBatches(prev => prev + 1);
      });

      // 페이지 크롤링 이벤트
      await listen('page-crawled', (event: any) => {
        console.log('페이지 크롤링됨:', event.payload);
        setTotalPages(prev => prev + 1);
      });

      // 제품 수집 이벤트
      await listen('product-collected', (event: any) => {
        console.log('제품 수집됨:', event.payload);
        setTotalProducts(prev => prev + 1);
      });

      // 배치 완료 이벤트
      await listen('batch-completed', (event: any) => {
        console.log('배치 완료됨:', event.payload);
        setActiveBatches(prev => Math.max(0, prev - 1));
      });

      // 크롤링 완료 이벤트
      await listen('crawling-completed', (event: any) => {
        console.log('크롤링 완료:', event.payload);
        setIsCrawlingActive(false);
        setCrawlingStatus('크롤링 완료');
      });

      // 에러 이벤트
      await listen('crawling-error', (event: any) => {
        console.error('크롤링 에러:', event.payload);
        setErrorCount(prev => prev + 1);
      });

    } catch (error) {
      console.error('이벤트 리스너 설정 실패:', error);
    }
  };

  // 컴포넌트 마운트
  onMount(() => {
    setupEventListeners();
  });

  // 컴포넌트 언마운트
  onCleanup(() => {
    // 필요시 정리 작업
  });

  return (
    <div class="h-full flex flex-col">
      <div class="flex-shrink-0 border-b border-gray-200 p-4">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h2 class="text-xl font-semibold text-gray-800">Live Production Line</h2>
            <p class="text-sm text-gray-600 mt-1">
              실시간 크롤링 프로세스를 3D 그래프로 시각화합니다
            </p>
          </div>
          
          {/* 크롤링 제어 버튼 */}
          <div class="flex items-center space-x-3">
            <button
              onClick={startCrawling}
              disabled={isCrawlingActive()}
              class="px-6 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:bg-gray-400 disabled:cursor-not-allowed flex items-center space-x-2"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h1m4 0h1m-6 4h1m4 0h1m-6-8h1m4 0h1M9 6h1m4 0h1M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
              </svg>
              <span>{isCrawlingActive() ? '크롤링 실행 중...' : '크롤링 시작'}</span>
            </button>
            
            <button
              onClick={stopCrawling}
              disabled={!isCrawlingActive()}
              class="px-6 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:bg-gray-400 disabled:cursor-not-allowed flex items-center space-x-2"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 10h6v4H9z" />
              </svg>
              <span>크롤링 중지</span>
            </button>
            
            <button
              onClick={resetCrawling}
              disabled={isCrawlingActive()}
              class="px-4 py-2 bg-gray-500 text-white rounded-lg hover:bg-gray-600 disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              리셋
            </button>
          </div>
        </div>
        
        {/* 실시간 상태 표시 */}
        <div class="grid grid-cols-5 gap-4 text-sm">
          <div class="bg-blue-50 border border-blue-200 rounded-lg p-3">
            <div class="text-blue-600 font-medium">상태</div>
            <div class="text-blue-800 font-semibold">{crawlingStatus()}</div>
          </div>
          
          <div class="bg-green-50 border border-green-200 rounded-lg p-3">
            <div class="text-green-600 font-medium">활성 배치</div>
            <div class="text-green-800 font-semibold">{activeBatches()}</div>
          </div>
          
          <div class="bg-purple-50 border border-purple-200 rounded-lg p-3">
            <div class="text-purple-600 font-medium">수집된 페이지</div>
            <div class="text-purple-800 font-semibold">{totalPages().toLocaleString()}</div>
          </div>
          
          <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-3">
            <div class="text-indigo-600 font-medium">수집된 제품</div>
            <div class="text-indigo-800 font-semibold">{totalProducts().toLocaleString()}</div>
          </div>
          
          <div class="bg-red-50 border border-red-200 rounded-lg p-3">
            <div class="text-red-600 font-medium">에러 수</div>
            <div class="text-red-800 font-semibold">{errorCount()}</div>
          </div>
        </div>
      </div>
      
      <div class="flex-1 overflow-hidden">
        <CrawlingProgressDashboard />
      </div>
    </div>
  );
};
