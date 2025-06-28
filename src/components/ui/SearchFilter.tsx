import { Show, For } from 'solid-js';
import { splitProps } from 'solid-js';
import type { SearchFilterProps } from '../../types/ui';
import Button from './Button';

/**
 * SearchFilter 컴포넌트
 * 
 * 기능:
 * - 텍스트 검색
 * - 드롭다운 필터링
 * - 정렬 옵션
 * - 초기화 기능
 * - 실시간 검색 또는 수동 검색
 * - 컴팩트/풀 모드
 */
const SearchFilter = (props: SearchFilterProps) => {
  const [local, others] = splitProps(props, [
    'searchQuery',
    'onSearchChange',
    'filterOptions',
    'selectedFilter', 
    'onFilterChange',
    'sortOptions',
    'selectedSort',
    'onSortChange',
    'onReset',
    'placeholder',
    'compact',
    'showReset',
    'class'
  ]);

  const handleSearchInput = (e: Event) => {
    const target = e.target as HTMLInputElement;
    local.onSearchChange?.(target.value);
  };

  const handleFilterChange = (e: Event) => {
    const target = e.target as HTMLSelectElement;
    local.onFilterChange?.(target.value);
  };

  const handleSortChange = (e: Event) => {
    const target = e.target as HTMLSelectElement;
    local.onSortChange?.(target.value);
  };

  const handleReset = () => {
    local.onReset?.();
  };

  const containerClasses = () => {
    return [
      local.compact ? 'flex items-center space-x-2' : 'space-y-4',
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  const searchInputClasses = () => {
    return [
      'block w-full px-3 py-2',
      'border border-gray-300 rounded-md',
      'placeholder-gray-400',
      'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500',
      'text-sm'
    ].join(' ');
  };

  const selectClasses = () => {
    return [
      'block px-3 py-2',
      'border border-gray-300 rounded-md',
      'bg-white',
      'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500',
      'text-sm',
      local.compact ? 'w-auto' : 'w-full'
    ].join(' ');
  };

  return (
    <div class={containerClasses()} {...others}>
      {/* Compact Mode Layout */}
      <Show when={local.compact}>
        <div class="flex items-center space-x-2 flex-1">
          {/* Search Input */}
          <div class="flex-1 min-w-0">
            <input
              type="text"
              class={searchInputClasses()}
              placeholder={local.placeholder || '검색...'}
              value={local.searchQuery || ''}
              onInput={handleSearchInput}
            />
          </div>

          {/* Filter Dropdown */}
          <Show when={local.filterOptions && local.filterOptions.length > 0}>
            <select
              class={selectClasses()}
              value={local.selectedFilter || ''}
              onChange={handleFilterChange}
            >
              <option value="">전체</option>
              <For each={local.filterOptions}>
                {(option) => (
                  <option value={option.value}>
                    {option.label}
                  </option>
                )}
              </For>
            </select>
          </Show>

          {/* Sort Dropdown */}
          <Show when={local.sortOptions && local.sortOptions.length > 0}>
            <select
              class={selectClasses()}
              value={local.selectedSort || ''}
              onChange={handleSortChange}
            >
              <option value="">정렬</option>
              <For each={local.sortOptions}>
                {(option) => (
                  <option value={option.value}>
                    {option.label}
                  </option>
                )}
              </For>
            </select>
          </Show>

          {/* Reset Button */}
          <Show when={local.showReset !== false}>
            <Button
              variant="outline"
              size="sm"
              onClick={handleReset}
            >
              초기화
            </Button>
          </Show>
        </div>
      </Show>

      {/* Full Mode Layout */}
      <Show when={!local.compact}>
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {/* Search Input */}
          <div class="lg:col-span-2">
            <label class="block text-sm font-medium text-gray-700 mb-1">
              검색
            </label>
            <input
              type="text"
              class={searchInputClasses()}
              placeholder={local.placeholder || '검색어를 입력하세요...'}
              value={local.searchQuery || ''}
              onInput={handleSearchInput}
            />
          </div>

          {/* Filter Dropdown */}
          <Show when={local.filterOptions && local.filterOptions.length > 0}>
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">
                필터
              </label>
              <select
                class={selectClasses()}
                value={local.selectedFilter || ''}
                onChange={handleFilterChange}
              >
                <option value="">전체</option>
                <For each={local.filterOptions}>
                  {(option) => (
                    <option value={option.value}>
                      {option.label}
                    </option>
                  )}
                </For>
              </select>
            </div>
          </Show>

          {/* Sort Dropdown */}
          <Show when={local.sortOptions && local.sortOptions.length > 0}>
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">
                정렬
              </label>
              <select
                class={selectClasses()}
                value={local.selectedSort || ''}
                onChange={handleSortChange}
              >
                <option value="">기본</option>
                <For each={local.sortOptions}>
                  {(option) => (
                    <option value={option.value}>
                      {option.label}
                    </option>
                  )}
                </For>
              </select>
            </div>
          </Show>
        </div>

        {/* Actions */}
        <Show when={local.showReset !== false}>
          <div class="flex justify-end">
            <Button
              variant="outline"
              onClick={handleReset}
            >
              필터 초기화
            </Button>
          </div>
        </Show>
      </Show>
    </div>
  );
};

export default SearchFilter;
