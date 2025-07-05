/**
 * StatusTab - 크롤링 상태 및 제어 탭 컴포넌트
 */

import { Component, createSignal, Show } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { CrawlingStatusCheck } from '../../types/crawling';

export const StatusTab: Component = () => {
  // 크롤링 상태
  const [crawlingStatus, setCrawlingStatus] = createSignal<'idle' | 'running' | 'paused' | 'completed'>('idle');
  const [progress, setProgress] = createSignal(0);
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(100);
  const [currentBatch, setCurrentBatch] = createSignal(0);
  const [totalBatches, setTotalBatches] = createSignal(10);
  const [estimatedTime, setEstimatedTime] = createSignal('계산 중...');

  // 상태 체크 결과
  const [statusCheckResult, setStatusCheckResult] = createSignal<CrawlingStatusCheck | null>(null);
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

  const startCrawling = async () => {
    if (statusCheckResult()) {
      // 상태 체크 결과가 있으면 추천 범위로 크롤링 시작
      const config = {
        // 기본 설정
        start_page: statusCheckResult()!.recommended_start_page,
        end_page: statusCheckResult()!.recommended_end_page,
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
        
        // URL
        base_url: "https://csa-iot.org",
        matter_filter_url: "https://csa-iot.org/csa_product/?p_type%5B%5D=14&f_program_type%5B%5D=1049",
        
        // 타임아웃
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

  const runStatusCheck = async () => {
    try {
      setIsCheckingStatus(true);
      setStatusCheckError('');
      setStatusCheckResult(null);
      
      console.log('🔍 상태 체크 시작...');
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
            <div style="font-size: 18px; font-weight: 600; color: #1f2937;">{currentBatch()}/{totalBatches()}</div>
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
            <strong>🎯 추천 크롤링:</strong> 페이지 {statusCheckResult()!.recommended_start_page}-{statusCheckResult()!.recommended_end_page} 
            (약 {statusCheckResult()!.estimated_new_products}개 신규 제품 예상)
          </div>
        )}
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={startCrawling}
            disabled={crawlingStatus() === 'running'}
            style={`padding: 12px 24px; background: ${crawlingStatus() === 'running' ? '#9ca3af' : statusCheckResult() ? '#10b981' : '#22c55e'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() === 'running' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            {statusCheckResult() ? '🤖 스마트 크롤링 시작' : '▶️ 기본 크롤링 시작'}
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
          <div style="margin-top: 12px; padding: 8px; background: #fef3c7; border-radius: 4px; font-size: 13px; color: #92400e;">
            💡 최적의 크롤링을 위해 먼저 "상태 체크"를 실행해주세요.
          </div>
        )}
      </div>

      {/* 상태 체크 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">상태 체크</h3>
        
        <button
          onClick={runStatusCheck}
          disabled={isCheckingStatus()}
          style={`padding: 12px 24px; background: ${isCheckingStatus() ? '#9ca3af' : '#3b82f6'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${isCheckingStatus() ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s; margin-bottom: 16px;`}
          onMouseOver={(e) => !isCheckingStatus() && (e.currentTarget.style.background = '#2563eb')}
          onMouseOut={(e) => !isCheckingStatus() && (e.currentTarget.style.background = '#3b82f6')}
        >
          {isCheckingStatus() ? '🔄 상태 확인 중...' : '🔍 로컬DB 상태 체크 실행'}
        </button>

        {statusCheckError() && (
          <div style="padding: 16px; background: #fef2f2; border-radius: 6px; border: 1px solid #fecaca; margin-bottom: 16px;">
            <div style="color: #dc2626; font-weight: 500;">❌ {statusCheckError()}</div>
          </div>
        )}

        {statusCheckResult() && (
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">📊 실시간 상태 체크 결과</h4>
            <div style="display: grid; gap: 8px; font-size: 14px;">
              
              {/* 사이트 상태 */}
              <div style="padding: 8px; background: #f8fafc; border-radius: 4px; border-left: 4px solid #3b82f6;">
                <strong>🌐 사이트 상태:</strong> 
                <span style={`color: ${statusCheckResult()?.site_accessible ? '#059669' : '#dc2626'}; margin-left: 8px;`}>
                  {statusCheckResult()?.site_accessible ? '✅ 접근 가능' : '❌ 접근 불가'}
                </span>
              </div>

              {/* 데이터베이스 정보 */}
              <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-top: 8px;">
                <div style="padding: 8px; background: #f0f9ff; border-radius: 4px;">
                  <strong>📦 로컬DB 제품 수:</strong><br/>
                  <span style="font-size: 18px; font-weight: 600; color: #1e40af;">
                    {statusCheckResult()?.local_db_product_count?.toLocaleString() || '0'}개
                  </span>
                </div>
                <div style="padding: 8px; background: #f0fdf4; border-radius: 4px;">
                  <strong>🌍 사이트 전체 제품:</strong><br/>
                  <span style="font-size: 18px; font-weight: 600; color: #166534;">
                    {statusCheckResult()?.estimated_total_products?.toLocaleString() || '확인 중'}개
                  </span>
                </div>
              </div>

              {/* 페이지 정보 */}
              <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-top: 8px;">
                <div style="padding: 8px; background: #fefce8; border-radius: 4px;">
                  <strong>📄 로컬DB 페이지 범위:</strong><br/>
                  <span style="color: #a16207;">
                    {statusCheckResult()?.local_db_page_range?.[0]}-{statusCheckResult()?.local_db_page_range?.[1]} 페이지
                  </span>
                </div>
                <div style="padding: 8px; background: #fff7ed; border-radius: 4px;">
                  <strong>🎯 사이트 최대 페이지:</strong><br/>
                  <span style="color: #c2410c;">
                    {statusCheckResult()?.detected_max_page || '확인 중'} 페이지
                  </span>
                </div>
              </div>

              {/* 크롤링 정보 */}
              <div style="margin-top: 12px; padding: 12px; background: #f0f9ff; border-radius: 6px; border: 2px solid #3b82f6;">
                <strong style="color: #1e40af;">📈 스마트 크롤링 추천:</strong>
                <div style="margin-top: 8px; display: grid; gap: 6px;">
                  <div><strong>추천 범위:</strong> 페이지 {statusCheckResult()?.recommended_start_page}-{statusCheckResult()?.recommended_end_page}</div>
                  <div><strong>예상 신규 제품:</strong> <span style="color: #dc2626; font-weight: bold;">{statusCheckResult()?.estimated_new_products?.toLocaleString() || '0'}개</span></div>
                  <div><strong>크롤링 효율성:</strong> 
                    <span style={`color: ${(statusCheckResult()?.crawling_efficiency_score || 0) > 0.7 ? '#059669' : (statusCheckResult()?.crawling_efficiency_score || 0) > 0.3 ? '#f59e0b' : '#dc2626'}; margin-left: 4px;`}>
                      {((statusCheckResult()?.crawling_efficiency_score || 0) * 100).toFixed(1)}%
                      {(statusCheckResult()?.crawling_efficiency_score || 0) > 0.7 ? ' 🟢 매우 효율적' : 
                       (statusCheckResult()?.crawling_efficiency_score || 0) > 0.3 ? ' 🟡 보통' : ' 🔴 비효율적'}
                    </span>
                  </div>
                </div>
              </div>

              {/* 추천 이유 */}
              <div style="margin-top: 8px; padding: 12px; background: linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 100%); border-radius: 6px; border-left: 4px solid #0ea5e9;">
                <strong>💡 추천 이유:</strong>
                <div style="margin-top: 4px; color: #0369a1; line-height: 1.4;">
                  {statusCheckResult()?.recommendation_reason}
                </div>
              </div>

              {/* 마지막 정보 */}
              <div style="margin-top: 8px; font-size: 12px; color: #6b7280;">
                <div><strong>마지막 크롤링:</strong> {statusCheckResult()?.last_crawl_time || '없음'}</div>
                <div><strong>상태 체크 시간:</strong> {new Date().toLocaleString('ko-KR')}</div>
              </div>
            </div>
          </div>
        )}
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
