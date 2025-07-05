/**
 * AnalysisTab - 데이터 분석 탭 컴포넌트
 */

import { Component, createSignal } from 'solid-js';

export const AnalysisTab: Component = () => {
  // 분석 데이터 (샘플)
  const [analysisData, setAnalysisData] = createSignal({
    totalCrawled: 1248,
    successRate: 94.2,
    errorRate: 5.8,
    avgResponseTime: 1.2,
    categoryCounts: {
      'Electronics': 456,
      'Computers': 325,
      'Home': 234,
      'Sports': 156,
      'Books': 77
    },
    dailyStats: [
      { date: '2025-07-01', count: 89 },
      { date: '2025-07-02', count: 124 },
      { date: '2025-07-03', count: 156 },
      { date: '2025-07-04', count: 203 },
      { date: '2025-07-05', count: 178 }
    ]
  });

  const generateReport = () => {
    alert('분석 보고서가 생성되었습니다.');
  };

  const exportChart = () => {
    alert('차트가 이미지로 내보내졌습니다.');
  };

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">📈 분석</h2>
      
      {/* 주요 지표 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">주요 지표</h3>
        
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
          <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 32px; font-weight: 700; color: #3b82f6; margin-bottom: 8px;">
              {analysisData().totalCrawled.toLocaleString()}
            </div>
            <div style="font-size: 14px; color: #6b7280; font-weight: 500;">총 크롤링 수</div>
          </div>
          
          <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 32px; font-weight: 700; color: #059669; margin-bottom: 8px;">
              {analysisData().successRate}%
            </div>
            <div style="font-size: 14px; color: #6b7280; font-weight: 500;">성공률</div>
          </div>
          
          <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 32px; font-weight: 700; color: #ef4444; margin-bottom: 8px;">
              {analysisData().errorRate}%
            </div>
            <div style="font-size: 14px; color: #6b7280; font-weight: 500;">오류율</div>
          </div>
          
          <div style="padding: 20px; background: white; border-radius: 8px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 32px; font-weight: 700; color: #f59e0b; margin-bottom: 8px;">
              {analysisData().avgResponseTime}s
            </div>
            <div style="font-size: 14px; color: #6b7280; font-weight: 500;">평균 응답시간</div>
          </div>
        </div>
      </div>

      {/* 카테고리별 분포 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">카테고리별 분포</h3>
        
        <div style="space-y: 12px;">
          {Object.entries(analysisData().categoryCounts).map(([category, count]) => {
            const percentage = (count / analysisData().totalCrawled * 100).toFixed(1);
            return (
              <div style="margin-bottom: 12px;">
                <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                  <span style="font-weight: 500; color: #374151;">{category}</span>
                  <span style="font-weight: 500; color: #6b7280;">{count} ({percentage}%)</span>
                </div>
                <div style="width: 100%; height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden;">
                  <div 
                    style={`height: 100%; background: linear-gradient(90deg, #3b82f6, #1d4ed8); width: ${percentage}%; transition: width 0.3s ease;`}
                  ></div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* 일별 크롤링 통계 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0fdf4;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">일별 크롤링 통계</h3>
        
        <div style="height: 300px; background: white; border-radius: 6px; border: 1px solid #e5e7eb; padding: 20px; position: relative;">
          {/* 간단한 막대 차트 */}
          <div style="display: flex; align-items: end; height: 240px; gap: 16px; justify-content: space-around;">
            {analysisData().dailyStats.map((stat) => {
              const height = (stat.count / 250) * 200; // 최대 높이를 200px로 정규화
              return (
                <div style="display: flex; flex-direction: column; align-items: center;">
                  <div style="margin-bottom: 8px; font-size: 12px; font-weight: 500; color: #374151;">
                    {stat.count}
                  </div>
                  <div 
                    style={`width: 40px; background: linear-gradient(to top, #3b82f6, #60a5fa); border-radius: 4px 4px 0 0; height: ${height}px; transition: height 0.3s ease;`}
                  ></div>
                  <div style="margin-top: 8px; font-size: 12px; color: #6b7280; transform: rotate(-45deg); white-space: nowrap;">
                    {stat.date.slice(5)}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* 성능 분석 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #fef3c7;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">성능 분석</h3>
        
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 16px;">
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">응답 시간 분포</h4>
            <div style="space-y: 8px;">
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">{'< 1초'}</span>
                <span style="font-size: 14px; font-weight: 500; color: #059669;">72%</span>
              </div>
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">1-3초</span>
                <span style="font-size: 14px; font-weight: 500; color: #f59e0b;">22%</span>
              </div>
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">{'> 3초'}</span>
                <span style="font-size: 14px; font-weight: 500; color: #ef4444;">6%</span>
              </div>
            </div>
          </div>
          
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb;">
            <h4 style="margin: 0 0 12px 0; font-size: 16px; font-weight: 500; color: #1f2937;">오류 유형 분석</h4>
            <div style="space-y: 8px;">
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">타임아웃</span>
                <span style="font-size: 14px; font-weight: 500; color: #ef4444;">18건</span>
              </div>
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">404 오류</span>
                <span style="font-size: 14px; font-weight: 500; color: #f59e0b;">12건</span>
              </div>
              <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                <span style="font-size: 14px; color: #6b7280;">파싱 오류</span>
                <span style="font-size: 14px; font-weight: 500; color: #8b5cf6;">8건</span>
              </div>
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
        
        <div style="space-y: 12px;">
          <div style="padding: 12px; background: #f9fafb; border-radius: 6px; border-left: 4px solid #3b82f6; margin-bottom: 12px;">
            <div style="font-size: 14px; font-weight: 500; color: #1f2937;">크롤링 작업 완료</div>
            <div style="font-size: 12px; color: #6b7280;">2025-07-05 14:30 - 178개 항목 처리</div>
          </div>
          
          <div style="padding: 12px; background: #f9fafb; border-radius: 6px; border-left: 4px solid #059669; margin-bottom: 12px;">
            <div style="font-size: 14px; font-weight: 500; color: #1f2937;">데이터베이스 백업 생성</div>
            <div style="font-size: 12px; color: #6b7280;">2025-07-04 23:30 - 백업 크기: 42.8MB</div>
          </div>
          
          <div style="padding: 12px; background: #f9fafb; border-radius: 6px; border-left: 4px solid #f59e0b; margin-bottom: 12px;">
            <div style="font-size: 14px; font-weight: 500; color: #1f2937;">성능 최적화 실행</div>
            <div style="font-size: 12px; color: #6b7280;">2025-07-04 15:20 - 응답시간 15% 개선</div>
          </div>
        </div>
      </div>
    </div>
  );
};
