import { Component, Show } from "solid-js";

interface Props {
  isOpen: () => boolean;
  onClose: () => void;
}

const HelpPanel: Component<Props> = (p) => {
  return (
    <Show when={p.isOpen()}>
      {/* 오버레이 */}
      <div
        class="fixed inset-0 bg-black/50 backdrop-blur-sm z-40 transition-opacity"
        onClick={p.onClose}
      />
      
      {/* 슬라이드 패널 */}
      <div class="fixed right-0 top-0 h-full w-full max-w-2xl bg-white shadow-2xl z-50 overflow-y-auto transform transition-transform">
        <div class="p-8">
          {/* 헤더 */}
          <div class="flex items-center justify-between mb-6">
            <h2 class="text-2xl font-bold text-gray-800">🎓 사용 가이드</h2>
            <button
              onClick={p.onClose}
              class="px-4 py-2 text-sm rounded-lg bg-gray-100 hover:bg-gray-200 transition-colors"
            >
              ✕ 닫기
            </button>
          </div>

          {/* 콘텐츠 */}
          <div class="space-y-8">
            {/* 크롤링 */}
            <section>
              <h3 class="text-lg font-semibold text-purple-700 mb-3 flex items-center gap-2">
                <span>🎭</span>
                <span>크롤링</span>
              </h3>
              <div class="bg-purple-50 rounded-lg p-4 space-y-2">
                <p class="text-sm text-gray-700"><strong>용도:</strong> 전체 통합 파이프라인 실행</p>
                <p class="text-sm text-gray-600">
                  Stage 1~5를 순차적으로 실행하여 완전한 크롤링을 수행합니다.
                </p>
                <ul class="text-sm text-gray-600 list-disc list-inside ml-2 space-y-1">
                  <li>Stage 1: 리스트 페이지 크롤링</li>
                  <li>Stage 2: 상세 페이지 크롤링</li>
                  <li>Stage 3: 데이터 검증</li>
                  <li>Stage 4: DB 스냅샷</li>
                  <li>Stage 5: 데이터 저장</li>
                </ul>
              </div>
            </section>

            {/* 범위 다시 계산 */}
            <section>
              <h3 class="text-lg font-semibold text-blue-700 mb-3 flex items-center gap-2">
                <span>📊</span>
                <span>범위 다시 계산</span>
              </h3>
              <div class="bg-blue-50 rounded-lg p-4 space-y-2">
                <p class="text-sm text-gray-700"><strong>용도:</strong> 사이트 상태를 다시 분석하여 크롤링 범위를 재계산</p>
                <p class="text-sm text-gray-600">
                  사이트의 현재 총 페이지 수와 로컬 DB 상태를 분석하여 최적의 크롤링 범위를 제안합니다.
                </p>
                <div class="text-sm text-gray-600">
                  <strong>사용 시나리오:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>사이트에 새로운 제품이 추가되었을 때</li>
                    <li>크롤링 전 최신 범위 확인</li>
                    <li>DB 상태 변경 후 재계산 필요 시</li>
                  </ul>
                </div>
              </div>
            </section>

            {/* 빠른 동기화 */}
            <section>
              <h3 class="text-lg font-semibold text-cyan-700 mb-3 flex items-center gap-2">
                <span>🏃</span>
                <span>빠른 동기화</span>
              </h3>
              <div class="bg-cyan-50 rounded-lg p-4 space-y-2">
                <p class="text-sm text-gray-700"><strong>용도:</strong> 전체 페이지의 좌표만 빠르게 동기화</p>
                <p class="text-sm text-gray-600">
                  상세 정보 크롤링 없이 products 테이블의 page_id, index_in_page만 갱신합니다.
                </p>
                <p class="text-sm text-gray-700"><strong>소요 시간:</strong> 5-8분</p>
                <div class="text-sm text-gray-600">
                  <strong>사용 시나리오:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>좌표 정보만 필요할 때</li>
                    <li>빠른 데이터 갱신이 필요할 때</li>
                    <li>스마트 동기화 전 사전 동기화</li>
                  </ul>
                </div>
              </div>
            </section>

            {/* 스마트 동기화 */}
            <section>
              <h3 class="text-lg font-semibold text-pink-700 mb-3 flex items-center gap-2">
                <span>🧠</span>
                <span>스마트 동기화</span>
              </h3>
              <div class="bg-pink-50 rounded-lg p-4 space-y-2">
                <p class="text-sm text-gray-700"><strong>용도:</strong> 얕은 크롤링 + 진단 + 누락 보완 통합</p>
                <p class="text-sm text-gray-700"><strong>소요 시간:</strong> 8-12분</p>
                <div class="text-sm text-gray-600">
                  <strong>실행 단계:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>Phase 1: 빠른 동기화 (좌표 갱신)</li>
                    <li>Phase 2: 누락된 product_details 분석</li>
                    <li>Phase 3: 누락된 상세 정보만 선택적으로 크롤링</li>
                  </ul>
                </div>
                <div class="text-sm text-gray-600">
                  <strong>사용 시나리오:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>완전한 데이터 동기화가 필요할 때</li>
                    <li>누락된 제품 상세 정보 자동 보완</li>
                    <li>정기적인 데이터 유지보수</li>
                  </ul>
                </div>
              </div>
            </section>

            {/* 수동 크롤링 */}
            <section>
              <h3 class="text-lg font-semibold text-indigo-700 mb-3 flex items-center gap-2">
                <span>🎯</span>
                <span>수동 크롤링</span>
              </h3>
              <div class="bg-indigo-50 rounded-lg p-4 space-y-2">
                <p class="text-sm text-gray-700"><strong>용도:</strong> 특정 페이지 범위를 직접 지정하여 크롤링</p>
                <div class="text-sm text-gray-600">
                  <strong>입력 형식:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>단일 페이지: <code class="bg-white px-1 rounded">498</code></li>
                    <li>범위: <code class="bg-white px-1 rounded">498-492</code> (역순으로 498부터 492까지)</li>
                    <li>복합: <code class="bg-white px-1 rounded">498-492,489,487-485</code></li>
                  </ul>
                </div>
                <div class="text-sm text-gray-600">
                  <strong>사용 시나리오:</strong>
                  <ul class="list-disc list-inside ml-2 mt-1 space-y-1">
                    <li>특정 페이지만 재크롤링</li>
                    <li>진단에서 발견된 문제 페이지 수정</li>
                    <li>테스트 목적의 소규모 크롤링</li>
                  </ul>
                </div>
              </div>
            </section>

            {/* 권장 사용 순서 */}
            <section class="border-t pt-6">
              <h3 class="text-lg font-semibold text-gray-800 mb-4">💡 권장 사용 순서</h3>
              
              <div class="space-y-4">
                <div class="bg-green-50 rounded-lg p-4">
                  <h4 class="font-semibold text-green-800 mb-2">초기 크롤링</h4>
                  <ol class="list-decimal list-inside text-sm text-gray-700 space-y-1">
                    <li>📊 범위 다시 계산</li>
                    <li>🎭 크롤링</li>
                  </ol>
                </div>

                <div class="bg-blue-50 rounded-lg p-4">
                  <h4 class="font-semibold text-blue-800 mb-2">정기 유지보수</h4>
                  <ol class="list-decimal list-inside text-sm text-gray-700 space-y-1">
                    <li>🏃 빠른 동기화</li>
                    <li>🧠 스마트 동기화</li>
                  </ol>
                </div>

                <div class="bg-orange-50 rounded-lg p-4">
                  <h4 class="font-semibold text-orange-800 mb-2">문제 해결</h4>
                  <ol class="list-decimal list-inside text-sm text-gray-700 space-y-1">
                    <li>진단 실행 (Stage X)</li>
                    <li>🎯 수동 크롤링 (문제 페이지만)</li>
                    <li>products→details 동기화</li>
                  </ol>
                </div>
              </div>
            </section>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default HelpPanel;
