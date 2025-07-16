/**
 * CrawlingProgressDashboard.tsx
 * @description 완전한 크롤링 시뮬레이션 - 배치 완료 후 DB 실린더 애니메이션 포함
 *              Start Simulation 버튼으로 전체 과정 시연 가능
 *              실제 크롤링 이벤트와 연결 가능
 */
import { Component, onMount, onCleanup, createSignal } from 'solid-js';
import { listen } from '@tauri-apps/api/event';

// D3.js will be loaded via CDN in index.html
declare const d3: any;

interface CrawlingEventData {
  batchId: string;
  pageId?: string;
  productId?: string;
  status: 'created' | 'processing' | 'completed' | 'failed';
  data?: any;
}

const CrawlingProgressDashboard: Component = () => {
  // 시뮬레이션 상태 관리
  const [isSimulationRunning, setIsSimulationRunning] = createSignal(false);
  const [isLiveMode, setIsLiveMode] = createSignal(false);
  const [currentPhase, setCurrentPhase] = createSignal('');
  const [processedBatches, setProcessedBatches] = createSignal(0);
  const [simulationProgress, setSimulationProgress] = createSignal(0);

  // 실제 크롤링 데이터 관리
  const [liveBatches, setLiveBatches] = createSignal<Map<string, any>>(new Map());
  const [livePages, setLivePages] = createSignal<Map<string, any>>(new Map());
  const [liveProducts, setLiveProducts] = createSignal<Map<string, any>>(new Map());

  // 애니메이션 변수
  let svg: any = null;
  let simulationTimeouts: any[] = [];
  let dbCylinders: any[] = [];
  let currentBatchIndex = 0;
  let maxBatches = 5;

  // 시뮬레이션 정리 함수
  const cleanupSimulation = () => {
    simulationTimeouts.forEach(timeout => clearTimeout(timeout));
    simulationTimeouts = [];
    currentBatchIndex = 0;
    setIsSimulationRunning(false);
    setCurrentPhase('');
    setProcessedBatches(0);
    setSimulationProgress(0);
  };

  // 1. 배치 생성 애니메이션 - 푱푱푱 생성 후 쌓이기
  const animateBatchCreation = () => {
    setCurrentPhase('배치 생성 - 푱푱푱');
    
    const batchGroup = svg.append('g').attr('class', 'batch-creation');
    const batchCounts = [3, 4, 5, 3, 4]; // 각 배치별 아이템 수
    
    batchCounts.forEach((count, batchIndex) => {
      simulationTimeouts.push(setTimeout(() => {
        // 배치들을 세로로 쌓아서 배치 (대기 상태)
        const stackY = 150 + batchIndex * 35; // 35px 간격으로 세로 배치
        
        const batch = batchGroup.append('circle')
          .attr('r', 0)
          .attr('cx', 150)
          .attr('cy', stackY)
          .attr('fill', '#3b82f6')
          .attr('opacity', 0.8)
          .attr('class', `batch-${batchIndex}`)
          .attr('data-original-y', stackY); // 원래 위치 저장
        
        // 푱푱푱 효과 - 작은 원들이 생성되면서 합쳐짐
        const sparkles = d3.range(count).map(() => ({
          x: 150 + Math.random() * 60 - 30,
          y: stackY + Math.random() * 60 - 30
        }));
        
        sparkles.forEach((sparkle: any, i: number) => {
          simulationTimeouts.push(setTimeout(() => {
            batchGroup.append('circle')
              .attr('r', 0)
              .attr('cx', sparkle.x)
              .attr('cy', sparkle.y)
              .attr('fill', '#60a5fa')
              .transition()
              .duration(300)
              .attr('r', 4)
              .transition()
              .duration(300)
              .attr('cx', 150)
              .attr('cy', stackY)
              .attr('r', 0)
              .remove();
          }, i * 100));
        });
        
        // 메인 배치 원 생성 (쌓인 형태)
        batch.transition()
          .duration(600)
          .attr('r', 20) // 조금 더 작게
          .attr('fill', '#1e40af');
          
        // 배치 라벨을 배치와 함께 움직이도록 그룹에 추가
        batchGroup.append('text')
          .attr('x', 150)
          .attr('y', stackY + 4)
          .attr('text-anchor', 'middle')
          .attr('fill', 'white')
          .attr('font-size', '10px')
          .attr('class', `batch-label-${batchIndex}`)
          .text(`B${batchIndex + 1}`);
          
      }, batchIndex * 800));
    });
    
    simulationTimeouts.push(setTimeout(() => {
      activateFirstBatch();
    }, 5000));
  };

  // 2. 첫 번째 배치 활성화 - 퓨우웅 튀어오름 (작업 위치로 이동)
  const activateFirstBatch = () => {
    setCurrentPhase('배치 활성화 - 퓨우웅');
    
    const firstBatch = svg.select('.batch-0');
    const firstBatchLabel = svg.select('.batch-label-0');
    const workingY = 100; // 작업 위치
    
    // 배치와 라벨을 함께 이동
    firstBatch
      .transition()
      .duration(500)
      .attr('cy', workingY - 30) // 위로 튀어오름
      .attr('r', 35)
      .attr('fill', '#f59e0b')
      .transition()
      .duration(300)
      .attr('cy', workingY) // 작업 위치로 정착
      .attr('r', 30)
      .attr('fill', '#d97706');
    
    // 라벨도 함께 이동
    firstBatchLabel
      .transition()
      .duration(500)
      .attr('y', workingY - 30 + 4)
      .transition()
      .duration(300)
      .attr('y', workingY + 4);
    
    // 활성화 효과 - 주변에 원형 파동
    const ripples = d3.range(3);
    ripples.forEach((_: any, i: number) => {
      simulationTimeouts.push(setTimeout(() => {
        svg.append('circle')
          .attr('cx', 150)
          .attr('cy', workingY)
          .attr('r', 30)
          .attr('fill', 'none')
          .attr('stroke', '#f59e0b')
          .attr('stroke-width', 2)
          .attr('opacity', 0.8)
          .transition()
          .duration(1000)
          .attr('r', 80)
          .attr('opacity', 0)
          .remove();
      }, i * 200));
    });
    
    simulationTimeouts.push(setTimeout(() => {
      animatePageEmission();
    }, 2000));
  };

  // 3. 페이지 방출 애니메이션 - 휙휙 concurrent 방출
  const animatePageEmission = () => {
    setCurrentPhase('페이지 방출 - 휙휙 concurrent');
    
    const pageCount = 8;
    const pageGroup = svg.append('g').attr('class', 'pages');
    
    // 휙휙 concurrent 방출 효과
    d3.range(pageCount).forEach((_: any, i: number) => {
      simulationTimeouts.push(setTimeout(() => {
        const angle = (i / pageCount) * Math.PI * 2;
        const targetX = 400 + Math.cos(angle) * 100;
        const targetY = 200 + Math.sin(angle) * 100;
        
        const page = pageGroup.append('rect')
          .attr('x', 150)
          .attr('y', 100)
          .attr('width', 0)
          .attr('height', 0)
          .attr('fill', '#10b981')
          .attr('rx', 2)
          .attr('class', `page-${i}`);
        
        // 빠른 확대 후 이동
        page.transition()
          .duration(200)
          .attr('width', 12)
          .attr('height', 16)
          .attr('x', 144)
          .attr('y', 92)
          .transition()
          .duration(800)
          .attr('x', targetX - 6)
          .attr('y', targetY - 8)
          .attr('fill', '#059669');
          
        // 방출 효과선
        svg.append('line')
          .attr('x1', 150)
          .attr('y1', 100)
          .attr('x2', 150)
          .attr('y2', 100)
          .attr('stroke', '#10b981')
          .attr('stroke-width', 2)
          .attr('opacity', 0.8)
          .transition()
          .duration(300)
          .attr('x2', targetX)
          .attr('y2', targetY)
          .transition()
          .duration(200)
          .attr('opacity', 0)
          .remove();
          
      }, i * 100));
    });
    
    simulationTimeouts.push(setTimeout(() => {
      animatePageCollection();
    }, 2500));
  };

  // 4. 페이지 수집 애니메이션 - 부유 효과 추가
  const animatePageCollection = () => {
    setCurrentPhase('페이지 수집 중 - 부유');
    
    const pages = svg.selectAll('.pages rect');
    
    // 부유 효과 - 더 큰 움직임으로 수집 중임을 표현
    pages.each(function(this: any) {
      const page = d3.select(this);
      const originalX = parseFloat(page.attr('x'));
      const originalY = parseFloat(page.attr('y'));
      
      // 부유 애니메이션 반복
      const floatAnimation = () => {
        page.transition()
          .duration(800)
          .attr('x', originalX + Math.random() * 40 - 20)
          .attr('y', originalY + Math.random() * 40 - 20)
          .attr('fill', '#6366f1')
          .transition()
          .duration(800)
          .attr('x', originalX + Math.random() * 40 - 20)
          .attr('y', originalY + Math.random() * 40 - 20)
          .attr('fill', '#4f46e5')
          .on('end', floatAnimation);
      };
      
      floatAnimation();
    });
    
    // 수집 완료 후 다음 단계
    simulationTimeouts.push(setTimeout(() => {
      // 부유 애니메이션 중지
      pages.interrupt();
      pages.transition()
        .duration(500)
        .attr('fill', '#10b981')
        .attr('stroke', '#059669')
        .attr('stroke-width', 2);
      
      animatePageFall();
    }, 3000));
  };

  // 5. 페이지 낙하 애니메이션 - 투두둑 떨어짐
  const animatePageFall = () => {
    setCurrentPhase('페이지 낙하 - 투두둑');
    
    const pages = svg.selectAll('.pages rect');
    const pageY = 400;
    
    pages.nodes().forEach((pageNode: any, i: number) => {
      const page = d3.select(pageNode);
      
      simulationTimeouts.push(setTimeout(() => {
        page.transition()
          .duration(600)
          .attr('y', pageY + Math.random() * 40 - 20)
          .attr('fill', '#ef4444')
          .transition()
          .duration(200)
          .attr('y', pageY)
          .attr('fill', '#dc2626');
          
        // 충돌 효과
        svg.append('circle')
          .attr('cx', parseInt(page.attr('x')) + 6)
          .attr('cy', pageY + 8)
          .attr('r', 0)
          .attr('fill', '#fbbf24')
          .attr('opacity', 0.8)
          .transition()
          .duration(300)
          .attr('r', 15)
          .attr('opacity', 0)
          .remove();
          
      }, i * 80));
    });
    
    simulationTimeouts.push(setTimeout(() => {
      animateProductEmission();
    }, 2000));
  };

  // 6. 제품 방출 애니메이션 - 페이지 분해 및 12개 제품 생성
  const animateProductEmission = () => {
    setCurrentPhase('제품 분해 - 페이지 → 12개 제품');
    
    const productGroup = svg.append('g').attr('class', 'products');
    const pages = svg.selectAll('.pages rect');
    const failedPages: any[] = [];
    
    pages.nodes().forEach((pageNode: any, i: number) => {
      const page = d3.select(pageNode);
      const pageX = parseFloat(page.attr('x'));
      const pageY = parseFloat(page.attr('y'));
      
      simulationTimeouts.push(setTimeout(() => {
        // 페이지 분해 효과
        page.transition()
          .duration(300)
          .attr('fill', '#fbbf24')
          .attr('transform', `scale(1.2)`)
          .transition()
          .duration(200)
          .attr('fill', '#f59e0b')
          .attr('transform', `scale(0.8)`)
          .on('end', () => {
            // 페이지 제거
            page.remove();
            
            // 성공/실패 결정 (80% 성공률)
            const isSuccess = Math.random() > 0.2;
            const productCount = isSuccess ? 12 : Math.floor(Math.random() * 8) + 4;
            
            if (isSuccess) {
              // 성공 - 12개 제품 생성
              for (let j = 0; j < productCount; j++) {
                const angle = (j / productCount) * Math.PI * 2;
                const radius = 30;
                const targetX = pageX + Math.cos(angle) * radius;
                const targetY = pageY + Math.sin(angle) * radius;
                
                const product = productGroup.append('circle')
                  .attr('cx', pageX + 6)
                  .attr('cy', pageY + 8)
                  .attr('r', 0)
                  .attr('fill', '#8b5cf6')
                  .attr('class', `product-${i}-${j}`)
                  .attr('data-status', 'processing');
                
                product.transition()
                  .delay(j * 50)
                  .duration(400)
                  .attr('r', 3)
                  .attr('cx', targetX)
                  .attr('cy', targetY)
                  .attr('fill', '#7c3aed')
                  .transition()
                  .delay(500)
                  .duration(600)
                  .attr('fill', Math.random() > 0.1 ? '#10b981' : '#ef4444') // 90% 성공률
                  .attr('data-status', Math.random() > 0.1 ? 'success' : 'failed');
              }
            } else {
              // 실패 - 페이지 전체 실패 처리
              const failedPage = productGroup.append('rect')
                .attr('x', pageX)
                .attr('y', pageY)
                .attr('width', 12)
                .attr('height', 16)
                .attr('fill', '#ef4444')
                .attr('rx', 2)
                .attr('class', `failed-page-${i}`)
                .attr('data-status', 'failed');
              
              failedPages.push(failedPage);
              
              // 실패 페이지를 하단 왼쪽으로 이동
              setTimeout(() => {
                failedPage.transition()
                  .duration(1000)
                  .attr('x', 50 + failedPages.length * 20)
                  .attr('y', 550)
                  .attr('fill', '#dc2626');
              }, 1000);
            }
          });
      }, i * 150));
    });
    
    simulationTimeouts.push(setTimeout(() => {
      animateProductCollection();
    }, 3000));
  };

  // 7. 제품 수집 애니메이션 - 상태별 색상 처리 후 DB 흡수
  const animateProductCollection = () => {
    setCurrentPhase('제품 수집 - 상태별 정리');
    
    const successProducts = svg.selectAll('.products circle[data-status="success"]');
    const failedProducts = svg.selectAll('.products circle[data-status="failed"]');
    
    // 성공한 제품들 - 파란색으로 수집
    successProducts.transition()
      .duration(1000)
      .attr('cy', 450)
      .attr('fill', '#06b6d4')
      .attr('r', 4);
    
    // 실패한 제품들 - 빨간색으로 하단 왼쪽 이동
    failedProducts.transition()
      .duration(1000)
      .attr('cx', (_: any, i: number) => 80 + i * 15)
      .attr('cy', 560)
      .attr('fill', '#dc2626')
      .attr('r', 3);
    
    // 성공한 제품들을 DB 실린더로 흡수하는 애니메이션
    simulationTimeouts.push(setTimeout(() => {
      setCurrentPhase('제품 DB 흡수 - 부스스');
      
      const batchX = 150;
      const batchY = 100;
      
      // 성공한 제품들을 배치 위치로 이동하며 흡수
      successProducts.transition()
        .duration(800)
        .attr('cx', batchX)
        .attr('cy', batchY)
        .attr('r', 2)
        .attr('opacity', 0.5)
        .transition()
        .duration(400)
        .attr('r', 0)
        .attr('opacity', 0)
        .remove();
      
      // 흡수 효과 - 부스스 파티클
      successProducts.nodes().forEach((productNode: any, i: number) => {
        const product = d3.select(productNode);
        const startX = parseFloat(product.attr('cx'));
        const startY = parseFloat(product.attr('cy'));
        
        simulationTimeouts.push(setTimeout(() => {
          // 작은 파티클들 생성
          for (let j = 0; j < 3; j++) {
            svg.append('circle')
              .attr('cx', startX + Math.random() * 10 - 5)
              .attr('cy', startY + Math.random() * 10 - 5)
              .attr('r', 1)
              .attr('fill', '#06b6d4')
              .attr('opacity', 0.8)
              .transition()
              .duration(600)
              .attr('cx', batchX)
              .attr('cy', batchY)
              .attr('r', 0)
              .attr('opacity', 0)
              .remove();
          }
        }, i * 20));
      });
      
      animateBatchToDbCylinder();
    }, 1500));
  };

  // 8. 배치를 DB 실린더로 변환하는 애니메이션 - 누적 성장 효과
  const animateBatchToDbCylinder = () => {
    setCurrentPhase('DB 저장 - 실린더 누적');
    
    const activeBatch = svg.select('.batch-0');
    const activeBatchLabel = svg.select('.batch-label-0');
    const batchX = 150;
    const batchY = 100;
    
    // 새로운 배치 실린더 높이
    const newCylinderHeight = 20;
    
    // 저장소 위치
    const storageX = 700;
    const storageY = 500;
    
    // 배치를 실린더로 변형
    const cylinder = svg.append('g').attr('class', `db-cylinder-${currentBatchIndex}`);
    
    // 실린더 상단 타원 (납작하게)
    const topEllipse = cylinder.append('ellipse')
      .attr('cx', batchX)
      .attr('cy', batchY)
      .attr('rx', 30)
      .attr('ry', 6)
      .attr('fill', '#1e40af')
      .attr('opacity', 0);
    
    // 실린더 몸체 (납작하게)
    const cylinderBody = cylinder.append('rect')
      .attr('x', batchX - 30)
      .attr('y', batchY)
      .attr('width', 60)
      .attr('height', 0)
      .attr('fill', '#1e40af')
      .attr('opacity', 0);
    
    // 실린더 하단 타원 (납작하게)
    const bottomEllipse = cylinder.append('ellipse')
      .attr('cx', batchX)
      .attr('cy', batchY)
      .attr('rx', 30)
      .attr('ry', 6)
      .attr('fill', '#1e3a8a')
      .attr('opacity', 0);
    
    // 배치 번호 라벨을 실린더에 추가
    const cylinderLabel = cylinder.append('text')
      .attr('x', batchX)
      .attr('y', batchY + newCylinderHeight/2)
      .attr('text-anchor', 'middle')
      .attr('fill', 'white')
      .attr('font-size', '10px')
      .text(`B${currentBatchIndex + 1}`)
      .attr('opacity', 0);
    
    // 배치와 기존 라벨 숨기기
    activeBatch.transition()
      .duration(500)
      .attr('opacity', 0);
    
    activeBatchLabel.transition()
      .duration(500)
      .attr('opacity', 0);
    
    // 실린더 생성 애니메이션
    topEllipse.transition()
      .duration(500)
      .attr('opacity', 1);
    
    cylinderBody.transition()
      .duration(800)
      .attr('height', newCylinderHeight)
      .attr('opacity', 1);
    
    bottomEllipse.transition()
      .delay(300)
      .duration(500)
      .attr('cy', batchY + newCylinderHeight)
      .attr('opacity', 1);
    
    // 실린더 라벨 표시
    cylinderLabel.transition()
      .delay(800)
      .duration(300)
      .attr('opacity', 1);
    
    simulationTimeouts.push(setTimeout(() => {
      // 저장소로 이동하면서 기존 실린더 위에 쌓기
      const stackedHeight = currentBatchIndex * newCylinderHeight;
      const targetY = storageY - stackedHeight - newCylinderHeight;
      
      // 새 실린더를 저장소로 이동
      cylinder.transition()
        .duration(1500)
        .attr('transform', `translate(${storageX - batchX}, ${targetY - batchY})`)
        .on('end', () => {
          dbCylinders.push(cylinder);
          
          // 전체 DB 라벨 업데이트
          svg.select('.db-total-label').remove();
          svg.append('text')
            .attr('class', 'db-total-label')
            .attr('x', storageX)
            .attr('y', targetY - 15)
            .attr('text-anchor', 'middle')
            .attr('fill', '#374151')
            .attr('font-size', '12px')
            .attr('font-weight', 'bold')
            .text(`DB Stack (${currentBatchIndex + 1}/${maxBatches})`);
          
          // 성공 효과
          svg.append('circle')
            .attr('cx', storageX)
            .attr('cy', targetY + newCylinderHeight/2)
            .attr('r', 0)
            .attr('fill', 'none')
            .attr('stroke', '#10b981')
            .attr('stroke-width', 2)
            .attr('opacity', 0.8)
            .transition()
            .duration(600)
            .attr('r', 50)
            .attr('opacity', 0)
            .remove();
          
          processNextBatch();
        });
    }, 1000));
  };

  // 다음 배치 처리
  const processNextBatch = () => {
    currentBatchIndex++;
    setProcessedBatches(currentBatchIndex);
    setSimulationProgress((currentBatchIndex / maxBatches) * 100);
    
    if (currentBatchIndex < maxBatches) {
      // 다음 배치 활성화 - 올바른 배치 번호 사용
      simulationTimeouts.push(setTimeout(() => {
        // 기존 활성 배치와 라벨 제거
        svg.selectAll('.batch-0').remove();
        svg.selectAll('.batch-label-0').remove();
        
        // 다음 배치를 활성 배치로 설정
        const nextBatch = svg.select(`.batch-${currentBatchIndex}`);
        const nextBatchLabel = svg.select(`.batch-label-${currentBatchIndex}`);
        
        if (nextBatch.node()) {
          nextBatch.classed('batch-0', true);
          nextBatch.classed(`batch-${currentBatchIndex}`, false);
          
          nextBatchLabel.classed('batch-label-0', true);
          nextBatchLabel.classed(`batch-label-${currentBatchIndex}`, false);
          
          activateFirstBatch();
        }
      }, 1000));
    } else {
      completeSimulation();
    }
  };

  // 시뮬레이션 완료
  const completeSimulation = () => {
    setCurrentPhase('시뮬레이션 완료');
    setIsSimulationRunning(false);
    
    // 완료 효과
    svg.append('text')
      .attr('x', 400)
      .attr('y', 100)
      .attr('text-anchor', 'middle')
      .attr('fill', '#10b981')
      .attr('font-size', '24px')
      .attr('font-weight', 'bold')
      .text('시뮬레이션 완료!')
      .attr('opacity', 0)
      .transition()
      .duration(800)
      .attr('opacity', 1);
  };

  // SVG 초기화
  const initializeSVG = () => {
    const container = document.getElementById('crawling-viz');
    if (!container) return;
    
    svg = d3.select(container)
      .append('svg')
      .attr('width', 800)
      .attr('height', 600)
      .style('background', 'linear-gradient(135deg, #f3f4f6 0%, #e5e7eb 100%)')
      .style('border-radius', '8px');
    
    // 배경 그리드
    const defs = svg.append('defs');
    const pattern = defs.append('pattern')
      .attr('id', 'grid')
      .attr('width', 20)
      .attr('height', 20)
      .attr('patternUnits', 'userSpaceOnUse');
    
    pattern.append('path')
      .attr('d', 'M 20 0 L 0 0 0 20')
      .attr('fill', 'none')
      .attr('stroke', '#d1d5db')
      .attr('stroke-width', 0.5)
      .attr('opacity', 0.3);
    
    svg.append('rect')
      .attr('width', '100%')
      .attr('height', '100%')
      .attr('fill', 'url(#grid)');
    
    // 영역 라벨
    svg.append('text')
      .attr('x', 150)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('배치 생성 영역');
    
    svg.append('text')
      .attr('x', 400)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('페이지 처리 영역');
    
    svg.append('text')
      .attr('x', 700)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('DB 저장소');
  };

  // 시뮬레이션 시작
  const startSimulation = () => {
    if (isSimulationRunning()) return;
    
    setIsSimulationRunning(true);
    setCurrentPhase('시뮬레이션 시작');
    setProcessedBatches(0);
    setSimulationProgress(0);
    
    // 기존 내용 지우기
    if (svg) {
      svg.selectAll('g').remove();
      svg.selectAll('text').remove();
      svg.selectAll('circle').remove();
      svg.selectAll('rect').remove();
      svg.selectAll('line').remove();
    }
    
    // 변수 초기화
    currentBatchIndex = 0;
    dbCylinders = [];
    
    // 영역 라벨 다시 추가
    svg.append('text')
      .attr('x', 150)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('배치 생성 영역');
    
    svg.append('text')
      .attr('x', 400)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('페이지 처리 영역');
    
    svg.append('text')
      .attr('x', 700)
      .attr('y', 30)
      .attr('text-anchor', 'middle')
      .attr('font-size', '14px')
      .attr('font-weight', 'bold')
      .attr('fill', '#374151')
      .text('DB 저장소');
    
    // 시뮬레이션 시작
    animateBatchCreation();
  };

  // 시뮬레이션 중지
  const stopSimulation = () => {
    cleanupSimulation();
    if (svg) {
      svg.selectAll('*').remove();
    }
    initializeSVG();
  };

  // 실제 이벤트 리스너 (향후 백엔드 연결용)
  const setupEventListeners = async () => {
    try {
      // 실제 크롤링 이벤트 리스너
      await listen('batch-created', (event: any) => {
        console.log('실제 배치 생성됨:', event.payload);
        handleLiveBatchCreated(event.payload);
      });
      
      await listen('page-crawled', (event: any) => {
        console.log('실제 페이지 크롤링됨:', event.payload);
        handleLivePageCrawled(event.payload);
      });
      
      await listen('product-collected', (event: any) => {
        console.log('실제 제품 수집됨:', event.payload);
        handleLiveProductCollected(event.payload);
      });
      
      await listen('batch-completed', (event: any) => {
        console.log('실제 배치 완료됨:', event.payload);
        handleLiveBatchCompleted(event.payload);
      });
      
    } catch (error) {
      console.error('이벤트 리스너 설정 실패:', error);
    }
  };

  // 실제 크롤링 이벤트 처리 함수들
  const handleLiveBatchCreated = (data: CrawlingEventData) => {
    setLiveBatches(prev => {
      const updated = new Map(prev);
      updated.set(data.batchId, { ...data, createdAt: new Date() });
      return updated;
    });
    
    if (isLiveMode()) {
      // 실제 배치 생성 시각화
      animateLiveBatchCreation(data);
    }
  };

  const handleLivePageCrawled = (data: CrawlingEventData) => {
    setLivePages(prev => {
      const updated = new Map(prev);
      updated.set(data.pageId || '', { ...data, crawledAt: new Date() });
      return updated;
    });
    
    if (isLiveMode()) {
      // 실제 페이지 크롤링 시각화
      animateLivePageCrawling(data);
    }
  };

  const handleLiveProductCollected = (data: CrawlingEventData) => {
    setLiveProducts(prev => {
      const updated = new Map(prev);
      updated.set(data.productId || '', { ...data, collectedAt: new Date() });
      return updated;
    });
    
    if (isLiveMode()) {
      // 실제 제품 수집 시각화
      animateLiveProductCollection(data);
    }
  };

  const handleLiveBatchCompleted = (data: CrawlingEventData) => {
    if (isLiveMode()) {
      // 실제 배치 완료 시각화
      animateLiveBatchCompletion(data);
    }
  };

  // 실제 크롤링 애니메이션 함수들
  const animateLiveBatchCreation = (data: CrawlingEventData) => {
    // 실제 배치 생성 애니메이션
    svg.append('circle')
      .attr('cx', 150)
      .attr('cy', 150)
      .attr('r', 0)
      .attr('fill', '#3b82f6')
      .attr('class', `live-batch-${data.batchId}`)
      .transition()
      .duration(500)
      .attr('r', 25)
      .attr('fill', '#1e40af');
  };

  const animateLivePageCrawling = (data: CrawlingEventData) => {
    // 실제 페이지 크롤링 애니메이션
    svg.append('rect')
      .attr('x', 400)
      .attr('y', 200)
      .attr('width', 0)
      .attr('height', 0)
      .attr('fill', '#10b981')
      .attr('class', `live-page-${data.pageId}`)
      .transition()
      .duration(300)
      .attr('width', 12)
      .attr('height', 16)
      .attr('fill', '#059669');
  };

  const animateLiveProductCollection = (data: CrawlingEventData) => {
    // 실제 제품 수집 애니메이션
    svg.append('circle')
      .attr('cx', 400)
      .attr('cy', 450)
      .attr('r', 0)
      .attr('fill', '#8b5cf6')
      .attr('class', `live-product-${data.productId}`)
      .transition()
      .duration(300)
      .attr('r', 4)
      .attr('fill', data.status === 'completed' ? '#10b981' : '#ef4444');
  };

  const animateLiveBatchCompletion = (data: CrawlingEventData) => {
    // 실제 배치 완료 애니메이션 - DB 실린더로 변환
    const batchElement = svg.select(`.live-batch-${data.batchId}`);
    
    if (batchElement.node()) {
      // 배치를 DB 실린더로 변환하는 애니메이션 실행
      animateBatchToDbCylinder();
    }
  };

  // 라이브 모드 토글과 컴포넌트 초기화
  const toggleLiveMode = () => {
    setIsLiveMode(!isLiveMode());
    if (isLiveMode()) {
      setCurrentPhase('라이브 모드 활성화');
      // 기존 시뮬레이션 정리
      cleanupSimulation();
    } else {
      setCurrentPhase('시뮬레이션 모드');
    }
  };

  // 컴포넌트 초기화 시 이벤트 리스너 설정
  onMount(() => {
    setupEventListeners();
  });

  // 컴포넌트 언마운트 시 정리
  onCleanup(() => {
    cleanupSimulation();
  });

  // 컴포넌트 마운트
  onMount(() => {
    initializeSVG();
    setupEventListeners();
  });

  // 컴포넌트 언마운트
  onCleanup(() => {
    cleanupSimulation();
  });

  return (
    <div class="crawling-progress-dashboard">
      <div class="dashboard-header">
        <h2 class="text-2xl font-bold text-gray-800 mb-4">
          크롤링 진행 상황 대시보드
        </h2>
        
        <div class="simulation-controls mb-4">
          <button
            onClick={startSimulation}
            disabled={isSimulationRunning()}
            class="px-6 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:bg-gray-400 disabled:cursor-not-allowed mr-2"
          >
            {isSimulationRunning() ? '시뮬레이션 실행 중...' : 'Start Simulation'}
          </button>
          
          <button
            onClick={stopSimulation}
            class="px-6 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600"
          >
            Stop Simulation
          </button>
        </div>
        
        <div class="simulation-status mb-4">
          <div class="text-sm text-gray-600">
            현재 단계: <span class="font-semibold text-blue-600">{currentPhase()}</span>
          </div>
          <div class="text-sm text-gray-600">
            처리된 배치: <span class="font-semibold text-green-600">{processedBatches()}</span> / {maxBatches}
          </div>
          <div class="text-sm text-gray-600">
            진행률: <span class="font-semibold text-purple-600">{simulationProgress().toFixed(1)}%</span>
          </div>
        </div>
      </div>
      
      <div id="crawling-viz" class="crawling-visualization"></div>
    </div>
  );
};

export default CrawlingProgressDashboard;
