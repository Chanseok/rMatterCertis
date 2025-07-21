/**
 * settingsStore - 애플리케이션 설정 관리 스토어
 * 백엔드와 연동하여 설정을 저장/로드
 */

import { createStore } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';
import { AppConfig, CONFIG_PRESETS } from '../types/config';

interface SettingsStore {
  settings: AppConfig;
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
  updateNestedField: (path: string, value: any) => void;
  applyPreset: (presetName: string) => void;
  toggleSection: (section: keyof SettingsStore['expandedSections']) => void;
  markDirty: () => void;
  getNestedValue: (path: string) => any;
}

// 기본 설정값
const DEFAULT_SETTINGS: AppConfig = {
  user: {
    request_delay_ms: 800,
    max_concurrent_requests: 24,
    verbose_logging: false,
    logging: {
      level: 'info',
      json_format: false,
      console_output: true,
      file_output: true,
      separate_frontend_backend: false,
      file_naming_strategy: 'unified',
      max_file_size_mb: 10,
      max_files: 5,
      auto_cleanup_logs: true,
      keep_only_latest: false,
      module_filters: {
        wry: 'warn',
        tokio: 'info',
        hyper: 'warn',
        tauri: 'info',
        reqwest: 'info',
        sqlx: 'warn',
        matter_certis_v2: 'info'
      }
    },
    batch: {
      batch_size: 12,
      batch_delay_ms: 1000,
      enable_batch_processing: true,
      batch_retry_limit: 3
    },
    crawling: {
      page_range_limit: 6,
      intelligent_mode: {
        enabled: true,
        max_range_limit: 1000,
        override_config_limit: true,
        site_analysis_ttl_minutes: 5,
        db_analysis_ttl_minutes: 10,
        range_calculation_ttl_minutes: 3,
        min_incremental_pages: 10,
        max_full_crawl_pages: 500
      },
      product_list_retry_count: 3,
      product_detail_retry_count: 3,
      auto_add_to_local_db: true,
      workers: {
        list_page_max_concurrent: 10,
        product_detail_max_concurrent: 20,
        request_timeout_seconds: 30,
        max_retries: 3,
        db_batch_size: 100,
        db_max_concurrency: 10
      },
      timing: {
        scheduler_interval_ms: 1000,
        shutdown_timeout_seconds: 30,
        stats_interval_seconds: 10,
        retry_delay_ms: 2000,
        operation_timeout_seconds: 30
      }
    }
  },
  advanced: {
    last_page_search_start: 450,
    max_search_attempts: 10,
    retry_attempts: 3,
    retry_delay_ms: 2000,
    product_selectors: ['div.post-feed article.type-product'],
    request_timeout_seconds: 30
  },
  app_managed: {
    last_known_max_page: 482,
    last_successful_crawl: null,
    last_crawl_product_count: 0,
    avg_products_per_page: 12.0,
    config_version: 1,
    window_state: null
  }
};

// 스토어 생성
const [settingsState, setSettingsState] = createStore<SettingsStore>({
  settings: DEFAULT_SETTINGS,
  isLoading: false,
  isDirty: false,
  lastSaved: null,
  expandedSections: {
    basic: true,
    logging: false,
    batch: false,
    advanced: false
  },

  async loadSettings() {
    try {
      setSettingsState('isLoading', true);
      const loadedSettings = await invoke<AppConfig>('get_app_settings');
      if (loadedSettings) {
        setSettingsState('settings', loadedSettings);
        setSettingsState('lastSaved', new Date().toISOString());
      }
    } catch (error) {
      console.error('설정 로드 실패:', error);
      // 로컬 스토리지에서 설정 로드 시도
      const localSettings = localStorage.getItem('app-settings');
      if (localSettings) {
        try {
          const parsed = JSON.parse(localSettings) as AppConfig;
          setSettingsState('settings', parsed);
        } catch (parseError) {
          console.error('로컬 설정 파싱 실패:', parseError);
        }
      }
    } finally {
      setSettingsState('isLoading', false);
    }
  },

  async saveSettings() {
    try {
      setSettingsState('isLoading', true);
      await invoke('save_app_settings', { settings: settingsState.settings });
      
      // 로컬 스토리지에도 백업
      localStorage.setItem('app-settings', JSON.stringify(settingsState.settings));
      
      setSettingsState('isDirty', false);
      setSettingsState('lastSaved', new Date().toISOString());
    } catch (error) {
      console.error('설정 저장 실패:', error);
      throw error;
    } finally {
      setSettingsState('isLoading', false);
    }
  },

  async resetToDefaults() {
    setSettingsState('settings', DEFAULT_SETTINGS);
    setSettingsState('isDirty', true);
    await this.saveSettings();
  },

  updateNestedField(path: string, value: any) {
    const keys = path.split('.');
    let current = { ...settingsState.settings } as any;
    
    // 깊은 복사를 위한 재귀 함수
    const deepCopy = (obj: any): any => {
      if (obj === null || typeof obj !== 'object') return obj;
      if (obj instanceof Date) return new Date(obj);
      if (Array.isArray(obj)) return obj.map(deepCopy);
      const copy: any = {};
      for (const key in obj) {
        copy[key] = deepCopy(obj[key]);
      }
      return copy;
    };

    current = deepCopy(current);
    let target = current;
    
    for (let i = 0; i < keys.length - 1; i++) {
      target = target[keys[i]];
    }
    target[keys[keys.length - 1]] = value;
    
    setSettingsState('settings', current);
    this.markDirty();
  },

  applyPreset(presetName: string) {
    const preset = CONFIG_PRESETS.find(p => p.name === presetName);
    if (preset) {
      setSettingsState('settings', preset.config);
      this.markDirty();
    }
  },

  getNestedValue(path: string) {
    const keys = path.split('.');
    let current = settingsState.settings as any;
    
    for (const key of keys) {
      current = current[key];
      if (current === undefined) return undefined;
    }
    return current;
  },

  toggleSection(section: keyof SettingsStore['expandedSections']) {
    setSettingsState('expandedSections', section, !settingsState.expandedSections[section]);
  },

  markDirty() {
    setSettingsState('isDirty', true);
  }
});

export { settingsState };
