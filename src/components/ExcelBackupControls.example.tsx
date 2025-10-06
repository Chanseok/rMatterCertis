// UI Component Example: Excel Backup/Restore Controls
// 로컬 DB 대시보드에 추가할 수 있는 예시 컴포넌트

import { createSignal } from 'solid-js';
import { localDbDashboardStore } from '@/stores/localDbDashboardStore';

export function ExcelBackupControls() {
  const handleExport = async () => {
    await localDbDashboardStore.exportFullDatabaseExcel();
  };
  
  const handleImport = async () => {
    // Tauri의 경우 파일 경로가 필요하므로 파일 선택 다이얼로그 사용
    // @ts-ignore - Tauri API
    const filePath = await window.__TAURI__.dialog.open({
      filters: [{ name: 'Excel Files', extensions: ['xlsx'] }]
    });
    
    if (filePath && typeof filePath === 'string') {
      await localDbDashboardStore.importFullDatabaseExcel(filePath);
    }
  };
  
  const handleDeleteAll = async () => {
    await localDbDashboardStore.deleteAllRecordsConfirmed();
  };
  
  return (
    <div class="space-y-4 p-4 border rounded-lg">
      <h3 class="text-lg font-semibold">Excel 백업/복원</h3>
      
      {/* Export Section */}
      <div class="space-y-2">
        <button
          onClick={handleExport}
          disabled={localDbDashboardStore.ui.working}
          class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400"
        >
          📊 전체 DB Excel 내보내기
        </button>
        
        {localDbDashboardStore.ui.exportStatus && (
          <div class="text-sm text-gray-600 whitespace-pre-wrap">
            {localDbDashboardStore.ui.exportStatus}
          </div>
        )}
      </div>
      
      {/* Import Section */}
      <div class="space-y-2">
        <button
          onClick={handleImport}
          disabled={localDbDashboardStore.ui.working}
          class="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:bg-gray-400"
        >
          📥 Excel에서 복원
        </button>
        
        {localDbDashboardStore.ui.importStatus && (
          <div class="space-y-1">
            <div class="text-sm font-medium">
              상태: {localDbDashboardStore.ui.importStatus}
            </div>
            {localDbDashboardStore.ui.importLog && (
              <pre class="text-xs bg-gray-100 p-2 rounded overflow-auto max-h-40">
                {localDbDashboardStore.ui.importLog}
              </pre>
            )}
          </div>
        )}
      </div>
      
      {/* Delete All Section */}
      <div class="border-t pt-4 space-y-2">
        <button
          onClick={handleDeleteAll}
          disabled={localDbDashboardStore.ui.working}
          class="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:bg-gray-400"
        >
          🗑️ 전체 레코드 삭제
        </button>
        
        {localDbDashboardStore.ui.deleteResult && (
          <div class="text-sm">
            {localDbDashboardStore.ui.deleteResult.cancelled ? (
              <span class="text-yellow-600">취소됨</span>
            ) : localDbDashboardStore.ui.deleteResult.error ? (
              <span class="text-red-600">
                오류: {localDbDashboardStore.ui.deleteResult.error}
              </span>
            ) : localDbDashboardStore.ui.deleteResult.message ? (
              <pre class="text-green-600 whitespace-pre-wrap">
                {localDbDashboardStore.ui.deleteResult.message}
              </pre>
            ) : null}
          </div>
        )}
      </div>
    </div>
  );
}

// 페이지 범위 삭제 컴포넌트
export function PageRangeDeleteControls() {
  const [fromPage, setFromPage] = createSignal(1);
  const [toPage, setToPage] = createSignal(10);
  
  const handlePreview = async () => {
    await localDbDashboardStore.previewDeleteRange(fromPage(), toPage());
  };
  
  const handleDelete = async () => {
    if (!localDbDashboardStore.ui.deletePreview) {
      alert('먼저 미리보기를 실행하세요');
      return;
    }
    
    if (confirm(`페이지 ${fromPage()}-${toPage()} 범위를 삭제하시겠습니까?`)) {
      await localDbDashboardStore.executeDeleteRange(fromPage(), toPage());
    }
  };
  
  return (
    <div class="space-y-4 p-4 border rounded-lg">
      <h3 class="text-lg font-semibold">페이지 범위 삭제</h3>
      
      <div class="flex gap-4 items-center">
        <label class="flex items-center gap-2">
          <span>시작 페이지:</span>
          <input
            type="number"
            value={fromPage()}
            onInput={(e) => setFromPage(parseInt(e.currentTarget.value) || 1)}
            min="1"
            class="border rounded px-2 py-1 w-24"
          />
        </label>
        
        <label class="flex items-center gap-2">
          <span>종료 페이지:</span>
          <input
            type="number"
            value={toPage()}
            onInput={(e) => setToPage(parseInt(e.currentTarget.value) || 1)}
            min="1"
            class="border rounded px-2 py-1 w-24"
          />
        </label>
      </div>
      
      <div class="flex gap-2">
        <button
          onClick={handlePreview}
          class="px-4 py-2 bg-yellow-600 text-white rounded hover:bg-yellow-700"
        >
          🔍 미리보기
        </button>
        
        <button
          onClick={handleDelete}
          disabled={!localDbDashboardStore.ui.deletePreview}
          class="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:bg-gray-400"
        >
          🗑️ 삭제 실행
        </button>
      </div>
      
      {localDbDashboardStore.ui.deletePreview && (
        <div class="bg-gray-100 p-3 rounded">
          <div class="font-medium mb-2">삭제 미리보기:</div>
          <ul class="text-sm space-y-1">
            <li>제품: {localDbDashboardStore.ui.deletePreview.products_count}개</li>
            <li>상세정보: {localDbDashboardStore.ui.deletePreview.product_details_count}개</li>
            <li>
              Primary Types: {localDbDashboardStore.ui.deletePreview.product_details_with_primary_types}개
            </li>
          </ul>
        </div>
      )}
      
      {localDbDashboardStore.ui.deleteResult && !localDbDashboardStore.ui.deleteResult.error && (
        <div class="bg-green-100 p-3 rounded">
          <div class="font-medium mb-2">삭제 완료:</div>
          <ul class="text-sm space-y-1">
            <li>제품: {localDbDashboardStore.ui.deleteResult.deleted_products}개</li>
            <li>상세정보: {localDbDashboardStore.ui.deleteResult.deleted_product_details}개</li>
          </ul>
        </div>
      )}
    </div>
  );
}

// 사용 예시:
// LocalDbDashboard.tsx에서:
/*
import { ExcelBackupControls, PageRangeDeleteControls } from './ExcelBackupControls';

export function LocalDbDashboard() {
  return (
    <div class="space-y-6">
      <ExcelBackupControls />
      <PageRangeDeleteControls />
      {/* 기존 대시보드 컴포넌트들 *\/}
    </div>
  );
}
*/
