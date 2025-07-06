/**
 * LocalDBTab - 로컬 데이터베이스 관리 탭 컴포넌트
 */

import { Component, createSignal, For } from 'solid-js';

export const LocalDBTab: Component = () => {
  // 데이터베이스 상태
  const [dbStats] = createSignal({
    totalRecords: 1248,
    lastUpdate: '2025-07-05 14:30:00',
    databaseSize: '45.2MB',
    indexSize: '8.7MB'
  });

  // 최근 데이터 (샘플)
  const [recentData] = createSignal([
    { id: 1, title: 'Samsung Galaxy S24 Ultra 케이스', category: 'Electronics', date: '2025-07-05', status: 'Valid' },
    { id: 2, title: 'Apple iPhone 15 Pro 충전기', category: 'Electronics', date: '2025-07-05', status: 'Valid' },
    { id: 3, title: 'LG 올레드 TV 65인치', category: 'Electronics', date: '2025-07-04', status: 'Valid' },
    { id: 4, title: 'Sony WH-1000XM5 헤드폰', category: 'Electronics', date: '2025-07-04', status: 'Valid' },
    { id: 5, title: 'Dell XPS 13 노트북', category: 'Computers', date: '2025-07-03', status: 'Valid' }
  ]);

  // 검색 및 필터링
  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedCategory, setSelectedCategory] = createSignal('All');

  const categories = ['All', 'Electronics', 'Computers', 'Home', 'Sports'];

  const filteredData = () => {
    return recentData().filter(item => {
      const matchesSearch = item.title.toLowerCase().includes(searchTerm().toLowerCase());
      const matchesCategory = selectedCategory() === 'All' || item.category === selectedCategory();
      return matchesSearch && matchesCategory;
    });
  };

  const exportData = () => {
    alert('데이터 내보내기 기능이 실행됩니다.');
  };

  const clearDatabase = () => {
    if (confirm('정말로 데이터베이스를 초기화하시겠습니까?')) {
      alert('데이터베이스가 초기화되었습니다.');
    }
  };

  const optimizeDatabase = () => {
    alert('데이터베이스 최적화가 완료되었습니다.');
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
              <For each={categories}>
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
        <div style="padding: 16px; background: #f9fafb; border-bottom: 1px solid #e5e7eb;">
          <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">최근 데이터</h3>
        </div>
        
        <div style="overflow-x: auto;">
          <table style="width: 100%; border-collapse: collapse;">
            <thead>
              <tr style="background: #f9fafb;">
                <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">ID</th>
                <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">제품명</th>
                <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">카테고리</th>
                <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">날짜</th>
                <th style="padding: 12px; text-align: left; font-weight: 500; color: #374151; border-bottom: 1px solid #e5e7eb;">상태</th>
              </tr>
            </thead>
            <tbody>
              <For each={filteredData()}>
                {(item) => (
                  <tr style="border-bottom: 1px solid #f3f4f6;">
                    <td style="padding: 12px; color: #6b7280;">{item.id}</td>
                    <td style="padding: 12px; color: #1f2937; font-weight: 500;">{item.title}</td>
                    <td style="padding: 12px; color: #6b7280;">
                      <span style="background: #dbeafe; color: #1e40af; padding: 4px 8px; border-radius: 4px; font-size: 12px;">
                        {item.category}
                      </span>
                    </td>
                    <td style="padding: 12px; color: #6b7280;">{item.date}</td>
                    <td style="padding: 12px;">
                      <span style="background: #dcfce7; color: #166534; padding: 4px 8px; border-radius: 4px; font-size: 12px;">
                        {item.status}
                      </span>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
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
          <div style="font-size: 14px; color: #6b7280; margin-bottom: 8px;">마지막 백업: 2025-07-04 23:30:00</div>
          <div style="font-size: 14px; color: #6b7280;">백업 파일 크기: 42.8MB</div>
        </div>
        
        <div style="display: flex; gap: 12px; flex-wrap: wrap;">
          <button
            onClick={() => alert('백업이 생성되었습니다.')}
            style="padding: 12px 24px; background: #8b5cf6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#7c3aed'}
            onMouseOut={(e) => e.currentTarget.style.background = '#8b5cf6'}
          >
            💾 백업 생성
          </button>
          
          <button
            onClick={() => alert('백업에서 복원되었습니다.')}
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
