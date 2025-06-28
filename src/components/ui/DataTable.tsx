import { For, Show, createSignal } from 'solid-js';
import { splitProps } from 'solid-js';
import type { DataTableProps, DataTableColumn } from '../../types/ui';
import Spinner from './Spinner';

/**
 * DataTable 컴포넌트
 * 
 * 기능:
 * - 동적 컬럼 설정
 * - 정렬 기능
 * - 페이지네이션
 * - 로딩 상태
 * - 행 클릭 이벤트
 * - 반응형 디자인
 * - 커스텀 렌더링
 */
const DataTable = <T extends Record<string, any>>(props: DataTableProps<T>) => {
  const [local, others] = splitProps(props, [
    'data',
    'columns',
    'loading',
    'pagination',
    'onRowClick',
    'rowKey',
    'class'
  ]);

  const [sortColumn, setSortColumn] = createSignal<string | null>(null);
  const [sortDirection, setSortDirection] = createSignal<'asc' | 'desc'>('asc');

  const handleSort = (column: DataTableColumn<T>) => {
    if (!column.sortable) return;

    const currentColumn = sortColumn();
    const currentDirection = sortDirection();

    if (currentColumn === String(column.key)) {
      // 같은 컬럼을 클릭한 경우 방향 토글
      setSortDirection(currentDirection === 'asc' ? 'desc' : 'asc');
    } else {
      // 다른 컬럼을 클릭한 경우 해당 컬럼으로 오름차순 정렬
      setSortColumn(String(column.key));
      setSortDirection('asc');
    }
  };

  const sortedData = () => {
    const column = sortColumn();
    if (!column) return local.data;

    return [...local.data].sort((a, b) => {
      const aValue = a[column as keyof T];
      const bValue = b[column as keyof T];
      const direction = sortDirection() === 'asc' ? 1 : -1;

      if (aValue === bValue) return 0;
      if (aValue == null) return 1;
      if (bValue == null) return -1;

      if (typeof aValue === 'string' && typeof bValue === 'string') {
        return aValue.localeCompare(bValue) * direction;
      }

      return (aValue < bValue ? -1 : 1) * direction;
    });
  };

  const getSortIcon = (column: DataTableColumn<T>) => {
    if (!column.sortable) return null;

    const currentColumn = sortColumn();
    const direction = sortDirection();

    if (currentColumn !== String(column.key)) {
      return (
        <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l4-4 4 4m0 6l-4 4-4-4" />
        </svg>
      );
    }

    return direction === 'asc' ? (
      <svg class="w-4 h-4 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7" />
      </svg>
    ) : (
      <svg class="w-4 h-4 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
      </svg>
    );
  };

  const getColumnAlign = (column: DataTableColumn<T>) => {
    const align = column.align || 'left';
    const alignClasses = {
      left: 'text-left',
      center: 'text-center',
      right: 'text-right'
    };
    return alignClasses[align];
  };

  const renderCellContent = (column: DataTableColumn<T>, record: T) => {
    const value = record[column.key];
    
    if (column.render) {
      return column.render(value, record);
    }
    
    return value?.toString() || '';
  };

  const tableClasses = () => {
    return [
      'w-full',
      'bg-white',
      'border',
      'border-gray-200',
      'rounded-lg',
      'overflow-hidden',
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  return (
    <div class={tableClasses()} {...others}>
      {/* Loading Overlay */}
      <Show when={local.loading}>
        <div class="relative">
          <div class="absolute inset-0 bg-white bg-opacity-75 flex items-center justify-center z-10">
            <Spinner size="lg" label="데이터 로딩 중..." center />
          </div>
        </div>
      </Show>

      {/* Table */}
      <div class="overflow-x-auto">
        <table class="w-full">
          {/* Header */}
          <thead class="bg-gray-50">
            <tr>
              <For each={local.columns}>
                {(column) => (
                  <th
                    class={`
                      px-6 py-3 
                      text-xs font-medium text-gray-500 uppercase tracking-wider
                      ${getColumnAlign(column)}
                      ${column.sortable ? 'cursor-pointer hover:bg-gray-100 select-none' : ''}
                    `}
                    style={column.width ? { width: column.width } : {}}
                    onClick={() => handleSort(column)}
                  >
                    <div class="flex items-center space-x-1">
                      <span>{column.title}</span>
                      {getSortIcon(column)}
                    </div>
                  </th>
                )}
              </For>
            </tr>
          </thead>

          {/* Body */}
          <tbody class="bg-white divide-y divide-gray-200">
            <Show
              when={!local.loading && sortedData().length > 0}
              fallback={
                <Show when={!local.loading}>
                  <tr>
                    <td 
                      colspan={local.columns.length} 
                      class="px-6 py-12 text-center text-gray-500"
                    >
                      데이터가 없습니다.
                    </td>
                  </tr>
                </Show>
              }
            >
              <For each={sortedData()}>
                {(record) => (
                  <tr
                    class={`
                      hover:bg-gray-50 transition-colors
                      ${local.onRowClick ? 'cursor-pointer' : ''}
                    `}
                    onClick={() => local.onRowClick?.(record)}
                  >
                    <For each={local.columns}>
                      {(column) => (
                        <td
                          class={`
                            px-6 py-4 whitespace-nowrap text-sm text-gray-900
                            ${getColumnAlign(column)}
                          `}
                        >
                          {renderCellContent(column, record)}
                        </td>
                      )}
                    </For>
                  </tr>
                )}
              </For>
            </Show>
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <Show when={local.pagination}>
        <div class="bg-white px-4 py-3 border-t border-gray-200 sm:px-6">
          <div class="flex items-center justify-between">
            <div class="flex-1 flex justify-between sm:hidden">
              {/* Mobile Pagination */}
              <button
                class="relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                disabled={local.pagination!.current <= 1}
                onClick={() => local.pagination!.onChange(local.pagination!.current - 1)}
              >
                이전
              </button>
              <button
                class="ml-3 relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
                disabled={local.pagination!.current >= Math.ceil(local.pagination!.total / local.pagination!.pageSize)}
                onClick={() => local.pagination!.onChange(local.pagination!.current + 1)}
              >
                다음
              </button>
            </div>

            <div class="hidden sm:flex-1 sm:flex sm:items-center sm:justify-between">
              <div>
                <p class="text-sm text-gray-700">
                  총 <span class="font-medium">{local.pagination!.total}</span>개 중{' '}
                  <span class="font-medium">
                    {(local.pagination!.current - 1) * local.pagination!.pageSize + 1}
                  </span>
                  -{' '}
                  <span class="font-medium">
                    {Math.min(local.pagination!.current * local.pagination!.pageSize, local.pagination!.total)}
                  </span>
                  개 표시
                </p>
              </div>
              
              <div class="flex items-center space-x-2">
                <button
                  class="relative inline-flex items-center px-2 py-2 border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed rounded-l-md"
                  disabled={local.pagination!.current <= 1}
                  onClick={() => local.pagination!.onChange(local.pagination!.current - 1)}
                >
                  이전
                </button>
                
                <span class="text-sm text-gray-700">
                  페이지 {local.pagination!.current} / {Math.ceil(local.pagination!.total / local.pagination!.pageSize)}
                </span>
                
                <button
                  class="relative inline-flex items-center px-2 py-2 border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed rounded-r-md"
                  disabled={local.pagination!.current >= Math.ceil(local.pagination!.total / local.pagination!.pageSize)}
                  onClick={() => local.pagination!.onChange(local.pagination!.current + 1)}
                >
                  다음
                </button>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default DataTable;
