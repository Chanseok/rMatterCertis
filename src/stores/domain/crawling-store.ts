/**
 * pimport type { CrawlingProgress, DatabaseStats } from '../../types/crawling';
import type { AtomicTaskEvent } from '../../types/atomic-events';

export interface CrawlingStore {
  // 상태 관리
  progress: () => CrawlingProgress | null;
  setProgress: (progress: CrawlingProgress) => void;
  
  stats: () => DatabaseStats | null;
  setStats: (stats: DatabaseStats) => void;d 구현 - Crawling Store for "Dual Channel" Event System
 * 
 * 크롤링 상태 관리를 위한 SolidJS 스토어
 * 고빈도 원자적 이벤트와 저빈도 상태 스냅샷을 분리 관리
 */

import { createSignal } from 'solid-js';
import type { CrawlingProgress, DatabaseStats } from '../../types/crawling';
import type { AtomicTaskEvent } from '../../types/atomic-events';

export interface CrawlingStore {
  // 상태 관리
  progress: () => CrawlingProgress | null;
  setProgress: (progress: CrawlingProgress) => void;
  
  stats: () => DatabaseStats | null;
  setStats: (stats: DatabaseStats) => void;
  
  // 기존 호환성을 위한 state 프로퍼티  
  state: {
    loading: boolean;
    error: string | null;
  };
  
  // 원자적 이벤트 추적
  recentAtomicEvents: () => AtomicTaskEvent[];
  addAtomicEvent: (event: AtomicTaskEvent) => void;
  clearAtomicEvents: () => void;
  
  // 활성 작업 추적
  activeTasks: () => Set<string>;
  addActiveTask: (taskId: string) => void;
  removeActiveTask: (taskId: string) => void;
  
  // 기존 호환성을 위한 메서드들
  loadActiveSessions: () => Promise<void>;
  stopAutoRefresh: () => void;
  isOperationPending: boolean;
  clearError: () => void;
}

/**
 * 크롤링 스토어 생성 함수
 */
export function createCrawlingStore(): CrawlingStore {
  const [progress, setProgress] = createSignal<CrawlingProgress | null>(null);
  const [stats, setStats] = createSignal<DatabaseStats | null>(null);
  const [recentAtomicEvents, setRecentAtomicEvents] = createSignal<AtomicTaskEvent[]>([]);
  const [activeTasks, setActiveTasks] = createSignal<Set<string>>(new Set());

  const addAtomicEvent = (event: AtomicTaskEvent) => {
    setRecentAtomicEvents(prev => {
      const newEvents = [event, ...prev].slice(0, 100); // 최근 100개만 유지
      return newEvents;
    });
    
    // 활성 작업 상태 업데이트
    if (event.event_type.type === 'TaskStarted') {
      setActiveTasks(prev => new Set([...prev, event.task_id]));
    } else if (event.event_type.type === 'TaskCompleted' || event.event_type.type === 'TaskFailed') {
      setActiveTasks(prev => {
        const newSet = new Set(prev);
        newSet.delete(event.task_id);
        return newSet;
      });
    }
  };

  const addActiveTask = (taskId: string) => {
    setActiveTasks(prev => new Set([...prev, taskId]));
  };

  const removeActiveTask = (taskId: string) => {
    setActiveTasks(prev => {
      const newSet = new Set(prev);
      newSet.delete(taskId);
      return newSet;
    });
  };

  const clearAtomicEvents = () => {
    setRecentAtomicEvents([]);
  };

  // 기존 호환성을 위한 임시 구현
  const [loading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  
  const state = {
    get loading() { return loading(); },
    get error() { return error(); },
  };
  
  const loadActiveSessions = async () => {
    // 임시 구현
  };
  
  const stopAutoRefresh = () => {
    // 임시 구현  
  };
  
  const clearError = () => {
    setError(null);
  };

  return {
    progress,
    setProgress,
    stats,
    setStats,
    state,
    recentAtomicEvents,
    addAtomicEvent,
    clearAtomicEvents,
    activeTasks,
    addActiveTask,
    removeActiveTask,
    loadActiveSessions,
    stopAutoRefresh,
    isOperationPending: false,
    clearError
  };
}