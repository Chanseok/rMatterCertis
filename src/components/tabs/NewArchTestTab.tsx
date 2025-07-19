/**
 * NewArchTestTab - 새로운 아키텍처 테스트 탭
 * Modern Rust 2024 Actor 시스템 검증을 위한 UI 컴포넌트
 */

import { Component, createSignal, createResource, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import './NewArchTestTab.css';

interface TestResult {
  success: boolean;
  message: string;
  details?: any;
  execution_time_ms: number;
}

interface SystemConfigInfo {
  session_timeout_secs: number;
  max_concurrent_sessions: number;
  control_buffer_size: number;
  event_buffer_size: number;
  batch_initial_size: number;
  max_concurrent_tasks: number;
}

export const NewArchTestTab: Component = () => {
  const [configInfo, setConfigInfo] = createSignal<SystemConfigInfo | null>(null);
  const [testResults, setTestResults] = createSignal<{ [key: string]: TestResult }>({});
  const [loading, setLoading] = createSignal<string | null>(null);

  const runTest = async (testName: string, command: string) => {
    setLoading(testName);
    try {
      const result = await invoke<TestResult>(command);
      setTestResults(prev => ({ ...prev, [testName]: result }));
    } catch (error) {
      setTestResults(prev => ({
        ...prev,
        [testName]: {
          success: false,
          message: `테스트 실행 실패: ${error}`,
          execution_time_ms: 0
        }
      }));
    } finally {
      setLoading(null);
    }
  };

  const loadConfig = async () => {
    setLoading('config');
    try {
      const config = await invoke<SystemConfigInfo>('get_new_arch_config');
      setConfigInfo(config);
    } catch (error) {
      console.error('설정 로드 실패:', error);
    } finally {
      setLoading(null);
    }
  };

  const runAllTests = async () => {
    await loadConfig();
    await runTest('channels', 'test_new_arch_channels');
    await runTest('session_actor', 'test_new_arch_session_actor');
    await runTest('batch_actor', 'test_new_arch_batch_actor');
    await runTest('integration', 'test_new_arch_integration');
    await runTest('performance', 'test_new_arch_performance');
  };

  const testButtons = [
    { key: 'channels', label: '채널 통신 테스트', command: 'test_new_arch_channels' },
    { key: 'session_actor', label: 'SessionActor 테스트', command: 'test_new_arch_session_actor' },
    { key: 'batch_actor', label: 'BatchActor 테스트', command: 'test_new_arch_batch_actor' },
    { key: 'integration', label: '통합 테스트', command: 'test_new_arch_integration' },
    { key: 'performance', label: '성능 테스트', command: 'test_new_arch_performance' }
  ];

  const ResultCard: Component<{ title: string; result?: TestResult }> = (props) => (
    <div class={`result-card ${props.result?.success ? 'success' : props.result ? 'failure' : 'pending'}`}>
      <h3>{props.title}</h3>
      {props.result ? (
        <>
          <div class={`status ${props.result.success ? 'success' : 'failure'}`}>
            {props.result.success ? '✅ 성공' : '❌ 실패'}
          </div>
          <p class="message">{props.result.message}</p>
          <div class="execution-time">실행 시간: {props.result.execution_time_ms}ms</div>
          {props.result.details && (
            <details class="details">
              <summary>상세 정보</summary>
              <pre>{JSON.stringify(props.result.details, null, 2)}</pre>
            </details>
          )}
        </>
      ) : (
        <div class="status pending">⏳ 대기중</div>
      )}
    </div>
  );

  const configItems = () => {
    const config = configInfo();
    if (!config) return [];
    return [
      { label: '세션 타임아웃', value: `${config.session_timeout_secs}초` },
      { label: '최대 동시 세션', value: `${config.max_concurrent_sessions}개` },
      { label: 'Control 버퍼 크기', value: config.control_buffer_size },
      { label: 'Event 버퍼 크기', value: config.event_buffer_size },
      { label: '초기 배치 크기', value: config.batch_initial_size },
      { label: '최대 동시 작업', value: `${config.max_concurrent_tasks}개` }
    ];
  };

  const testResultsList = [
    { key: 'channels', title: '채널 통신' },
    { key: 'session_actor', title: 'SessionActor' },
    { key: 'batch_actor', title: 'BatchActor' },
    { key: 'integration', title: '통합 테스트' },
    { key: 'performance', title: '성능 테스트' }
  ];

  const getTestSummary = () => {
    const results = testResults();
    const total = Object.keys(results).length;
    const success = Object.values(results).filter(r => r.success).length;
    const failure = Object.values(results).filter(r => !r.success).length;
    const avgTime = total > 0 
      ? Math.round(Object.values(results).reduce((sum, r) => sum + r.execution_time_ms, 0) / total)
      : 0;
    
    return { total, success, failure, avgTime };
  };

  return (
    <div class="new-arch-test">
      <div class="header">
        <h1>🏗️ 새로운 아키텍처 테스트</h1>
        <p>Modern Rust 2024 Actor 시스템 검증</p>
      </div>

      <div class="controls">
        <button 
          onClick={loadConfig} 
          disabled={loading() === 'config'}
          class="config-btn"
        >
          {loading() === 'config' ? '로딩 중...' : '설정 정보 로드'}
        </button>
        <button 
          onClick={runAllTests} 
          disabled={loading() !== null}
          class="run-all-btn"
        >
          {loading() ? '테스트 실행 중...' : '전체 테스트 실행'}
        </button>
      </div>

      {configInfo() && (
        <div class="config-info">
          <h2>📋 시스템 설정 정보</h2>
          <div class="config-grid">
            <For each={configItems()}>
              {(item) => (
                <div class="config-item">
                  <label>{item.label}</label>
                  <span>{item.value}</span>
                </div>
              )}
            </For>
          </div>
        </div>
      )}

      <div class="test-grid">
        <div class="test-section">
          <h2>🔧 개별 테스트</h2>
          <div class="individual-tests">
            <For each={testButtons}>
              {(test) => (
                <button
                  onClick={() => runTest(test.key, test.command)}
                  disabled={loading() === test.key}
                  class="test-btn"
                >
                  {loading() === test.key ? '실행 중...' : test.label}
                </button>
              )}
            </For>
          </div>
        </div>

        <div class="results-section">
          <h2>📊 테스트 결과</h2>
          <div class="results-grid">
            <For each={testResultsList}>
              {(test) => (
                <ResultCard title={test.title} result={testResults()[test.key]} />
              )}
            </For>
          </div>
        </div>
      </div>

      <div class="summary">
        <h2>📈 테스트 요약</h2>
        <div class="summary-stats">
          <div class="stat">
            <label>실행된 테스트</label>
            <span>{getTestSummary().total}/5</span>
          </div>
          <div class="stat">
            <label>성공</label>
            <span class="success">{getTestSummary().success}</span>
          </div>
          <div class="stat">
            <label>실패</label>
            <span class="failure">{getTestSummary().failure}</span>
          </div>
          <div class="stat">
            <label>평균 실행 시간</label>
            <span>{getTestSummary().avgTime}ms</span>
          </div>
        </div>
      </div>
    </div>
  );
};
