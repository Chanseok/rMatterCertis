/**
 * StatusTab - 크롤링 상태 및 제어 탭 (단순화된 레이아웃)
 * @description 사이트/DB 상태 확인, 크롤링 제어, 결과 표시라는 핵심 기능에 집중한 간소화된 버전입니다.
 */
import { Component, createSignal } from 'solid-js';
import { confirm } from '@tauri-apps/plugin-dialog';
import { tauriApi } from '../../services/tauri-api';
import { crawlerStore } from '../../stores/crawlerStore';
import type { CrawlingStatusCheck } from '../../types/crawling';
import { CrawlingStatus } from '../../types/crawling';

export const StatusTab: Component = () => {
  console.log('🚀 간소화된 StatusTab 컴포넌트가 로드되었습니다');

  // --- 상태 관리 ---
  // 진행/상태는 crawlerStore의 actor-event 기반 스트림을 사용합니다.
  const [statusCheckResult, setStatusCheckResult] = createSignal<CrawlingStatusCheck | null>(null);
  const [isCheckingStatus, setIsCheckingStatus] = createSignal(false);
  const [statusCheckError, setStatusCheckError] = createSignal<string>('');
  
  // 사이트 분석 결과는 글로벌 store에서 가져옵니다.
  const siteAnalysisResult = crawlerStore.siteAnalysisResult;


  // --- API 호출 함수 ---

  const runSiteAnalysis = async () => {
    setIsCheckingStatus(true);
    setStatusCheckError('');
    setStatusCheckResult(null);
    try {
      console.log('🔍 사이트 종합 분석 시작...');
      await crawlerStore.performSiteAnalysis();
      console.log('✅ 사이트 분석 완료:', crawlerStore.siteAnalysisResult());
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setStatusCheckError(`사이트 분석 실패: ${errorMessage}`);
      console.error('❌ 사이트 분석 실패:', error);
    } finally {
      setIsCheckingStatus(false);
    }
  };

  const runStatusCheck = async () => {
    setIsCheckingStatus(true);
    setStatusCheckError('');
    setStatusCheckResult(null);
    try {
      console.log('📊 크롤링 상태 체크 시작...');
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

  const startCrawling = async () => {
    const userConfirmed = await confirm('백엔드 지능형 계산을 사용하여 크롤링을 시작하시겠습니까?', {
      title: '크롤링 시작 확인',
      kind: 'info',
    });

    if (!userConfirmed) {
      console.log('❌ 사용자가 크롤링을 취소했습니다.');
      return;
    }

  try {
      console.log('🚀 백엔드 지능형 크롤링 시작...');
      await tauriApi.startCrawling(undefined, undefined);
    } catch (error) {
      console.error('❌ 크롤링 시작 실패:', error);
      alert(`크롤링 시작에 실패했습니다: ${error}`);
    }
  };

  const pauseCrawling = async () => {
    try {
      await tauriApi.pauseCrawling();
      console.log('⏸️ 크롤링 일시정지됨');
    } catch (error) {
      console.error('❌ 크롤링 일시정지 실패:', error);
    }
  };

  const stopCrawling = async () => {
    try {
      await tauriApi.stopCrawling();
      console.log('⏹️ 크롤링 중지됨');
    } catch (error) {
      console.error('❌ 크롤링 중지 실패:', error);
    }
  };

  // --- 헬퍼 및 렌더링 함수 ---
  // 상태는 crawlerStore의 반응형 값으로 대체 (actor-event 기반)
  const uiCrawlingStatus = () => {
    const st = crawlerStore.status();
    switch (st) {
      case CrawlingStatus.Running: return 'running' as const;
      case CrawlingStatus.Paused: return 'paused' as const;
      case CrawlingStatus.Completed: return 'completed' as const;
      default: return 'idle' as const;
    }
  };
  const progress = () => crawlerStore.progressPercentage();

  const getStatusInfo = () => {
    switch (uiCrawlingStatus()) {
      case 'running': return { text: '실행 중', color: '#22c55e' };
      case 'paused': return { text: '일시 정지', color: '#f59e0b' };
      case 'completed': return { text: '완료', color: '#3b82f6' };
      default: return { text: '대기 중', color: '#6b7280' };
    }
  };

  const renderResultItem = (label: string, value: any) => (
    <div class="flex justify-between py-2 border-b border-gray-200">
      <span class="text-sm text-gray-600">{label}</span>
      <span class="text-sm font-medium text-gray-800">{value}</span>
    </div>
  );

  const ResultsDisplay = () => {
    const checkResult = statusCheckResult();
    const analysisResult = siteAnalysisResult();

    if (isCheckingStatus()) {
      return <div class="text-center p-8">🔍 확인 중...</div>;
    }
    if (statusCheckError()) {
        return <div class="text-center p-8 text-red-500">{statusCheckError()}</div>;
    }
    if (!checkResult && !analysisResult) {
      return <div class="text-center p-8 text-gray-500">상태 체크 또는 사이트 분석을 실행해주세요.</div>;
    }

    return (
      <div class="space-y-6">
        {checkResult && (
          <div>
            <h4 class="font-bold text-md mb-2 text-blue-600">실시간 상태 체크 결과</h4>
            <div class="bg-gray-50 p-4 rounded-lg">
              {renderResultItem('DB 제품 수', `${checkResult.database_status.total_products.toLocaleString()} 개`)}
              {renderResultItem('DB 페이지 범위', `${checkResult.database_status.page_range[0]} - ${checkResult.database_status.page_range[1]}`)}
              {renderResultItem('사이트 접근성', checkResult.site_status.is_accessible ? '✅ 정상' : '❌ 불가')}
              {renderResultItem('사이트 최대 페이지', `${checkResult.site_status.total_pages} 페이지`)}
              {renderResultItem('추천 액션', `${checkResult.recommendation.action} (${checkResult.recommendation.priority})`)}
              {renderResultItem('추천 범위', checkResult.recommendation.suggested_range 
                ? `${checkResult.recommendation.suggested_range[0]} - ${checkResult.recommendation.suggested_range[1]}`
                : '정보 없음'
              )}
            </div>
          </div>
        )}
        {analysisResult && (
          <div>
            <h4 class="font-bold text-md mb-2 text-green-600">사이트 종합 분석 결과</h4>
            <div class="bg-gray-50 p-4 rounded-lg">
              {renderResultItem('사이트 접근성', analysisResult.site_status.is_accessible ? '✅ 정상' : '❌ 불가')}
              {renderResultItem('응답 시간', `${analysisResult.site_status.response_time_ms} ms`)}
              {renderResultItem('예상 최대 페이지', `${analysisResult.site_status.total_pages} 페이지`)}
              {renderResultItem('DB 제품 수', `${analysisResult.database_status.total_products.toLocaleString()} 개`)}
              {renderResultItem('동기화율', `${analysisResult.sync_comparison.sync_percentage.toFixed(1)} %`)}
              {renderResultItem('추천', analysisResult.recommendation.reason)}
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div class="p-6 bg-gray-50 min-h-screen font-sans">
      <h2 class="text-2xl font-bold text-gray-800 mb-6">📊 상태 및 제어</h2>
      
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* --- 왼쪽: 제어판 --- */}
        <div class="bg-white p-6 rounded-xl shadow-sm">
          <div class="mb-8">
            <h3 class="text-lg font-semibold text-gray-700 mb-4 border-b pb-2">상태 체크 & 분석</h3>
            <div class="flex flex-col space-y-3">
              <button
                onClick={runSiteAnalysis}
                disabled={isCheckingStatus()}
                class="w-full text-left p-4 rounded-lg bg-green-50 hover:bg-green-100 disabled:bg-gray-200 transition"
              >
                <div class="font-semibold text-green-800">🔍 사이트 종합 분석 (사전 조사)</div>
                <div class="text-sm text-green-700 mt-1">크롤링 전, 실제 사이트와 DB를 비교 분석합니다.</div>
              </button>
              <button
                onClick={runStatusCheck}
                disabled={isCheckingStatus()}
                class="w-full text-left p-4 rounded-lg bg-blue-50 hover:bg-blue-100 disabled:bg-gray-200 transition"
              >
                <div class="font-semibold text-blue-800">📊 크롤링 상태 조회 (실시간)</div>
                <div class="text-sm text-blue-700 mt-1">현재 진행중인 크롤링 상태를 메모리에서 조회합니다.</div>
              </button>
            </div>
          </div>

          <div>
            <h3 class="text-lg font-semibold text-gray-700 mb-4 border-b pb-2">크롤링 제어</h3>
            <div class="space-y-3">
              <button
                onClick={startCrawling}
                disabled={uiCrawlingStatus() === 'running'}
                class="w-full p-4 rounded-lg bg-indigo-600 text-white font-bold hover:bg-indigo-700 disabled:bg-gray-400 transition shadow-lg"
              >
                ▶️ 크롤링 시작
              </button>
              <div class="grid grid-cols-2 gap-3">
                <button
                  onClick={pauseCrawling}
                  disabled={uiCrawlingStatus() !== 'running'}
                  class="w-full p-3 rounded-lg bg-amber-500 text-white font-semibold hover:bg-amber-600 disabled:bg-gray-300 transition"
                >
                  ⏸️ 일시정지
                </button>
                <button
                  onClick={stopCrawling}
                  disabled={uiCrawlingStatus() === 'idle'}
                  class="w-full p-3 rounded-lg bg-red-600 text-white font-semibold hover:bg-red-700 disabled:bg-gray-300 transition"
                >
                  ⏹️ 중지
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* --- 오른쪽: 상태 및 결과 --- */}
        <div class="bg-white p-6 rounded-xl shadow-sm">
          <div class="mb-6">
            <h3 class="text-lg font-semibold text-gray-700 mb-4">현재 크롤링 상태</h3>
            <div class="bg-gray-100 p-4 rounded-lg">
              <div class="flex items-center justify-between mb-2">
                <span class="font-bold text-lg" style={{ color: getStatusInfo().color }}>
                  {getStatusInfo().text}
                </span>
                <span class="font-bold text-lg text-gray-700">{progress().toFixed(1)}%</span>
              </div>
              <div class="w-full bg-gray-300 rounded-full h-2.5">
                <div 
                  class="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
                  style={{ width: `${progress()}%`, 'background-color': getStatusInfo().color }}
                ></div>
              </div>
            </div>
          </div>

          <div>
            <h3 class="text-lg font-semibold text-gray-700 mb-4">분석 결과</h3>
            <div class="min-h-[200px]">
              <ResultsDisplay />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};