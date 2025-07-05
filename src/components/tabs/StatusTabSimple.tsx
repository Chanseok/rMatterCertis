/**
 * StatusTab - 크롤링 상태 및 제어 탭 컴포넌트
 */

import { Component, createSignal } from 'solid-js';

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
  const [statusCheckResult, setStatusCheckResult] = createSignal<any>(null);

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

  const startCrawling = () => {
    setCrawlingStatus('running');
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

  const pauseCrawling = () => {
    setCrawlingStatus('paused');
  };

  const stopCrawling = () => {
    setCrawlingStatus('idle');
    setProgress(0);
    setCurrentPage(0);
  };

  const runStatusCheck = () => {
    setStatusCheckResult({
      localDbCount: 1248,
      lastCrawlTime: '2025-07-05 14:30:00',
      recommendedRange: '페이지 1249-1500',
      estimatedNewItems: 252,
      duplicateRisk: 'Low'
    });
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

      {/* 제어 버튼 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #fefefe;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">크롤링 제어</h3>
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={startCrawling}
            disabled={crawlingStatus() === 'running'}
            style={`padding: 12px 24px; background: ${crawlingStatus() === 'running' ? '#9ca3af' : '#22c55e'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${crawlingStatus() === 'running' ? 'not-allowed' : 'pointer'}; transition: background-color 0.2s;`}
          >
            ▶️ 시작
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
      </div>

      {/* 상태 체크 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">상태 체크</h3>
        
        <button
          onClick={runStatusCheck}
          style="padding: 12px 24px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s; margin-bottom: 16px;"
          onMouseOver={(e) => e.currentTarget.style.background = '#2563eb'}
          onMouseOut={(e) => e.currentTarget.style.background = '#3b82f6'}
        >
          🔍 로컬DB 상태 체크 실행
        </button>

        {statusCheckResult() && (
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">상태 체크 결과</h4>
            <div style="display: grid; gap: 8px; font-size: 14px;">
              <div><strong>로컬DB 데이터 수:</strong> {statusCheckResult()?.localDbCount}개</div>
              <div><strong>마지막 크롤링:</strong> {statusCheckResult()?.lastCrawlTime}</div>
              <div style="color: #059669;"><strong>추천 크롤링 범위:</strong> {statusCheckResult()?.recommendedRange}</div>
              <div><strong>예상 신규 아이템:</strong> {statusCheckResult()?.estimatedNewItems}개</div>
              <div style={`color: ${statusCheckResult()?.duplicateRisk === 'Low' ? '#059669' : '#dc2626'};`}>
                <strong>중복 위험도:</strong> {statusCheckResult()?.duplicateRisk}
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
