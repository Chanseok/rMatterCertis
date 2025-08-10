// @ts-nocheck
/**
 * Domain Status Dashboard - 새로운 타입 시스템을 활용한 현대적 대시보드
 * 
 * 자동 생성된 Rust 타입들을 사용하여 백엔드와 완벽히 동기화된 상태 표시
 */

import { createSignal, createEffect, Show, For } from 'solid-js';
import type {
  SiteStatus,
  DatabaseAnalysis,
  ProcessingStrategy,
  Product,
  CrawlingRangeRecommendation
} from '@/types';

interface DomainStatusDashboardProps {
  siteStatus: SiteStatus | null;
  databaseAnalysis: DatabaseAnalysis | null;
  processingStrategy: ProcessingStrategy | null;
  recentProducts: Product[];
}

export function DomainStatusDashboard(props: DomainStatusDashboardProps) {
  const [refreshTime, setRefreshTime] = createSignal(new Date());

  // 5초마다 새로고침 시간 업데이트
  createEffect(() => {
    const interval = setInterval(() => {
      setRefreshTime(new Date());
    }, 5000);
    
    return () => clearInterval(interval);
  });

  const formatHealthScore = (score: number) => {
    const percentage = Math.round(score * 100);
    const color = score >= 0.8 ? 'text-green-600' : 
                  score >= 0.6 ? 'text-yellow-600' : 'text-red-600';
    return { percentage, color };
  };

  const getCrawlingRecommendationBadge = (recommendation: CrawlingRangeRecommendation) => {
    switch (recommendation) {
      case 'Full':
        return { text: '전체 크롤링', color: 'bg-blue-100 text-blue-800' };
      case 'None':
        return { text: '크롤링 불필요', color: 'bg-gray-100 text-gray-800' };
      default:
        if (typeof recommendation === 'object' && 'Partial' in recommendation) {
          return { 
            text: `부분 크롤링 (${recommendation.Partial}페이지)`, 
            color: 'bg-yellow-100 text-yellow-800' 
          };
        }
        return { text: '알 수 없음', color: 'bg-gray-100 text-gray-800' };
    }
  };

  const getDataChangeStatusInfo = (status: any) => {
    if (typeof status === 'object') {
      if ('Increased' in status) {
        return {
          text: `증가 (+${status.Increased.new_count - status.Increased.previous_count})`,
          color: 'text-green-600',
          icon: '📈'
        };
      }
      if ('Decreased' in status) {
        return {
          text: `감소 (-${status.Decreased.decrease_amount})`,
          color: 'text-red-600',
          icon: '📉'
        };
      }
      if ('Stable' in status) {
        return {
          text: `안정 (${status.Stable.count})`,
          color: 'text-blue-600',
          icon: '📊'
        };
      }
      if ('Initial' in status) {
        return {
          text: `초기 (${status.Initial.count})`,
          color: 'text-purple-600',
          icon: '🆕'
        };
      }
    }
    if (status === 'Inaccessible') {
      return {
        text: '접근 불가',
        color: 'text-red-600',
        icon: '🚫'
      };
    }
    return { text: '알 수 없음', color: 'text-gray-600', icon: '❓' };
  };

  return (
    <div class="p-6 space-y-6 bg-gray-50 min-h-screen">
      {/* 헤더 */}
      <div class="flex justify-between items-center">
        <h1 class="text-2xl font-bold text-gray-900">도메인 상태 대시보드</h1>
        <div class="text-sm text-gray-500">
          마지막 업데이트: {refreshTime().toLocaleTimeString()}
        </div>
      </div>

      {/* 사이트 상태 카드 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h2 class="text-lg font-semibold text-gray-800 mb-4">🌐 사이트 상태</h2>
        <Show 
          when={props.siteStatus} 
          fallback={
            <div class="text-gray-500">사이트 상태 정보를 불러오는 중...</div>
          }
        >
          {(status) => (
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
              {/* 접근성 및 성능 */}
              <div class="space-y-3">
                <div class="flex items-center space-x-2">
                  <span class={`w-3 h-3 rounded-full ${status().is_accessible ? 'bg-green-500' : 'bg-red-500'}`}></span>
                  <span class="text-sm font-medium">
                    {status().is_accessible ? '접근 가능' : '접근 불가'}
                  </span>
                </div>
                <div class="text-sm text-gray-600">
                  응답 시간: {Number(status().response_time_ms)}ms
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">건강도: </span>
                  <span class={formatHealthScore(status().health_score).color}>
                    {formatHealthScore(status().health_score).percentage}%
                  </span>
                </div>
              </div>

              {/* 데이터 현황 */}
              <div class="space-y-3">
                <div class="text-sm">
                  <span class="text-gray-600">총 페이지: </span>
                  <span class="font-medium">{status().total_pages.toLocaleString()}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">예상 제품 수: </span>
                  <span class="font-medium">{status().estimated_products.toLocaleString()}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">마지막 페이지 제품: </span>
                  <span class="font-medium">{status().products_on_last_page}</span>
                </div>
              </div>

              {/* 상태 변화 및 권장사항 */}
              <div class="space-y-3">
                <div class="flex items-center space-x-2">
                  <span>{getDataChangeStatusInfo(status().data_change_status).icon}</span>
                  <span class={`text-sm font-medium ${getDataChangeStatusInfo(status().data_change_status).color}`}>
                    {getDataChangeStatusInfo(status().data_change_status).text}
                  </span>
                </div>
                <div>
                  <span class={`inline-block px-2 py-1 rounded-full text-xs font-medium ${getCrawlingRecommendationBadge(status().crawling_range_recommendation).color}`}>
                    {getCrawlingRecommendationBadge(status().crawling_range_recommendation).text}
                  </span>
                </div>
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* 데이터베이스 분석 카드 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h2 class="text-lg font-semibold text-gray-800 mb-4">🗄️ 데이터베이스 분석</h2>
        <Show 
          when={props.databaseAnalysis} 
          fallback={
            <div class="text-gray-500">데이터베이스 분석 정보를 불러오는 중...</div>
          }
        >
          {(analysis) => (
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* 기본 통계 */}
              <div class="space-y-3">
                <div class="text-sm">
                  <span class="text-gray-600">총 제품: </span>
                  <span class="font-medium text-lg">{analysis().total_products.toLocaleString()}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">고유 제품: </span>
                  <span class="font-medium">{analysis().unique_products.toLocaleString()}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">중복 항목: </span>
                  <span class="font-medium text-yellow-600">{analysis().duplicate_count.toLocaleString()}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">데이터 품질: </span>
                  <span class={formatHealthScore(analysis().data_quality_score).color}>
                    {formatHealthScore(analysis().data_quality_score).percentage}%
                  </span>
                </div>
              </div>

              {/* 필드 분석 */}
              <div class="space-y-3">
                <h3 class="text-sm font-medium text-gray-700">누락 필드 분석</h3>
                <div class="space-y-2 text-sm">
                  <div class="flex justify-between">
                    <span class="text-gray-600">회사명:</span>
                    <span class="text-red-600">{analysis().missing_fields_analysis.missing_company}</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-600">모델명:</span>
                    <span class="text-red-600">{analysis().missing_fields_analysis.missing_model}</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-600">Matter 버전:</span>
                    <span class="text-red-600">{analysis().missing_fields_analysis.missing_matter_version}</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-gray-600">연결성:</span>
                    <span class="text-red-600">{analysis().missing_fields_analysis.missing_connectivity}</span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* 스마트 크롤링 권장사항 카드 */}
      <div class="bg-gradient-to-r from-blue-50 to-indigo-50 rounded-lg shadow-md p-6 border border-blue-200">
        <h2 class="text-lg font-semibold text-blue-800 mb-4">🤖 스마트 크롤링 권장사항</h2>
        <Show 
          when={props.siteStatus && props.databaseAnalysis} 
          fallback={
            <div class="text-blue-600">분석 데이터를 수집하는 중...</div>
          }
        >
          <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* 권장 범위 */}
            <div class="bg-white rounded-lg p-4 border border-blue-100">
              <h3 class="text-sm font-semibold text-blue-800 mb-3">📄 권장 크롤링 범위</h3>
              <div class="space-y-2">
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">시작 페이지:</span>
                  <span class="text-sm font-medium text-blue-700">
                    {(() => {
                      const siteStatus = props.siteStatus!;
                      const recommendation = siteStatus.crawling_range_recommendation;
                      if (typeof recommendation === 'object' && 'Partial' in recommendation) {
                        return Math.max(1, siteStatus.total_pages - recommendation.Partial);
                      } else if (recommendation === 'Full') {
                        return 1;
                      } else {
                        return Math.max(1, siteStatus.total_pages - 10);
                      }
                    })()}
                  </span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">종료 페이지:</span>
                  <span class="text-sm font-medium text-blue-700">
                    {(() => {
                      const siteStatus = props.siteStatus!;
                      const recommendation = siteStatus.crawling_range_recommendation;
                      if (typeof recommendation === 'object' && 'Partial' in recommendation) {
                        return siteStatus.total_pages;
                      } else if (recommendation === 'Full') {
                        return Math.min(siteStatus.total_pages, 50);
                      } else {
                        return siteStatus.total_pages;
                      }
                    })()}
                  </span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">예상 제품 수:</span>
                  <span class="text-sm font-medium text-green-600">
                    {(() => {
                      const siteStatus = props.siteStatus!;
                      const recommendation = siteStatus.crawling_range_recommendation;
                      let pageCount = 10; // 기본값
                      if (typeof recommendation === 'object' && 'Partial' in recommendation) {
                        pageCount = recommendation.Partial;
                      } else if (recommendation === 'Full') {
                        pageCount = Math.min(siteStatus.total_pages, 50);
                      }
                      return pageCount * 12;
                    })()} 개
                  </span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">예상 소요 시간:</span>
                  <span class="text-sm font-medium text-purple-600">
                    {(() => {
                      const siteStatus = props.siteStatus!;
                      const recommendation = siteStatus.crawling_range_recommendation;
                      let pageCount = 10; // 기본값
                      if (typeof recommendation === 'object' && 'Partial' in recommendation) {
                        pageCount = recommendation.Partial;
                      } else if (recommendation === 'Full') {
                        pageCount = Math.min(siteStatus.total_pages, 50);
                      }
                      return Math.round(pageCount * 2.5);
                    })()} 분
                  </span>
                </div>
              </div>
            </div>

            {/* 최적화 전략 */}
            <div class="bg-white rounded-lg p-4 border border-blue-100">
              <h3 class="text-sm font-semibold text-blue-800 mb-3">⚙️ 자동 최적화 전략</h3>
              <div class="space-y-2">
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">배치 크기:</span>
                  <span class="text-sm font-medium">5개 (동적 조정)</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">동시 실행:</span>
                  <span class="text-sm font-medium">3개 (사이트 부하 고려)</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">요청 간격:</span>
                  <span class="text-sm font-medium">1000ms (안정성 우선)</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-sm text-gray-600">중복 처리:</span>
                  <span class="text-sm font-medium text-blue-600">스마트 감지</span>
                </div>
              </div>
            </div>
          </div>
        </Show>
        
        {/* 실행 가이드 */}
        <div class="mt-4 p-4 bg-blue-100 rounded-lg">
          <div class="flex items-start space-x-2">
            <span class="text-blue-600 text-sm">💡</span>
            <div class="text-sm text-blue-800">
              <strong>실행 방법:</strong> Advanced Engine 탭에서 위의 권장 페이지 범위를 입력하고 크롤링을 시작하세요. 
              시스템이 자동으로 최적의 설정을 적용하여 안전하고 효율적인 크롤링을 진행합니다.
            </div>
          </div>
        </div>
      </div>

      {/* 처리 전략 카드 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h2 class="text-lg font-semibold text-gray-800 mb-4">⚙️ 처리 전략</h2>
        <Show 
          when={props.processingStrategy} 
          fallback={
            <div class="text-gray-500">처리 전략 정보를 불러오는 중...</div>
          }
        >
          {(strategy) => (
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div class="space-y-3">
                <div class="text-sm">
                  <span class="text-gray-600">권장 배치 크기: </span>
                  <span class="font-medium">{strategy().recommended_batch_size}</span>
                </div>
                <div class="text-sm">
                  <span class="text-gray-600">권장 동시성: </span>
                  <span class="font-medium">{strategy().recommended_concurrency}</span>
                </div>
              </div>
              <div class="space-y-3">
                <div class="flex items-center space-x-2">
                  <span class={`w-3 h-3 rounded-full ${strategy().should_skip_duplicates ? 'bg-green-500' : 'bg-gray-300'}`}></span>
                  <span class="text-sm">중복 건너뛰기</span>
                </div>
                <div class="flex items-center space-x-2">
                  <span class={`w-3 h-3 rounded-full ${strategy().should_update_existing ? 'bg-green-500' : 'bg-gray-300'}`}></span>
                  <span class="text-sm">기존 항목 업데이트</span>
                </div>
              </div>
            </div>
          )}
        </Show>
      </div>

      {/* 최근 제품 카드 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h2 class="text-lg font-semibold text-gray-800 mb-4">📦 최근 수집된 제품</h2>
        <Show 
          when={props.recentProducts.length > 0} 
          fallback={
            <div class="text-gray-500">아직 수집된 제품이 없습니다.</div>
          }
        >
          <div class="space-y-3 max-h-64 overflow-y-auto">
            <For each={props.recentProducts.slice(0, 10)}>
              {(product) => (
                <div class="border border-gray-200 rounded-lg p-3 hover:bg-gray-50">
                  <div class="flex justify-between items-start">
                    <div class="flex-1 min-w-0">
                      <div class="text-sm font-medium text-gray-900 truncate">
                        {product.manufacturer || '제조사 정보 없음'} - {product.model || '모델 정보 없음'}
                      </div>
                      <div class="text-xs text-gray-500 mt-1">
                        인증 ID: {product.certificateId || 'N/A'}
                      </div>
                    </div>
                    <div class="text-xs text-gray-400 ml-2">
                      페이지 {product.pageId || 'N/A'}
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
}
