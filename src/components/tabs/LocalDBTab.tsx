/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트 (실제 데이터 사용)
 */

import { Component, createSignal, For, onMount } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';

export const LocalDBTab: Component = () => {
  // 데이터베이스 상태 (실제 데이터)
  const [dbStats, setDbStats] = createSignal({
    totalRecords: 0,
    lastUpdate: 'Loading...',
    databaseSize: '0MB',
    indexSize: '0MB'
  });

  // 실제 제품 데이터
  const [recentData, setRecentData] = createSignal<any[]>([]);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string>('');

  // 페이지네이션
  const [currentPage, setCurrentPage] = createSignal(1);
  const [totalPages, setTotalPages] = createSignal(1);
  const [totalProducts, setTotalProducts] = createSignal(0);

  // 검색 및 필터링
  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedCategory, setSelectedCategory] = createSignal('All');

  // 실제 데이터에서 카테고리 추출
  const categories = () => {
    const uniqueCompanies = new Set<string>();
    recentData().forEach(item => {
      if (item.company && item.company !== 'Unknown') {
        uniqueCompanies.add(item.company);
      }
    });
    return ['All', ...Array.from(uniqueCompanies).sort()];
  };

  // 실제 데이터베이스에서 통계 로드
  const loadDbStats = async () => {
    try {
      const stats = await tauriApi.getLocalDbStats();
      setDbStats(stats);
    } catch (err) {
      console.error('Failed to load DB stats:', err);
      setError(`DB 통계 로드 실패: ${err}`);
    }
  };

  // 실제 데이터베이스에서 제품 데이터 로드
  const loadProducts = async (page: number = 1) => {
    try {
      setIsLoading(true);
      const result = await tauriApi.getProducts(page, 20);
      setRecentData(result.products || []);
      setTotalPages(result.total_pages || 1);
      setTotalProducts(result.total || 0);
      setCurrentPage(page);
    } catch (err) {
      console.error('Failed to load products:', err);
      setError(`제품 데이터 로드 실패: ${err}`);
      setRecentData([]);
    } finally {
      setIsLoading(false);
    }
  };

  // 컴포넌트 마운트 시 데이터 로드
  onMount(() => {
    loadDbStats();
    loadProducts(1);
  });

  const filteredData = () => {
    return recentData().filter(item => {
      const matchesSearch = item.title?.toLowerCase().includes(searchTerm().toLowerCase()) || false;
      const matchesCategory = selectedCategory() === 'All' || item.company === selectedCategory();
      return matchesSearch && matchesCategory;
    });
  };

  const exportData = async () => {
    try {
      const exportPath = await tauriApi.exportDatabaseData('csv');
      alert(`데이터가 내보내졌습니다: ${exportPath}`);
    } catch (err) {
      alert(`데이터 내보내기 실패: ${err}`);
    }
  };

  const clearDatabase = () => {
    if (confirm('정말로 데이터베이스를 초기화하시겠습니까? 이 작업은 되돌릴 수 없습니다.')) {
      alert('데이터베이스 초기화 기능은 개발 중입니다.');
    }
  };

  const optimizeDatabase = async () => {
    try {
      await tauriApi.optimizeDatabase();
      alert('데이터베이스 최적화가 완료되었습니다.');
      loadDbStats(); // 통계 새로고침
    } catch (err) {
      alert(`데이터베이스 최적화 실패: ${err}`);
    }
  };

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">🗄️ 로컬DB</h2>
      
      {/* 데이터베이스 통계 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f8fafc;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">데이터베이스 통계</h3>
        
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 24px; font-weight: 600; color: #3b82f6; margin-bottom: 4px;">{dbStats().totalRecords.toLocaleString()}</div>
            <div style="font-size: 14px; color: #6b7280;">총 레코드 수</div>
          </div>
          
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 18px; font-weight: 600; color: #059669; margin-bottom: 4px;">{dbStats().databaseSize}</div>
            <div style="font-size: 14px; color: #6b7280;">데이터베이스 크기</div>
          </div>
          
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 18px; font-weight: 600; color: #f59e0b; margin-bottom: 4px;">{dbStats().indexSize}</div>
            <div style="font-size: 14px; color: #6b7280;">인덱스 크기</div>
          </div>
          
          <div style="padding: 16px; background: white; border-radius: 6px; border: 1px solid #e5e7eb; text-align: center;">
            <div style="font-size: 14px; font-weight: 600; color: #8b5cf6; margin-bottom: 4px;">{dbStats().lastUpdate}</div>
            <div style="font-size: 14px; color: #6b7280;">마지막 업데이트</div>
          </div>
        </div>
      </div>

      {/* 검색 및 필터링 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">검색 및 필터링</h3>
        
        <div style="display: flex; gap: 16px; margin-bottom: 16px; flex-wrap: wrap;">
          <div style="flex: 1; min-width: 200px;">
            <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #374151;">검색어</label>
            <input
              type="text"
              placeholder="제품명으로 검색..."
              value={searchTerm()}
              onInput={(e) => setSearchTerm(e.currentTarget.value)}
              style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px;"
            />
          </div>
          
          <div style="min-width: 150px;">
            <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #374151;">카테고리</label>
            <select
              value={selectedCategory()}
              onChange={(e) => setSelectedCategory(e.currentTarget.value)}
              style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; background: white; font-size: 14px;"
            >
              <For each={categories()}>
                {(category) => <option value={category}>{category}</option>}
              </For>
            </select>
          </div>
        </div>
        
        <div style="font-size: 14px; color: #6b7280;">
          검색 결과: {filteredData().length}개 항목
        </div>
      </div>

      {/* 데이터 테이블 */}
      <div style="margin-bottom: 32px; border: 1px solid #e5e7eb; border-radius: 8px; overflow: hidden; background: white;">
        <div style="padding: 16px; background: #f9fafb; border-bottom: 1px solid #e5e7eb; display: flex; justify-content: space-between; align-items: center;">
          <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">
            실제 데이터 ({totalProducts()}개 제품)
          </h3>
          <button
            onClick={() => loadProducts(currentPage())}
            style="padding: 6px 12px; background: #3b82f6; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: pointer;"
          >
            새로고침
          </button>
        </div>

        {/* 에러 표시 */}
        {error() && (
          <div style="padding: 16px; background: #fef2f2; border-bottom: 1px solid #fecaca; color: #dc2626;">
            {error()}
          </div>
        )}

        {/* 로딩 상태 */}
        {isLoading() && (
          <div style="padding: 32px; text-align: center; color: #6b7280;">
            <div style="margin-bottom: 8px;">데이터를 로드하는 중...</div>
            <div style="width: 24px; height: 24px; border: 2px solid #e5e7eb; border-top: 2px solid #3b82f6; border-radius: 50%; animation: spin 1s linear infinite; margin: 0 auto;"></div>
          </div>
        )}

        {/* 데이터가 없는 경우 */}
        {!isLoading() && !error() && filteredData().length === 0 && (
          <div style="padding: 32px; text-align: center; color: #6b7280;">
            <div style="font-size: 48px; margin-bottom: 16px;">📭</div>
            <div style="font-size: 18px; font-weight: 500; margin-bottom: 8px;">데이터가 없습니다</div>
            <div style="font-size: 14px;">크롤링을 실행하여 데이터를 수집해보세요.</div>
          </div>
        )}

        {/* 실제 데이터 테이블 */}
        {!isLoading() && !error() && filteredData().length > 0 && (
          <div style="overflow-x: auto;">
            <table style="width: 100%; border-collapse: collapse;">
              <thead>
                <tr style="background: #f9fafb;">
                  <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">ID</th>
                  <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">제품명</th>
                  <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">회사</th>
                  <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">인증일</th>
                  <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">상태</th>
                </tr>
              </thead>
              <tbody>
                <For each={filteredData()}>
                  {(item) => (
                    <tr style="border-bottom: 1px solid #f3f4f6;">
                      <td style="padding: 12px; color: #6b7280; font-family: monospace;">{item.id}</td>
                      <td style="padding: 12px; color: #1f2937; font-weight: 500;">
                        {item.title || 'Unknown Product'}
                      </td>
                      <td style="padding: 12px; color: #6b7280;">
                        <span style="background: #dbeafe; color: #1e40af; padding: 4px 8px; border-radius: 4px; font-size: 12px;">
                          {item.company || 'Unknown'}
                        </span>
                      </td>
                      <td style="padding: 12px; color: #6b7280; font-family: monospace; font-size: 12px;">
                        {item.certification_date || 'N/A'}
                      </td>
                      <td style="padding: 12px;">
                        <span style={`background: ${item.status === 'Valid' ? '#dcfce7' : '#fef3c7'}; color: ${item.status === 'Valid' ? '#166534' : '#92400e'}; padding: 4px 8px; border-radius: 4px; font-size: 12px;`}>
                          {item.status}
                        </span>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        )}

        {/* 페이지네이션 */}
        {!isLoading() && !error() && totalPages() > 1 && (
          <div style="padding: 16px; background: #f9fafb; border-top: 1px solid #e5e7eb; display: flex; justify-content: between; align-items: center;">
            <div style="font-size: 14px; color: #6b7280;">
              페이지 {currentPage()} / {totalPages()} (총 {totalProducts()}개)
            </div>
            <div style="display: flex; gap: 8px;">
              <button
                onClick={() => loadProducts(Math.max(1, currentPage() - 1))}
                disabled={currentPage() <= 1}
                style={`padding: 6px 12px; border: 1px solid #d1d5db; border-radius: 4px; background: ${currentPage() <= 1 ? '#f9fafb' : 'white'}; color: ${currentPage() <= 1 ? '#9ca3af' : '#374151'}; cursor: ${currentPage() <= 1 ? 'not-allowed' : 'pointer'}; font-size: 12px;`}
              >
                이전
              </button>
              <button
                onClick={() => loadProducts(Math.min(totalPages(), currentPage() + 1))}
                disabled={currentPage() >= totalPages()}
                style={`padding: 6px 12px; border: 1px solid #d1d5db; border-radius: 4px; background: ${currentPage() >= totalPages() ? '#f9fafb' : 'white'}; color: ${currentPage() >= totalPages() ? '#9ca3af' : '#374151'}; cursor: ${currentPage() >= totalPages() ? 'not-allowed' : 'pointer'}; font-size: 12px;`}
              >
                다음
              </button>
            </div>
          </div>
        )}
      </div>

      {/* 데이터베이스 관리 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #fef3c7;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">데이터베이스 관리</h3>
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={exportData}
            style="padding: 12px 24px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#2563eb'}
            onMouseOut={(e) => e.currentTarget.style.background = '#3b82f6'}
          >
            📤 데이터 내보내기
          </button>
          
          <button
            onClick={optimizeDatabase}
            style="padding: 12px 24px; background: #059669; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#047857'}
            onMouseOut={(e) => e.currentTarget.style.background = '#059669'}
          >
            ⚡ 데이터베이스 최적화
          </button>
          
          <button
            onClick={clearDatabase}
            style="padding: 12px 24px; background: #ef4444; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#dc2626'}
            onMouseOut={(e) => e.currentTarget.style.background = '#ef4444'}
          >
            🗑️ 데이터베이스 초기화
          </button>
        </div>
      </div>

      {/* 백업 및 복원 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0fdf4;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">백업 및 복원</h3>
        
        <div style="margin-bottom: 16px;">
          <div style="font-size: 14px; color: #6b7280; margin-bottom: 8px;">백업 상태: 백업 기록을 확인하세요</div>
          <div style="font-size: 14px; color: #6b7280;">백업 기능은 데이터베이스 관리 도구를 통해 제공됩니다</div>
        </div>
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={async () => {
              try {
                const backupPath = await tauriApi.backupDatabase();
                alert(`백업이 생성되었습니다: ${backupPath}`);
              } catch (err) {
                alert(`백업 생성 실패: ${err}`);
              }
            }}
            style="padding: 12px 24px; background: #8b5cf6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#7c3aed'}
            onMouseOut={(e) => e.currentTarget.style.background = '#8b5cf6'}
          >
            💾 백업 생성
          </button>
          
          <button
            onClick={() => alert('백업 복원 기능은 개발 중입니다.')}
            style="padding: 12px 24px; background: #f59e0b; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#d97706'}
            onMouseOut={(e) => e.currentTarget.style.background = '#f59e0b'}
          >
            📁 백업에서 복원
          </button>
        </div>
      </div>
    </div>
  );
};
