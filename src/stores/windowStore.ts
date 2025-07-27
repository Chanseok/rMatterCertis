/**
 * windowStore - 윈도우 상태 관리 스토어
 * 윈도우 위치, 크기, 줌 레벨, 마지막 탭 등을 저장하고 복원
 */

import { createStore } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';

// ts-rs로 생성된 타입들 import (Modern Rust 2024 ts-rs 정책)
import type { WindowState as TauriWindowState } from '../types/generated/WindowState';
import type { WindowPosition } from '../types/generated/WindowPosition';
import type { WindowSize } from '../types/generated/WindowSize';

// 내부 상태용 camelCase 타입 정의
interface InternalWindowState {
  position: WindowPosition;
  size: WindowSize;
  zoomLevel: number;
  lastActiveTab: string;
  isMaximized: boolean;
}

interface WindowStore {
  state: InternalWindowState;
  isInitialized: boolean;
  saveState: () => Promise<void>;
  restoreState: () => Promise<void>;
  applyWindowSettings: () => Promise<void>;
  setPosition: (x: number, y: number) => void;
  setSize: (width: number, height: number) => void;
  setZoomLevel: (level: number) => void;
  setLastActiveTab: (tab: string) => void;
  setMaximized: (maximized: boolean) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  resetZoom: () => void;
}

const DEFAULT_STATE: InternalWindowState = {
  position: { x: 100, y: 100 },
  size: { width: 1200, height: 800 },
  zoomLevel: 1.0,
  lastActiveTab: 'settings',
  isMaximized: false
};

const [windowState, setWindowState] = createStore<WindowStore>({
  state: { ...DEFAULT_STATE },
  isInitialized: false,

  // 상태 저장
  async saveState() {
    try {
      // Backend expects snake_case field names
      const stateToSave = {
        position: windowState.state.position,
        size: windowState.state.size,
        zoom_level: windowState.state.zoomLevel,  // camelCase to snake_case
        last_active_tab: windowState.state.lastActiveTab,  // camelCase to snake_case
        is_maximized: windowState.state.isMaximized  // camelCase to snake_case
      };
      await invoke('save_window_state', { state: stateToSave });
      console.log('🔧 Window state saved:', stateToSave);
    } catch (error) {
      console.error('❌ Failed to save window state:', error);
      // Fallback to localStorage
      localStorage.setItem('windowState', JSON.stringify(windowState.state));
    }
  },

  // 상태 복원
  async restoreState() {
    try {
      // Tauri에서 상태 로드 시도
      const savedState = await invoke<any>('load_window_state'); // any 타입으로 받아서 변환
      if (savedState) {
        // snake_case를 camelCase로 변환
        const convertedState: InternalWindowState = {
          position: savedState.position,
          size: savedState.size,
          zoomLevel: savedState.zoom_level || savedState.zoomLevel || 1.0,
          lastActiveTab: savedState.last_active_tab || savedState.lastActiveTab || 'settings',
          isMaximized: savedState.is_maximized || savedState.isMaximized || false
        };
        
        setWindowState('state', convertedState);
        console.log('🔧 Window state restored from Tauri:', convertedState);
        
        // 윈도우 위치와 크기 적용
        await windowState.applyWindowSettings();
        setWindowState('isInitialized', true);
        return;
      }
    } catch (error) {
      console.warn('⚠️ Failed to load from Tauri, trying localStorage:', error);
    }

    // Fallback to localStorage
    try {
      const savedState = localStorage.getItem('windowState');
      if (savedState) {
        const parsed = JSON.parse(savedState) as InternalWindowState;
        setWindowState('state', { ...DEFAULT_STATE, ...parsed });
        console.log('🔧 Window state restored from localStorage:', parsed);
      }
    } catch (error) {
      console.error('❌ Failed to restore window state:', error);
    }

    setWindowState('isInitialized', true);
  },

  // 윈도우 설정 적용
  async applyWindowSettings() {
    try {
      const { position, size, isMaximized } = windowState.state;
      
      if (isMaximized) {
        await invoke('maximize_window');
      } else {
        await invoke('set_window_position', { x: position.x, y: position.y });
        await invoke('set_window_size', { width: size.width, height: size.height });
      }
      
      // 줌 레벨 적용 (안전하게)
      if (typeof document !== 'undefined' && document.documentElement) {
        document.documentElement.style.zoom = windowState.state.zoomLevel.toString();
      }
    } catch (error) {
      console.error('❌ Failed to apply window settings:', error);
    }
  },

  // 위치 설정
  setPosition(x: number, y: number) {
    setWindowState('state', 'position', { x, y });
    windowState.saveState();
  },

  // 크기 설정
  setSize(width: number, height: number) {
    setWindowState('state', 'size', { width, height });
    windowState.saveState();
  },

  // 줌 레벨 설정
  setZoomLevel(level: number) {
    const clampedLevel = Math.max(0.5, Math.min(3.0, level));
    setWindowState('state', 'zoomLevel', clampedLevel);
    
    // CSS zoom 속성 적용 (안전하게)
    try {
      if (typeof document !== 'undefined' && document.documentElement) {
        document.documentElement.style.zoom = clampedLevel.toString();
      }
    } catch (error) {
      console.warn('Failed to set zoom level:', error);
    }
    
    windowState.saveState();
  },

  // 마지막 활성 탭 설정
  setLastActiveTab(tab: string) {
    setWindowState('state', 'lastActiveTab', tab);
    windowState.saveState();
  },

  // 최대화 상태 설정
  setMaximized(maximized: boolean) {
    setWindowState('state', 'isMaximized', maximized);
    windowState.saveState();
  },

  // 줌 인
  zoomIn() {
    const newLevel = windowState.state.zoomLevel + 0.1;
    windowState.setZoomLevel(newLevel);
  },

  // 줌 아웃
  zoomOut() {
    const newLevel = windowState.state.zoomLevel - 0.1;
    windowState.setZoomLevel(newLevel);
  },

  // 줌 리셋
  resetZoom() {
    windowState.setZoomLevel(1.0);
  }
});

export { windowState };
