/**
 * CrawlingProcessDashboard.tsx
 * @description 'Live Production Line' UI를 구현하는 동적 그래프 시각화 컴포넌트입니다.
 *              three.js와 d3-force-3d를 사용하여 크롤링 프로세스를 시각화하고,
 *              하단 Progress Dock에서 진행 상황을 애니메이션으로 보여줍니다.
 */
import { Component, onMount, onCleanup, createSignal, For } from 'solid-js';
import * as THREE from 'three';
import { tauriApi } from '../services/tauri-api';
import type { SystemStatePayload, LiveSystemState } from '../types/events';

// --- 타입 정의 ---
type NodeStatus = 'idle' | 'running' | 'success' | 'error';

interface SimulationNode {
  id: string;
  status: NodeStatus;
  mesh: THREE.Mesh;
  x: number;
  y: number;
  z: number;
}

interface LinkData {
  source: SimulationNode;
  target: SimulationNode;
  line: THREE.Line;
}

interface PageInfo {
    id: string;
    status: 'success' | 'error';
}

// --- 색상 및 재질 상수 ---
const COLORS: Record<NodeStatus, THREE.Color> = {
  idle: new THREE.Color(0x888888),
  running: new THREE.Color(0x3b82f6),
  success: new THREE.Color(0x22c55e),
  error: new THREE.Color(0xef4444),
};

const createNodeMaterial = (status: NodeStatus) => new THREE.MeshStandardMaterial({
  color: COLORS[status],
  emissive: COLORS[status],
  emissiveIntensity: 0.5,
  metalness: 0.3,
  roughness: 0.4,
});

// --- 노드/링크 생성 함수 ---
const createNode = (id: string, status: NodeStatus): SimulationNode => {
  const geometry = new THREE.SphereGeometry(2, 32, 32);
  const material = createNodeMaterial(status);
  const mesh = new THREE.Mesh(geometry, material);
  
  const x = Math.random() * 20 - 10;
  const y = Math.random() * 20 - 10;
  const z = Math.random() * 20 - 10;
  
  mesh.position.set(x, y, z);

  return { id, status, mesh, x, y, z };
};

const createLink = (source: SimulationNode, target: SimulationNode): LinkData => {
  const material = new THREE.LineBasicMaterial({ color: 0xaaaaaa, transparent: true, opacity: 0.5 });
  const geometry = new THREE.BufferGeometry().setFromPoints([
    source.mesh.position,
    target.mesh.position
  ]);
  const line = new THREE.Line(geometry, material);
  return { source, target, line };
};

// --- 3D 씬 설정 함수 ---
const setupScene = (
  container: HTMLDivElement,
  getNodes: () => SimulationNode[],
  getLinks: () => LinkData[]
) => {
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1a1a1a);
  const camera = new THREE.PerspectiveCamera(75, container.clientWidth / container.clientHeight, 0.1, 1000);
  camera.position.z = 60;
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setSize(container.clientWidth, container.clientHeight);
  container.appendChild(renderer.domElement);

  const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
  scene.add(ambientLight);
  const pointLight = new THREE.PointLight(0xffffff, 0.8);
  pointLight.position.set(0, 0, 80);
  scene.add(pointLight);

  let animationFrameId: number;
  const animate = () => {
    animationFrameId = requestAnimationFrame(animate);

    getNodes().forEach(node => {
      node.mesh.position.set(node.x, node.y, node.z);
      
      if (node.status === 'running') {
        const scale = 1 + Math.sin(Date.now() * 0.005) * 0.1;
        node.mesh.scale.set(scale, scale, scale);
        (node.mesh.material as THREE.MeshStandardMaterial).emissiveIntensity = 1 + Math.sin(Date.now() * 0.005);
      } else {
        node.mesh.scale.set(1, 1, 1);
        (node.mesh.material as THREE.MeshStandardMaterial).emissiveIntensity = 0.5;
      }
    });

    getLinks().forEach(link => {
      const positions = link.line.geometry.attributes.position as THREE.BufferAttribute;
      positions.setXYZ(0, link.source.x, link.source.y, link.source.z);
      positions.setXYZ(1, link.target.x, link.target.y, link.target.z);
      positions.needsUpdate = true;
    });

    renderer.render(scene, camera);
  };
  animate();

  const handleResize = () => {
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(container.clientWidth, container.clientHeight);
  };
  window.addEventListener('resize', handleResize);

  const cleanup = () => {
    window.removeEventListener('resize', handleResize);
    cancelAnimationFrame(animationFrameId);
    container.removeChild(renderer.domElement);
    console.log('Three.js scene cleaned up');
  };

  return { scene, cleanup };
};

// --- 메인 컴포넌트 ---
export const CrawlingProcessDashboard: Component = () => {
  let container: HTMLDivElement | undefined;
  let scene: THREE.Scene | undefined;
  let cleanupFunctions: (() => void)[] = [];

  const [nodes, setNodes] = createSignal<SimulationNode[]>([]);
  const [links, setLinks] = createSignal<LinkData[]>([]);
  const [runningPages, setRunningPages] = createSignal<string[]>([]);
  const [completedPages, setCompletedPages] = createSignal<PageInfo[]>([]);
  
  // Live Production Line 상태 관리
  const [systemState, setSystemState] = createSignal<SystemStatePayload | null>(null);
  const [currentBatch, setCurrentBatch] = createSignal<any>(null);
  const [activeStages, setActiveStages] = createSignal<any[]>([]);
  const [batchNodes, setBatchNodes] = createSignal<SimulationNode[]>([]);
  const [stageNodes, setStageNodes] = createSignal<SimulationNode[]>([]);
  
  // Actor System 상태 관리 (새로운 아키텍처)
  const [actorSessionActive, setActorSessionActive] = createSignal(false);
  const [currentSession, setCurrentSession] = createSignal<any>(null);
  const [activeBatches, setActiveBatches] = createSignal<any[]>([]);
  const [completedBatches, setCompletedBatches] = createSignal<any[]>([]);
  const [actorSystemStats, setActorSystemStats] = createSignal({
    totalPages: 0,
    processedPages: 0,
    successRate: 0,
    averageBatchTime: 0,
  });
  // 개발용 로그 및 성능 정보
  const [eventLog, setEventLog] = createSignal<string[]>([]);
  const [eventCounts, setEventCounts] = createSignal({
    systemState: 0,
    atomicTask: 0,
    liveState: 0,
    total: 0
  });
  const [performanceStats, setPerformanceStats] = createSignal({
    estimatedTimeRemaining: 0,
    averageTaskTime: 0,
    successRate: 0,
    currentBatchSize: 0,
    itemsPerMinute: 0
  });

  const addLogEntry = (type: string, message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    const logEntry = `[${timestamp}] ${type}: ${message}`;
    setEventLog(prev => [...prev.slice(-19), logEntry]); // 최근 20개 로그만 유지
    
    // 이벤트 카운트 업데이트
    setEventCounts(prev => {
      const newCounts = { ...prev, total: prev.total + 1 };
      if (type === 'SYSTEM') newCounts.systemState++;
      else if (type === 'ATOMIC') newCounts.atomicTask++;
      else if (type === 'LIVE') newCounts.liveState++;
      return newCounts;
    });
  };

  // 동적 그래프 노드 생성 및 관리
  const createBatchNode = (batchId: number, position: { x: number; y: number; z: number }) => {
    const node = createNode(`batch-${batchId}`, 'running');
    node.x = position.x;
    node.y = position.y;
    node.z = position.z;
    node.mesh.position.set(position.x, position.y, position.z);
    return node;
  };

  const createStageNode = (stageName: string, batchId: number, position: { x: number; y: number; z: number }) => {
    const node = createNode(`stage-${stageName}-${batchId}`, 'running');
    node.x = position.x;
    node.y = position.y;
    node.z = position.z;
    node.mesh.position.set(position.x, position.y, position.z);
    return node;
  };

  // 배치 및 스테이지 노드 동적 생성
  const updateProductionLineGraph = (liveState: LiveSystemState) => {
    if (!scene) return;

    // 배치 노드 업데이트
    if (liveState.current_batch && !batchNodes().find(n => n.id === `batch-${liveState.current_batch!.id}`)) {
      const batchNode = createBatchNode(liveState.current_batch.id, {
        x: -20,
        y: 0,
        z: 0
      });
      setBatchNodes(prev => [...prev, batchNode]);
      scene.add(batchNode.mesh);
      
      // 센터 노드와 연결
      const centerNode = nodes().find(n => n.id === 'center');
      if (centerNode) {
        const link = createLink(centerNode, batchNode);
        scene.add(link.line);
        setLinks(prev => [...prev, link]);
      }
    }

    // 스테이지 노드 업데이트
    liveState.stages.forEach((stage, index) => {
      const stageNodeId = `stage-${stage.name}-${liveState.current_batch?.id || 0}`;
      if (!stageNodes().find(n => n.id === stageNodeId)) {
        const stageNode = createStageNode(stage.name, liveState.current_batch?.id || 0, {
          x: -20 + (index + 1) * 15,
          y: Math.sin(index * 0.5) * 10,
          z: Math.cos(index * 0.5) * 10
        });
        setStageNodes(prev => [...prev, stageNode]);
        if (scene) {
          scene.add(stageNode.mesh);
          
          // 배치 노드와 연결
          const batchNode = batchNodes().find(n => n.id === `batch-${liveState.current_batch?.id || 0}`);
          if (batchNode) {
            const link = createLink(batchNode, stageNode);
            scene.add(link.line);
            setLinks(prev => [...prev, link]);
          }
        }
      }
    });
  };

  // const addNodeAndLink = (newNode: SimulationNode, parentNodeId: string) => {
  //   const parentNode = nodes().find(n => n.id === parentNodeId);
  //   if (!parentNode || !scene) return;

  //   scene.add(newNode.mesh);
  //   const newLink = createLink(parentNode, newNode);
  //   scene.add(newLink.line);

  //   setNodes([...nodes(), newNode]);
  //   setLinks([...links(), newLink]);
  // };

  // 테스트용 Actor 시스템 시뮬레이션
  const testActorSystem = async () => {
    try {
      console.log('🎭 Testing Actor System integration...');
      addLogEntry('ACTOR', 'Starting Actor System test...');
      
      // Actor 기반 크롤링 시작 (시뮬레이션)
      await tauriApi.startActorBasedCrawling({
        startPage: 1,
        endPage: 5,
        batchSize: 2,
        concurrencyLimit: 3
      });
      
      addLogEntry('ACTOR', 'Actor System test initiated successfully');
    } catch (error) {
      console.error('❌ Actor System test failed:', error);
      addLogEntry('ACTOR', `Test failed: ${error}`);
    }
  };

  // 테스트용 AtomicTaskEvent 시뮬레이션
  const simulateAtomicTaskEvent = () => {
    const taskId = `test-task-${Math.floor(Math.random() * 1000)}`;
    const stages = ['ListPageCollection', 'ProductDetailCollection', 'DatabaseSave'];
    const stageName = stages[Math.floor(Math.random() * stages.length)];
    const statuses = ['Pending', 'Active', 'Success', 'Error'];
    const status = statuses[Math.floor(Math.random() * statuses.length)];
    
    const mockEvent = {
      task_id: taskId,
      batch_id: 1,
      stage_name: stageName,
      status: status,
      progress: Math.random(),
      message: `Simulated ${status} event for ${taskId}`,
      timestamp: new Date().toISOString()
    };
    
    console.log('Simulating atomic task event:', mockEvent);
    addLogEntry('ATOMIC', `Test Task ${mockEvent.task_id} [${mockEvent.stage_name}]: ${mockEvent.status} (${(mockEvent.progress * 100).toFixed(1)}%)`);
    
    // 작업 상태에 따른 페이지 관리
    if (mockEvent.status === 'Active') {
      setRunningPages(prev => [...prev, mockEvent.task_id]);
    } else if (mockEvent.status === 'Success') {
      setRunningPages(prev => prev.filter(id => id !== mockEvent.task_id));
      setCompletedPages(prev => [...prev, { id: mockEvent.task_id, status: 'success' }]);
    } else if (mockEvent.status === 'Error') {
      setRunningPages(prev => prev.filter(id => id !== mockEvent.task_id));
      setCompletedPages(prev => [...prev, { id: mockEvent.task_id, status: 'error' }]);
    }
    
    // 노드 생성 및 그래프 업데이트
    if (scene) {
      const newNode = createNode(taskId, mockEvent.status as NodeStatus);
      newNode.x = (Math.random() - 0.5) * 40;
      newNode.y = (Math.random() - 0.5) * 40;
      newNode.z = (Math.random() - 0.5) * 40;
      newNode.mesh.position.set(newNode.x, newNode.y, newNode.z);
      
      setNodes(prev => [...prev, newNode]);
      scene.add(newNode.mesh);
      
      // 센터 노드와 연결
      const centerNode = nodes().find(n => n.id === 'center');
      if (centerNode) {
        const link = createLink(centerNode, newNode);
        scene.add(link.line);
        setLinks(prev => [...prev, link]);
      }
    }
  };

  onMount(async () => {
    if (container) {
      const { scene: s, cleanup } = setupScene(container, nodes, links);
      scene = s;
      cleanupFunctions.push(cleanup);

      console.log('🎨 Live Production Line UI가 마운트되고 씬과 물리엔진이 설정되었습니다.');

      const centerNode = createNode('center', 'running');
      setNodes([centerNode]);
      scene.add(centerNode.mesh);

      // Live Production Line 이벤트 구독
      const unlisteners = await tauriApi.subscribeToLiveProductionLineEvents({
        onSystemStateUpdate: (state) => {
          console.log('System state received:', state);
          setSystemState(state);
          addLogEntry('SYSTEM', `Running: ${state.is_running}, DB Products: ${state.db_total_products}, ETA: ${state.session_eta_seconds}s`);
          
          // 성능 정보 업데이트
          setPerformanceStats(prev => ({
            ...prev,
            estimatedTimeRemaining: state.session_eta_seconds * 1000,
            itemsPerMinute: state.items_per_minute,
            currentBatchSize: state.session_target_items
          }));
        },
        
        onAtomicTaskUpdate: (event) => {
          console.log('Atomic task event received:', event);
          addLogEntry('ATOMIC', `Task ${event.task_id} [${event.stage_name}]: ${event.status} (${(event.progress * 100).toFixed(1)}%)`);
          
          // 작업 상태에 따른 페이지 관리
          if (event.status === 'Active') {
            setRunningPages(prev => [...prev, event.task_id]);
          } else if (event.status === 'Success') {
            setRunningPages(prev => prev.filter(id => id !== event.task_id));
            setCompletedPages(prev => [...prev, { id: event.task_id, status: 'success' }]);
          } else if (event.status === 'Error') {
            setRunningPages(prev => prev.filter(id => id !== event.task_id));
            setCompletedPages(prev => [...prev, { id: event.task_id, status: 'error' }]);
          }
        },
        
        onLiveStateUpdate: (liveState) => {
          console.log('Live state received:', liveState);
          addLogEntry('LIVE', `Batch ${liveState.current_batch?.id || 'N/A'} - ${liveState.stages.length} stages active`);
          
          // 동적 그래프 업데이트
          updateProductionLineGraph(liveState);
          
          // 배치 및 스테이지 정보 업데이트
          setCurrentBatch(liveState.current_batch);
          setActiveStages(liveState.stages);
        }
      });
      
      // Actor 시스템 이벤트 구독 (새로운 아키텍처 테스트)
      const actorCleanup = await tauriApi.subscribeToActorSystemEvents({
        onSessionStarted: (data) => {
          console.log('🎭 Actor Session Started:', data);
          addLogEntry('ACTOR', `Session ${data.session_id} started - ${data.total_pages} pages`);
          setActorSessionActive(true);
          setCurrentSession(data);
        },
        
        onBatchStarted: (data) => {
          console.log('🎭 Actor Batch Started:', data);
          addLogEntry('ACTOR', `Batch ${data.batch_number}/${data.total_batches} started - ${data.pages_in_batch} pages`);
          setActiveBatches(prev => [...prev, data]);
          
          // 3D 그래프에 배치 노드 추가
          if (scene) {
            const batchNode = createBatchNode(data.batch_number, {
              x: -30 + data.batch_number * 8,
              y: Math.sin(data.batch_number * 0.5) * 5,
              z: Math.cos(data.batch_number * 0.5) * 5
            });
            setBatchNodes(prev => [...prev, batchNode]);
            scene.add(batchNode.mesh);
            
            // 센터 노드와 연결
            const centerNode = nodes().find(n => n.id === 'center');
            if (centerNode) {
              const link = createLink(centerNode, batchNode);
              scene.add(link.line);
              setLinks(prev => [...prev, link]);
            }
          }
        },
        
        onStageCompleted: (data) => {
          console.log('🎭 Actor Stage Completed:', data);
          addLogEntry('ACTOR', `Stage ${data.stage_name} in ${data.batch_id}: ${data.success ? 'SUCCESS' : 'FAILED'} (${data.processing_time_ms}ms)`);
          
          // 성공률 업데이트
          setActorSystemStats(prev => ({
            ...prev,
            processedPages: prev.processedPages + data.items_processed,
            successRate: data.success ? Math.min(prev.successRate + 0.1, 1.0) : Math.max(prev.successRate - 0.05, 0.0)
          }));
        },
        
        onBatchCompleted: (data) => {
          console.log('🎭 Actor Batch Completed:', data);
          addLogEntry('ACTOR', `Batch ${data.batch_id} completed - ${data.total_items_processed} items (${data.batch_duration_ms}ms)`);
          setActiveBatches(prev => prev.filter(b => b.batch_id !== data.batch_id));
          setCompletedBatches(prev => [...prev, data]);
        },
        
        onSessionCompleted: (data) => {
          console.log('🎭 Actor Session Completed:', data);
          addLogEntry('ACTOR', `Session completed - ${data.total_pages_processed} pages, ${data.success_rate}% success`);
          setActorSessionActive(false);
          setCurrentSession(null);
          setActiveBatches([]);
        }
      });
      
      // 정리 함수 등록
      unlisteners.forEach(unlisten => cleanupFunctions.push(unlisten));
      cleanupFunctions.push(actorCleanup);
    }
  });

  onCleanup(() => {
    cleanupFunctions.forEach(cleanup => cleanup());
  });

  // --- 테스트용 함수 ---
  // const handleAddTestProcess = () => {
  //   const pageId = `Page-${Math.round(Math.random() * 1000)}`;
  //   const newNode = createNode(pageId, 'running');
  //   addNodeAndLink(newNode, 'center');

  //   // 1. '실행 중' 목록에 추가
  //   setRunningPages(prev => [...prev, pageId]);

  //   // 2. 몇 초 후 '완료' 상태로 전환 (시뮬레이션)
  //   setTimeout(() => {
  //     // 3D 노드 상태 변경
  //     newNode.status = Math.random() > 0.2 ? 'success' : 'error';
  //     (newNode.mesh.material as THREE.MeshStandardMaterial).color.set(COLORS[newNode.status]);
  //     (newNode.mesh.material as THREE.MeshStandardMaterial).emissive.set(COLORS[newNode.status]);

  //     // '실행 중' 목록에서 제거
  //     setRunningPages(prev => prev.filter(p => p !== pageId));
  //     // '완료' 목록에 추가
  //     setCompletedPages(prev => [...prev, { id: pageId, status: newNode.status === 'success' ? 'success' : 'error' }]);

  //   }, 2000 + Math.random() * 3000);
  // };

  return (
    <div class="w-full h-screen relative font-sans">
      {/* 애니메이션을 위한 스타일 정의 */}
      <style>
        {`
          @keyframes bounce {
            0%, 100% { transform: translateY(0); }
            50% { transform: translateY(-10px); }
          }
          .bouncing-item {
            animation: bounce 1.5s ease-in-out infinite;
          }
        `}
      </style>

      {/* 메인 3D 캔버스 */}
      <div ref={container} class="w-full h-full absolute top-0 left-0" />

      {/* 테스트용 컨트롤 버튼들 */}
      <div class="absolute top-4 left-4 flex flex-col gap-2">
        <button onClick={simulateAtomicTaskEvent} class="bg-blue-500 text-white font-bold py-2 px-4 rounded shadow-lg">
          Add Test Process
        </button>
        <button onClick={testActorSystem} class="bg-purple-500 text-white font-bold py-2 px-4 rounded shadow-lg">
          🎭 Test Actor System
        </button>
      </div>

      {/* Actor 시스템 상태 표시 */}
      <div class="absolute top-4 right-4 bg-black bg-opacity-50 text-white p-3 rounded-lg text-sm">
        <div class="flex items-center gap-2 mb-2">
          <div class={`w-3 h-3 rounded-full ${actorSessionActive() ? 'bg-green-400' : 'bg-gray-500'}`}></div>
          <span class="font-semibold">Actor System</span>
        </div>
        <div class="space-y-1 text-xs">
          <div>Active Batches: {activeBatches().length}</div>
          <div>Completed: {completedBatches().length}</div>
          <div>Success Rate: {(actorSystemStats().successRate * 100).toFixed(1)}%</div>
          <div>Processed: {actorSystemStats().processedPages} pages</div>
        </div>
      </div>

      {/* 하단 Progress Dock */}
      <div class="absolute bottom-0 left-0 right-0 h-48 bg-black bg-opacity-40 backdrop-blur-md p-4 flex flex-col text-white">
        
        {/* 성능 및 이벤트 통계 */}
        <div class="h-12 flex justify-between items-center border-b border-gray-600 mb-2">
          <div class="flex gap-6 text-sm">
            <div class="flex flex-col">
              <span class="text-xs text-gray-400">예상 소요 시간</span>
              <span class="font-mono">
                {performanceStats().estimatedTimeRemaining > 0 
                  ? `${Math.ceil(performanceStats().estimatedTimeRemaining / 1000)}초`
                  : '계산 중...'}
              </span>
            </div>
            <div class="flex flex-col">
              <span class="text-xs text-gray-400">성공률</span>
              <span class="font-mono">{performanceStats().successRate}</span>
            </div>
          </div>
          
          <div class="flex gap-4 text-sm">
            <span class="text-blue-400">시스템: {eventCounts().systemState}</span>
            <span class="text-yellow-400">작업: {eventCounts().atomicTask}</span>
            <span class="text-green-400">Live: {eventCounts().liveState}</span>
            <span class="text-gray-400">총: {eventCounts().total}</span>
          </div>
        </div>
        
        {/* 기존 실행 중/완료 섹션 */}
        <div class="flex-1 flex justify-between items-stretch">
          {/* 실행 중 섹션 */}
          <div class="w-1/3 h-full overflow-y-auto pr-2">
            <h4 class="text-sm font-semibold mb-2 sticky top-0 bg-transparent">실행 중</h4>
            <div class="flex flex-wrap gap-2">
              <For each={runningPages()}>
                {(pageId) => (
                  <div class="bouncing-item bg-blue-500 px-2 py-1 rounded-md text-xs font-mono">
                    {pageId}
                  </div>
                )}
              </For>
            </div>
          </div>
          
          {/* 완료 섹션 */}
          <div class="w-1/3 h-full border-l-2 border-gray-600 pl-4 overflow-y-auto">
            <h4 class="text-sm font-semibold mb-2 sticky top-0 bg-transparent">완료</h4>
            <div class="flex flex-wrap gap-1.5">
              <For each={completedPages()}>
                {(page) => (
                  <div 
                    class="w-16 h-8 rounded flex items-center justify-center text-xs font-mono shadow-md"
                    classList={{
                      'bg-green-600': page.status === 'success',
                      'bg-red-600': page.status === 'error',
                    }}
                    title={page.id}
                  >
                    {page.id.split('-')[1]}
                  </div>
                )}
              </For>
            </div>
          </div>
          
          {/* 개발용 이벤트 로그 섹션 */}
          <div class="w-1/3 h-full border-l-2 border-gray-600 pl-4 overflow-y-auto">
            <h4 class="text-sm font-semibold mb-2 sticky top-0 bg-transparent">실시간 이벤트 로그</h4>
            <div class="text-xs font-mono space-y-1">
              <For each={eventLog()}>
                {(logEntry) => (
                  <div class="text-gray-300 leading-tight">
                    {logEntry}
                  </div>
                )}
              </For>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};