/**
 * AppWithTabs - 새로운 탭 기반 메인 애플리케이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createMemo, onMount } from 'solid-js';
import { AppLayout } from './layout/AppLayout';
import CrawlingEngineTabSimple from './tabs/CrawlingEngineTabSimple';
import { CrawlingEngineTab } from './tabs/CrawlingEngineTab';
import { SettingsTab } from './tabs/SettingsTab';
import { StatusTab } from './tabs/StatusTab';
import { LocalDBTab } from './tabs/LocalDBTab';
import { AnalysisTab } from './tabs/AnalysisTab';
import SimpleEventDisplay from './SimpleEventDisplay';
// Archived tabs removed from runtime imports:
// LiveProductionTab, LiveCrawlingTab, NewArchTestTab, ActorSystemTab,
// DomainDashboardTab, RealtimeDashboardTab, HierarchicalEventMonitor
import { tabState, restoreLastActiveTab } from '../stores/tabStore';
let auditEnabled = false; // dev event audit flag
import { windowState } from '../stores/windowStore';
import { eventStore } from '../stores/eventStore';

export const AppWithTabs: Component = () => {
  const currentTab = createMemo(() => tabState.activeTab);

  // 앱 시작 시 초기화
  onMount(async () => {
    console.log('🚀 AppWithTabs 초기화 시작...');
    
    try {
  // 전역 이벤트 스토어 초기화 (비활성 탭이어도 이벤트 버퍼링)
  await eventStore.initOnce();

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

  const renderTabContent = () => {
    switch (currentTab()) {
      case 'crawlingEngine':
  // Use the Simple variant (unified start + EventConsole) for Advanced Engine tab
  return <CrawlingEngineTabSimple />;
      case 'settings':
        return <SettingsTab />;
      case 'localDB':
        return <LocalDBTab />;
      case 'analysis':
        return <AnalysisTab />;
      case 'events':
        return <SimpleEventDisplay />;
      default:
  return <CrawlingEngineTabSimple />;
    }
  };

  return (
    <AppLayout>
      {renderTabContent()}
    </AppLayout>
  );
};
