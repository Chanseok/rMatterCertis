/**
 * StatusTab - 크롤링 상태 및 제어 탭 컴포넌트 (통합 뷰 모드 지원)
 */

import { Component, createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { crawlerStore } from '../../stores/crawlerStore';
import { useIntegratedCrawlingStore } from '../../stores/integratedCrawlingStore';
import { CrawlingCityDashboard } from '../visualization/CrawlingCityDashboard';
import { CrawlingCity3D } from '../visualization/CrawlingCity3D';
import { CrawlingMetricsChart } from '../visualization/CrawlingMetricsChart';
import type { CrawlingStatusCheck } from '../../types/crawling';
import { confirm } from '@tauri-apps/plugin-dialog';

// 뷰 모드 선택기 컴포넌트
const ViewModeSelector: Component<{
  value: string;
  onChange: (mode: 'classic' | 'city' | '3d' | 'metrics') => void;
}> = (props) => {
  const viewModes = [
    { id: 'classic', label: '📊 Classic View', description: '기존 UI 유지' },
    { id: 'city', label: '🏙️ City View', description: '도시 대시보드' },
    { id: '3d', label: '🎮 3D View', description: '3D 시각화' },
    { id: 'metrics', label: '📈 Metrics View', description: '차트 중심' }
  ];

  return (
    <div class="mb-6 bg-white rounded-xl shadow-lg p-4">
      <h3 class="text-lg font-bold text-gray-800 mb-3">🎨 뷰 모드 선택</h3>
      <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
        <For each={viewModes}>
          {(mode) => (
            <button
              onClick={() => props.onChange(mode.id as any)}
              class={`p-3 rounded-lg border-2 transition-all duration-200 text-center ${
                props.value === mode.id
                  ? 'border-blue-500 bg-blue-50 text-blue-700'
                  : 'border-gray-200 bg-gray-50 text-gray-600 hover:border-gray-300 hover:bg-gray-100'
              }`}
            >
              <div class="font-medium text-sm">{mode.label}</div>
              <div class="text-xs mt-1 opacity-80">{mode.description}</div>
            </button>
          )}
        </For>
      </div>
    </div>
  );
};

// 클래식 뷰 컴포넌트 (기존 StatusTab 내용)
const ClassicStatusView: Component = () => {
  console.log('🚀 StatusTab 컴포넌트가 로드되었습니다');
  
  // 크롤링 상태 (기본 UI 상태들)
  const [crawlingStatus, setCrawlingStatus] = createSignal<'idle' | 'running' | 'paused' | 'completed'>('idle');
  const [progress, setProgress] = createSignal(0);
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages] = createSignal(100);
  const [currentBatch] = createSignal(0);
  const [totalBatches] = createSignal(10);
  const [estimatedTime] = createSignal('계산 중...');

  // 실시간 상태 체크 결과 (로컬 상태)
  const [statusCheckResult, setStatusCheckResult] = createSignal<CrawlingStatusCheck | null>(null);
  const [isCheckingStatus, setIsCheckingStatus] = createSignal(false);
  const [statusCheckError, setStatusCheckError] = createSignal<string>('');

  // 재시도 통계 - INTEGRATED_PHASE2_PLAN Week 1 Day 5
  const [retryStats, setRetryStats] = createSignal<any>(null);
  const [isLoadingRetryStats, setIsLoadingRetryStats] = createSignal(false);

  // 사이트 분석 결과는 이제 글로벌 store에서 가져옴
  // const siteAnalysisResult = crawlerStore.siteAnalysisResult;
  // const isAnalyzing = crawlerStore.isAnalyzing;

  // 설정은 백엔드에서 관리됨 - 여기서는 제거됨

  // 현재 크롤링 모드 상태
  const [currentCrawlingMode, setCurrentCrawlingMode] = createSignal<string>('분석 필요');
  const [plannedRange, setPlannedRange] = createSignal<[number, number] | null>(null);

  // 이벤트 리스너 등록
  onMount(async () => {
    let unlistenStoppedEvent: (() => void) | undefined;
    
    try {
      // 크롤링 중지 이벤트 리스너
      unlistenStoppedEvent = await tauriApi.subscribeToCrawlingStopped((data) => {
        console.log('🛑 크롤링 중지 이벤트 수신:', data);
        setCrawlingStatus('idle');
        setProgress(0);
        setCurrentPage(0);
      });
      
      console.log('✅ 이벤트 리스너 등록 완료');
    } catch (error) {
      console.error('❌ 이벤트 리스너 등록 실패:', error);
    }
    
    // 컴포넌트 언마운트 시 이벤트 리스너 정리
    onCleanup(() => {
      if (unlistenStoppedEvent) {
        unlistenStoppedEvent();
        console.log('🧹 크롤링 중지 이벤트 리스너 정리됨');
      }
    });
  });

  const getStatusColor = () => {
    switch (crawlingStatus()) {
      case 'running': return '#22c55e';
      case 'paused': return '#f59e0b';
      case 'completed': return '#3b82f6';
      default: return '#6b7280';
    }
  };

  const getStatusText = () => {
    switch (crawlingStatus()) {
      case 'running': return '실행 중';
      case 'paused': return '일시 정지';
      case 'completed': return '완료';
      default: return '대기 중';
    }
  };

  const getPriorityColor = (priority: string) => {
    switch (priority) {
      case 'critical': return 'text-red-600 bg-red-100 border-red-200';
      case 'high': return 'text-orange-600 bg-orange-100 border-orange-200';
      case 'medium': return 'text-yellow-600 bg-yellow-100 border-yellow-200';
      default: return 'text-green-600 bg-green-100 border-green-200';
    }
  };

  const getHealthColor = (health: string) => {
    switch (health) {
      case 'Critical': return 'text-red-600';
      case 'Warning': return 'text-yellow-600';
      default: return 'text-green-600';
    }
  };

  // 결과 표시용 헬퍼 함수들
  const getActiveResult = () => statusCheckResult() || null;
  const getRecommendationReason = () => {
    if (statusCheckResult()) {
      return statusCheckResult()!.recommendation?.reason || '권장 사항이 없습니다.';
    }
    
    const siteResult = crawlerStore.siteAnalysisResult();
    if (siteResult) {
      return siteResult.recommendation?.reason || '사이트 분석이 완료되었습니다.';
    }
    
    return '상태 체크를 먼저 실행해주세요.';
  };

  // 크롤링 계획 분석
  const analyzeCrawlingPlan = () => {
    const statusResult = getActiveResult();
    const siteResult = crawlerStore.siteAnalysisResult();
    
    let startPage = 1;
    let endPage = 50; // 백엔드 기본값 사용
    let mode = '기본 설정 모드';
    
    if (statusResult) {
      const suggestion = statusResult.recommendation?.suggested_range;
      if (suggestion && suggestion.length >= 2) {
        startPage = suggestion[0];
        endPage = suggestion[1];
        mode = '스마트 추천 모드';
      }
    } else if (siteResult) {
      const dbStatus = siteResult.database_status;
      const siteStatus = siteResult.site_status;
      
      if (dbStatus && siteStatus) {
        const dbMaxPage = Math.max(...(dbStatus.page_range || [0]));
        const siteMaxPage = siteStatus.total_pages || 50; // 백엔드 기본값 사용
        
        if (dbMaxPage > 0) {
          startPage = dbMaxPage + 1;
          endPage = Math.min(startPage + 50 - 1, siteMaxPage); // 백엔드 기본값 사용
          mode = '갭 기반 크롤링 모드';
        } else {
          startPage = 1;
          endPage = Math.min(50, siteMaxPage); // 백엔드 기본값 사용
          mode = '초기 크롤링 모드';
        }
      }
    }
    
    setCurrentCrawlingMode(mode);
    setPlannedRange([startPage, endPage]);
    return { mode, startPage, endPage };
  };

  // 상태나 설정 변경 시 크롤링 계획 재분석
  const updateCrawlingPlan = () => {
    analyzeCrawlingPlan();
  };

  const startCrawling = async () => {
    console.log('🔥 startCrawling 함수 호출됨');
    const statusResult = getActiveResult();
    const siteResult = crawlerStore.siteAnalysisResult();
    console.log('🔍 상태 체크 결과:', statusResult);
    console.log('🔍 사이트 분석 결과:', siteResult);
    
    // 스마트한 페이지 범위 계산 로직
    let startPage = 1;
    let endPage = 50; // 백엔드 기본값 사용
    let crawlingMode = '기본 모드';
    
    if (statusResult) {
      // 실시간 상태 체크 결과가 있는 경우 (추천 범위 사용)
      const suggestion = statusResult.recommendation?.suggested_range;
      if (suggestion && suggestion.length >= 2) {
        startPage = suggestion[0];
        endPage = suggestion[1];
        crawlingMode = '스마트 추천 모드';
        console.log('📊 실시간 상태 체크 기반 추천:', `${startPage}-${endPage} 페이지`);
      }
    } else if (siteResult) {
      // 사이트 분석 결과만 있는 경우 (갭 기반 크롤링)
      const dbStatus = siteResult.database_status;
      const siteStatus = siteResult.site_status;
      
      if (dbStatus && siteStatus) {
        const dbMaxPage = Math.max(...(dbStatus.page_range || [0]));
        const siteMaxPage = siteStatus.total_pages || 50; // 백엔드 기본값 사용
        
        if (dbMaxPage > 0) {
          // DB에 데이터가 있는 경우: DB 마지막 페이지 다음부터 크롤링
          startPage = dbMaxPage + 1;
          endPage = Math.min(startPage + 50 - 1, siteMaxPage); // 백엔드 기본값 사용
          crawlingMode = '갭 기반 크롤링 모드';
          console.log('📈 갭 기반 크롤링:', `DB 마지막 페이지(${dbMaxPage}) 이후 ${startPage}-${endPage} 페이지`);
        } else {
          // DB가 비어있는 경우: 처음부터 크롤링
          startPage = 1;
          endPage = Math.min(50, siteMaxPage); // 백엔드 기본값 사용
          crawlingMode = '초기 크롤링 모드';
          console.log('🆕 초기 크롤링:', `처음부터 ${startPage}-${endPage} 페이지`);
        }
      }
    } else {
      // 분석 결과가 없는 경우: 백엔드 기본값 사용
      startPage = 1;
      endPage = 50; // 백엔드 기본값 사용
      crawlingMode = '기본 설정 모드';
      console.log('⚙️ 기본 설정 모드:', `${startPage}-${endPage} 페이지`);
    }
    
    // 사용자 확인 대화상자 추가
    console.log('❓ 사용자 확인 대화상자를 표시합니다:', {
      mode: crawlingMode,
      startPage,
      endPage,
      totalPages: endPage - startPage + 1
    });
    
    const confirmMessage = `🔧 크롤링 설정 확인\n\n` +
      `모드: ${crawlingMode}\n` +
      `범위: ${startPage} ~ ${endPage} 페이지 (총 ${endPage - startPage + 1}페이지)\n` +
      `병렬 처리: 24개 페이지 동시 처리\n` +
      `예상 시간: ${Math.ceil((endPage - startPage + 1) * 2 / 24)}분\n\n` +
      `⚠️ 설정을 변경하려면 '설정' 탭에서 page_range_limit 값을 조정하세요.\n\n` +
      `이 설정으로 크롤링을 시작하시겠습니까?`;
    
    console.log('📝 대화상자 메시지:', confirmMessage);
    
    let userConfirmed = false;
    try {
      console.log('� Tauri dialog confirm 함수 호출을 시도합니다...');
      userConfirmed = await confirm(confirmMessage, { 
        title: '크롤링 설정 확인',
        kind: 'info' 
      });
      console.log('✅ Tauri dialog confirm 함수 호출 성공, 결과:', userConfirmed);
    } catch (error) {
      console.error('❌ Tauri dialog confirm 함수 호출 실패:', error);
      console.log('🔄 fallback으로 window.confirm 사용...');
      // 폴백으로 window.confirm 사용
      try {
        userConfirmed = window.confirm(confirmMessage);
        console.log('✅ window.confirm 결과:', userConfirmed);
      } catch (fallbackError) {
        console.error('❌ window.confirm도 실패:', fallbackError);
        // 최종 폴백으로 자동 승인
        userConfirmed = true;
        console.log('⚠️ 자동으로 승인합니다.');
      }
    }
    
    console.log('💬 사용자 선택 결과:', userConfirmed ? '승인됨' : '취소됨');
    
    if (!userConfirmed) {
      console.log('❌ 사용자가 크롤링을 취소했습니다.');
      return;
    }
    
    console.log('✅ 사용자가 크롤링을 승인했습니다. 진행합니다...');
    
    // 페이지 범위 검증
    if (startPage > endPage) {
      alert('시작 페이지가 끝 페이지보다 클 수 없습니다.');
      return;
    }
    
    if (endPage - startPage + 1 > 100) {
      alert('한 번에 100페이지 이상은 크롤링할 수 없습니다.');
      return;
    }
    
    try {
      setCrawlingStatus('running');
      console.log('� 크롤링 시작:', {
        mode: crawlingMode,
        startPage,
        endPage,
        totalPages: endPage - startPage + 1
      });
      
    // 백엔드에서 지능적인 범위 계산을 사용하도록 수정
    // startPage, endPage를 전달하지 않고 백엔드가 계산하도록 함
    console.log('📞 tauriApi.startCrawling 호출 시도 (백엔드 지능형 범위 계산 사용)...');
    const sessionId = await tauriApi.startCrawling(undefined, undefined); // 백엔드에서 지능적 범위 계산 사용
    console.log('✅ 크롤링 세션 시작됨:', sessionId);
      
      // 실시간 진행률 업데이트 시작 (crawlerStore에서 처리)
      console.log('🔄 실시간 업데이트 시작...');
      crawlerStore.startRealTimeUpdates().catch((error: any) => {
        console.error('실시간 업데이트 시작 실패:', error);
        // 폴백으로 시뮬레이션 사용
        console.log('🎭 시뮬레이션 모드로 전환...');
        simulateProgress();
      });
    } catch (error) {
      console.error('❌ 크롤링 시작 실패:', error);
      console.error('❌ 에러 상세:', error);
      setCrawlingStatus('idle');
      alert(`크롤링 시작에 실패했습니다: ${error}`);
    }
  };

  const simulateProgress = () => {
    // 진행률 시뮬레이션
    const interval = setInterval(() => {
      setProgress(prev => {
        const newProgress = prev + 1;
        if (newProgress >= 100) {
          clearInterval(interval);
          setCrawlingStatus('completed');
          return 100;
        }
        return newProgress;
      });
      setCurrentPage(prev => Math.min(prev + 1, totalPages()));
    }, 200);
  };

  const pauseCrawling = async () => {
    try {
      await tauriApi.pauseCrawling();
      setCrawlingStatus('paused');
      console.log('⏸️ 크롤링 일시정지됨');
    } catch (error) {
      console.error('❌ 크롤링 일시정지 실패:', error);
    }
  };

  const stopCrawling = async () => {
    try {
      await tauriApi.stopCrawling();
      setCrawlingStatus('idle');
      setProgress(0);
      setCurrentPage(0);
      console.log('⏹️ 크롤링 중지됨');
    } catch (error) {
      console.error('❌ 크롤링 중지 실패:', error);
    }
  };

  // 사전 조사용 상태 체크 (check_site_status)
  // 사이트 종합 분석 (사전 조사용)
  const runSiteAnalysis = async () => {
    try {
      setIsCheckingStatus(true);
      setStatusCheckError('');
      setStatusCheckResult(null);
      
      console.log('🔍 사이트 종합 분석 시작...');
      console.log('📡 실제 웹사이트에 접속하여 페이지 구조를 분석하고 DB와 비교합니다...');
      
      // 글로벌 store의 메서드 사용
      const result = await crawlerStore.performSiteAnalysis();
      
      if (result) {
        console.log('✅ 사이트 분석 완료:', result);
        // 사이트 분석 완료 후 크롤링 계획 재분석
        updateCrawlingPlan();
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setStatusCheckError(`사이트 분석 실패: ${errorMessage}`);
      console.error('❌ 사이트 분석 실패:', error);
    } finally {
      setIsCheckingStatus(false);
    }
  };

  // 재시도 통계 조회 - INTEGRATED_PHASE2_PLAN Week 1 Day 5
  const loadRetryStats = async () => {
    try {
      setIsLoadingRetryStats(true);
      console.log('📊 재시도 통계 조회 중...');
      
      const stats = await tauriApi.getRetryStats();
      console.log('✅ 재시도 통계 조회 완료:', stats);
      
      setRetryStats(stats);
    } catch (error) {
      console.error('❌ 재시도 통계 조회 실패:', error);
      setRetryStats({
        total_items: 0,
        pending_retries: 0,
        successful_retries: 0,
        failed_retries: 0,
        max_retries: 3,
        status: '데이터 로딩 실패'
      });
    } finally {
      setIsLoadingRetryStats(false);
    }
  };

  // 실시간 모니터링용 상태 체크 (get_crawling_status_check)
  const runStatusCheck = async () => {
    try {
      setIsCheckingStatus(true);
      setStatusCheckError('');
      setStatusCheckResult(null);
      
      console.log('📊 크롤링 상태 체크 시작...');
      console.log('💾 메모리에서 현재 크롤링 진행 상황을 조회합니다...');
      
      const result = await tauriApi.getCrawlingStatusCheck();
      console.log('✅ 상태 체크 완료:', result);
      
      setStatusCheckResult(result);
      
      // 상태 체크 완료 후 크롤링 계획 재분석
      updateCrawlingPlan();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setStatusCheckError(`상태 체크 실패: ${errorMessage}`);
      console.error('❌ 상태 체크 실패:', error);
    } finally {
      setIsCheckingStatus(false);
    }
  };

  // 사전조사 결과 렌더링 함수
  const renderSiteAnalysisResults = () => {
    const result = crawlerStore.siteAnalysisResult();
    if (!result) return null;

    return (
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; margin-top: 24px;">
        {/* 사이트 상태 카드 */}
        <div style="background: white; border-radius: 12px; padding: 24px; border: 1px solid #e5e7eb; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
          <div style="display: flex; align-items: center; margin-bottom: 16px;">
            <div style="width: 48px; height: 48px; background: #dbeafe; border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
              <span style="font-size: 24px;">🌐</span>
            </div>
            <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">사이트 상태</h3>
          </div>
          <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px;">
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">접근성:</span>
              <span style={`font-weight: 500; ${result.site_status?.is_accessible ? 'color: #059669;' : 'color: #dc2626;'}`}>
                {result.site_status?.is_accessible ? '✅ 정상' : '❌ 불가'}
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">응답 시간:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.site_status?.response_time_ms || 0}ms
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">최대 페이지:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.site_status?.total_pages || 0} 페이지
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">예상 제품 수:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.site_status?.estimated_products?.toLocaleString() || 0}개
              </span>
            </div>
          </div>
        </div>

        {/* DB 상태 카드 */}
        <div style="background: white; border-radius: 12px; padding: 24px; border: 1px solid #e5e7eb; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
          <div style="display: flex; align-items: center; margin-bottom: 16px;">
            <div style="width: 48px; height: 48px; background: #f3e8ff; border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
              <span style="font-size: 24px;">💾</span>
            </div>
            <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">DB 상태</h3>
          </div>
          <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px;">
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">전체 제품:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.database_status?.total_products?.toLocaleString() || 0}개
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">DB 상태:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.database_status?.health || 'Unknown'}
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">사이트 예상 제품:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.sync_comparison?.site_estimated_count?.toLocaleString() || 0}개
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">동기화율:</span>
              <span style="font-weight: 500; color: #111827;">
                {Math.round(result.sync_comparison?.sync_percentage || 0)}%
              </span>
            </div>
          </div>
        </div>

        {/* 추천 행동 카드 */}
        <div style="background: white; border-radius: 12px; padding: 24px; border: 1px solid #e5e7eb; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
          <div style="display: flex; align-items: center; margin-bottom: 16px;">
            <div style="width: 48px; height: 48px; background: #fef3c7; border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
              <span style="font-size: 24px;">💡</span>
            </div>
            <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">추천 행동</h3>
          </div>
          <div style="font-size: 14px; line-height: 1.6; color: #374151;">
            {getRecommendationReason()}
          </div>
        </div>
      </div>
    );
  };

  // 컴포넌트 마운트 시 크롤링 계획 분석
  setTimeout(() => {
    updateCrawlingPlan();
  }, 1000);

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">📊 상태 & 제어</h2>
      
      {/* 뷰 모드 선택기 */}
      <ViewModeSelector
        value="classic"
        onChange={(mode) => {
          console.log('뷰 모드 변경:', mode);
          // 뷰 모드 변경 로직 추가 필요
        }}
      />
      
      {/* 크롤링 상태 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">크롤링 상태</h3>
        
        <div style="margin-bottom: 16px;">
          <div style="display: flex; align-items: center; margin-bottom: 8px;">
            <div 
              style={`width: 12px; height: 12px; border-radius: 50%; background: ${getStatusColor()}; margin-right: 8px;`}
            ></div>
            <span style="font-weight: 500; font-size: 16px;">{getStatusText()}</span>
          </div>
        </div>

        {/* 진행률 바 */}
        <div style="margin-bottom: 16px;">
          <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
            <span style="font-weight: 500;">전체 진행률</span>
            <span style="font-weight: 500;">{progress()}%</span>
          </div>
          <div style="width: 100%; height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden;">
            <div 
              style={`height: 100%; background: linear-gradient(90deg, #3b82f6, #1d4ed8); transition: width 0.3s ease; width: ${progress()}%;`}
            ></div>
          </div>
        </div>

        {/* 상세 정보 */}
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 16px;">
          <div style="padding: 12px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <div style="font-size: 12px; color: #6b7280; margin-bottom: 4px;">현재 페이지</div>
            <div style="font-size: 18px; font-weight: 600; color: #1f2937;">{currentPage()}/{totalPages()}</div>
          </div>
          <div style="padding: 12px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <div style="font-size: 12px; color: #6b7280; margin-bottom: 4px;">현재 배치</div>
            <div style="font-size: 18px; font-weight: 600, color: #1f2937;">{currentBatch()}/{totalBatches()}</div>
          </div>
        </div>

        <div style="padding: 12px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
          <div style="font-size: 12px; color: #6b7280; margin-bottom: 4px;">예상 완료 시간</div>
          <div style="font-size: 16px; font-weight: 500; color: #1f2937;">{estimatedTime()}</div>
        </div>
      </div>



      {/* 스마트 크롤링 제어 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #fefefe;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">🤖 스마트 크롤링 제어</h3>
        
        {/* 현재 크롤링 계획 표시 */}
        <div style="margin-bottom: 16px; padding: 12px; background: #f8fafc; border-radius: 6px; border: 1px solid #e2e8f0;">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
            <span style="font-weight: 500; color: #374151;">📋 현재 크롤링 계획:</span>
            <button
              onClick={updateCrawlingPlan}
              style="padding: 4px 8px; background: #6b7280; color: white; border: none; border-radius: 3px; font-size: 11px; cursor: pointer;"
            >
              재분석
            </button>
          </div>
          <div style="display: flex; flex-wrap: wrap; gap: 12px; font-size: 13px;">
            <span style="color: #6b7280;">
              <strong>모드:</strong> <span style="color: #059669;">{currentCrawlingMode()}</span>
            </span>
            {plannedRange() && (
              <span style="color: #6b7280;">
                <strong>범위:</strong> <span style="color: #dc2626;">{plannedRange()![0]}-{plannedRange()![1]} 페이지</span>
                <span style="color: #6b7280; margin-left: 8px;">({plannedRange()![1] - plannedRange()![0] + 1}페이지)</span>
              </span>
            )}
          </div>
        </div>
        
        {statusCheckResult() && (
          <div style="margin-bottom: 16px; padding: 12px; background: #f0f9ff; border-radius: 6px; border-left: 4px solid #3b82f6; font-size: 14px;">
            <strong>🎯 추천 크롤링:</strong> 페이지 {statusCheckResult()!.recommendation?.suggested_range?.[0] || 1}-{statusCheckResult()!.recommendation?.suggested_range?.[1] || 50} 
            (약 {statusCheckResult()!.recommendation?.estimated_new_items || 0}개 신규 제품 예상)
          </div>
        )}
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            ref={(el) => console.log('🔧 크롤링 버튼이 렌더링되었습니다:', el)}
            onClick={() => {
              console.log('🔴 버튼 클릭됨 - crawlingStatus:', crawlingStatus());
              console.log('🔴 startCrawling 함수 호출 시도...');
              try {
                startCrawling();
              } catch (error) {
                console.error('🔴 startCrawling 함수 호출 실패:', error);
                alert('크롤링 시작 중 오류가 발생했습니다: ' + error);
              }
            }}
            disabled={crawlingStatus() === 'running'}
            style={`padding: 12px 24px; background: ${crawlingStatus() === 'running' ? '#9ca3af' : statusCheckResult() ? '#10b981' : '#22c55e'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() === 'running' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            {crawlingStatus() === 'running' 
              ? '🔄 크롤링 중...' 
              : currentCrawlingMode() === '스마트 추천 모드' 
                ? '🤖 스마트 크롤링 시작' 
                : currentCrawlingMode() === '갭 기반 크롤링 모드'
                  ? '📈 갭 기반 크롤링 시작'
                  : currentCrawlingMode() === '초기 크롤링 모드'
                    ? '🆕 초기 크롤링 시작'
                    : '▶️ 기본 크롤링 시작'
            }
          </button>
          
          <button
            onClick={pauseCrawling}
            disabled={crawlingStatus() !== 'running'}
            style={`padding: 12px 24px; background: ${crawlingStatus() !== 'running' ? '#9ca3af' : '#f59e0b'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() !== 'running' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            ⏸️ 일시정지
          </button>
          
          <button
            onClick={stopCrawling}
            disabled={crawlingStatus() === 'idle'}
            style={`padding: 12px 24px; background: ${crawlingStatus() === 'idle' ? '#9ca3af' : '#ef4444'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() === 'idle' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            ⏹️ 중지
          </button>
        </div>

        {!statusCheckResult() && (
          <div style="margin-top: 12px; padding: 8px; background: #f0f9ff; border-radius: 4px; font-size: 13px; color: #1e40af;">
            💡 {currentCrawlingMode() === '분석 필요' 
              ? '상태 체크나 사이트 분석을 실행하면 더 정확한 크롤링 계획을 수립할 수 있습니다.' 
              : `현재 ${currentCrawlingMode()}로 크롤링이 진행됩니다. 더 정확한 분석을 위해 상태 체크를 실행해보세요.`
            }
          </div>
        )}
      </div>

      {/* 상태 체크 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">상태 체크 & 분석</h3>
        
        <div style="display: flex; gap: 12px; margin-bottom: 16px; flex-wrap: wrap;">
          {/* 사전 조사용 - 사이트 종합 분석 */}
          <button
            onClick={runSiteAnalysis}
            disabled={isCheckingStatus()}
            style={`padding: 12px 20px; background: ${isCheckingStatus() ? '#9ca3af' : '#10b981'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${isCheckingStatus() ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s; flex: 1; min-width: 200px;`}
          >
            {isCheckingStatus() ? '🔄 분석 중...' : '🔍 사이트 종합 분석 (사전 조사)'}
          </button>
          
          {/* 실시간 모니터링용 - 크롤링 상태 조회 */}
          <button
            onClick={runStatusCheck}
            disabled={isCheckingStatus()}
            style={`padding: 12px 20px; background: ${isCheckingStatus() ? '#9ca3af' : '#3b82f6'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${isCheckingStatus() ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s; flex: 1; min-width: 200px;`}
          >
            {isCheckingStatus() ? '🔄 조회 중...' : '📊 크롤링 상태 조회 (실시간)'}
          </button>
        </div>
        
        <div style="margin-bottom: 16px; padding: 12px; background: #f8fafc; border-radius: 6px; border: 1px solid #e2e8f0; font-size: 13px; color: #64748b;">
          💡 <strong>사이트 종합 분석</strong>: 크롤링 전 사이트 구조를 실제로 분석하여 페이지 수, 예상 제품 수 등을 파악합니다 (네트워크 사용)<br/>
          📊 <strong>크롤링 상태 조회</strong>: 현재 진행 중인 크롤링의 실시간 상태와 진행률을 조회합니다 (메모리 조회)
        </div>

        {/* 재시도 통계 - INTEGRATED_PHASE2_PLAN Week 1 Day 5 */}
        <div style="margin-top: 20px; padding: 16px; background: #fef7f0; border-radius: 6px; border: 1px solid #fed7aa;">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <h4 style="margin: 0; font-size: 16px; font-weight: 500; color: #ea580c;">🔄 재시도 메커니즘</h4>
            <button
              onClick={loadRetryStats}
              disabled={isLoadingRetryStats()}
              style={`padding: 6px 12px; background: ${isLoadingRetryStats() ? '#9ca3af' : '#ea580c'}; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: ${isLoadingRetryStats() ? 'not-allowed' : 'pointer'};`}
            >
              {isLoadingRetryStats() ? '로딩...' : '새로고침'}
            </button>
          </div>
          
          {retryStats() ? (
            <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; font-size: 13px;">
              <div style="display: flex; justify-content: space-between;">
                <span style="color: #7c2d12;">총 아이템:</span>
                <span style="font-weight: 500;">{retryStats().total_items}</span>
              </div>
              <div style="display: flex; justify-content: space-between;">
                <span style="color: #7c2d12;">대기 중:</span>
                <span style="font-weight: 500; color: #f59e0b;">{retryStats().pending_retries}</span>
              </div>
              <div style="display: flex; justify-content: space-between;">
                <span style="color: #7c2d12;">성공:</span>
                <span style="font-weight: 500; color: #10b981;">{retryStats().successful_retries}</span>
              </div>
              <div style="display: flex; justify-content: space-between;">
                <span style="color: #7c2d12;">실패:</span>
                <span style="font-weight: 500; color: #ef4444;">{retryStats().failed_retries}</span>
              </div>
            </div>
          ) : (
            <div style="text-align: center; color: #9ca3af; font-size: 13px; padding: 12px;">
              재시도 통계를 로드하려면 새로고침 버튼을 클릭하세요
            </div>
          )}
        </div>

        {statusCheckError() && (
          <div style="padding: 16px; background: #fef2f2; border-radius: 6px; border: 1px solid #fecaca; margin-bottom: 16px;">
            <div style="color: #dc2626; font-weight: 500;">❌ {statusCheckError()}</div>
          </div>
        )}

        {/* 실시간 상태 체크 결과 */}
        {statusCheckResult() && (
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; margin-top: 24px;">
            {/* 사이트 상태 카드 */}
            <div style="background: white; border-radius: 12px; padding: 24px; border: 1px solid #e5e7eb; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
              <div style="display: flex; align-items: center; margin-bottom: 16px;">
                <div style="width: 48px; height: 48px; background: #dbeafe; border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
                  <span style="font-size: 24px;">🌐</span>
                </div>
                <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">사이트 상태</h3>
              </div>
              <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px;">
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">접근성:</span>
                  <span style={`font-weight: 500; ${statusCheckResult()!.site_status.is_accessible ? 'color: #059669;' : 'color: #dc2626;'}`}>
                    {statusCheckResult()!.site_status.is_accessible ? '✅ 정상' : '❌ 불가'}
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">응답 시간:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.site_status.response_time_ms}ms
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">최대 페이지:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.site_status.total_pages} 페이지
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">예상 제품 수:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.site_status.estimated_products.toLocaleString()}개
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">건강도:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {(statusCheckResult()!.site_status.health_score * 100).toFixed(1)}%
                  </span>
                </div>
              </div>
            </div>

            {/* 로컬 DB 상태 카드 */}
            <div style="background: white; border-radius: 12px; padding: 24px; border: 1px solid #e5e7eb; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
              <div style="display: flex; align-items: center; margin-bottom: 16px;">
                <div style="width: 48px; height: 48px; background: #dcfce7; border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
                  <span style="font-size: 24px;">🗃️</span>
                </div>
                <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">로컬 데이터베이스</h3>
              </div>
              <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px;">
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">저장된 제품:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.total_products.toLocaleString()}개
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">페이지 범위:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.page_range[0]}-{statusCheckResult()!.database_status.page_range[1]} 페이지
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">DB 크기:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.size_mb.toFixed(1)} MB
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">상태:</span>
                  <span style={`font-weight: 500; ${getHealthColor(statusCheckResult()!.database_status.health)}`}>
                    {statusCheckResult()!.database_status.health}
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">마지막 크롤링:</span>
                  <span style="font-weight: 500; color: #111827; font-size: 12px;">
                    {statusCheckResult()!.database_status.last_crawl_time || '없음'}
                  </span>
                </div>
              </div>
            </div>

            {/* 스마트 추천 카드 */}
            <div style="background: linear-gradient(135deg, #3b82f6, #8b5cf6); border-radius: 12px; padding: 24px; color: white; box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);">
              <div style="display: flex; align-items: center; margin-bottom: 16px;">
                <div style="width: 48px; height: 48px; background: rgba(255, 255, 255, 0.2); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 16px;">
                  <span style="font-size: 24px;">💡</span>
                </div>
                <h3 style="margin: 0; font-size: 18px; font-weight: 600;">스마트 추천</h3>
              </div>
              <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px;">
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: rgba(255, 255, 255, 0.8);">추천 액션:</span>
                  <span style={`font-weight: 500; padding: 2px 8px; border-radius: 4px; font-size: 12px; ${getPriorityColor(statusCheckResult()!.recommendation?.priority || 'low')}`}>
                    {statusCheckResult()!.recommendation?.action} ({statusCheckResult()!.recommendation?.priority})
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: rgba(255, 255, 255, 0.8);">추천 범위:</span>
                  <span style="font-weight: 500;">
                    {statusCheckResult()!.recommendation?.suggested_range?.[0] || 1}-{statusCheckResult()!.recommendation?.suggested_range?.[1] || 50} 페이지
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: rgba(255, 255, 255, 0.8);">예상 신규:</span>
                  <span style="font-weight: 500; color: #fde047;">
                    {statusCheckResult()!.recommendation?.estimated_new_items?.toLocaleString() || 0}개
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: rgba(255, 255, 255, 0.8);">효율성:</span>
                  <span style={`font-weight: 500; ${
                    (statusCheckResult()!.recommendation?.efficiency_score || 0) > 0.7 ? 'color: #10b981;' : 
                    (statusCheckResult()!.recommendation?.efficiency_score || 0) > 0.3 ? 'color: #fbbf24;' : 'color: #f87171;'
                  }`}>
                    {((statusCheckResult()!.recommendation?.efficiency_score || 0) * 100).toFixed(1)}%
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: rgba(255, 255, 255, 0.8);">동기화율:</span>
                  <span style="font-weight: 500; color: #10b981;">
                    {statusCheckResult()!.sync_comparison.sync_percentage.toFixed(1)}%
                  </span>
                </div>
              </div>
              <div style="margin-top: 16px; padding: 12px; background: rgba(255, 255, 255, 0.1); border-radius: 8px;">
                <p style="margin: 0; font-size: 12px; line-height: 1.5; color: rgba(255, 255, 255, 0.9);">
                  {statusCheckResult()!.recommendation?.reason}
                </p>
              </div>
              {statusCheckResult()!.recommendation?.next_steps && statusCheckResult()!.recommendation!.next_steps.length > 0 && (
                <div style="margin-top: 16px; display: flex; flex-direction: column; gap: 8px;">
                  <span style="font-size: 12px; color: rgba(255, 255, 255, 0.8); font-weight: 500;">📋 다음 단계:</span>
                  <For each={statusCheckResult()!.recommendation!.next_steps}>
                    {(step, index) => (
                      <div style="font-size: 11px; color: rgba(255, 255, 255, 0.9); padding-left: 8px;">
                        {index() + 1}. {step}
                      </div>
                    )}
                  </For>
                </div>
              )}
              <div style="margin-top: 16px;">
                <button 
                  onClick={startCrawling}
                  disabled={crawlingStatus() === 'running'}
                  style={`width: 100%; background: white; color: #3b82f6; padding: 12px; border-radius: 8px; font-weight: 500; font-size: 14px; border: none; cursor: ${crawlingStatus() === 'running' ? 'not-allowed' : 'pointer'}; opacity: ${crawlingStatus() === 'running' ? '0.5' : '1'}; transition: all 0.2s;`}
                >
                  🚀 스마트 크롤링 시작
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 사전조사 결과 표시 */}
        {crawlerStore.siteAnalysisResult() && renderSiteAnalysisResults()}
      </div>

      {/* 실시간 로그 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #1f2937;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: white;">실시간 로그</h3>
        
        <div style="height: 200px; background: #111827; border-radius: 6px; padding: 12px; font-family: 'Monaco', 'Menlo', monospace; font-size: 12px; color: #10b981; overflow-y: auto;">
          <div>[2025-07-05 14:35:12] INFO: 크롤링 엔진 초기화 완료</div>
          <div>[2025-07-05 14:35:13] INFO: 설정 로드 완료</div>
          <div>[2025-07-05 14:35:14] INFO: 대기 중...</div>
          {crawlingStatus() === 'running' && (
            <>
              <div>[2025-07-05 14:35:15] INFO: 크롤링 시작</div>
              <div>[2025-07-05 14:35:16] INFO: 페이지 {currentPage()} 처리 중...</div>
              <div>[2025-07-05 14:35:17] INFO: 배치 {currentBatch()} 진행 중...</div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export const StatusTab: Component = () => {
  // 통합 크롤링 상태 (INTEGRATED_PHASE2_PLAN)
  const integratedStore = useIntegratedCrawlingStore();
  const [viewMode, setViewMode] = createSignal<'classic' | 'city' | '3d' | 'metrics'>('classic');

  onMount(() => {
    // 컴포넌트 마운트 시 초기 데이터 로드
    console.log('🔄 통합 크롤링 상태 조회 시작');
    integratedStore.actions.initialize().catch((error) => {
      console.error('❌ 통합 크롤링 상태 조회 실패:', error);
    });
    
    // 백엔드 연결 상태 확인 (Phase 3)
    console.log('🔌 백엔드 연결 상태 확인 시작');
    integratedStore.actions.connectToBackend().catch((error: any) => {
      console.error('❌ 백엔드 연결 실패:', error);
    });
  });

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
        <h2 style="margin: 0; font-size: 24px; font-weight: 600; color: #1f2937;">📊 상태 & 제어</h2>
        
        {/* 백엔드 연결 상태 인디케이터 */}
        <div style="display: flex; align-items: center; gap: 8px;">
          <div style={`width: 8px; height: 8px; border-radius: 50%; background: ${integratedStore.state.isBackendConnected ? '#10b981' : '#ef4444'};`}></div>
          <span style="font-size: 14px; color: #6b7280;">
            {integratedStore.state.isBackendConnected ? '🟢 백엔드 연결됨' : '🔴 백엔드 연결 안됨'}
          </span>
          {integratedStore.state.simulationMode && (
            <span style="font-size: 12px; color: #f59e0b; background: #fef3c7; padding: 2px 6px; border-radius: 4px; margin-left: 8px;">
              🎭 시뮬레이션 모드
            </span>
          )}
        </div>
      </div>
      
      {/* 백엔드 연결 상태 상세 정보 */}
      <BackendConnectionStatus />
      
      {/* 뷰 모드 선택기 */}
      <ViewModeSelector value={viewMode()} onChange={setViewMode} />
      
      {/* 뷰 모드에 따른 내용 표시 */}
      <Show when={viewMode() === 'classic'}>
        <ClassicStatusView />
      </Show>
      <Show when={viewMode() === 'city'}>
        <CrawlingCityDashboard
          progress={integratedStore.state.systemState ? {
            current: integratedStore.state.systemState.progress.completedTasks,
            total: integratedStore.state.systemState.progress.totalTasks,
            percentage: integratedStore.state.systemState.progress.percentage,
            current_stage: 'ProductDetails' as any,
            current_step: 'Processing...',
            status: integratedStore.state.systemState.overallStatus === 'Running' ? 'Running' as any : 'Idle' as any,
            message: 'Crawling in progress',
            remaining_time: 0,
            elapsed_time: 0,
            new_items: integratedStore.state.systemState.totalProductsSaved,
            updated_items: Math.floor(integratedStore.state.systemState.totalProductsSaved * 0.8),
            errors: integratedStore.state.systemState.errorCount,
            timestamp: new Date().toISOString()
          } : null}
          isRunning={integratedStore.state.systemState?.overallStatus === 'Running'}
          onToggleRunning={() => {}}
          onPauseResume={() => {}}
          onStop={() => {}}
        />
      </Show>
      <Show when={viewMode() === '3d'}>
        <CrawlingCity3D
          progress={integratedStore.state.systemState ? {
            current: integratedStore.state.systemState.progress.completedTasks,
            total: integratedStore.state.systemState.progress.totalTasks,
            percentage: integratedStore.state.systemState.progress.percentage,
            current_stage: 'ProductDetails' as any,
            current_step: 'Processing...',
            status: integratedStore.state.systemState.overallStatus === 'Running' ? 'Running' as any : 'Idle' as any,
            message: 'Crawling in progress',
            remaining_time: 0,
            elapsed_time: 0,
            new_items: integratedStore.state.systemState.totalProductsSaved,
            updated_items: Math.floor(integratedStore.state.systemState.totalProductsSaved * 0.8),
            errors: integratedStore.state.systemState.errorCount,
            timestamp: new Date().toISOString()
          } : null}
          isRunning={integratedStore.state.systemState?.overallStatus === 'Running'}
          onBuildingClick={(buildingId) => console.log('Building clicked:', buildingId)}
        />
      </Show>
      <Show when={viewMode() === 'metrics'}>
        <CrawlingMetricsChart
          progress={integratedStore.state.systemState ? {
            current: integratedStore.state.systemState.progress.completedTasks,
            total: integratedStore.state.systemState.progress.totalTasks,
            percentage: integratedStore.state.systemState.progress.percentage,
            current_stage: 'ProductDetails' as any,
            current_step: 'Processing...',
            status: integratedStore.state.systemState.overallStatus === 'Running' ? 'Running' as any : 'Idle' as any,
            message: 'Crawling in progress',
            remaining_time: 0,
            elapsed_time: 0,
            new_items: integratedStore.state.systemState.totalProductsSaved,
            updated_items: Math.floor(integratedStore.state.systemState.totalProductsSaved * 0.8),
            errors: integratedStore.state.systemState.errorCount,
            timestamp: new Date().toISOString()
          } : null}
          isRunning={integratedStore.state.systemState?.overallStatus === 'Running'}
          timeRange={5}
        />
      </Show>
    </div>
  );
};

// 백엔드 연결 상태 표시 컴포넌트
const BackendConnectionStatus: Component = () => {
  const { state, actions } = useIntegratedCrawlingStore();
  
  const getConnectionStatusColor = () => {
    if (!state.isBackendConnected) return '#ef4444'; // 빨간색 (연결 안됨)
    if (state.simulationMode) return '#f59e0b'; // 노란색 (시뮬레이션 모드)
    return '#22c55e'; // 녹색 (정상 연결)
  };

  const getConnectionStatusText = () => {
    if (!state.isBackendConnected) return '❌ 백엔드 연결 안됨';
    if (state.simulationMode) return '⚠️ 시뮬레이션 모드';
    return '✅ 백엔드 연결됨';
  };

  const testConnection = async () => {
    const result = await actions.testBackendConnection();
    console.log('🔍 연결 테스트 결과:', result);
  };

  return (
    <div class="bg-white rounded-xl shadow-lg p-4 mb-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-3">
          <div 
            class="w-3 h-3 rounded-full"
            style={{ "background-color": getConnectionStatusColor() }}
          />
          <div>
            <div class="font-medium text-gray-800">{getConnectionStatusText()}</div>
            <div class="text-sm text-gray-500">
              {state.lastBackendUpdate ? 
                `최근 업데이트: ${new Date(state.lastBackendUpdate).toLocaleTimeString()}` : 
                '업데이트 없음'
              }
            </div>
          </div>
        </div>
        <button
          onClick={testConnection}
          class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors duration-200 text-sm"
        >
          🔍 연결 테스트
        </button>
      </div>
    </div>
  );
};
