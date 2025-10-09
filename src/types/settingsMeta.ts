/**
 * 고급 설정 메타데이터
 * 각 설정의 위험도, 설명, 권장값 등을 정의
 */

export type DangerLevel = 'safe' | 'caution' | 'danger';

export interface SettingMeta {
  /** 위험도 레벨 */
  level: DangerLevel;
  /** 설정 설명 */
  description: string;
  /** 권장 값 또는 범위 */
  recommended?: string;
  /** 주의사항 (경고 메시지) */
  warnings?: string[];
  /** 연관된 설정들 */
  related?: string[];
  /** 기본값 */
  defaultValue?: any;
}

export interface CategoryMeta {
  icon: string;
  title: string;
  description: string;
}

/**
 * 카테고리 정의
 */
export const CATEGORIES: Record<string, CategoryMeta> = {
  logging: {
    icon: '📝',
    title: '로깅 설정',
    description: '로그 파일 관리 및 출력 형식'
  },
  retry: {
    icon: '🔄',
    title: '재시도 정책',
    description: '크롤링 실패 시 재시도 동작'
  },
  network: {
    icon: '🌐',
    title: '네트워크 및 워커',
    description: 'HTTP 요청 및 네트워크 설정'
  },
  database: {
    icon: '💾',
    title: '데이터베이스',
    description: 'DB 저장 및 동시성 제어'
  },
  timing: {
    icon: '⏱️',
    title: '타이밍 및 스케줄링',
    description: '타임아웃 및 스케줄 간격'
  },
  engine: {
    icon: '⚙️',
    title: '엔진 내부',
    description: '크롤링 엔진 고급 설정'
  }
};

/**
 * 설정별 메타데이터
 */
export const SETTINGS_META: Record<string, SettingMeta> = {
  // ===== 📝 로깅 설정 (6개) =====
  'user.logging.file_naming_strategy': {
    level: 'caution',
    description: '로그 파일 이름 생성 방식',
    recommended: 'unified (단일 파일) 또는 daily (일별 분리)',
    warnings: ['timestamp는 파일이 많아질 수 있음'],
    defaultValue: 'unified'
  },
  'user.logging.max_files': {
    level: 'caution',
    description: '보관할 로그 파일 최대 개수',
    recommended: '5-10개 (디스크 공간 고려)',
    warnings: ['너무 작으면 과거 로그 손실', '너무 크면 디스크 공간 부족'],
    defaultValue: 5
  },
  'user.logging.auto_cleanup_logs': {
    level: 'safe',
    description: '앱 종료 시 자동으로 오래된 로그 정리',
    recommended: 'true (자동 정리 권장)',
    defaultValue: false
  },
  'user.logging.keep_only_latest': {
    level: 'safe',
    description: '가장 최신 로그만 유지',
    recommended: 'false (여러 세션 로그 보관 시)',
    related: ['user.logging.max_files'],
    defaultValue: false
  },
  'user.logging.concise_startup': {
    level: 'safe',
    description: '시작 시 간결한 로그 출력',
    recommended: 'true (불필요한 로그 감소)',
    defaultValue: true
  },
  'user.logging.separate_frontend_backend': {
    level: 'caution',
    description: 'UI와 백엔드 로그를 별도 파일로 분리',
    recommended: 'true (문제 추적 시 유용)',
    warnings: ['true 설정 시 로그 파일 2배'],
    defaultValue: false
  },

  // ===== 🔄 재시도 정책 (4개) =====
  'user.batch.batch_retry_limit': {
    level: 'danger',
    description: '배치 처리 실패 시 재시도 횟수',
    recommended: '3-5회',
    warnings: [
      '너무 높으면 시스템 과부하',
      '무한 재시도 방지 필요',
      '5회 초과는 비효율적'
    ],
    related: ['user.crawling.timing.retry_delay_ms'],
    defaultValue: 3
  },
  'user.crawling.product_list_retry_count': {
    level: 'danger',
    description: '목록 페이지 크롤링 실패 시 재시도 횟수',
    recommended: '3-5회',
    warnings: [
      '높은 값은 느린 응답 시간',
      '목록 페이지는 중요하므로 적절한 재시도 필요'
    ],
    related: ['user.crawling.product_detail_retry_count'],
    defaultValue: 9
  },
  'user.crawling.product_detail_retry_count': {
    level: 'danger',
    description: '상세 페이지 크롤링 실패 시 재시도 횟수',
    recommended: '3-5회',
    warnings: [
      '높은 값은 전체 크롤링 시간 증가',
      '상세 페이지는 개별 실패 허용 가능'
    ],
    related: ['user.crawling.product_list_retry_count'],
    defaultValue: 9
  },
  'user.crawling.workers.max_retries': {
    level: 'danger',
    description: 'HTTP 요청 재시도 최대 횟수 (워커 레벨)',
    recommended: '3-5회',
    warnings: [
      '네트워크 일시 오류 대응',
      '과도한 재시도는 자원 낭비'
    ],
    defaultValue: 5
  },

  // ===== 🌐 네트워크 및 워커 (5개) =====
  'user.crawling.workers.user_agent': {
    level: 'safe',
    description: 'HTTP 요청 User-Agent 헤더',
    recommended: '실제 브라우저와 유사한 값 사용',
    warnings: ['빈 값이면 봇으로 감지될 수 있음'],
    defaultValue: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)...'
  },
  'user.crawling.workers.user_agent_sync': {
    level: 'safe',
    description: '동기화 작업 전용 User-Agent (선택)',
    recommended: '기본값과 다른 값 사용 (추적 용이)',
    defaultValue: null
  },
  'user.crawling.workers.follow_redirects': {
    level: 'caution',
    description: 'HTTP 리다이렉트 자동 추적',
    recommended: 'true (대부분의 경우)',
    warnings: ['무한 리다이렉트 가능성'],
    defaultValue: true
  },
  'user.crawling.workers.respect_robots_txt': {
    level: 'caution',
    description: 'robots.txt 준수 여부',
    recommended: 'false (내부 테스트), true (공개 크롤링)',
    warnings: [
      'true 설정 시 일부 페이지 크롤링 불가',
      '법적 준수 필요 시 true 권장'
    ],
    defaultValue: false
  },
  'user.crawling.workers.db_batch_size': {
    level: 'danger',
    description: 'DB 저장 시 배치 크기 (레코드 수)',
    recommended: '100-500개',
    warnings: [
      '너무 크면 메모리 부족',
      '너무 작으면 DB 쓰기 오버헤드',
      '1000 이상은 위험'
    ],
    related: ['user.crawling.workers.db_max_concurrency'],
    defaultValue: 100
  },

  // ===== 💾 데이터베이스 (2개) =====
  'user.crawling.workers.db_max_concurrency': {
    level: 'danger',
    description: 'DB 작업 최대 동시 실행 수',
    recommended: '3-10개',
    warnings: [
      '너무 높으면 DB 락 경합',
      '너무 낮으면 처리 속도 저하',
      '20 초과는 매우 위험'
    ],
    related: ['user.crawling.workers.db_batch_size'],
    defaultValue: 5
  },

  // ===== ⏱️ 타이밍 및 스케줄링 (5개) =====
  'user.crawling.timing.scheduler_interval_ms': {
    level: 'caution',
    description: '백그라운드 스케줄러 실행 간격 (ms)',
    recommended: '100-1000ms',
    warnings: ['너무 짧으면 CPU 과부하'],
    defaultValue: 100
  },
  'user.crawling.timing.shutdown_timeout_seconds': {
    level: 'caution',
    description: '앱 종료 시 정리 작업 최대 대기 시간 (초)',
    recommended: '10-30초',
    warnings: ['너무 짧으면 데이터 손실 가능'],
    defaultValue: 30
  },
  'user.crawling.timing.stats_interval_seconds': {
    level: 'safe',
    description: '진행 상황 통계 출력 간격 (초)',
    recommended: '5-30초',
    defaultValue: 10
  },
  'user.crawling.timing.retry_delay_ms': {
    level: 'danger',
    description: '재시도 전 대기 시간 (ms)',
    recommended: '1000-3000ms',
    warnings: [
      '너무 짧으면 서버 부하',
      '너무 길면 전체 시간 증가'
    ],
    related: ['user.batch.batch_retry_limit'],
    defaultValue: 2000
  },
  'user.crawling.timing.operation_timeout_seconds': {
    level: 'danger',
    description: '전체 작업 최대 실행 시간 (초)',
    recommended: '60-300초',
    warnings: [
      '너무 짧으면 작업 중단',
      '큰 크롤링 작업은 충분한 시간 필요'
    ],
    defaultValue: 300
  },

  // ===== ⚙️ 엔진 내부 (10개) =====
  'advanced.last_page_search_start': {
    level: 'danger',
    description: '마지막 페이지 탐색 시작 위치',
    recommended: '100-500 (사이트에 따라 다름)',
    warnings: [
      '잘못된 값은 불필요한 요청 증가',
      '사이트별 최적화 필요'
    ],
    defaultValue: 100
  },
  'advanced.max_search_attempts': {
    level: 'caution',
    description: '마지막 페이지 탐색 최대 시도 횟수',
    recommended: '5-10회',
    warnings: ['너무 많으면 시간 낭비'],
    defaultValue: 10
  },
  'advanced.request_timeout_seconds': {
    level: 'danger',
    description: '고급 HTTP 요청 타임아웃 (초)',
    recommended: '30-60초',
    warnings: [
      '너무 짧으면 느린 응답 실패',
      'operation_timeout_seconds와 중복 가능'
    ],
    related: ['user.crawling.timing.operation_timeout_seconds'],
    defaultValue: 60
  },
  'advanced.product_selectors': {
    level: 'danger',
    description: '상품 영역 CSS 선택자 목록',
    recommended: '사이트 구조에 맞는 정확한 선택자',
    warnings: [
      '잘못된 선택자는 크롤링 실패',
      '복수 선택자는 쉼표로 구분'
    ],
    defaultValue: ['.product-item', '.product-card']
  },
  'advanced.failure_policy.failure_threshold': {
    level: 'danger',
    description: '연속 실패 허용 임계값',
    recommended: '5-10회',
    warnings: [
      '너무 낮으면 세션 조기 종료',
      '너무 높으면 무의미한 재시도 지속'
    ],
    related: ['advanced.failure_policy.removal_grace_secs'],
    defaultValue: 5
  },
  'advanced.failure_policy.removal_grace_secs': {
    level: 'caution',
    description: '실패 세션 정리 유예 시간 (초)',
    recommended: '10-30초',
    warnings: ['너무 짧으면 복구 기회 없음'],
    related: ['advanced.failure_policy.failure_threshold'],
    defaultValue: 10
  },
  'user.crawling.intelligent_mode.override_config_limit': {
    level: 'caution',
    description: '지능형 모드에서 설정 제한 무시',
    recommended: 'true (지능형 계산 신뢰 시)',
    warnings: ['예상보다 많은 페이지 크롤링 가능'],
    related: ['user.crawling.intelligent_mode.max_range_limit'],
    defaultValue: true
  },
  'user.crawling.intelligent_mode.max_range_limit': {
    level: 'danger',
    description: '지능형 모드 최대 페이지 범위',
    recommended: '500-1000',
    warnings: [
      '너무 크면 과도한 크롤링',
      'page_range_limit와 함께 고려'
    ],
    related: ['user.crawling.intelligent_mode.override_config_limit'],
    defaultValue: 1000
  }
};

/**
 * 카테고리별 설정 분류
 */
export const CATEGORY_SETTINGS: Record<string, string[]> = {
  logging: [
    'user.logging.file_naming_strategy',
    'user.logging.max_files',
    'user.logging.auto_cleanup_logs',
    'user.logging.keep_only_latest',
    'user.logging.concise_startup',
    'user.logging.separate_frontend_backend'
  ],
  retry: [
    'user.batch.batch_retry_limit',
    'user.crawling.product_list_retry_count',
    'user.crawling.product_detail_retry_count',
    'user.crawling.workers.max_retries'
  ],
  network: [
    'user.crawling.workers.user_agent',
    'user.crawling.workers.user_agent_sync',
    'user.crawling.workers.follow_redirects',
    'user.crawling.workers.respect_robots_txt',
    'user.crawling.workers.db_batch_size'
  ],
  database: [
    'user.crawling.workers.db_max_concurrency'
  ],
  timing: [
    'user.crawling.timing.scheduler_interval_ms',
    'user.crawling.timing.shutdown_timeout_seconds',
    'user.crawling.timing.stats_interval_seconds',
    'user.crawling.timing.retry_delay_ms',
    'user.crawling.timing.operation_timeout_seconds'
  ],
  engine: [
    'advanced.last_page_search_start',
    'advanced.max_search_attempts',
    'advanced.request_timeout_seconds',
    'advanced.product_selectors',
    'advanced.failure_policy.failure_threshold',
    'advanced.failure_policy.removal_grace_secs',
    'user.crawling.intelligent_mode.override_config_limit',
    'user.crawling.intelligent_mode.max_range_limit'
  ]
};

/**
 * 위험도별 색상 스타일
 */
export const DANGER_LEVEL_STYLES: Record<DangerLevel, {
  bg: string;
  border: string;
  text: string;
  badge: string;
  icon: string;
}> = {
  safe: {
    bg: 'bg-green-50',
    border: 'border-green-200',
    text: 'text-green-700',
    badge: '🟢 안전',
    icon: '🟢'
  },
  caution: {
    bg: 'bg-amber-50',
    border: 'border-amber-300',
    text: 'text-amber-700',
    badge: '🟡 주의',
    icon: '⚠️'
  },
  danger: {
    bg: 'bg-red-50',
    border: 'border-red-300',
    text: 'text-red-700',
    badge: '🔴 위험',
    icon: '⚠️'
  }
};

/**
 * 카테고리별 위험도 통계 계산
 */
export function getCategoryDangerStats(categoryKey: string): {
  safe: number;
  caution: number;
  danger: number;
} {
  const settings = CATEGORY_SETTINGS[categoryKey] || [];
  const stats = { safe: 0, caution: 0, danger: 0 };
  
  settings.forEach(settingKey => {
    const meta = SETTINGS_META[settingKey];
    if (meta) {
      stats[meta.level]++;
    }
  });
  
  return stats;
}
