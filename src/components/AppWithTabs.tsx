/**
 * AppWithTabs - 새로운 탭 기반 메인 애플리케이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createMemo, onMount } from 'solid-js';
import { AppLayout } from './layout/AppLayout';
import CrawlingEngineTabSimple from './tabs/CrawlingEngineTabSimple';
import { SettingsTab } from './tabs/SettingsTab';
import { StatusTab } from './tabs/StatusTab';
import { LocalDBTab } from './tabs/LocalDBTab';
import { AnalysisTab } from './tabs/AnalysisTab';
import { LiveProductionTab } from './tabs/LiveProductionTab';
import { NewArchTestTab } from './tabs/NewArchTestTab';
import { ActorSystemTab } from './tabs/ActorSystemTab';
import { DomainDashboardTab } from './tabs/DomainDashboardTab';
import { SimpleEventDisplay } from './SimpleEventDisplay';
import { tabState, restoreLastActiveTab } from '../stores/tabStore';
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
    } catch (error) {
      console.error('❌ AppWithTabs 초기화 실패:', error);
    }
  });

  const renderTabContent = () => {
    switch (currentTab()) {
      case 'crawlingEngine':
        return <CrawlingEngineTabSimple />;
      case 'settings':
        return <SettingsTab />;
      case 'status':
        return <StatusTab />;
      case 'localDB':
        return <LocalDBTab />;
      case 'liveProduction':
        return <LiveProductionTab />;
      case 'analysis':
        return <AnalysisTab />;
      case 'newArchTest':
        return <NewArchTestTab />;
      case 'actorSystem':
        return <ActorSystemTab />;
      case 'domainDashboard':
        return <DomainDashboardTab />;
      case 'eventDisplay':
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
