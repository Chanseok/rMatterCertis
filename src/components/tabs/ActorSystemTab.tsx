/**
 * ActorSystemTab - OneShot Actor 시스템 통합 대시보드 탭
 * Phase C: UI 개선 - Actor 시스템과 크롤링 진행률을 통합한 모니터링 인터페이스
 */

import { Component, createSignal } from 'solid-js';
import { ActorSystemDashboard } from '../actor-system/ActorSystemDashboard';
import { CrawlingProgressMonitor } from '../actor-system/CrawlingProgressMonitor';
import './ActorSystemTab.css';

export const ActorSystemTab: Component = () => {
  const [activeView, setActiveView] = createSignal<'dashboard' | 'progress'>('dashboard');

  const handleViewChange = (view: 'dashboard' | 'progress') => {
    setActiveView(view);
  };

  return (
    <div class="actor-system-tab">
      {/* 탭 네비게이션 */}
      <div class="tab-navigation">
        <div class="nav-header">
          <h1 class="tab-title">
            <span class="icon">🎭</span>
            OneShot Actor System
          </h1>
          <p class="tab-description">
            Modern Rust 2024 기반 Actor 시스템 모니터링 및 실시간 크롤링 진행률 추적
          </p>
        </div>
        
        <div class="nav-buttons">
          <button 
            class={`nav-btn ${activeView() === 'dashboard' ? 'active' : ''}`}
            onClick={() => handleViewChange('dashboard')}
          >
            <span class="btn-icon">📊</span>
            <div class="btn-content">
              <span class="btn-title">시스템 대시보드</span>
              <span class="btn-subtitle">Actor 상태 & 성능 모니터링</span>
            </div>
          </button>
          
          <button 
            class={`nav-btn ${activeView() === 'progress' ? 'active' : ''}`}
            onClick={() => handleViewChange('progress')}
          >
            <span class="btn-icon">🚀</span>
            <div class="btn-content">
              <span class="btn-title">크롤링 진행률</span>
              <span class="btn-subtitle">실시간 작업 진행률 모니터링</span>
            </div>
          </button>
        </div>
      </div>

      {/* 탭 컨텐츠 */}
      <div class="tab-content">
        {activeView() === 'dashboard' && <ActorSystemDashboard />}
        {activeView() === 'progress' && <CrawlingProgressMonitor />}
      </div>
    </div>
  );
};
