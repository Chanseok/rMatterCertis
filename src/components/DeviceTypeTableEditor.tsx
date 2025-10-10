/**
 * DeviceTypeTableEditor - Device Types를 테이블 형식으로 관리하는 컴포넌트
 * 기능: 레코드 추가/삭제, 정렬, 필터링, 편집
 */

import { Component, createSignal, For, Show, createMemo } from 'solid-js';

interface DeviceType {
  id: number;
  hex: string;
  name: string;
  category: string;
  introduced_in: string | null;
}

interface DeviceTypeTableEditorProps {
  deviceTypes: DeviceType[];
  onSave: (deviceTypes: DeviceType[]) => void;
  onReload: () => void;
}

export const DeviceTypeTableEditor: Component<DeviceTypeTableEditorProps> = (props) => {
  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedCategory, setSelectedCategory] = createSignal('All');
  const [sortField, setSortField] = createSignal<keyof DeviceType>('id');
  const [sortDirection, setSortDirection] = createSignal<'asc' | 'desc'>('asc');
  const [selectedIds, setSelectedIds] = createSignal<Set<number>>(new Set());
  const [editingId, setEditingId] = createSignal<number | null>(null);
  const [editForm, setEditForm] = createSignal<Partial<DeviceType>>({});
  const [showAddDialog, setShowAddDialog] = createSignal(false);
  const [newDeviceType, setNewDeviceType] = createSignal<Partial<DeviceType>>({
    id: 0,
    hex: '0x',
    name: '',
    category: 'Other',
    introduced_in: null
  });

  // 카테고리 목록 추출
  const categories = createMemo(() => {
    const cats = new Set<string>();
    props.deviceTypes.forEach(dt => cats.add(dt.category));
    return ['All', ...Array.from(cats).sort()];
  });

  // 필터링 및 정렬
  const filteredAndSorted = createMemo(() => {
    let result = [...props.deviceTypes];

    // 검색 필터
    const search = searchTerm().toLowerCase();
    if (search) {
      result = result.filter(dt => 
        dt.name.toLowerCase().includes(search) ||
        dt.hex.toLowerCase().includes(search) ||
        dt.category.toLowerCase().includes(search) ||
        dt.id.toString().includes(search)
      );
    }

    // 카테고리 필터
    if (selectedCategory() !== 'All') {
      result = result.filter(dt => dt.category === selectedCategory());
    }

    // 정렬
    const field = sortField();
    const dir = sortDirection();
    result.sort((a, b) => {
      const aVal = a[field];
      const bVal = b[field];
      
      if (aVal === null && bVal === null) return 0;
      if (aVal === null) return dir === 'asc' ? 1 : -1;
      if (bVal === null) return dir === 'asc' ? -1 : 1;
      
      if (typeof aVal === 'number' && typeof bVal === 'number') {
        return dir === 'asc' ? aVal - bVal : bVal - aVal;
      }
      
      const aStr = String(aVal);
      const bStr = String(bVal);
      return dir === 'asc' ? aStr.localeCompare(bStr) : bStr.localeCompare(aStr);
    });

    return result;
  });

  // 정렬 토글
  const toggleSort = (field: keyof DeviceType) => {
    if (sortField() === field) {
      setSortDirection(sortDirection() === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDirection('asc');
    }
  };

  // 선택 토글
  const toggleSelection = (id: number) => {
    const newSet = new Set(selectedIds());
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    setSelectedIds(newSet);
  };

  // 전체 선택/해제
  const toggleSelectAll = () => {
    if (selectedIds().size === filteredAndSorted().length) {
      setSelectedIds(new Set<number>());
    } else {
      setSelectedIds(new Set(filteredAndSorted().map(dt => dt.id)));
    }
  };

  // 선택된 항목 삭제
  const deleteSelected = () => {
    if (selectedIds().size === 0) return;
    
    if (confirm(`${selectedIds().size}개 항목을 삭제하시겠습니까?`)) {
      const remaining = props.deviceTypes.filter(dt => !selectedIds().has(dt.id));
      props.onSave(remaining);
      setSelectedIds(new Set<number>());
    }
  };

  // 편집 시작
  const startEdit = (deviceType: DeviceType) => {
    setEditingId(deviceType.id);
    setEditForm({ ...deviceType });
  };

  // 편집 저장
  const saveEdit = () => {
    const id = editingId();
    if (id === null) return;

    const updated = props.deviceTypes.map(dt => 
      dt.id === id ? { ...dt, ...editForm() } as DeviceType : dt
    );
    props.onSave(updated);
    setEditingId(null);
    setEditForm({});
  };

  // 편집 취소
  const cancelEdit = () => {
    setEditingId(null);
    setEditForm({});
  };

  // 새 항목 추가
  const addNewDeviceType = () => {
    const newDT = newDeviceType();
    
    // 유효성 검사
    if (!newDT.name || !newDT.hex || newDT.id === undefined) {
      alert('모든 필수 항목을 입력해주세요 (ID, Hex, Name)');
      return;
    }

    // ID 중복 체크
    if (props.deviceTypes.some(dt => dt.id === newDT.id)) {
      alert('이미 존재하는 ID입니다.');
      return;
    }

    const fullDeviceType: DeviceType = {
      id: newDT.id!,
      hex: newDT.hex!,
      name: newDT.name!,
      category: newDT.category || 'Other',
      introduced_in: newDT.introduced_in || null
    };

    props.onSave([...props.deviceTypes, fullDeviceType]);
    setShowAddDialog(false);
    setNewDeviceType({
      id: 0,
      hex: '0x',
      name: '',
      category: 'Other',
      introduced_in: null
    });
  };

  // 정렬 표시 화살표
  const SortArrow: Component<{ field: keyof DeviceType }> = (p) => {
    return (
      <Show when={sortField() === p.field}>
        <span class="ml-1 text-xs">{sortDirection() === 'asc' ? '▲' : '▼'}</span>
      </Show>
    );
  };

  return (
    <div class="space-y-4">
      {/* 상단 컨트롤 바 */}
      <div class="flex flex-wrap gap-3 items-center bg-slate-100 p-4 rounded-lg">
        <input
          type="text"
          placeholder="🔍 검색 (ID, Name, Hex, Category)"
          value={searchTerm()}
          onInput={(e) => setSearchTerm(e.currentTarget.value)}
          class="flex-1 min-w-[200px] px-3 py-2 border rounded-md focus:ring-2 focus:ring-indigo-500 focus:outline-none"
        />
        
        <select
          value={selectedCategory()}
          onChange={(e) => setSelectedCategory(e.currentTarget.value)}
          class="px-3 py-2 border rounded-md bg-white focus:ring-2 focus:ring-indigo-500 focus:outline-none"
        >
          <For each={categories()}>
            {(cat) => <option value={cat}>{cat}</option>}
          </For>
        </select>

        <button
          onClick={() => setShowAddDialog(true)}
          class="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-md font-medium transition-colors"
        >
          ➕ 추가
        </button>

        <button
          onClick={deleteSelected}
          disabled={selectedIds().size === 0}
          class={`px-4 py-2 rounded-md font-medium transition-colors ${
            selectedIds().size === 0
              ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
              : 'bg-rose-600 hover:bg-rose-700 text-white'
          }`}
        >
          🗑️ 삭제 ({selectedIds().size})
        </button>

        <button
          onClick={props.onReload}
          class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white rounded-md font-medium transition-colors"
        >
          🔄 새로고침
        </button>

        <div class="text-sm text-gray-600">
          총 {filteredAndSorted().length} / {props.deviceTypes.length}개
        </div>
      </div>

      {/* 테이블 */}
      <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
        <div class="overflow-x-auto max-h-[600px] overflow-y-auto">
          <table class="w-full">
            <thead class="bg-slate-50 sticky top-0 z-10">
              <tr>
                <th class="px-4 py-3 text-left">
                  <input
                    type="checkbox"
                    checked={selectedIds().size === filteredAndSorted().length && filteredAndSorted().length > 0}
                    onChange={toggleSelectAll}
                    class="w-4 h-4 cursor-pointer"
                  />
                </th>
                <th 
                  class="px-4 py-3 text-left font-semibold text-gray-700 cursor-pointer hover:bg-slate-100 transition-colors"
                  onClick={() => toggleSort('id')}
                >
                  ID <SortArrow field="id" />
                </th>
                <th 
                  class="px-4 py-3 text-left font-semibold text-gray-700 cursor-pointer hover:bg-slate-100 transition-colors"
                  onClick={() => toggleSort('hex')}
                >
                  Hex <SortArrow field="hex" />
                </th>
                <th 
                  class="px-4 py-3 text-left font-semibold text-gray-700 cursor-pointer hover:bg-slate-100 transition-colors"
                  onClick={() => toggleSort('name')}
                >
                  Name <SortArrow field="name" />
                </th>
                <th 
                  class="px-4 py-3 text-left font-semibold text-gray-700 cursor-pointer hover:bg-slate-100 transition-colors"
                  onClick={() => toggleSort('category')}
                >
                  Category <SortArrow field="category" />
                </th>
                <th 
                  class="px-4 py-3 text-left font-semibold text-gray-700 cursor-pointer hover:bg-slate-100 transition-colors"
                  onClick={() => toggleSort('introduced_in')}
                >
                  Introduced In <SortArrow field="introduced_in" />
                </th>
                <th class="px-4 py-3 text-center font-semibold text-gray-700">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              <For each={filteredAndSorted()}>
                {(dt) => (
                  <tr class="border-t hover:bg-slate-50 transition-colors">
                    <td class="px-4 py-2">
                      <input
                        type="checkbox"
                        checked={selectedIds().has(dt.id)}
                        onChange={() => toggleSelection(dt.id)}
                        class="w-4 h-4 cursor-pointer"
                      />
                    </td>
                    
                    <Show
                      when={editingId() === dt.id}
                      fallback={
                        <>
                          <td class="px-4 py-2 font-mono text-sm text-gray-800">{dt.id}</td>
                          <td class="px-4 py-2 font-mono text-xs text-indigo-600">{dt.hex}</td>
                          <td class="px-4 py-2 font-medium text-gray-900">{dt.name}</td>
                          <td class="px-4 py-2">
                            <span class="px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs font-medium">
                              {dt.category}
                            </span>
                          </td>
                          <td class="px-4 py-2 text-sm text-gray-600">{dt.introduced_in || '-'}</td>
                          <td class="px-4 py-2 text-center">
                            <button
                              onClick={() => startEdit(dt)}
                              class="px-3 py-1 bg-amber-500 hover:bg-amber-600 text-white rounded text-sm font-medium transition-colors"
                            >
                              ✏️ 수정
                            </button>
                          </td>
                        </>
                      }
                    >
                      {/* 편집 모드 */}
                      <td class="px-4 py-2">
                        <input
                          type="number"
                          value={editForm().id || dt.id}
                          onInput={(e) => setEditForm({ ...editForm(), id: parseInt(e.currentTarget.value) })}
                          class="w-20 px-2 py-1 border rounded text-sm"
                        />
                      </td>
                      <td class="px-4 py-2">
                        <input
                          type="text"
                          value={editForm().hex || dt.hex}
                          onInput={(e) => setEditForm({ ...editForm(), hex: e.currentTarget.value })}
                          class="w-24 px-2 py-1 border rounded text-sm font-mono"
                        />
                      </td>
                      <td class="px-4 py-2">
                        <input
                          type="text"
                          value={editForm().name || dt.name}
                          onInput={(e) => setEditForm({ ...editForm(), name: e.currentTarget.value })}
                          class="w-full px-2 py-1 border rounded text-sm"
                        />
                      </td>
                      <td class="px-4 py-2">
                        <input
                          type="text"
                          value={editForm().category || dt.category}
                          onInput={(e) => setEditForm({ ...editForm(), category: e.currentTarget.value })}
                          class="w-32 px-2 py-1 border rounded text-sm"
                        />
                      </td>
                      <td class="px-4 py-2">
                        <input
                          type="text"
                          value={editForm().introduced_in || dt.introduced_in || ''}
                          onInput={(e) => setEditForm({ ...editForm(), introduced_in: e.currentTarget.value || null })}
                          class="w-20 px-2 py-1 border rounded text-sm"
                        />
                      </td>
                      <td class="px-4 py-2 text-center space-x-2">
                        <button
                          onClick={saveEdit}
                          class="px-3 py-1 bg-emerald-600 hover:bg-emerald-700 text-white rounded text-sm font-medium transition-colors"
                        >
                          ✓ 저장
                        </button>
                        <button
                          onClick={cancelEdit}
                          class="px-3 py-1 bg-gray-400 hover:bg-gray-500 text-white rounded text-sm font-medium transition-colors"
                        >
                          ✕ 취소
                        </button>
                      </td>
                    </Show>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>

        <Show when={filteredAndSorted().length === 0}>
          <div class="p-12 text-center text-gray-400">
            <div class="text-4xl mb-2">📭</div>
            <div>검색 결과가 없습니다</div>
          </div>
        </Show>
      </div>

      {/* 추가 다이얼로그 */}
      <Show when={showAddDialog()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={() => setShowAddDialog(false)}>
          <div class="bg-white rounded-2xl shadow-2xl max-w-md w-full" onClick={(e) => e.stopPropagation()}>
            <div class="bg-gradient-to-r from-emerald-500 to-teal-600 px-6 py-4 rounded-t-2xl">
              <h3 class="text-xl font-bold text-white">➕ 새 Device Type 추가</h3>
            </div>
            
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">ID *</label>
                <input
                  type="number"
                  value={newDeviceType().id}
                  onInput={(e) => setNewDeviceType({ ...newDeviceType(), id: parseInt(e.currentTarget.value) })}
                  class="w-full px-3 py-2 border rounded-md focus:ring-2 focus:ring-emerald-500 focus:outline-none"
                  placeholder="예: 256"
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Hex Code *</label>
                <input
                  type="text"
                  value={newDeviceType().hex}
                  onInput={(e) => setNewDeviceType({ ...newDeviceType(), hex: e.currentTarget.value })}
                  class="w-full px-3 py-2 border rounded-md font-mono focus:ring-2 focus:ring-emerald-500 focus:outline-none"
                  placeholder="예: 0x0100"
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Name *</label>
                <input
                  type="text"
                  value={newDeviceType().name}
                  onInput={(e) => setNewDeviceType({ ...newDeviceType(), name: e.currentTarget.value })}
                  class="w-full px-3 py-2 border rounded-md focus:ring-2 focus:ring-emerald-500 focus:outline-none"
                  placeholder="예: Smart Light"
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Category *</label>
                <select
                  value={newDeviceType().category}
                  onChange={(e) => setNewDeviceType({ ...newDeviceType(), category: e.currentTarget.value })}
                  class="w-full px-3 py-2 border rounded-md focus:ring-2 focus:ring-emerald-500 focus:outline-none"
                >
                  <For each={categories().filter(c => c !== 'All')}>
                    {(cat) => <option value={cat}>{cat}</option>}
                  </For>
                </select>
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">Introduced In (선택)</label>
                <input
                  type="text"
                  value={newDeviceType().introduced_in || ''}
                  onInput={(e) => setNewDeviceType({ ...newDeviceType(), introduced_in: e.currentTarget.value || null })}
                  class="w-full px-3 py-2 border rounded-md focus:ring-2 focus:ring-emerald-500 focus:outline-none"
                  placeholder="예: 1.0, 1.2"
                />
              </div>
            </div>

            <div class="bg-gray-50 px-6 py-4 rounded-b-2xl flex justify-end gap-2">
              <button
                onClick={() => setShowAddDialog(false)}
                class="px-4 py-2 bg-gray-200 hover:bg-gray-300 text-gray-800 rounded-md font-medium transition-colors"
              >
                취소
              </button>
              <button
                onClick={addNewDeviceType}
                class="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white rounded-md font-medium transition-colors"
              >
                추가
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
