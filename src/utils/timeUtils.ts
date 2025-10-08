/**
 * 시간 관련 유틸리티 함수
 */

/**
 * 밀리초를 사람이 읽기 쉬운 형태로 변환
 * @param ms - 밀리초
 * @returns "1시간 23분" 또는 "45분" 또는 "30초" 형태의 문자열
 */
export function formatDuration(ms: number): string {
  if (ms < 1000) {
    return '1초 미만';
  }

  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) {
    const remainingHours = hours % 24;
    if (remainingHours > 0) {
      return `${days}일 ${remainingHours}시간`;
    }
    return `${days}일`;
  }

  if (hours > 0) {
    const remainingMinutes = minutes % 60;
    if (remainingMinutes > 0) {
      return `${hours}시간 ${remainingMinutes}분`;
    }
    return `${hours}시간`;
  }

  if (minutes > 0) {
    const remainingSeconds = seconds % 60;
    if (remainingSeconds > 10) {
      return `${minutes}분 ${remainingSeconds}초`;
    }
    return `${minutes}분`;
  }

  return `${seconds}초`;
}

/**
 * ISO 시간 문자열을 현지 시간 형식으로 변환
 * @param isoString - ISO 8601 형식의 시간 문자열
 * @returns "오후 3:45" 형태의 문자열
 */
export function formatTime(isoString: string): string {
  const date = new Date(isoString);
  
  return date.toLocaleTimeString('ko-KR', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: true,
  });
}

/**
 * ISO 시간 문자열을 날짜와 시간 형식으로 변환
 * @param isoString - ISO 8601 형식의 시간 문자열
 * @returns "1월 15일 오후 3:45" 형태의 문자열
 */
export function formatDateTime(isoString: string): string {
  const date = new Date(isoString);
  
  const dateStr = date.toLocaleDateString('ko-KR', {
    month: 'long',
    day: 'numeric',
  });
  
  const timeStr = date.toLocaleTimeString('ko-KR', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: true,
  });
  
  return `${dateStr} ${timeStr}`;
}

/**
 * 두 시간 사이의 경과 시간을 계산
 * @param startTime - 시작 시간 (Date 또는 ISO 문자열)
 * @param endTime - 종료 시간 (Date 또는 ISO 문자열, 기본값: 현재 시간)
 * @returns 경과 시간 (밀리초)
 */
export function getElapsedTime(
  startTime: Date | string,
  endTime: Date | string = new Date()
): number {
  const start = typeof startTime === 'string' ? new Date(startTime) : startTime;
  const end = typeof endTime === 'string' ? new Date(endTime) : endTime;
  
  return end.getTime() - start.getTime();
}

/**
 * 평균 속도를 계산
 * @param completed - 완료된 항목 수
 * @param elapsedMs - 경과 시간 (밀리초)
 * @returns 초당 처리 속도
 */
export function calculateRate(completed: number, elapsedMs: number): number {
  if (elapsedMs === 0) return 0;
  return (completed * 1000) / elapsedMs;
}

/**
 * 남은 시간을 예측
 * @param completed - 완료된 항목 수
 * @param total - 전체 항목 수
 * @param elapsedMs - 경과 시간 (밀리초)
 * @returns 예상 남은 시간 (밀리초)
 */
export function estimateRemainingTime(
  completed: number,
  total: number,
  elapsedMs: number
): number {
  if (completed === 0 || total === 0) return 0;
  
  const rate = calculateRate(completed, elapsedMs);
  const remaining = total - completed;
  
  if (rate === 0) return 0;
  
  return (remaining * 1000) / rate;
}

/**
 * 진행률 표시용 시간 정보를 생성
 * @param startTime - 시작 시간
 * @param completed - 완료된 항목 수
 * @param total - 전체 항목 수
 * @returns 경과 시간과 남은 시간 정보
 */
export function getProgressTimeInfo(
  startTime: Date | string,
  completed: number,
  total: number
) {
  const elapsedMs = getElapsedTime(startTime);
  const remainingMs = estimateRemainingTime(completed, total, elapsedMs);
  const rate = calculateRate(completed, elapsedMs);
  
  return {
    elapsed: formatDuration(elapsedMs),
    remaining: formatDuration(remainingMs),
    elapsedMs,
    remainingMs,
    rate: rate.toFixed(2),
  };
}
