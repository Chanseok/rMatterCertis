/**
 * StatusTab - 크롤링 상태 및 제어 탭 컴포넌트 (개선된 UI)
 */

import { Component, createSignal, For } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { crawlerStore } from '../../stores/crawlerStore';
import type { CrawlingStatusCheck } from '../../types/crawling';

export const StatusTab: Component = () => {
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
  const getSuggestedRange = () => statusCheckResult()?.recommendation?.suggested_range || null;
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

  const startCrawling = async () => {
    console.log('🔥 startCrawling 함수 호출됨');
    const result = getActiveResult();
    console.log('🔍 getActiveResult 결과:', result);
    
    // 상태 체크 결과가 있으면 추천 범위로, 없으면 기본 설정으로 크롤링 시작
    const suggestion = result ? getSuggestedRange() : null;
    const config = {
      // 기본 설정 (상태 체크 결과가 없어도 동작)
      start_page: suggestion ? suggestion[0] : 1,
      end_page: suggestion ? suggestion[1] : 10, // 기본값을 10으로 설정
      concurrency: 3,
      delay_ms: 1000,
      
      // 고급 설정
      page_range_limit: 500,
      product_list_retry_count: 3,
      product_detail_retry_count: 3,
      products_per_page: 12,
      auto_add_to_local_db: true,
      auto_status_check: true,
      crawler_type: "smart",
      
      // 배치 처리
      batch_size: 50,
      batch_delay_ms: 2000,
      enable_batch_processing: true,
      batch_retry_limit: 3,
      
      // URL 설정
      base_url: "https://csa-iot.org",
      matter_filter_url: "https://csa-iot.org/csa_product/?p_type%5B%5D=14&f_program_type%5B%5D=1049",
      
      // 타임아웃 설정
      page_timeout_ms: 30000,
      product_detail_timeout_ms: 20000,
      
      // 동시성 및 성능
      initial_concurrency: 3,
      detail_concurrency: 5,
      retry_concurrency: 2,
      min_request_delay_ms: 500,
      max_request_delay_ms: 2000,
      retry_start: 1,
      retry_max: 3,
      cache_ttl_ms: 300000,
      
      // 브라우저 설정
      headless_browser: true,
      max_concurrent_tasks: 10,
      request_delay: 1000,
      custom_user_agent: "rMatterCertis/2.0",
      
      // 로깅
      logging: {
        level: "info",
        enable_stack_trace: false,
        enable_timestamp: true,
        components: {
          "crawler": "info",
          "http": "warn",
          "database": "info"
        }
      }
    };
    
    try {
      setCrawlingStatus('running');
      console.log('🚀 크롤링 시작:', config);
      console.log('📊 상태 체크 결과 기반:', result ? '✅ 스마트 모드' : '🔧 기본 모드');
      
      // 실제 크롤링 시작
      console.log('📞 tauriApi.startCrawling 호출 시도...');
      const sessionId = await tauriApi.startCrawling(config);
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

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">📊 상태 & 제어</h2>
      
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
        
        {statusCheckResult() && (
          <div style="margin-bottom: 16px; padding: 12px; background: #f0f9ff; border-radius: 6px; border-left: 4px solid #3b82f6; font-size: 14px;">
            <strong>🎯 추천 크롤링:</strong> 페이지 {statusCheckResult()!.recommendation?.suggested_range?.[0] || 1}-{statusCheckResult()!.recommendation?.suggested_range?.[1] || 50} 
            (약 {statusCheckResult()!.recommendation?.estimated_new_items || 0}개 신규 제품 예상)
          </div>
        )}
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={() => {
              console.log('🔴 버튼 클릭됨 - crawlingStatus:', crawlingStatus());
              startCrawling();
            }}
            disabled={crawlingStatus() === 'running'}
            style={`padding: 12px 24px; background: ${crawlingStatus() === 'running' ? '#9ca3af' : statusCheckResult() ? '#10b981' : '#22c55e'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() === 'running' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            {crawlingStatus() === 'running' 
              ? '🔄 크롤링 중...' 
              : statusCheckResult() 
                ? '🤖 스마트 크롤링 시작' 
                : '▶️ 기본 크롤링 시작 (1-10 페이지)'
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
            💡 상태 체크 없이도 기본 크롤링(1-10 페이지)을 시작할 수 있습니다. 최적화된 크롤링을 원하시면 먼저 "상태 체크"를 실행해주세요.
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
