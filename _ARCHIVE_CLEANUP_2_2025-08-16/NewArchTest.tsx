import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import './NewArchTest.css';

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

const NewArchTest: React.FC = () => {
  const [configInfo, setConfigInfo] = useState<SystemConfigInfo | null>(null);
  const [testResults, setTestResults] = useState<{ [key: string]: TestResult }>({});
  const [loading, setLoading] = useState<string | null>(null);

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

  const ResultCard: React.FC<{ title: string; result?: TestResult }> = ({ title, result }) => (
    <div className={`result-card ${result?.success ? 'success' : result ? 'failure' : 'pending'}`}>
      <h3>{title}</h3>
      {result ? (
        <>
          <div className={`status ${result.success ? 'success' : 'failure'}`}>
            {result.success ? '✅ 성공' : '❌ 실패'}
          </div>
          <p className="message">{result.message}</p>
          <div className="execution-time">실행 시간: {result.execution_time_ms}ms</div>
          {result.details && (
            <details className="details">
              <summary>상세 정보</summary>
              <pre>{JSON.stringify(result.details, null, 2)}</pre>
            </details>
          )}
        </>
      ) : (
        <div className="status pending">⏳ 대기중</div>
      )}
    </div>
  );

  return (
    <div className="new-arch-test">
      <div className="header">
        <h1>🏗️ 새로운 아키텍처 테스트</h1>
        <p>Modern Rust 2024 Actor 시스템 검증</p>
      </div>

      <div className="controls">
        <button 
          onClick={loadConfig} 
          disabled={loading === 'config'}
          className="config-btn"
        >
          {loading === 'config' ? '로딩 중...' : '설정 정보 로드'}
        </button>
        <button 
          onClick={runAllTests} 
          disabled={loading !== null}
          className="run-all-btn"
        >
          {loading ? '테스트 실행 중...' : '전체 테스트 실행'}
        </button>
      </div>

      {configInfo && (
        <div className="config-info">
          <h2>📋 시스템 설정 정보</h2>
          <div className="config-grid">
            <div className="config-item">
              <label>세션 타임아웃</label>
              <span>{configInfo.session_timeout_secs}초</span>
            </div>
            <div className="config-item">
              <label>최대 동시 세션</label>
              <span>{configInfo.max_concurrent_sessions}개</span>
            </div>
            <div className="config-item">
              <label>Control 버퍼 크기</label>
              <span>{configInfo.control_buffer_size}</span>
            </div>
            <div className="config-item">
              <label>Event 버퍼 크기</label>
              <span>{configInfo.event_buffer_size}</span>
            </div>
            <div className="config-item">
              <label>초기 배치 크기</label>
              <span>{configInfo.batch_initial_size}</span>
            </div>
            <div className="config-item">
              <label>최대 동시 작업</label>
              <span>{configInfo.max_concurrent_tasks}개</span>
            </div>
          </div>
        </div>
      )}

      <div className="test-grid">
        <div className="test-section">
          <h2>🔧 개별 테스트</h2>
          <div className="individual-tests">
            <button
              onClick={() => runTest('channels', 'test_new_arch_channels')}
              disabled={loading === 'channels'}
              className="test-btn"
            >
              {loading === 'channels' ? '실행 중...' : '채널 통신 테스트'}
            </button>
            <button
              onClick={() => runTest('session_actor', 'test_new_arch_session_actor')}
              disabled={loading === 'session_actor'}
              className="test-btn"
            >
              {loading === 'session_actor' ? '실행 중...' : 'SessionActor 테스트'}
            </button>
            <button
              onClick={() => runTest('batch_actor', 'test_new_arch_batch_actor')}
              disabled={loading === 'batch_actor'}
              className="test-btn"
            >
              {loading === 'batch_actor' ? '실행 중...' : 'BatchActor 테스트'}
            </button>
            <button
              onClick={() => runTest('integration', 'test_new_arch_integration')}
              disabled={loading === 'integration'}
              className="test-btn"
            >
              {loading === 'integration' ? '실행 중...' : '통합 테스트'}
            </button>
            <button
              onClick={() => runTest('performance', 'test_new_arch_performance')}
              disabled={loading === 'performance'}
              className="test-btn"
            >
              {loading === 'performance' ? '실행 중...' : '성능 테스트'}
            </button>
          </div>
        </div>

        <div className="results-section">
          <h2>📊 테스트 결과</h2>
          <div className="results-grid">
            <ResultCard title="채널 통신" result={testResults.channels} />
            <ResultCard title="SessionActor" result={testResults.session_actor} />
            <ResultCard title="BatchActor" result={testResults.batch_actor} />
            <ResultCard title="통합 테스트" result={testResults.integration} />
            <ResultCard title="성능 테스트" result={testResults.performance} />
          </div>
        </div>
      </div>

      <div className="summary">
        <h2>📈 테스트 요약</h2>
        <div className="summary-stats">
          <div className="stat">
            <label>실행된 테스트</label>
            <span>{Object.keys(testResults).length}/5</span>
          </div>
          <div className="stat">
            <label>성공</label>
            <span className="success">
              {Object.values(testResults).filter(r => r.success).length}
            </span>
          </div>
          <div className="stat">
            <label>실패</label>
            <span className="failure">
              {Object.values(testResults).filter(r => !r.success).length}
            </span>
          </div>
          <div className="stat">
            <label>평균 실행 시간</label>
            <span>
              {Object.values(testResults).length > 0
                ? Math.round(
                    Object.values(testResults).reduce((sum, r) => sum + r.execution_time_ms, 0) /
                    Object.values(testResults).length
                  )
                : 0}ms
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default NewArchTest;
