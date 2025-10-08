/**
 * AppWithTabs - 새로운 탭 기반 메인 애플리케이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, Show, createMemo, onMount } from 'solid-js';
import { AppLayout } from './layout/AppLayout';
import CrawlingEngineTabSimple from './tabs/CrawlingEngineTabSimple';
import { SettingsTab } from './tabs/SettingsTab';
import { LocalDBTab } from './tabs/LocalDBTab';
// Archived tabs removed from runtime imports:
// LiveProductionTab, LiveCrawlingTab, NewArchTestTab, ActorSystemTab,
// DomainDashboardTab, RealtimeDashboardTab, HierarchicalEventMonitor
// AnalysisTab, DatabaseDiagnostics (2025-10-08: 사용하지 않는 탭 제거)
import { tabState, restoreLastActiveTab } from '../stores/tabStore';
let auditEnabled = false; // dev event audit flag
import { windowState } from '../stores/windowStore';

export const AppWithTabs: Component = () => {
  const currentTab = createMemo(() => tabState.activeTab);

  // 앱 시작 시 초기화
  onMount(async () => {
    console.log('🚀 AppWithTabs 초기화 시작...');
    
    try {
      // 윈도우 상태 복원 (위치, 크기, 줌 레벨, 마지막 탭 등)
      await windowState.restoreState();
      
      // 마지막 활성 탭 복원
      if (windowState.state.lastActiveTab) {
        console.log(`🔄 마지막 활성 탭 복원: ${windowState.state.lastActiveTab}`);
        restoreLastActiveTab(windowState.state.lastActiveTab);
      }
      
      console.log('✅ AppWithTabs 초기화 완료');
      if (import.meta.env.VITE_EVENT_AUDIT === 'true' && !auditEnabled) {
        auditEnabled = true;
        import('../dev/eventAudit').then(m => m.enableEventAudit()).catch(e => console.error('event audit load failed', e));
      }
    } catch (error) {
      console.error('❌ AppWithTabs 초기화 실패:', error);
    }
  });

  return (
    <AppLayout>
      {/* Advanced Engine tab: keep mounted to preserve state across tab switches */}
      <div class={currentTab() === 'crawlingEngine' ? '' : 'hidden'}>
        <CrawlingEngineTabSimple />
      </div>

      {/* Other tabs: mount/unmount as before */}
      <Show when={currentTab() === 'settings'}>
        <SettingsTab />
      </Show>
      <Show when={currentTab() === 'localDB'}>
        <LocalDBTab />
      </Show>
  {/* 기존 'Local DB Dashboard' 중복 탭 제거 (6B 재정비 단계) */}
  {/* Analysis, Diagnostics 탭 제거 (2025-10-08: 사용하지 않는 탭) */}
    </AppLayout>
  );
};
