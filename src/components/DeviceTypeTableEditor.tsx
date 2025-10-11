/**
 * DeviceTypeTableEditor - Device Types를 테이블 형식으로 관리하는 컴포넌트
 * 기능: 레코드 추가/삭제, 정렬, 필터링, 편집
 */

import { Component, createSignal, For, Show, createMemo } from 'solid-js';
import { Portal } from 'solid-js/web';

interface DeviceType {
  id: number;
  hex: string;
  name: string;
  category: string;
  introduced_in: string | null;
}

interface DeviceTypeTableEditorProps {
  deviceTypes: DeviceType[];
  onReload: () => void;
  onExport?: () => void;
  onImport?: (filePath: string, replaceAll: boolean) => void;
  tauriApi: any; // Tauri API instance for DB operations
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
  const [showImportDialog, setShowImportDialog] = createSignal(false);
  const [importFilePath, setImportFilePath] = createSignal('');
  const [importReplaceAll, setImportReplaceAll] = createSignal(false);
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
  const deleteSelected = async () => {
    if (selectedIds().size === 0) return;
    
    if (confirm(`${selectedIds().size}개 항목을 삭제하시겠습니까?`)) {
      try {
        const ids = Array.from(selectedIds());
        await props.tauriApi.deleteDeviceTypesFromDb(ids);
        setSelectedIds(new Set<number>());
        await props.onReload(); // DB에서 다시 로드
      } catch (e: any) {
        alert(`삭제 실패: ${e}`);
      }
    }
  };

  // 편집 시작
  const startEdit = (deviceType: DeviceType) => {
    setEditingId(deviceType.id);
    setEditForm({ ...deviceType });
  };

  // 편집 저장
  const saveEdit = async () => {
    const id = editingId();
    if (id === null) return;

    try {
      const formData = editForm();
      await props.tauriApi.updateDeviceTypeInDb({
        id,
        hex: formData.hex!,
        name: formData.name!,
        category: formData.category || 'Other',
        introduced_in: formData.introduced_in || null
      });
      setEditingId(null);
      setEditForm({});
      await props.onReload(); // DB에서 다시 로드
    } catch (e: any) {
      alert(`업데이트 실패: ${e}`);
    }
  };

  // 편집 취소
  const cancelEdit = () => {
    setEditingId(null);
    setEditForm({});
  };

  // 새 항목 추가
  const addNewDeviceType = async () => {
    const newDT = newDeviceType();
    
    // 유효성 검사 (ID 제거, Hex와 Name만 체크)
    if (!newDT.name || !newDT.hex) {
      alert('모든 필수 항목을 입력해주세요 (Hex, Name)');
      return;
    }

    // Hex 중복 체크 (프론트엔드 사전 체크)
    if (props.deviceTypes.some(dt => dt.hex === newDT.hex)) {
      alert('이미 존재하는 Hex Code입니다.');
      return;
    }

    try {
      // ID는 DB에서 자동 생성 (AUTOINCREMENT)
      await props.tauriApi.addDeviceTypeToDb({
        id: 0, // DB에서 무시됨
        hex: newDT.hex!,
        name: newDT.name!,
        category: newDT.category || 'Other',
        introduced_in: newDT.introduced_in || null
      });
      
      setShowAddDialog(false);
      setNewDeviceType({
        id: 0,
        hex: '0x',
        name: '',
        category: 'Other',
        introduced_in: null
      });
      await props.onReload(); // DB에서 다시 로드
    } catch (e: any) {
      alert(`추가 실패: ${e}`);
    }
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
          placeholder="🔍 검색 (Name, Hex, Category)"
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

        <Show when={props.onExport}>
          <button
            onClick={props.onExport}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-md font-medium transition-colors"
          >
            📤 Export JSON
          </button>
        </Show>

        <Show when={props.onImport}>
          <button
            onClick={() => setShowImportDialog(true)}
            class="px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-md font-medium transition-colors"
          >
            📥 Import JSON
          </button>
        </Show>

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
        <Portal>
          <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999] p-4" onClick={() => setShowAddDialog(false)}>
            <div class="bg-white rounded-2xl shadow-2xl max-w-md w-full" onClick={(e) => e.stopPropagation()}>
            <div class="bg-gradient-to-r from-emerald-500 to-teal-600 px-6 py-4 rounded-t-2xl">
              <h3 class="text-xl font-bold text-white">➕ 새 Device Type 추가</h3>
            </div>
            
            <div class="p-6 space-y-4">
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
        </Portal>
      </Show>

      {/* Import Dialog */}
      <Show when={showImportDialog()}>
        <Portal>
          <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[9999] p-4">
            <div class="bg-white rounded-lg shadow-xl max-w-lg w-full p-6">
            <h3 class="text-xl font-bold mb-4">Device Types JSON Import</h3>
            
            <div class="space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">파일 선택</label>
                <button
                  onClick={async () => {
                    try {
                      const defaultDir = await props.tauriApi.getExportsDirectory();
                      const { open } = await import('@tauri-apps/plugin-dialog');
                      const selected = await open({
                        multiple: false,
                        filters: [{ name: 'JSON Files', extensions: ['json'] }],
                        title: 'Device Types JSON 파일 선택',
                        defaultPath: defaultDir
                      });
                      
                      if (selected && typeof selected === 'string') {
                        setImportFilePath(selected);
                      }
                    } catch (e: any) {
                      console.error('파일 선택 실패:', e);
                    }
                  }}
                  class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-md font-medium transition-colors"
                >
                  📁 JSON 파일 선택
                </button>
                <Show when={importFilePath()}>
                  <p class="text-xs text-gray-600 mt-2 break-all">
                    <strong>선택된 파일:</strong> {importFilePath()}
                  </p>
                </Show>
              </div>

              <div class="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="replace-all"
                  checked={importReplaceAll()}
                  onChange={(e) => setImportReplaceAll(e.currentTarget.checked)}
                  class="w-4 h-4 cursor-pointer"
                />
                <label for="replace-all" class="text-sm text-gray-700 cursor-pointer">
                  모든 기존 데이터 삭제 후 Import (체크 해제 시 Upsert)
                </label>
              </div>

              <div class="bg-amber-50 border border-amber-200 rounded p-3 text-sm">
                <p class="text-amber-800">
                  <strong>⚠️ 주의:</strong> Import는 DB의 device_types 테이블을 직접 수정합니다.
                  {importReplaceAll() && ' 기존 데이터가 모두 삭제됩니다!'}
                </p>
              </div>
            </div>

            <div class="flex gap-3 mt-6">
              <button
                onClick={() => {
                  if (importFilePath() && props.onImport) {
                    props.onImport(importFilePath(), importReplaceAll());
                    setShowImportDialog(false);
                    setImportFilePath('');
                    setImportReplaceAll(false);
                  }
                }}
                disabled={!importFilePath()}
                class={`flex-1 px-4 py-2 rounded-md font-medium transition-colors ${
                  importFilePath()
                    ? 'bg-purple-600 hover:bg-purple-700 text-white'
                    : 'bg-gray-300 text-gray-500 cursor-not-allowed'
                }`}
              >
                Import 실행
              </button>
              <button
                onClick={() => {
                  setShowImportDialog(false);
                  setImportFilePath('');
                  setImportReplaceAll(false);
                }}
                class="flex-1 px-4 py-2 bg-gray-200 hover:bg-gray-300 text-gray-700 rounded-md font-medium transition-colors"
              >
                취소
              </button>
            </div>
          </div>
        </div>
        </Portal>
      </Show>
    </div>
  );
};
