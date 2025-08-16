/**
 * 게임 대시보드 데모 페이지
 * 새로운 시각화 컴포넌트들을 테스트할 수 있는 독립적인 페이지
 */

import { Component } from 'solid-js';
import { GameDashboardTab } from './components/tabs/GameDashboardTab';

export const GameDashboardDemo: Component = () => {
  return (
    <div class="w-full h-screen bg-gray-100">
      <GameDashboardTab />
    </div>
  );
};

export default GameDashboardDemo;
