/**
 * CrawlingCity3D - 3D 아이소메트릭 스타일의 크롤링 도시 시각화
 * 각 작업 단계를 3D 건물로 표현하여 직관적인 시각화 제공
 */

import { Component, createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import type { CrawlingProgress } from '../../types/crawling';

export interface CrawlingCity3DProps {
  progress: CrawlingProgress | null;
  isRunning: boolean;
  onBuildingClick: (buildingId: string) => void;
}

interface Building3D {
  id: string;
  name: string;
  type: 'fetcher' | 'parser' | 'processor' | 'saver' | 'control';
  position: { x: number; y: number; z: number };
  size: { width: number; height: number; depth: number };
  color: string;
  workers: number;
  maxWorkers: number;
  queueSize: number;
  maxQueueSize: number;
  status: 'idle' | 'working' | 'busy' | 'error';
  particles: Array<{ id: string; x: number; y: number; opacity: number }>;
  connections: string[]; // 연결된 건물들의 ID
}

export const CrawlingCity3D: Component<CrawlingCity3DProps> = (props) => {
  const [buildings, setBuildings] = createSignal<Building3D[]>([
    {
      id: 'control',
      name: 'Control Tower',
      type: 'control',
      position: { x: 400, y: 200, z: 0 },
      size: { width: 80, height: 120, depth: 80 },
      color: '#3B82F6',
      workers: 1,
      maxWorkers: 1,
      queueSize: 0,
      maxQueueSize: 0,
      status: 'working',
      particles: [],
      connections: ['fetcher-1', 'fetcher-2', 'fetcher-3']
    },
    {
      id: 'fetcher-1',
      name: 'List Fetcher',
      type: 'fetcher',
      position: { x: 200, y: 100, z: 0 },
      size: { width: 60, height: 80, depth: 60 },
      color: '#10B981',
      workers: 3,
      maxWorkers: 5,
      queueSize: 12,
      maxQueueSize: 50,
      status: 'working',
      particles: [],
      connections: ['parser-1']
    },
    {
      id: 'fetcher-2',
      name: 'Detail Fetcher',
      type: 'fetcher',
      position: { x: 600, y: 100, z: 0 },
      size: { width: 60, height: 80, depth: 60 },
      color: '#10B981',
      workers: 4,
      maxWorkers: 8,
      queueSize: 25,
      maxQueueSize: 100,
      status: 'busy',
      particles: [],
      connections: ['parser-2']
    },
    {
      id: 'parser-1',
      name: 'List Parser',
      type: 'parser',
      position: { x: 200, y: 300, z: 0 },
      size: { width: 70, height: 90, depth: 70 },
      color: '#F59E0B',
      workers: 2,
      maxWorkers: 4,
      queueSize: 8,
      maxQueueSize: 30,
      status: 'working',
      particles: [],
      connections: ['fetcher-2']
    },
    {
      id: 'parser-2',
      name: 'Detail Parser',
      type: 'parser',
      position: { x: 600, y: 300, z: 0 },
      size: { width: 70, height: 90, depth: 70 },
      color: '#F59E0B',
      workers: 3,
      maxWorkers: 6,
      queueSize: 18,
      maxQueueSize: 60,
      status: 'working',
      particles: [],
      connections: ['processor-1']
    },
    {
      id: 'processor-1',
      name: 'Data Processor',
      type: 'processor',
      position: { x: 400, y: 400, z: 0 },
      size: { width: 80, height: 100, depth: 80 },
      color: '#8B5CF6',
      workers: 2,
      maxWorkers: 4,
      queueSize: 15,
      maxQueueSize: 40,
      status: 'working',
      particles: [],
      connections: ['saver-1']
    },
    {
      id: 'saver-1',
      name: 'Database Saver',
      type: 'saver',
      position: { x: 400, y: 500, z: 0 },
      size: { width: 90, height: 70, depth: 90 },
      color: '#EF4444',
      workers: 1,
      maxWorkers: 2,
      queueSize: 5,
      maxQueueSize: 20,
      status: 'working',
      particles: [],
      connections: []
    }
  ]);

  const [viewTransform, setViewTransform] = createSignal({
    rotation: 0,
    zoom: 1,
    offsetX: 0,
    offsetY: 0
  });

  const [selectedBuilding, setSelectedBuilding] = createSignal<string | null>(null);

  let animationFrame: number;
  let particleId = 0;

  onMount(() => {
    startAnimation();
  });

  onCleanup(() => {
    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
    }
  });

  const startAnimation = () => {
    const animate = () => {
      if (props.isRunning) {
        updateParticles();
        updateBuildingStates();
      }
      animationFrame = requestAnimationFrame(animate);
    };
    animate();
  };

  const updateParticles = () => {
    // 데이터 플로우를 시각화하는 파티클 생성
    setBuildings(prev => prev.map(building => {
      if (building.status === 'working' && Math.random() < 0.3) {
        const newParticles = [...building.particles];
        
        // 새 파티클 생성
        if (newParticles.length < 10) {
          newParticles.push({
            id: `particle-${particleId++}`,
            x: Math.random() * building.size.width,
            y: Math.random() * building.size.height,
            opacity: 1
          });
        }
        
        // 기존 파티클 업데이트
        const updatedParticles = newParticles.map(particle => ({
          ...particle,
          y: particle.y + 2,
          opacity: particle.opacity - 0.02
        })).filter(particle => particle.opacity > 0 && particle.y < building.size.height + 50);
        
        return { ...building, particles: updatedParticles };
      }
      return building;
    }));
  };

  const updateBuildingStates = () => {
    setBuildings(prev => prev.map(building => {
      const queueVariation = (Math.random() - 0.5) * 4;
      const newQueueSize = Math.max(0, Math.min(building.maxQueueSize, building.queueSize + queueVariation));
      
      let newStatus = building.status;
      if (newQueueSize > building.maxQueueSize * 0.8) {
        newStatus = 'busy';
      } else if (newQueueSize > building.maxQueueSize * 0.3) {
        newStatus = 'working';
      } else {
        newStatus = 'idle';
      }
      
      return { ...building, queueSize: newQueueSize, status: newStatus };
    }));
  };

  const get3DTransform = (building: Building3D) => {
    const { zoom, offsetX, offsetY } = viewTransform();
    
    // 아이소메트릭 투영
    const isoX = (building.position.x - building.position.y) * 0.866;
    const isoY = (building.position.x + building.position.y) * 0.5 - building.position.z;
    
    return `translate(${isoX * zoom + offsetX}px, ${isoY * zoom + offsetY}px) scale(${zoom})`;
  };

  const getBuildingStyle = (building: Building3D) => {
    const isSelected = selectedBuilding() === building.id;
    
    return {
      transform: get3DTransform(building),
      'z-index': Math.floor(building.position.x + building.position.y),
      filter: isSelected ? 'brightness(1.2) drop-shadow(0 0 20px rgba(59, 130, 246, 0.8))' : 
              building.status === 'working' ? 'brightness(1.1)' : 'brightness(1)',
      transition: 'all 0.3s ease'
    };
  };

  const getStatusColor = (status: Building3D['status']) => {
    switch (status) {
      case 'working': return '#10B981';
      case 'busy': return '#F59E0B';
      case 'error': return '#EF4444';
      default: return '#6B7280';
    }
  };

  const handleBuildingClick = (buildingId: string) => {
    setSelectedBuilding(buildingId);
    props.onBuildingClick(buildingId);
  };

  const handleZoom = (delta: number) => {
    setViewTransform(prev => ({
      ...prev,
      zoom: Math.max(0.5, Math.min(2, prev.zoom + delta))
    }));
  };

  const handleRotate = (delta: number) => {
    setViewTransform(prev => ({
      ...prev,
      rotation: prev.rotation + delta
    }));
  };

  return (
    <div class="w-full h-full relative bg-gradient-to-br from-slate-100 to-slate-200 overflow-hidden">
      {/* 3D 뷰포트 */}
      <div class="absolute inset-0">
        <svg 
          class="w-full h-full" 
          viewBox="0 0 800 600"
          style={{ 
            background: 'radial-gradient(circle at 50% 50%, rgba(59, 130, 246, 0.1) 0%, transparent 50%)',
            perspective: '1000px'
          }}
        >
          {/* 그리드 배경 */}
          <defs>
            <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
              <path d="M 40 0 L 0 0 0 40" fill="none" stroke="rgba(0,0,0,0.1)" stroke-width="0.5"/>
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#grid)" />
          
          {/* 건물 연결선 */}
          <For each={buildings()}>
            {(building) => (
              <For each={building.connections}>
                {(connectionId) => {
                  const connectedBuilding = buildings().find(b => b.id === connectionId);
                  if (!connectedBuilding) return null;
                  
                  const startX = (building.position.x - building.position.y) * 0.866;
                  const startY = (building.position.x + building.position.y) * 0.5;
                  const endX = (connectedBuilding.position.x - connectedBuilding.position.y) * 0.866;
                  const endY = (connectedBuilding.position.x + connectedBuilding.position.y) * 0.5;
                  
                  return (
                    <line
                      x1={startX}
                      y1={startY}
                      x2={endX}
                      y2={endY}
                      stroke={props.isRunning ? '#3B82F6' : '#9CA3AF'}
                      stroke-width="2"
                      stroke-dasharray={props.isRunning ? '5,5' : 'none'}
                      opacity="0.6"
                    >
                      <Show when={props.isRunning}>
                        <animate
                          attributeName="stroke-dashoffset"
                          values="0;-10"
                          dur="1s"
                          repeatCount="indefinite"
                        />
                      </Show>
                    </line>
                  );
                }}
              </For>
            )}
          </For>
        </svg>
        
        {/* 3D 건물들 */}
        <div class="absolute inset-0 pointer-events-none">
          <For each={buildings()}>
            {(building) => (
              <div
                class="absolute cursor-pointer pointer-events-auto"
                style={getBuildingStyle(building)}
                onClick={() => handleBuildingClick(building.id)}
              >
                {/* 건물 그림자 */}
                <div
                  class="absolute bg-black opacity-20 rounded-full"
                  style={{
                    width: `${building.size.width * 1.2}px`,
                    height: `${building.size.depth * 0.6}px`,
                    transform: `translateX(-10px) translateY(${building.size.height + 10}px) skewX(-15deg)`,
                    'z-index': -1
                  }}
                />
                
                {/* 메인 건물 */}
                <div
                  class="relative transition-all duration-300 hover:scale-105"
                  style={{
                    width: `${building.size.width}px`,
                    height: `${building.size.height}px`,
                    'background-color': building.color,
                    'border-radius': '4px',
                    border: '2px solid rgba(255,255,255,0.3)',
                    'box-shadow': `inset 0 0 20px rgba(255,255,255,0.2), 0 5px 15px rgba(0,0,0,0.3)`,
                    background: `linear-gradient(135deg, ${building.color} 0%, ${building.color}dd 100%)`
                  }}
                >
                  {/* 상태 표시등 */}
                  <div
                    class="absolute top-2 right-2 w-3 h-3 rounded-full"
                    style={{
                      'background-color': getStatusColor(building.status),
                      'box-shadow': `0 0 10px ${getStatusColor(building.status)}`
                    }}
                  >
                    <Show when={building.status === 'working'}>
                      <div class="w-full h-full bg-white rounded-full animate-ping opacity-75" />
                    </Show>
                  </div>
                  
                  {/* 창문들 */}
                  <div class="absolute inset-2 grid grid-cols-3 gap-1">
                    <For each={Array.from({ length: 9 }, (_, i) => i)}>
                      {(i) => (
                        <div
                          class="bg-yellow-200 rounded-sm opacity-80"
                          style={{
                            'background-color': building.workers > i ? '#FEF3C7' : '#374151',
                            'box-shadow': 'inset 0 0 3px rgba(0,0,0,0.3)'
                          }}
                        />
                      )}
                    </For>
                  </div>
                  
                  {/* 큐 시각화 */}
                  <Show when={building.queueSize > 0}>
                    <div
                      class="absolute -top-4 left-1/2 transform -translate-x-1/2 bg-blue-500 text-white text-xs px-2 py-1 rounded"
                      style={{
                        'background-color': building.queueSize > building.maxQueueSize * 0.8 ? '#EF4444' : '#3B82F6'
                      }}
                    >
                      {building.queueSize}
                    </div>
                  </Show>
                  
                  {/* 파티클 효과 */}
                  <For each={building.particles}>
                    {(particle) => (
                      <div
                        class="absolute w-1 h-1 bg-white rounded-full"
                        style={{
                          left: `${particle.x}px`,
                          top: `${particle.y}px`,
                          opacity: particle.opacity,
                          'box-shadow': '0 0 3px rgba(255,255,255,0.8)'
                        }}
                      />
                    )}
                  </For>
                </div>
                
                {/* 건물 라벨 */}
                <div class="absolute -bottom-8 left-1/2 transform -translate-x-1/2 text-center">
                  <div class="text-xs font-bold text-gray-700 bg-white px-2 py-1 rounded shadow">
                    {building.name}
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    {building.workers}/{building.maxWorkers}
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
      
      {/* 컨트롤 패널 */}
      <div class="absolute top-4 right-4 bg-white rounded-lg shadow-lg p-4 min-w-48">
        <h3 class="font-bold text-gray-800 mb-3">🎮 View Controls</h3>
        
        <div class="space-y-2">
          <div class="flex gap-2">
            <button 
              onClick={() => handleZoom(0.1)}
              class="flex-1 px-3 py-1 bg-blue-500 text-white rounded text-sm hover:bg-blue-600"
            >
              🔍 Zoom In
            </button>
            <button 
              onClick={() => handleZoom(-0.1)}
              class="flex-1 px-3 py-1 bg-blue-500 text-white rounded text-sm hover:bg-blue-600"
            >
              🔍 Zoom Out
            </button>
          </div>
          
          <div class="flex gap-2">
            <button 
              onClick={() => handleRotate(0.1)}
              class="flex-1 px-3 py-1 bg-purple-500 text-white rounded text-sm hover:bg-purple-600"
            >
              ↻ Rotate
            </button>
            <button 
              onClick={() => setViewTransform({ rotation: 0, zoom: 1, offsetX: 0, offsetY: 0 })}
              class="flex-1 px-3 py-1 bg-gray-500 text-white rounded text-sm hover:bg-gray-600"
            >
              🏠 Reset
            </button>
          </div>
        </div>
      </div>
      
      {/* 건물 상세 정보 */}
      <Show when={selectedBuilding()}>
        {(buildingId) => {
          const building = buildings().find(b => b.id === buildingId());
          if (!building) return null;
          
          return (
            <div class="absolute bottom-4 left-4 bg-white rounded-lg shadow-lg p-4 min-w-64">
              <div class="flex justify-between items-start mb-3">
                <h3 class="font-bold text-gray-800">{building.name}</h3>
                <button 
                  onClick={() => setSelectedBuilding(null)}
                  class="text-gray-400 hover:text-gray-600"
                >
                  ✕
                </button>
              </div>
              
              <div class="space-y-2 text-sm">
                <div class="flex justify-between">
                  <span class="text-gray-600">Workers:</span>
                  <span class="font-medium">{building.workers}/{building.maxWorkers}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-600">Queue:</span>
                  <span class="font-medium">{building.queueSize}/{building.maxQueueSize}</span>
                </div>
                <div class="flex justify-between">
                  <span class="text-gray-600">Status:</span>
                  <span class={`font-medium ${
                    building.status === 'working' ? 'text-green-600' :
                    building.status === 'busy' ? 'text-yellow-600' :
                    building.status === 'error' ? 'text-red-600' : 'text-gray-600'
                  }`}>
                    {building.status.toUpperCase()}
                  </span>
                </div>
                
                {/* 큐 상태 바 */}
                <div class="mt-3">
                  <div class="text-gray-600 text-xs mb-1">Queue Status</div>
                  <div class="w-full bg-gray-200 rounded-full h-2">
                    <div
                      class={`h-2 rounded-full transition-all duration-500 ${
                        building.queueSize > building.maxQueueSize * 0.8 ? 'bg-red-500' :
                        building.queueSize > building.maxQueueSize * 0.5 ? 'bg-yellow-500' : 'bg-green-500'
                      }`}
                      style={{ width: `${(building.queueSize / building.maxQueueSize) * 100}%` }}
                    />
                  </div>
                </div>
              </div>
            </div>
          );
        }}
      </Show>
    </div>
  );
};
