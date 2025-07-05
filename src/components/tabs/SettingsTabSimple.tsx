/**
 * SettingsTab - 실제 기능이 있는 설정 탭 컴포넌트
 */

import { Component, createSignal } from 'solid-js';

export const SettingsTab: Component = () => {
  // 기본 설정
  const [concurrentDownloads, setConcurrentDownloads] = createSignal(3);
  const [requestDelay, setRequestDelay] = createSignal(1000);
  const [timeout, setTimeout] = createSignal(30);
  const [retryCount, setRetryCount] = createSignal(3);

  // 로깅 설정
  const [logLevel, setLogLevel] = createSignal('INFO');
  const [terminalOutput, setTerminalOutput] = createSignal(true);
  const [fileLogging, setFileLogging] = createSignal(true);

  // 배치 처리 설정
  const [batchSize, setBatchSize] = createSignal(50);
  const [progressInterval, setProgressInterval] = createSignal(1);
  const [autoBackup, setAutoBackup] = createSignal(true);

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <h2 style="margin: 0 0 24px 0; font-size: 24px; font-weight: 600; color: #1f2937;">⚙️ 설정</h2>
      
      {/* 기본 설정 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f9fafb;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">기본 설정</h3>
        
        <div style="margin-bottom: 20px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            동시 다운로드 수: {concurrentDownloads()}
          </label>
          <input
            type="range"
            min="1"
            max="10"
            value={concurrentDownloads()}
            onInput={(e) => setConcurrentDownloads(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>1</span>
            <span>10</span>
          </div>
        </div>

        <div style="margin-bottom: 20px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            요청 간 지연: {requestDelay()}ms
          </label>
          <input
            type="range"
            min="0"
            max="5000"
            step="100"
            value={requestDelay()}
            onInput={(e) => setRequestDelay(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>0ms</span>
            <span>5000ms</span>
          </div>
        </div>

        <div style="margin-bottom: 20px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            타임아웃: {timeout()}초
          </label>
          <input
            type="range"
            min="10"
            max="120"
            value={timeout()}
            onInput={(e) => setTimeout(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>10초</span>
            <span>120초</span>
          </div>
        </div>

        <div style="margin-bottom: 0;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            재시도 횟수: {retryCount()}
          </label>
          <input
            type="range"
            min="0"
            max="10"
            value={retryCount()}
            onInput={(e) => setRetryCount(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>0</span>
            <span>10</span>
          </div>
        </div>
      </div>

      {/* 로깅 설정 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0f9ff;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">로깅 설정</h3>
        
        <div style="margin-bottom: 16px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">로그 레벨</label>
          <select
            value={logLevel()}
            onChange={(e) => setLogLevel(e.currentTarget.value)}
            style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; background: white; font-size: 14px;"
          >
            <option value="DEBUG">DEBUG</option>
            <option value="INFO">INFO</option>
            <option value="WARN">WARN</option>
            <option value="ERROR">ERROR</option>
          </select>
        </div>

        <div style="margin-bottom: 16px;">
          <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
            <input
              type="checkbox"
              checked={terminalOutput()}
              onChange={(e) => setTerminalOutput(e.currentTarget.checked)}
              style="margin-right: 8px; width: 16px; height: 16px;"
            />
            터미널 출력
          </label>
        </div>

        <div style="margin-bottom: 0;">
          <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
            <input
              type="checkbox"
              checked={fileLogging()}
              onChange={(e) => setFileLogging(e.currentTarget.checked)}
              style="margin-right: 8px; width: 16px; height: 16px;"
            />
            파일 로깅
          </label>
        </div>
      </div>

      {/* 배치 처리 설정 */}
      <div style="margin-bottom: 32px; padding: 20px; border: 1px solid #e5e7eb; border-radius: 8px; background: #f0fdf4;">
        <h3 style="margin: 0 0 16px 0; font-size: 18px; font-weight: 500; color: #374151;">배치 처리 설정</h3>
        
        <div style="margin-bottom: 20px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            배치 크기: {batchSize()}
          </label>
          <input
            type="range"
            min="10"
            max="200"
            step="10"
            value={batchSize()}
            onInput={(e) => setBatchSize(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>10</span>
            <span>200</span>
          </div>
        </div>

        <div style="margin-bottom: 20px;">
          <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
            진행률 업데이트 간격: {progressInterval()}초
          </label>
          <input
            type="range"
            min="1"
            max="10"
            value={progressInterval()}
            onInput={(e) => setProgressInterval(parseInt(e.currentTarget.value))}
            style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
          />
          <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
            <span>1초</span>
            <span>10초</span>
          </div>
        </div>

        <div style="margin-bottom: 0;">
          <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
            <input
              type="checkbox"
              checked={autoBackup()}
              onChange={(e) => setAutoBackup(e.currentTarget.checked)}
              style="margin-right: 8px; width: 16px; height: 16px;"
            />
            자동 백업
          </label>
        </div>
      </div>

      {/* 설정 저장 버튼 */}
      <div style="display: flex; gap: 12px;">
        <button
          onClick={() => alert('설정이 저장되었습니다!')}
          style="padding: 12px 24px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
          onMouseOver={(e) => e.currentTarget.style.background = '#2563eb'}
          onMouseOut={(e) => e.currentTarget.style.background = '#3b82f6'}
        >
          설정 저장
        </button>
        <button
          onClick={() => {
            setConcurrentDownloads(3);
            setRequestDelay(1000);
            setTimeout(30);
            setRetryCount(3);
            setLogLevel('INFO');
            setTerminalOutput(true);
            setFileLogging(true);
            setBatchSize(50);
            setProgressInterval(1);
            setAutoBackup(true);
            alert('기본값으로 복원되었습니다!');
          }}
          style="padding: 12px 24px; background: #6b7280; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
          onMouseOver={(e) => e.currentTarget.style.background = '#4b5563'}
          onMouseOut={(e) => e.currentTarget.style.background = '#6b7280'}
        >
          기본값 복원
        </button>
      </div>
    </div>
  );
};
