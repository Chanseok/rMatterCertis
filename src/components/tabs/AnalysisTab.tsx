/**
 * AnalysisTab - 데이터 분석 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, onMount, For } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';

export const AnalysisTab: Component = () => {
  // 실제 분석 데이터
  const [analysisData, setAnalysisData] = createSignal({
    totalCrawled: 0,
    successRate: 0,
    errorRate: 0,
    avgResponseTime: 0,
    categoryCounts: {},
    dailyStats: []
  });
  
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string>('');

  // 실제 분석 데이터 로드
  const loadAnalysisData = async () => {
    try {
      setIsLoading(true);
      const data = await tauriApi.getAnalysisData();
      setAnalysisData(data);
    } catch (err) {
      console.error('Failed to load analysis data:', err);
      setError(`분석 데이터 로드 실패: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  // 컴포넌트 마운트 시 데이터 로드
  onMount(() => {
    loadAnalysisData();
  });

  const generateReport = async () => {
    try {
      const reportPath = await tauriApi.exportCrawlingResults();
      alert(`분석 보고서가 생성되었습니다: ${reportPath}`);
    } catch (err) {
      alert(`보고서 생성 실패: ${err}`);
    }
  };

  const exportChart = () => {
    alert('차트 내보내기 기능은 개발 중입니다.');
  };

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">📈 분석 (실제 데이터)</h2>
      
      {/* 에러 표시 */}
      {error() && (
        <div style="margin-bottom: 16px; padding: 16px; background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px; color: #dc2626;">
          {error()}
        </div>
      )}

      {/* 로딩 상태 */}
      {isLoading() && (
        <div style="padding: 32px; text-align: center; color: #6b7280;">
          <div style="margin-bottom: 8px;">분석 데이터를 로드하는 중...</div>
          <div style="width: 24px; height: 24px; border: 2px solid #e5e7eb; border-top: 2px solid #3b82f6; border-radius: 50%; animation: spin 1s linear infinite; margin: 0 auto;"></div>
        </div>
      )}

      {/* 데이터가 없는 경우 */}
      {!isLoading() && !error() && analysisData().totalCrawled === 0 && (
        <div style="padding: 32px; text-align: center; color: #6b7280; border: 1px solid #e5e7eb; border-radius: 8px;">
          <div style="font-size: 48px; margin-bottom: 16px;">📊</div>
          <div style="font-size: 18px; font-weight: 500; margin-bottom: 8px;">분석할 데이터가 없습니다</div>
          <div style="font-size: 14px;">크롤링을 실행하여 데이터를 수집한 후 분석을 확인할 수 있습니다.</div>
          <button
            onClick={loadAnalysisData}
            style="margin-top: 16px; padding: 8px 16px; background: #3b82f6; color: white; border: none; border-radius: 6px; cursor: pointer;"
          >
            다시 시도
          </button>
        </div>
      )}

      {/* 실제 분석 데이터 */}
      {!isLoading() && !error() && analysisData().totalCrawled > 0 && (
        <>
          {/* 주요 지표 */}
          <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
              <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">주요 지표</h3>
              <button
                onClick={loadAnalysisData}
                style="padding: 6px 12px; background: #6b7280; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: pointer;"
              >
                새로고침
              </button>
            </div>
            
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
              <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
                <div style="font-size: 32px; font-weight: 700; color: #3b82f6; margin-bottom: 8px;">
                  {analysisData().totalCrawled.toLocaleString()}
                </div>
                <div style="font-size: 14px; color: #6b7280; font-weight: 500;">총 크롤링 수</div>
              </div>
              
              <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
                <div style="font-size: 32px; font-weight: 700; color: #059669; margin-bottom: 8px;">
                  {analysisData().successRate.toFixed(1)}%
                </div>
                <div style="font-size: 14px; color: #6b7280; font-weight: 500;">데이터 완성률</div>
              </div>
              
              <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
                <div style="font-size: 32px; font-weight: 700; color: #ef4444; margin-bottom: 8px;">
                  {analysisData().errorRate.toFixed(1)}%
                </div>
                <div style="font-size: 14px; color: #6b7280; font-weight: 500;">데이터 누락률</div>
              </div>
              
              <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
                <div style="font-size: 32px; font-weight: 700; color: #f59e0b; margin-bottom: 8px;">
                  {analysisData().avgResponseTime.toFixed(1)}s
                </div>
                <div style="font-size: 14px; color: #6b7280; font-weight: 500;">평균 응답시간</div>
              </div>
            </div>
          </div>

          {/* 회사별 분포 */}
          <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
            <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">회사별 제품 분포</h3>
            
            {Object.keys(analysisData().categoryCounts).length === 0 ? (
              <div style="text-align: center; color: #6b7280; padding: 20px;">
                회사별 데이터가 없습니다.
              </div>
            ) : (
              <div style="space-y: 12px;">
                <For each={Object.entries(analysisData().categoryCounts)}>
                  {([company, count]) => {
                    const percentage = analysisData().totalCrawled > 0 
                      ? ((count as number) / analysisData().totalCrawled * 100).toFixed(1)
                      : '0';
                    return (
                      <div style="margin-bottom: 12px;">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                          <span style="font-weight: 500; color: #374151;">{company}</span>
                          <span style="font-weight: 500; color: #6b7280;">{(count as number).toLocaleString()} ({percentage}%)</span>
                        </div>
                        <div style="width: 100%; background: #e5e7eb; border-radius: 4px; height: 8px;">
                          <div style={`width: ${percentage}%; background: #3b82f6; border-radius: 4px; height: 8px;`}></div>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            )}
          </div>

          {/* 일별 통계 */}
          <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0fdf4;">
            <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">최근 7일 크롤링 통계</h3>
            
            {analysisData().dailyStats.length === 0 ? (
              <div style="text-align: center; color: #6b7280; padding: 20px;">
                일별 통계 데이터가 없습니다.
              </div>
            ) : (
              <div style="display: flex; align-items: end; gap: 8px; height: 250px; overflow-x: auto; justify-content: space-around;">
                <For each={analysisData().dailyStats}>
                  {(stat: any) => {
                    const maxCount = Math.max(...analysisData().dailyStats.map((s: any) => s.count));
                    const height = maxCount > 0 ? (stat.count / maxCount) * 200 : 0;
                    return (
                      <div style="display: flex; flex-direction: column; align-items: center; min-width: 60px;">
                        <div style="margin-bottom: 8px; font-size: 12px; font-weight: 500; color: #374151;">
                          {stat.count}
                        </div>
                        <div style={`width: 40px; background: linear-gradient(to top, #3b82f6, #60a5fa); border-radius: 4px 4px 0 0; height: ${height}px; min-height: 4px;`}></div>
                        <div style="margin-top: 8px; font-size: 12px; color: #6b7280; transform: rotate(-45deg); white-space: nowrap;">
                          {stat.date?.slice(5) || 'N/A'}
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            )}
          </div>
        </>
      )}

      {/* 성능 분석 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #fef3c7;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">성능 분석</h3>
        
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 16px;">
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">응답 시간 분포</h4>
            <div style="text-align: center; color: #6b7280; padding: 20px;">
              <div style="font-size: 14px;">성능 데이터는 크롤링 실행 후 제공됩니다</div>
            </div>
          </div>
          
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">오류 유형 분석</h4>
            <div style="text-align: center; color: #6b7280; padding: 20px;">
              <div style="font-size: 14px;">오류 통계는 크롤링 실행 후 제공됩니다</div>
            </div>
          </div>
        </div>
      </div>

      {/* 분석 도구 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f3e8ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">분석 도구</h3>
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={generateReport}
            style="padding: 12px 24px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#2563eb'}
            onMouseOut={(e) => e.currentTarget.style.background = '#3b82f6'}
          >
            📊 분석 보고서 생성
          </button>
          
          <button
            onClick={exportChart}
            style="padding: 12px 24px; background: #059669; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#047857'}
            onMouseOut={(e) => e.currentTarget.style.background = '#059669'}
          >
            📈 차트 내보내기
          </button>
          
          <button
            onClick={() => alert('데이터 필터링 옵션이 열렸습니다.')}
            style="padding: 12px 24px; background: #8b5cf6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#7c3aed'}
            onMouseOut={(e) => e.currentTarget.style.background = '#8b5cf6'}
          >
            🔍 데이터 필터링
          </button>
        </div>
      </div>

      {/* 최근 활동 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: white;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">최근 활동</h3>
        
        <div style="text-align: center; color: #6b7280; padding: 32px;">
          <div style="font-size: 48px; margin-bottom: 16px;">📝</div>
          <div style="font-size: 16px; font-weight: 500; margin-bottom: 8px;">활동 로그가 없습니다</div>
          <div style="font-size: 14px;">크롤링을 실행하면 활동 기록이 표시됩니다.</div>
        </div>
      </div>
    </div>
  );
};
