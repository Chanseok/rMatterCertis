/**
 * StatusTab - 크롤링 상태 및 제어 탭 컴포넌트 (개선된 UI)
 */

import { Component, createSignal, For } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { CrawlingStatusCheck } from '../../types/crawling';

export const StatusTab: Component = () => {
  // 크롤링 상태
  const [crawlingStatus, setCrawlingStatus] = createSignal<'idle' | 'running' | 'paused' | 'completed'>('idle');
  const [progress, setProgress] = createSignal(0);
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages] = createSignal(100);
  const [currentBatch] = createSignal(0);
  const [totalBatches] = createSignal(10);
  const [estimatedTime] = createSignal('계산 중...');

  // 상태 체크 결과 (두 가지 타입 모두 지원)
  const [statusCheckResult, setStatusCheckResult] = createSignal<CrawlingStatusCheck | null>(null);
  const [siteAnalysisResult, setSiteAnalysisResult] = createSignal<any>(null);
  const [isCheckingStatus, setIsCheckingStatus] = createSignal(false);
  const [statusCheckError, setStatusCheckError] = createSignal<string>('');

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

  const getRecommendationReason = () => {
    // 실시간 상태 체크 결과 우선 사용
    if (statusCheckResult()) {
      return statusCheckResult()!.recommendation?.reason || '권장 사항이 없습니다.';
    }
    
    // 사전조사 결과도 확인 (구조가 다를 수 있음)
    if (siteAnalysisResult()) {
      const result = siteAnalysisResult()!;
      if (result.comparison?.recommended_action) {
        switch (result.comparison.recommended_action) {
          case 'crawling_needed':
            return '사이트에 새로운 데이터가 있어 크롤링이 필요합니다.';
          case 'cleanup_needed':
            return '중복 데이터가 발견되어 정리가 필요합니다.';
          case 'up_to_date':
            return '현재 데이터가 최신 상태입니다.';
          default:
            return '분석 결과를 확인해주세요.';
        }
      }
    }
    
    return '상태 체크를 먼저 실행해주세요.';
  };

  const getActiveResult = () => {
    // 실시간 상태 체크 결과가 있으면 그것을 우선 사용
    return statusCheckResult() || null;
  };

  const getSuggestedRange = () => {
    if (statusCheckResult()) {
      return statusCheckResult()!.recommendation?.suggested_range;
    }
    return null;
  };

  const startCrawling = async () => {
    const result = getActiveResult();
    if (result) {
      // 상태 체크 결과가 있으면 추천 범위로 크롤링 시작
      const suggestion = getSuggestedRange();
      const config = {
        // 기본 설정
        start_page: suggestion ? suggestion[0] : 1,
        end_page: suggestion ? suggestion[1] : 50,
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
        
        // 재시도 설정
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
        console.log('🚀 스마트 크롤링 시작:', config);
        
        // 실제 크롤링 시작
        const sessionId = await tauriApi.startCrawling(config);
        console.log('✅ 크롤링 세션 시작됨:', sessionId);
        
        // TODO: 실시간 진행률 업데이트 구현
        // 임시로 시뮬레이션 유지
        simulateProgress();
      } catch (error) {
        console.error('❌ 크롤링 시작 실패:', error);
        setCrawlingStatus('idle');
        alert(`크롤링 시작에 실패했습니다: ${error}`);
      }
    } else {
      // 상태 체크 결과가 없으면 먼저 상태 체크 실행 권장
      alert('먼저 "상태 체크"를 실행하여 최적의 크롤링 범위를 확인해주세요.');
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
  const runSiteAnalysis = async () => {
    try {
      setIsCheckingStatus(true);
      setStatusCheckError('');
      setStatusCheckResult(null);
      setSiteAnalysisResult(null);
      
      console.log('🔍 사이트 종합 분석 시작...');
      console.log('📡 실제 웹사이트에 접속하여 페이지 구조를 분석하고 DB와 비교합니다...');
      
      const result = await tauriApi.checkSiteStatus();
      console.log('✅ 사이트 분석 완료:', result);
      
      // 사전조사 결과는 별도 signal에 저장 (구조가 다름)
      setSiteAnalysisResult(result);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setStatusCheckError(`사이트 분석 실패: ${errorMessage}`);
      console.error('❌ 사이트 분석 실패:', error);
    } finally {
      setIsCheckingStatus(false);
    }
  };

  // 실시간 모니터링용 상태 체크 (get_crawling_status_check)
  const runStatusCheck = async () => {
    try {
      setIsCheckingStatus(true);
      setStatusCheckError('');
      setStatusCheckResult(null);
      setSiteAnalysisResult(null);
      
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

  const renderSiteAnalysisResults = () => {
    const result = siteAnalysisResult();
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
              <span style={`font-weight: 500; ${result.site_status?.accessible ? 'color: #059669;' : 'color: #dc2626;'}`}>
                {result.site_status?.accessible ? '✅ 정상' : '❌ 불가'}
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
                {result.database_analysis?.total_products?.toLocaleString() || 0}개
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">고유 제품:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.database_analysis?.unique_products?.toLocaleString() || 0}개
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">중복 제품:</span>
              <span style="font-weight: 500; color: #111827;">
                {result.database_analysis?.duplicate_count?.toLocaleString() || 0}개
              </span>
            </div>
            <div style="display: flex; justify-content: space-between;">
              <span style="color: #6b7280;">동기화율:</span>
              <span style="font-weight: 500; color: #111827;">
                {Math.round(result.comparison?.sync_percentage || 0)}%
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
          <div style="display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 14px;">
            <span>전체 진행률</span>
            <span>{progress()}%</span>
          </div>
          <div style="width: 100%; background: #e5e7eb; border-radius: 4px; height: 8px; overflow: hidden;">
            <div 
              style={`height: 100%; background: ${getStatusColor()}; transition: width 0.3s ease; width: ${progress()}%;`}
            ></div>
          </div>
        </div>

        {/* 상세 정보 */}
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; font-size: 14px;">
          <div>
            <span style="color: #6b7280;">현재 페이지:</span>
            <span style="margin-left: 8px; font-weight: 500;">{currentPage()}/{totalPages()}</span>
          </div>
          <div>
            <span style="color: #6b7280;">현재 배치:</span>
            <span style="margin-left: 8px; font-weight: 500;">{currentBatch()}/{totalBatches()}</span>
          </div>
          <div>
            <span style="color: #6b7280;">예상 완료 시간:</span>
            <span style="margin-left: 8px; font-weight: 500;">{estimatedTime()}</span>
          </div>
        </div>
      </div>

      {/* 스마트 크롤링 제어 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">🤖 스마트 크롤링 제어</h3>
        
        <div style="display: flex; gap: 12px; margin-bottom: 16px;">
          <button
            onClick={startCrawling}
            disabled={crawlingStatus() === 'running'}
            style={`padding: 10px 20px; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; 
                   transition: all 0.2s; ${
              crawlingStatus() === 'running' 
                ? 'background: #9ca3af; color: white; cursor: not-allowed;' 
                : 'background: #22c55e; color: white;'
            }`}
          >
            ▶️ 기본 크롤링 시작
          </button>

          <button
            onClick={pauseCrawling}
            disabled={crawlingStatus() !== 'running'}
            style={`padding: 10px 20px; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; 
                   transition: all 0.2s; ${
              crawlingStatus() !== 'running' 
                ? 'background: #9ca3af; color: white; cursor: not-allowed;' 
                : 'background: #f59e0b; color: white;'
            }`}
          >
            ⏸️ 일시정지
          </button>

          <button
            onClick={stopCrawling}
            disabled={crawlingStatus() === 'idle'}
            style={`padding: 10px 20px; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; 
                   transition: all 0.2s; ${
              crawlingStatus() === 'idle' 
                ? 'background: #9ca3af; color: white; cursor: not-allowed;' 
                : 'background: #ef4444; color: white;'
            }`}
          >
            ⏹️ 중지
          </button>
        </div>
        
        <div style="margin-bottom: 16px; padding: 12px; background: #f8fafc; border-radius: 6px; border: 1px solid #e2e8f0; font-size: 13px; color: #64748b;">
          💡 최적의 크롤링을 위해 먼저 "상태 체크"를 실행해주세요. 
          <strong>{getRecommendationReason()}</strong>
        </div>
      </div>

      {/* 상태 체크 & 분석 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">상태 체크 & 분석</h3>
        
        <div style="display: flex; gap: 12px; margin-bottom: 16px;">
          <button
            onClick={runSiteAnalysis}
            disabled={isCheckingStatus()}
            style={`padding: 12px 20px; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; 
                   transition: all 0.2s; flex: 1; ${
              isCheckingStatus() 
                ? 'background: #9ca3af; color: white; cursor: not-allowed;' 
                : 'background: #6b7280; color: white;'
            }`}
          >
            🔍 사이트 종합 분석 (사전조사)
          </button>

          <button
            onClick={runStatusCheck}
            disabled={isCheckingStatus()}
            style={`padding: 12px 20px; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; 
                   transition: all 0.2s; flex: 1; ${
              isCheckingStatus() 
                ? 'background: #9ca3af; color: white; cursor: not-allowed;' 
                : 'background: #3b82f6; color: white;'
            }`}
          >
            📊 크롤링 상태 조회 (실시간)
          </button>
        </div>
        
        <div style="margin-bottom: 16px; padding: 12px; background: #f8fafc; border-radius: 6px; border: 1px solid #e2e8f0; font-size: 13px; color: #64748b;">
          💡 <strong>사이트 종합 분석</strong>: 크롤링 전 사이트 구조를 실제로 분석하여 페이지 수, 예상 제품 수 등을 파악합니다 (네트워크 사용)<br/>
          📊 <strong>크롤링 상태 조회</strong>: 현재 진행 중인 크롤링의 실시간 상태와 진행률을 조회합니다 (메모리 조회)
        </div>

        {statusCheckError() && (
          <div style="padding: 16px; background: #fef2f2; border-radius: 6px; border: 1px solid #fecaca; margin-bottom: 16px;">
            <div style="color: #dc2626; font-weight: 500;">❌ {statusCheckError()}</div>
          </div>
        )}

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
                  <span style="font-weight: 500; color: #059669;">
                    {Math.round(statusCheckResult()!.site_status.health_score * 100)}%
                  </span>
                </div>
              </div>
              <div style="margin-top: 16px; padding: 12px; background: #f8fafc; border-radius: 6px; font-size: 13px; color: #6b7280;">
                <strong>🎯 추천 크롤링:</strong> 페이지 {statusCheckResult()!.recommendation?.suggested_range?.[0] || 1}-{statusCheckResult()!.recommendation?.suggested_range?.[1] || 50} 
                ({statusCheckResult()!.recommendation?.estimated_new_items || 0}개 예상)
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
                  <span style="color: #6b7280;">저장된 제품:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.total_products.toLocaleString()}개
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">페이지 범위:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.page_range[0]}-{statusCheckResult()!.database_status.page_range[1]}
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">DB 크기:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.database_status.size_mb.toFixed(1)}MB
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">건강도:</span>
                  <span style={`font-weight: 500; ${
                    statusCheckResult()!.database_status.health === 'Healthy' ? 'color: #059669;' :
                    statusCheckResult()!.database_status.health === 'Warning' ? 'color: #f59e0b;' : 'color: #dc2626;'
                  }`}>
                    {statusCheckResult()!.database_status.health === 'Healthy' ? '✅ 양호' :
                     statusCheckResult()!.database_status.health === 'Warning' ? '⚠️ 주의' : '❌ 위험'}
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
                <h3 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">스마트 추천</h3>
              </div>
              <div style="margin-bottom: 16px;">
                <div style={`padding: 8px 12px; border-radius: 6px; display: inline-block; font-size: 12px; font-weight: 500; border: 1px solid; ${
                  getPriorityColor(statusCheckResult()!.recommendation?.priority || 'low')
                }`}>
                  {statusCheckResult()!.recommendation?.priority === 'critical' ? '🔴 긴급' :
                   statusCheckResult()!.recommendation?.priority === 'high' ? '🟠 높음' :
                   statusCheckResult()!.recommendation?.priority === 'medium' ? '🟡 보통' : '🟢 낮음'}
                </div>
              </div>
              <div style="font-size: 14px; line-height: 1.6; color: #374151; margin-bottom: 16px;">
                {statusCheckResult()!.recommendation?.reason}
              </div>
              <div style="display: flex; flex-direction: column; gap: 8px; font-size: 13px;">
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">권장 행동:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.recommendation?.action === 'crawl' ? '🚀 크롤링' :
                     statusCheckResult()!.recommendation?.action === 'cleanup' ? '🧹 정리' :
                     statusCheckResult()!.recommendation?.action === 'wait' ? '⏳ 대기' : '🔍 수동 확인'}
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">추천 범위:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.recommendation?.suggested_range?.[0] || 1}-{statusCheckResult()!.recommendation?.suggested_range?.[1] || 50} 페이지
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">예상 신규:</span>
                  <span style="font-weight: 500; color: #111827;">
                    {statusCheckResult()!.recommendation?.estimated_new_items || 0}개
                  </span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                  <span style="color: #6b7280;">효율성:</span>
                  <span style="font-weight: 500; color: #059669;">
                    {Math.round((statusCheckResult()!.recommendation?.efficiency_score || 0) * 100)}%
                  </span>
                </div>
              </div>
              
              {statusCheckResult()!.recommendation?.next_steps && statusCheckResult()!.recommendation!.next_steps.length > 0 && (
                <div style="margin-top: 16px; padding: 12px; background: #f8fafc; border-radius: 6px;">
                  <div style="font-size: 13px; font-weight: 500; color: #374151; margin-bottom: 8px;">📋 다음 단계:</div>
                  <For each={statusCheckResult()!.recommendation!.next_steps}>
                    {(step, index) => (
                      <div style="font-size: 12px; color: #6b7280; margin-bottom: 4px;">
                        {index() + 1}. {step}
                      </div>
                    )}
                  </For>
                </div>
              )}
            </div>
          </div>
        )}

        {siteAnalysisResult() && renderSiteAnalysisResults()}
      </div>
    </div>
  );
};
