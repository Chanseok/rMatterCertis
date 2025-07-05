/**
 * settingsStore - 애플리케이션 설정 관리 스토어
 * 백엔드와 연동하여 설정을 저장/로드
 */

import { createStore } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';

// 설정 타입 정의
export interface CrawlingSettings {
  concurrent_downloads: number;
  request_delay_ms: number;
  timeout_seconds: number;
  retry_count: number;
}

export interface LoggingSettings {
  level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
  terminal_output: boolean;
  file_logging: boolean;
  max_file_size_mb: number;
  max_files: number;
  auto_cleanup_logs: boolean;
}

export interface BatchSettings {
  batch_size: number;
  progress_interval_ms: number;
  auto_backup: boolean;
  batch_delay_ms: number;
  enable_batch_processing: boolean;
}

export interface AppSettings {
  crawling: CrawlingSettings;
  logging: LoggingSettings;
  batch: BatchSettings;
}

interface SettingsStore {
  settings: AppSettings;
  isLoading: boolean;
  isDirty: boolean;
  lastSaved: string | null;
  expandedSections: {
    basic: boolean;
    logging: boolean;
    batch: boolean;
    advanced: boolean;
  };
  
  // 메서드들
  loadSettings: () => Promise<void>;
  saveSettings: () => Promise<void>;
  resetToDefaults: () => Promise<void>;
  updateCrawlingSettings: (settings: Partial<CrawlingSettings>) => void;
  updateLoggingSettings: (settings: Partial<LoggingSettings>) => void;
  updateBatchSettings: (settings: Partial<BatchSettings>) => void;
  toggleSection: (section: keyof SettingsStore['expandedSections']) => void;
  markDirty: () => void;
}

// 기본 설정값
const DEFAULT_SETTINGS: AppSettings = {
  crawling: {
    concurrent_downloads: 3,
    request_delay_ms: 1000,
    timeout_seconds: 30,
    retry_count: 3
  },
  logging: {
    level: 'INFO',
    terminal_output: true,
    file_logging: true,
    max_file_size_mb: 10,
    max_files: 5,
    auto_cleanup_logs: true
  },
  batch: {
    batch_size: 50,
    progress_interval_ms: 1000,
    auto_backup: true,
    batch_delay_ms: 100,
    enable_batch_processing: true
  }
};

const [settingsState, setSettingsState] = createStore<SettingsStore>({
  settings: { ...DEFAULT_SETTINGS },
  isLoading: false,
  isDirty: false,
  lastSaved: null,
  expandedSections: {
    basic: true,
    logging: true,
    batch: true,
    advanced: false
  },

  // 설정 로드
  async loadSettings() {
    setSettingsState('isLoading', true);
    try {
      console.log('🔧 Loading settings from backend...');
      const loadedSettings = await invoke<AppSettings>('get_app_settings');
      
      if (loadedSettings) {
        setSettingsState('settings', { ...DEFAULT_SETTINGS, ...loadedSettings });
        console.log('✅ Settings loaded:', loadedSettings);
      }
    } catch (error) {
      console.error('❌ Failed to load settings:', error);
      
      // Fallback to localStorage
      try {
        const localSettings = localStorage.getItem('appSettings');
        if (localSettings) {
          const parsed = JSON.parse(localSettings) as AppSettings;
          setSettingsState('settings', { ...DEFAULT_SETTINGS, ...parsed });
          console.log('✅ Settings loaded from localStorage');
        }
      } catch (localError) {
        console.error('❌ Failed to load from localStorage:', localError);
      }
    } finally {
      setSettingsState('isLoading', false);
      setSettingsState('isDirty', false);
    }
  },

  // 설정 저장
  async saveSettings() {
    if (!settingsState.isDirty) {
      console.log('ℹ️ No changes to save');
      return;
    }

    setSettingsState('isLoading', true);
    try {
      console.log('🔧 Saving settings to backend...');
      await invoke('save_app_settings', { settings: settingsState.settings });
      
      setSettingsState('lastSaved', new Date().toISOString());
      setSettingsState('isDirty', false);
      console.log('✅ Settings saved successfully');
      
      // Also save to localStorage as backup
      localStorage.setItem('appSettings', JSON.stringify(settingsState.settings));
    } catch (error) {
      console.error('❌ Failed to save settings:', error);
      
      // Fallback to localStorage
      try {
        localStorage.setItem('appSettings', JSON.stringify(settingsState.settings));
        setSettingsState('lastSaved', new Date().toISOString());
        setSettingsState('isDirty', false);
        console.log('✅ Settings saved to localStorage');
      } catch (localError) {
        console.error('❌ Failed to save to localStorage:', localError);
        throw localError;
      }
    } finally {
      setSettingsState('isLoading', false);
    }
  },

  // 기본값으로 리셋
  async resetToDefaults() {
    setSettingsState('settings', { ...DEFAULT_SETTINGS });
    settingsState.markDirty();
    await settingsState.saveSettings();
    console.log('✅ Settings reset to defaults');
  },

  // 크롤링 설정 업데이트
  updateCrawlingSettings(newSettings: Partial<CrawlingSettings>) {
    setSettingsState('settings', 'crawling', (prev) => ({ ...prev, ...newSettings }));
    settingsState.markDirty();
  },

  // 로깅 설정 업데이트
  updateLoggingSettings(newSettings: Partial<LoggingSettings>) {
    setSettingsState('settings', 'logging', (prev) => ({ ...prev, ...newSettings }));
    settingsState.markDirty();
  },

  // 배치 설정 업데이트
  updateBatchSettings(newSettings: Partial<BatchSettings>) {
    setSettingsState('settings', 'batch', (prev) => ({ ...prev, ...newSettings }));
    settingsState.markDirty();
  },

  // 섹션 토글
  toggleSection(section: keyof SettingsStore['expandedSections']) {
    setSettingsState('expandedSections', section, (prev) => !prev);
    // 섹션 상태도 localStorage에 저장
    localStorage.setItem('settingsExpandedSections', JSON.stringify(settingsState.expandedSections));
  },

  // 변경사항 표시
  markDirty() {
    setSettingsState('isDirty', true);
  }
});

// 초기화 시 섹션 상태 복원
try {
  const savedSections = localStorage.getItem('settingsExpandedSections');
  if (savedSections) {
    const parsed = JSON.parse(savedSections);
    setSettingsState('expandedSections', parsed);
  }
} catch (error) {
  console.warn('Failed to restore section states:', error);
}

export { settingsState };
