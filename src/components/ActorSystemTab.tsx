import { Component, createSignal } from 'solid-js';
import { ActorSystemDashboard } from './actor-system/ActorSystemDashboard';
import { CrawlingProgressMonitor } from './actor-system/CrawlingProgressMonitor';

// ActorSystemTab - OneShot Actor System과 크롤링 모니터링을 위한 통합 탭
const ActorSystemTab: Component = () => {
  const [activeView, setActiveView] = createSignal<'dashboard' | 'progress'>('dashboard');

  // Tab Navigation Header
  const TabHeader = () => (
    <div class="flex space-x-1 bg-gray-100 p-1 rounded-lg mb-6">
      <button
        onClick={() => setActiveView('dashboard')}
        class={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
          activeView() === 'dashboard'
            ? 'bg-white text-blue-600 shadow-sm'
            : 'text-gray-600 hover:text-gray-900'
        }`}
      >
        🎭 Actor System Dashboard
      </button>
      <button
        onClick={() => setActiveView('progress')}
        class={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
          activeView() === 'progress'
            ? 'bg-white text-blue-600 shadow-sm'
            : 'text-gray-600 hover:text-gray-900'
        }`}
      >
        📊 Crawling Progress Monitor
      </button>
    </div>
  );

  return (
    <div class="p-6 max-w-7xl mx-auto">
      {/* Header Section */}
      <div class="mb-6">
        <h1 class="text-3xl font-bold text-gray-900 mb-2">
          OneShot Actor System
        </h1>
        <p class="text-lg text-gray-600">
          실시간 Actor 시스템 모니터링 및 크롤링 세션 관리
        </p>
      </div>

      {/* Tab Navigation */}
      <TabHeader />

      {/* Tab Content */}
      <div class="min-h-[600px]">
        {activeView() === 'dashboard' && <ActorSystemDashboard />}
        {activeView() === 'progress' && <CrawlingProgressMonitor />}
      </div>
    </div>
  );
};

export default ActorSystemTab;
