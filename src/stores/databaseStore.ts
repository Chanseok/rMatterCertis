/**
 * Database Store - 데이터베이스 전용 상태 관리
 * 
 * 이 스토어는 데이터베이스 관련 상태와 기능을 담당하며,
 * 실시간 DB 통계 업데이트와 관리 기능을 제공합니다.
 */

import { createStore } from 'solid-js/store';
import { createSignal, onCleanup } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import type { DatabaseStats } from '../types/crawling';

// 데이터베이스 상태 인터페이스
interface DatabaseState {
  // 통계 정보
  stats: DatabaseStats | null;
  
  // 연결 및 상태
  isConnected: boolean;
  isLoading: boolean;
  lastUpdated: Date | null;
  
  // 에러 상태
  lastError: string | null;
  
  // 관리 작업 상태
  isBackingUp: boolean;
  isOptimizing: boolean;
  isExporting: boolean;
  
  // 작업 히스토리
  backupHistory: Array<{
    id: string;
    path: string;
    timestamp: Date;
    success: boolean;
  }>;
  
  exportHistory: Array<{
    id: string;
    format: string;
    path: string;
    timestamp: Date;
    success: boolean;
  }>;
}

// 초기 상태
const initialState: DatabaseState = {
  stats: null,
  isConnected: false,
  isLoading: false,
  lastUpdated: null,
  lastError: null,
  isBackingUp: false,
  isOptimizing: false,
  isExporting: false,
  backupHistory: [],
  exportHistory: [],
};

// 반응형 상태 생성
const [databaseState, setDatabaseState] = createStore<DatabaseState>(initialState);

// 이벤트 구독 관리
const [eventSubscriptions] = createSignal<(() => void)[]>([]);

/**
 * 데이터베이스 스토어 클래스
 */
class DatabaseStore {
  // =========================================================================
  // 상태 접근자 (Getters)
  // =========================================================================

  get state() {
    return databaseState;
  }

  get stats() {
    return () => databaseState.stats;
  }

  get isConnected() {
    return () => databaseState.isConnected;
  }

  get isLoading() {
    return () => databaseState.isLoading;
  }

  get lastUpdated() {
    return () => databaseState.lastUpdated;
  }

  get lastError() {
    return () => databaseState.lastError;
  }

  get isBackingUp() {
    return () => databaseState.isBackingUp;
  }

  get isOptimizing() {
    return () => databaseState.isOptimizing;
  }

  get isExporting() {
    return () => databaseState.isExporting;
  }

  get backupHistory() {
    return () => databaseState.backupHistory;
  }

  get exportHistory() {
    return () => databaseState.exportHistory;
  }

  // =========================================================================
  // 편의 접근자
  // =========================================================================

  get hasError() {
    return () => databaseState.lastError !== null;
  }

  get healthStatus() {
    return () => databaseState.stats?.health_status || 'Warning';
  }

  get isHealthy() {
    return () => this.healthStatus() === 'Healthy';
  }

  get totalRecords() {
    return () => (databaseState.stats?.total_products || 0) + (databaseState.stats?.total_devices || 0);
  }

  get storageSize() {
    return () => databaseState.stats?.storage_size || '0 MB';
  }

  get incompleteRecords() {
    return () => databaseState.stats?.incomplete_records || 0;
  }

  get hasIncompleteRecords() {
    return () => this.incompleteRecords() > 0;
  }

  get canBackup() {
    return () => this.isConnected() && !this.isBackingUp() && !this.isOptimizing();
  }

  get canOptimize() {
    return () => this.isConnected() && !this.isBackingUp() && !this.isOptimizing();
  }

  get canExport() {
    return () => this.isConnected() && !this.isExporting() && this.totalRecords() > 0;
  }

  // =========================================================================
  // 상태 업데이트 메서드
  // =========================================================================

  setStats(stats: DatabaseStats) {
    setDatabaseState('stats', stats);
    setDatabaseState('lastUpdated', new Date());
    setDatabaseState('lastError', null);
  }

  setConnected(connected: boolean) {
    setDatabaseState('isConnected', connected);
  }

  setLoading(loading: boolean) {
    setDatabaseState('isLoading', loading);
  }

  setError(error: string | null) {
    setDatabaseState('lastError', error);
  }

  setBackingUp(backing: boolean) {
    setDatabaseState('isBackingUp', backing);
  }

  setOptimizing(optimizing: boolean) {
    setDatabaseState('isOptimizing', optimizing);
  }

  setExporting(exporting: boolean) {
    setDatabaseState('isExporting', exporting);
  }

  addBackupRecord(path: string, success: boolean) {
    const record = {
      id: Date.now().toString(),
      path,
      timestamp: new Date(),
      success,
    };
    
    setDatabaseState('backupHistory', (prev) => [record, ...prev.slice(0, 9)]); // 최대 10개 유지
  }

  addExportRecord(format: string, path: string, success: boolean) {
    const record = {
      id: Date.now().toString(),
      format,
      path,
      timestamp: new Date(),
      success,
    };
    
    setDatabaseState('exportHistory', (prev) => [record, ...prev.slice(0, 9)]); // 최대 10개 유지
  }

  clearErrors() {
    setDatabaseState('lastError', null);
  }

  reset() {
    setDatabaseState(initialState);
  }

  // =========================================================================
  // 데이터베이스 관리 메서드
  // =========================================================================

  async refreshStats(): Promise<void> {
    try {
      this.setLoading(true);
      this.clearErrors();
      
      const stats = await tauriApi.getDatabaseStats();
      this.setStats(stats);
      
      console.log('📊 데이터베이스 통계 업데이트됨:', stats);
    } catch (error) {
      const errorMessage = `통계 로드 실패: ${error}`;
      this.setError(errorMessage);
      console.error('❌ 데이터베이스 통계 로드 실패:', error);
    } finally {
      this.setLoading(false);
    }
  }

  async backupDatabase(): Promise<string> {
    if (!this.canBackup()) {
      throw new Error('현재 백업을 실행할 수 없습니다');
    }

    try {
      this.setBackingUp(true);
      this.clearErrors();
      
      console.log('💾 데이터베이스 백업 시작...');
      const backupPath = await tauriApi.backupDatabase();
      
      this.addBackupRecord(backupPath, true);
      console.log('✅ 데이터베이스 백업 완료:', backupPath);
      
      return backupPath;
    } catch (error) {
      const errorMessage = `백업 실패: ${error}`;
      this.setError(errorMessage);
      this.addBackupRecord('', false);
      
      console.error('❌ 데이터베이스 백업 실패:', error);
      throw new Error(errorMessage);
    } finally {
      this.setBackingUp(false);
    }
  }

  async optimizeDatabase(): Promise<void> {
    if (!this.canOptimize()) {
      throw new Error('현재 최적화를 실행할 수 없습니다');
    }

    try {
      this.setOptimizing(true);
      this.clearErrors();
      
      console.log('⚡ 데이터베이스 최적화 시작...');
      await tauriApi.optimizeDatabase();
      
      // 최적화 후 통계 새로고침
      await this.refreshStats();
      
      console.log('✅ 데이터베이스 최적화 완료');
    } catch (error) {
      const errorMessage = `최적화 실패: ${error}`;
      this.setError(errorMessage);
      
      console.error('❌ 데이터베이스 최적화 실패:', error);
      throw new Error(errorMessage);
    } finally {
      this.setOptimizing(false);
    }
  }

  async exportData(format: 'csv' | 'json' | 'excel'): Promise<string> {
    if (!this.canExport()) {
      throw new Error('현재 내보내기를 실행할 수 없습니다');
    }

    try {
      this.setExporting(true);
      this.clearErrors();
      
      console.log(`📤 데이터 내보내기 시작 (${format.toUpperCase()})...`);
      const exportPath = await tauriApi.exportDatabaseData(format);
      
      this.addExportRecord(format, exportPath, true);
      console.log('✅ 데이터 내보내기 완료:', exportPath);
      
      return exportPath;
    } catch (error) {
      const errorMessage = `내보내기 실패: ${error}`;
      this.setError(errorMessage);
      this.addExportRecord(format, '', false);
      
      console.error('❌ 데이터 내보내기 실패:', error);
      throw new Error(errorMessage);
    } finally {
      this.setExporting(false);
    }
  }

  // =========================================================================
  // 초기화 및 정리
  // =========================================================================

  async initialize(): Promise<void> {
    try {
      console.log('🔧 데이터베이스 스토어 초기화 중...');
      
      // 초기 통계 로드
      await this.refreshStats();
      
      // 실시간 이벤트 구독
      await this.subscribeToEvents();
      
      this.setConnected(true);
      console.log('✅ 데이터베이스 스토어 초기화 완료');
    } catch (error) {
      console.error('❌ 데이터베이스 스토어 초기화 실패:', error);
      this.setError(`초기화 실패: ${error}`);
      this.setConnected(false);
    }
  }

  private async subscribeToEvents(): Promise<void> {
    try {
      // unified actor-event 구독 (variant=DatabaseStats)
      const unsubActor = await tauriApi.subscribeToUnifiedActorEvents({
        variants: ['DatabaseStats'],
        onEvent: (payload) => {
          const stats = payload?.stats ?? payload; // tolerate wrapped or flat shapes
          if (stats) {
            console.log('📊 [actor] DatabaseStats 업데이트:', stats);
            this.setStats(stats);
          }
        },
      });
  eventSubscriptions()[0] = unsubActor;
      
      console.log('📡 데이터베이스 이벤트 구독 완료');
    } catch (error) {
      console.error('❌ 데이터베이스 이벤트 구독 실패:', error);
      throw error;
    }
  }

  cleanup(): void {
    console.log('🧹 데이터베이스 스토어 정리 중...');
    
    // 이벤트 구독 해제
    const unsubs = eventSubscriptions();
    unsubs.forEach(unsub => unsub?.());
    
    // 상태 초기화
    this.reset();
    
    console.log('✅ 데이터베이스 스토어 정리 완료');
  }
}

// 싱글톤 인스턴스 생성
export const databaseStore = new DatabaseStore();

// 자동 정리 설정
onCleanup(() => {
  databaseStore.cleanup();
});
