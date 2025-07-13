/**
 * GameDashboardTab - 게임 스타일 크롤링 대시보드 탭
 * 도시 성장 게임과 유사한 시각화로 크롤링 과정을 재미있게 표현
 */

import { Component, createSignal, onMount, onCleanup } from 'solid-js';
import { CrawlingDashboard } from '../CrawlingDashboard';
import type { CrawlingProgress } from '../../types/crawling';
import { CrawlingStage, CrawlingStatus } from '../../types/crawling';

export const GameDashboardTab: Component = () => {
  const [isRunning, setIsRunning] = createSignal(false);
  const [crawlingProgress, setCrawlingProgress] = createSignal<CrawlingProgress | null>(null);
  const [, setSessionId] = createSignal<string | null>(null);

  let progressInterval: number;

  onMount(() => {
    // 시뮬레이션 데이터 초기화
    initializeSimulation();
  });

  onCleanup(() => {
    if (progressInterval) {
      clearInterval(progressInterval);
    }
  });

  const initializeSimulation = () => {
    setCrawlingProgress({
      current: 0,
      total: 1000,
      percentage: 0,
      current_stage: CrawlingStage.Idle,
      current_step: 'Ready to start crawling',
      status: CrawlingStatus.Idle,
      message: 'Click Start to begin crawling simulation',
      remaining_time: 0,
      elapsed_time: 0,
      new_items: 0,
      updated_items: 0,
      errors: 0,
      timestamp: new Date().toISOString()
    });
  };

  const handleToggleRunning = () => {
    const newRunning = !isRunning();
    setIsRunning(newRunning);

    if (newRunning) {
      startCrawlingSimulation();
    } else {
      stopCrawlingSimulation();
    }
  };

  const startCrawlingSimulation = () => {
    const newSessionId = `sim-${Date.now()}`;
    setSessionId(newSessionId);

    const startTime = Date.now();
    const stages = [
      { name: 'StatusCheck', duration: 5000, message: 'Checking system status...' },
      { name: 'DatabaseAnalysis', duration: 8000, message: 'Analyzing database...' },
      { name: 'TotalPages', duration: 3000, message: 'Calculating total pages...' },
      { name: 'ProductList', duration: 30000, message: 'Fetching product lists...' },
      { name: 'ProductDetails', duration: 45000, message: 'Crawling product details...' },
      { name: 'DatabaseSave', duration: 10000, message: 'Saving to database...' }
    ];

    let currentStageIndex = 0;
    let stageStartTime = startTime;
    let itemsProcessed = 0;

    progressInterval = setInterval(() => {
      const now = Date.now();
      const elapsedTime = Math.floor((now - startTime) / 1000);
      
      const currentStage = stages[currentStageIndex];
      const stageElapsed = now - stageStartTime;
      const stageProgress = Math.min(1, stageElapsed / currentStage.duration);
      
      // 단계 진행률 계산
      const completedStages = currentStageIndex;
      const totalStages = stages.length;
      const overallProgress = (completedStages + stageProgress) / totalStages;
      
      // 아이템 처리 시뮬레이션
      if (currentStage.name === 'ProductList' || currentStage.name === 'ProductDetails') {
        const itemsPerSecond = Math.random() * 5 + 2;
        itemsProcessed += itemsPerSecond;
      }

      setCrawlingProgress(prev => ({
        ...prev!,
        current: Math.floor(overallProgress * 1000),
        percentage: overallProgress * 100,
        current_stage: currentStage.name as CrawlingStage,
        current_step: currentStage.message,
        status: CrawlingStatus.Running,
        message: `${currentStage.message} (${Math.floor(stageProgress * 100)}%)`,
        elapsed_time: elapsedTime,
        new_items: Math.floor(itemsProcessed),
        updated_items: Math.floor(itemsProcessed * 0.8),
        timestamp: new Date().toISOString()
      }));

      // 다음 단계로 이동
      if (stageProgress >= 1 && currentStageIndex < stages.length - 1) {
        currentStageIndex++;
        stageStartTime = now;
      }

      // 완료 처리
      if (currentStageIndex >= stages.length - 1 && stageProgress >= 1) {
        setCrawlingProgress(prev => ({
          ...prev!,
          status: CrawlingStatus.Completed,
          current_stage: CrawlingStage.Idle, // 완료된 상태로 변경
          message: 'Crawling completed successfully!',
          percentage: 100,
          current: 1000,
          timestamp: new Date().toISOString()
        }));
        setIsRunning(false);
        clearInterval(progressInterval);
      }
    }, 1000);
  };

  const stopCrawlingSimulation = () => {
    if (progressInterval) {
      clearInterval(progressInterval);
    }
    
    setCrawlingProgress(prev => ({
      ...prev!,
      status: CrawlingStatus.Cancelled,
      message: 'Crawling stopped by user',
      timestamp: new Date().toISOString()
    }));
  };

  const handlePauseResume = () => {
    if (isRunning()) {
      setIsRunning(false);
      if (progressInterval) {
        clearInterval(progressInterval);
      }
      setCrawlingProgress(prev => ({
        ...prev!,
        status: CrawlingStatus.Paused,
        message: 'Crawling paused',
        timestamp: new Date().toISOString()
      }));
    } else {
      setIsRunning(true);
      startCrawlingSimulation();
    }
  };

  const handleStop = () => {
    setIsRunning(false);
    stopCrawlingSimulation();
    
    // 리셋
    setTimeout(() => {
      initializeSimulation();
    }, 2000);
  };

  return (
    <div class="w-full h-full">
      <CrawlingDashboard
        progress={crawlingProgress()}
        isRunning={isRunning()}
        onToggleRunning={handleToggleRunning}
        onPauseResume={handlePauseResume}
        onStop={handleStop}
      />
    </div>
  );
};
