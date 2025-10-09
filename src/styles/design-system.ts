/**
 * 전역 디자인 시스템
 * Matter Certis v2의 모든 탭에서 일관된 시각적 스타일을 적용하기 위한 디자인 토큰
 * 
 * @created 2025-10-09
 * @purpose UI 일관성 확보, 유지보수성 향상
 */

/**
 * 색상 팔레트
 * 
 * 보라색 그라데이션에서 파랑/인디고 그라데이션으로 통일
 */
export const COLORS = {
  // Primary colors (메인 액센트 색상)
  primary: {
    gradient: 'bg-gradient-to-r from-blue-600 via-indigo-600 to-blue-700',
    gradientHover: 'hover:from-blue-700 hover:via-indigo-700 hover:to-blue-800',
    solid: 'bg-blue-600',
    solidHover: 'hover:bg-blue-700',
    text: 'text-blue-600',
    textHover: 'hover:text-blue-700',
    border: 'border-blue-500',
    bg: 'bg-blue-50',
  },

  // Success colors (성공 상태)
  success: {
    gradient: 'bg-gradient-to-r from-emerald-500 to-green-600',
    solid: 'bg-emerald-500',
    text: 'text-emerald-600',
    border: 'border-emerald-500',
    bg: 'bg-emerald-50',
  },

  // Danger colors (오류/위험 상태)
  danger: {
    gradient: 'bg-gradient-to-r from-rose-500 to-red-600',
    solid: 'bg-rose-500',
    text: 'text-rose-600',
    border: 'border-rose-500',
    bg: 'bg-rose-50',
  },

  // Warning colors (경고 상태)
  warning: {
    gradient: 'bg-gradient-to-r from-amber-500 to-orange-600',
    solid: 'bg-amber-500',
    text: 'text-amber-600',
    border: 'border-amber-500',
    bg: 'bg-amber-50',
  },

  // Neutral colors (중립 상태)
  neutral: {
    gradient: 'bg-gradient-to-r from-gray-500 to-slate-600',
    solid: 'bg-gray-500',
    text: 'text-gray-700',
    textLight: 'text-gray-500',
    border: 'border-gray-200',
    bg: 'bg-gray-50',
  },
};

/**
 * 카드 스타일
 * 
 * 모든 탭에서 일관된 카드 디자인 적용
 */
export const CARD_STYLES = {
  // 기본 카드 (흰색 배경 + 그림자)
  base: 'bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20',
  
  // 헤더 카드 (탭 상단의 제목 영역)
  header: 'bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6',
  
  // 섹션 카드 (설정 그룹, 통계 카드 등)
  section: 'bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6',
  
  // 콤팩트 카드 (작은 정보 표시)
  compact: 'bg-white/80 rounded-lg border border-gray-200 p-3',
  
  // 하이라이트 카드 (중요한 정보 강조)
  highlight: 'bg-gradient-to-br from-blue-50 to-indigo-50 rounded-2xl shadow-lg border border-blue-100 p-6',
};

/**
 * 버튼 스타일
 * 
 * 일관된 버튼 디자인 (variant별로 구분)
 */
export const BUTTON_STYLES = {
  // Primary 버튼 (주요 액션)
  primary: `px-4 py-2 rounded-lg text-white shadow ${COLORS.primary.gradient} ${COLORS.primary.gradientHover} transition-all duration-200`,
  
  // Secondary 버튼 (부수적 액션)
  secondary: 'px-4 py-2 rounded-lg text-gray-700 bg-white border border-gray-200 hover:bg-gray-50 transition-all duration-200',
  
  // Danger 버튼 (삭제, 초기화 등)
  danger: `px-4 py-2 rounded-lg text-white shadow ${COLORS.danger.gradient} hover:from-rose-600 hover:to-red-700 transition-all duration-200`,
  
  // Success 버튼 (저장, 완료 등)
  success: `px-4 py-2 rounded-lg text-white shadow ${COLORS.success.gradient} hover:from-emerald-600 hover:to-green-700 transition-all duration-200`,
  
  // Link 버튼 (텍스트 링크 스타일)
  link: `text-sm ${COLORS.primary.text} ${COLORS.primary.textHover} underline-offset-2 hover:underline transition-colors duration-200`,
  
  // Icon 버튼 (아이콘만 있는 버튼)
  icon: 'p-2 rounded-lg hover:bg-gray-100 transition-colors duration-200',
  
  // Disabled 상태
  disabled: 'px-4 py-2 rounded-lg bg-gray-400 text-white cursor-not-allowed opacity-60',
};

/**
 * 입력 필드 스타일
 */
export const INPUT_STYLES = {
  // 기본 텍스트 입력
  text: 'px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-300 transition-all duration-200',
  
  // 숫자 입력 (오른쪽 정렬)
  number: 'px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-300 text-right transition-all duration-200',
  
  // 체크박스
  checkbox: 'w-4 h-4 text-blue-600 rounded focus:ring-2 focus:ring-blue-300',
  
  // 셀렉트 박스
  select: 'px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-300 transition-all duration-200',
};

/**
 * 레이블 스타일
 */
export const LABEL_STYLES = {
  // 기본 레이블
  base: 'text-sm font-medium text-gray-800',
  
  // 설명 텍스트
  description: 'text-xs text-gray-500',
  
  // 섹션 제목
  sectionTitle: 'text-lg font-semibold text-gray-800',
  
  // 필수 필드 표시
  required: 'text-rose-500',
};

/**
 * 그리드 레이아웃
 */
export const GRID_STYLES = {
  // 1열 (모바일)
  col1: 'grid grid-cols-1 gap-4',
  
  // 2열 (태블릿)
  col2: 'grid grid-cols-1 md:grid-cols-2 gap-4',
  
  // 3열 (데스크톱)
  col3: 'grid grid-cols-1 md:grid-cols-3 gap-4',
  
  // 4열 (와이드 데스크톱)
  col4: 'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4',
};

/**
 * 타이틀 스타일
 */
export const TITLE_STYLES = {
  // 페이지 메인 타이틀 (h1)
  page: 'text-2xl md:text-3xl font-bold text-gray-800',
  
  // 페이지 타이틀 (그라데이션)
  pageGradient: `text-2xl md:text-3xl font-bold ${COLORS.primary.gradient} bg-clip-text text-transparent`,
  
  // 섹션 타이틀 (h2)
  section: 'text-xl font-semibold text-gray-800',
  
  // 서브섹션 타이틀 (h3)
  subsection: 'text-lg font-semibold text-gray-800',
};

/**
 * 토스트/알림 스타일
 */
export const TOAST_STYLES = {
  success: `fixed top-5 right-5 z-[1000] px-4 py-2 rounded-md text-white shadow-lg ${COLORS.success.solid}`,
  error: `fixed top-5 right-5 z-[1000] px-4 py-2 rounded-md text-white shadow-lg ${COLORS.danger.solid}`,
  warning: `fixed top-5 right-5 z-[1000] px-4 py-2 rounded-md text-white shadow-lg ${COLORS.warning.solid}`,
  info: `fixed top-5 right-5 z-[1000] px-4 py-2 rounded-md text-white shadow-lg ${COLORS.primary.solid}`,
};

/**
 * 배경 그라데이션
 */
export const BACKGROUND_STYLES = {
  // 페이지 배경 (밝은 그라데이션)
  page: 'min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50',
  
  // 모달 배경 (어두운 오버레이)
  modalOverlay: 'fixed inset-0 bg-black/50 backdrop-blur-sm z-[1000]',
};

/**
 * 아이콘 스타일
 */
export const ICON_STYLES = {
  small: 'w-4 h-4',
  medium: 'w-5 h-5',
  large: 'w-6 h-6',
};

/**
 * 트랜지션/애니메이션
 */
export const TRANSITION_STYLES = {
  fast: 'transition-all duration-150',
  normal: 'transition-all duration-200',
  slow: 'transition-all duration-300',
};

/**
 * 헬퍼 함수: 여러 클래스를 조합
 */
export function cn(...classes: (string | undefined | false)[]): string {
  return classes.filter(Boolean).join(' ');
}

/**
 * 사용 예시:
 * 
 * import { COLORS, BUTTON_STYLES, CARD_STYLES } from '@/styles/design-system';
 * 
 * // 카드
 * <div class={CARD_STYLES.section}>
 *   <h2 class={TITLE_STYLES.section}>섹션 제목</h2>
 * </div>
 * 
 * // 버튼
 * <button class={BUTTON_STYLES.primary}>저장</button>
 * <button class={BUTTON_STYLES.secondary}>취소</button>
 * 
 * // 조합
 * <button class={cn(BUTTON_STYLES.primary, "w-full")}>전체 너비 버튼</button>
 */
