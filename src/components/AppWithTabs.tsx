/**
 * AppWithTabs - 새로운 탭 기반 메인 애플리케이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createMemo } from 'solid-js';
import { AppLayout } from './layout/AppLayout';
import { SettingsTab } from './tabs/SettingsTab';
import { StatusTab } from './tabs/StatusTab';
import { LocalDBTab } from './tabs/LocalDBTab';
import { AnalysisTab } from './tabs/AnalysisTab';
import { LiveProductionTab } from './tabs/LiveProductionTab';
import { tabState } from '../stores/tabStore';

export const AppWithTabs: Component = () => {
  const currentTab = createMemo(() => tabState.activeTab);

  const renderTabContent = () => {
    switch (currentTab()) {
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
      default:
        return <StatusTab />;
    }
  };

  return (
    <AppLayout>
      {renderTabContent()}
    </AppLayout>
  );
};
