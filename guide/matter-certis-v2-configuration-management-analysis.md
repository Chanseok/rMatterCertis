# Matter Certis 애플리케이션 설정 관리 시스템 분석 및 향후 계획

> 이 문서는 현재 TypeScript/Electron 프로젝트의 설정 관리 구조를 상세히 분석하고, 코드와 설정의 분리 패턴을 파악하여 향후 Rust/Tauri 전환 시의 설정 관리 전략을 제시합니다.

## 📋 목차

1. [설정 관리 시스템 개요](#설정-관리-시스템-개요)
2. [현재 설정 구조 분석](#현재-설정-구조-분석)
3. [핵심 URL 및 환경 설정](#핵심-url-및-환경-설정)
4. [설정과 코드 분리 패턴](#설정과-코드-분리-패턴)
5. [환경별 설정 관리](#환경별-설정-관리)
6. [설정 검증 및 보안](#설정-검증-및-보안)
7. [향후 Rust/Tauri 전환 계획](#향후-rust/tauri-전환-계획)
8. [마이그레이션 전략](#마이그레이션-전략)

---

## 설정 관리 시스템 개요

### 현재 아키텍처 구조

```
설정 관리 시스템
├── 📄 JSON 설정 파일 (data/crawler-config.json)
├── 🔧 ConfigManager (src/electron/ConfigManager.ts)
├── 🛠️ ConfigUtils (src/shared/utils/ConfigUtils.ts)
├── 🎯 SessionConfigManager (src/ui/services/domain/SessionConfigManager.ts)
├── 📊 ConfigurationViewModel (src/ui/viewmodels/ConfigurationViewModel.ts)
└── 🌍 Environment Detection (src/ui/utils/environment.ts)
```

### 설정 관리 계층 구조

1. **파일 레이어**: JSON 설정 파일
2. **백엔드 레이어**: ConfigManager (Electron Main Process)
3. **공유 레이어**: ConfigUtils (Validation & Utilities)
4. **서비스 레이어**: ConfigurationService (IPC Bridge)
5. **세션 레이어**: SessionConfigManager (UI State Management)
6. **뷰모델 레이어**: ConfigurationViewModel (UI Binding)

---

## 현재 설정 구조 분석

### 1. 핵심 설정 파일 위치

```bash
# 메인 설정 파일
/data/crawler-config.json

# 사용자 데이터 디렉토리 (런타임)
~/Library/Application Support/crawlMatterCertis/crawler-config.json  # macOS
~/.config/crawlMatterCertis/crawler-config.json                     # Linux
%APPDATA%/crawlMatterCertis/crawler-config.json                     # Windows
```

### 2. 완전한 설정 스키마

```typescript
interface CrawlerConfig {
  // === 핵심 크롤링 설정 ===
  pageRangeLimit: number;              // 기본값: 10
  productListRetryCount: number;       // 기본값: 9
  productDetailRetryCount: number;     // 기본값: 9
  productsPerPage: number;             // 기본값: 12
  autoAddToLocalDB: boolean;           // 기본값: true
  autoStatusCheck: boolean;            // 기본값: true
  crawlerType: 'axios' | 'playwright'; // 기본값: 'axios'

  // === 배치 처리 설정 ===
  batchSize: number;                   // 기본값: 30
  batchDelayMs: number;                // 기본값: 2000
  enableBatchProcessing: boolean;      // 기본값: true
  batchRetryLimit: number;             // 기본값: 3

  // === 핵심 URL 설정 ===
  baseUrl: string;                     // CSA-IoT 기본 URL
  matterFilterUrl: string;             // Matter 필터 적용된 URL
  
  // === 타임아웃 설정 ===
  pageTimeoutMs: number;               // 기본값: 90000
  productDetailTimeoutMs: number;      // 기본값: 90000
  
  // === 동시성 및 성능 설정 ===
  initialConcurrency: number;          // 기본값: 16
  detailConcurrency: number;           // 기본값: 16
  retryConcurrency: number;            // 기본값: 9
  minRequestDelayMs: number;           // 기본값: 100
  maxRequestDelayMs: number;           // 기본값: 2200
  retryStart: number;                  // 기본값: 2
  retryMax: number;                    // 기본값: 10
  cacheTtlMs: number;                  // 기본값: 300000

  // === 브라우저 설정 ===
  headlessBrowser: boolean;            // 기본값: true
  maxConcurrentTasks: number;          // 기본값: 16
  requestDelay: number;                // 기본값: 100
  customUserAgent?: string;            // 선택적
  
  // === 파일 경로 설정 ===
  lastExcelExportPath?: string;        // 마지막 Excel 내보내기 경로

  // === 로깅 설정 ===
  logging: {
    level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
    components: Record<string, any>;
    enableStackTrace: boolean;
    enableTimestamp: boolean;
  };
}
```

### 3. 기본값 정의 (Single Source of Truth)

```typescript
// src/shared/utils/ConfigUtils.ts
export const DEFAULT_CONFIG: MutableCrawlerConfig = {
  // Core crawler settings
  pageRangeLimit: 10,
  productListRetryCount: 9,
  productDetailRetryCount: 9,
  productsPerPage: 12,
  autoAddToLocalDB: true,
  autoStatusCheck: true,
  crawlerType: 'axios',
  
  // Batch processing
  batchSize: 30,
  batchDelayMs: 2000,
  enableBatchProcessing: true,
  batchRetryLimit: 3,

  // URLs and timeouts
  baseUrl: 'https://csa-iot.org/csa-iot_products/',
  matterFilterUrl: 'https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=',
  pageTimeoutMs: 90000,
  productDetailTimeoutMs: 90000,
  
  // Concurrency and performance
  initialConcurrency: 16,
  detailConcurrency: 16,
  retryConcurrency: 9,
  minRequestDelayMs: 100,
  maxRequestDelayMs: 2200,
  retryStart: 2,
  retryMax: 10,
  cacheTtlMs: 300000,
  
  // Browser settings
  headlessBrowser: true,
  maxConcurrentTasks: 16,
  requestDelay: 100,
  customUserAgent: undefined,
  lastExcelExportPath: undefined,
  
  // Logging configuration
  logging: {
    level: 'INFO' as const,
    components: {},
    enableStackTrace: false,
    enableTimestamp: true
  }
};
```

---

## 핵심 URL 및 환경 설정

### 1. 크롤링 대상 URL 구조

#### 기본 URL 설정
```typescript
const CSA_IOT_URLS = {
  // 기본 제품 페이지
  BASE_URL: 'https://csa-iot.org/csa-iot_products/',
  
  // Matter 필터가 적용된 URL (핵심)
  MATTER_FILTER_URL: 'https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=',
  
  // 페이지네이션 패턴
  PAGINATION_PATTERN: 'https://csa-iot.org/csa-iot_products/page/{pageNumber}/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049',
  
  // 개별 제품 상세 URL 패턴
  PRODUCT_DETAIL_PATTERN: 'https://csa-iot.org/csa-iot_products/{productSlug}/'
};
```

#### URL 매개변수 의미 분석
```typescript
interface CSAFilterParams {
  p_keywords: string;           // 키워드 검색 (빈 문자열 = 전체)
  'p_type[]': number;          // 제품 타입 (14 = Matter 제품)
  'p_program_type[]': number;  // 프로그램 타입 (1049 = Matter 인증)
  p_certificate: string;       // 인증서 번호 필터
  p_family: string;           // 제품 패밀리 필터
  p_firmware_ver: string;     // 펌웨어 버전 필터
}
```

### 2. 환경별 URL 관리

#### 개발 환경 설정
```typescript
// src/ui/utils/environment.ts
export function isDevelopment(): boolean {
  const isLocalhost = typeof window !== 'undefined' && 
                     (window.location.hostname === 'localhost' || 
                      window.location.hostname === '127.0.0.1');
  
  const isDevPort = typeof window !== 'undefined' && 
                   window.location.port === '5173'; // Vite dev server
  
  const isNodeDev = typeof process !== 'undefined' && 
                   process.env.NODE_ENV === 'development';
  
  return isLocalhost || isDevPort || isNodeDev;
}

export function getUIPath() {
  if (isDev()) {
    return 'http://localhost:5123'; // 개발 서버 URL
  } else {
    return path.join(app.getAppPath(), 'dist-react', 'index.html');
  }
}
```

#### 환경별 데이터베이스 경로
```typescript
const getDatabasePath = (env: 'development' | 'production') => {
  const userDataPath = {
    development: path.join(os.homedir(), 'Library', 'Application Support', 'crawlMatterCertis'),
    production: app.getPath('userData')
  }[env];

  return {
    development: path.join(userDataPath, 'dev-database.sqlite'),
    production: path.join(userDataPath, 'production-database.sqlite')
  }[env];
};
```

### 3. API 엔드포인트 설정

#### 개발 API 서버
```typescript
// scripts/devApiServer.ts
const DEV_API_CONFIG = {
  PORT: process.env.PORT || 3001,
  DB_PATH: process.env.DB_PATH || './dev-db.sqlite',
  CORS_ORIGIN: ['http://localhost:5123', 'http://localhost:5173'],
  API_BASE_PATH: '/api'
};

const API_ENDPOINTS = {
  PRODUCTS: '/api/products',
  PRODUCT_BY_ID: '/api/products/:id',
  DATABASE_SUMMARY: '/api/summary',
  SEARCH: '/api/search',
  EXPORT: '/api/export'
};
```

---

## 설정과 코드 분리 패턴

### 1. 설정 파일 계층 구조

#### A. 기본 설정 (코드 내 정의)
```typescript
// src/shared/utils/ConfigUtils.ts - 기본값 정의
// 하드코딩된 안전한 기본값들
// 애플리케이션 최초 실행 시 사용
```

#### B. 런타임 설정 (JSON 파일)
```json
// data/crawler-config.json - 개발 시 기본 설정
// ~/Library/Application Support/crawlMatterCertis/crawler-config.json - 사용자 설정
```

#### C. 환경 변수 (선택적)
```bash
# 개발 환경
NODE_ENV=development
DEV_API_PORT=3001
DEV_DB_PATH=./dev-database.sqlite

# 프로덕션 환경
NODE_ENV=production
LOG_LEVEL=INFO
```

### 2. 설정 우선순위 체계

```typescript
class ConfigManager {
  private resolveConfig(): CrawlerConfig {
    // 1. 기본값 (최하위 우선순위)
    const config = { ...DEFAULT_CONFIG };
    
    // 2. 사용자 설정 파일 오버라이드
    const userConfig = this.loadUserConfig();
    Object.assign(config, userConfig);
    
    // 3. 환경 변수 오버라이드 (최고 우선순위)
    const envOverrides = this.getEnvironmentOverrides();
    Object.assign(config, envOverrides);
    
    return config;
  }

  private getEnvironmentOverrides(): Partial<CrawlerConfig> {
    const overrides: Partial<CrawlerConfig> = {};
    
    if (process.env.PAGE_RANGE_LIMIT) {
      overrides.pageRangeLimit = parseInt(process.env.PAGE_RANGE_LIMIT);
    }
    
    if (process.env.CRAWLER_BASE_URL) {
      overrides.baseUrl = process.env.CRAWLER_BASE_URL;
    }
    
    if (process.env.LOG_LEVEL) {
      overrides.logging = {
        ...DEFAULT_CONFIG.logging,
        level: process.env.LOG_LEVEL as any
      };
    }
    
    return overrides;
  }
}
```

### 3. 설정 검증 및 타입 안전성

```typescript
// src/shared/domain/ConfigurationValue.ts
export class ConfigurationValidator {
  static validateConfig(config: CrawlerConfig): ValidationResult {
    const errors: Record<string, string[]> = {};

    // URL 검증
    if (!this.isValidUrl(config.baseUrl)) {
      errors.baseUrl = ['Invalid base URL format'];
    }

    if (!this.isValidUrl(config.matterFilterUrl)) {
      errors.matterFilterUrl = ['Invalid matter filter URL format'];
    }

    // 숫자 범위 검증
    if (config.pageRangeLimit < 1 || config.pageRangeLimit > 1000) {
      errors.pageRangeLimit = ['Page range limit must be between 1 and 1000'];
    }

    if (config.batchSize < 1 || config.batchSize > 100) {
      errors.batchSize = ['Batch size must be between 1 and 100'];
    }

    // 타임아웃 검증
    if (config.pageTimeoutMs < 5000 || config.pageTimeoutMs > 300000) {
      errors.pageTimeoutMs = ['Page timeout must be between 5000ms and 300000ms'];
    }

    return {
      isValid: Object.keys(errors).length === 0,
      errors
    };
  }

  private static isValidUrl(url: string): boolean {
    try {
      const urlObj = new URL(url);
      return ['http:', 'https:'].includes(urlObj.protocol);
    } catch {
      return false;
    }
  }
}
```

### 4. 세션 기반 설정 관리

```typescript
// src/ui/services/domain/SessionConfigManager.ts
export class SessionConfigManager {
  private readonly CACHE_TTL_MS = 30000; // 30초 캐시
  
  private config: CrawlerConfig | null = null;
  private lastUpdated: Date | null = null;
  private cacheValidUntil: Date | null = null;
  private isConfigLocked: boolean = false; // 크롤링 중 설정 잠금

  public async loadConfig(forceRefresh = false): Promise<CrawlerConfig> {
    if (!forceRefresh && this.isCacheValid()) {
      return this.config!;
    }

    // 설정 로드 및 캐시 업데이트
    const config = await this.configService.getConfig();
    this.updateCache(config);
    
    return config;
  }

  public lockConfig(): void {
    this.isConfigLocked = true;
  }

  public unlockConfig(): void {
    this.isConfigLocked = false;
  }

  private isCacheValid(): boolean {
    return this.config !== null && 
           this.cacheValidUntil !== null && 
           new Date() < this.cacheValidUntil;
  }
}
```

---

## 환경별 설정 관리

### 1. 개발 환경 설정

#### 개발 모드 감지
```typescript
// src/ui/utils/environment.ts
export function isDevelopment(): boolean {
  // 다중 조건 검사로 개발 환경 확실히 감지
  const checks = [
    typeof window !== 'undefined' && window.location.hostname === 'localhost',
    typeof window !== 'undefined' && window.location.port === '5173',
    typeof process !== 'undefined' && process.env.NODE_ENV === 'development',
    typeof window !== 'undefined' && window.electron && 'isDev' in window.electron
  ];
  
  return checks.some(Boolean);
}
```

#### 개발 전용 설정
```typescript
const DEV_CONFIG_OVERRIDES: Partial<CrawlerConfig> = {
  // 개발 시 더 짧은 타임아웃
  pageTimeoutMs: 30000,
  productDetailTimeoutMs: 30000,
  
  // 개발 시 작은 배치 크기
  batchSize: 5,
  pageRangeLimit: 3,
  
  // 개발 시 상세 로깅
  logging: {
    level: 'DEBUG',
    enableStackTrace: true,
    enableTimestamp: true,
    components: {
      crawler: 'DEBUG',
      database: 'DEBUG',
      ui: 'INFO'
    }
  },
  
  // 개발 시 헤드리스 브라우저 비활성화 (디버깅 용이)
  headlessBrowser: false
};
```

### 2. 프로덕션 환경 설정

#### 프로덕션 최적화 설정
```typescript
const PROD_CONFIG_OVERRIDES: Partial<CrawlerConfig> = {
  // 프로덕션 시 더 긴 타임아웃 (안정성)
  pageTimeoutMs: 120000,
  productDetailTimeoutMs: 120000,
  
  // 프로덕션 시 큰 배치 크기 (효율성)
  batchSize: 50,
  pageRangeLimit: 100,
  
  // 프로덕션 시 최소 로깅
  logging: {
    level: 'INFO',
    enableStackTrace: false,
    enableTimestamp: true,
    components: {}
  },
  
  // 프로덕션 시 헤드리스 브라우저 (리소스 절약)
  headlessBrowser: true,
  
  // 프로덕션 시 더 높은 동시성
  maxConcurrentTasks: 32,
  initialConcurrency: 24
};
```

### 3. 테스트 환경 설정

#### 테스트 전용 Mock 설정
```typescript
const TEST_CONFIG: CrawlerConfig = {
  ...DEFAULT_CONFIG,
  
  // 테스트용 Mock URL
  baseUrl: 'http://localhost:3001/mock/products',
  matterFilterUrl: 'http://localhost:3001/mock/products/matter',
  
  // 테스트용 짧은 타임아웃
  pageTimeoutMs: 5000,
  productDetailTimeoutMs: 5000,
  
  // 테스트용 작은 배치
  batchSize: 2,
  pageRangeLimit: 3,
  
  // 테스트용 로깅
  logging: {
    level: 'DEBUG',
    enableStackTrace: true,
    enableTimestamp: false, // 테스트 출력 간소화
    components: {
      test: 'DEBUG'
    }
  }
};
```

---

## 설정 검증 및 보안

### 1. 설정 값 검증 체계

#### URL 검증
```typescript
export class URLValidator {
  static validateCrawlingUrls(config: CrawlerConfig): ValidationResult {
    const errors: string[] = [];

    // CSA-IoT 도메인 검증
    if (!config.baseUrl.includes('csa-iot.org')) {
      errors.push('Base URL must be from csa-iot.org domain');
    }

    // HTTPS 강제
    if (!config.baseUrl.startsWith('https://')) {
      errors.push('Base URL must use HTTPS protocol');
    }

    // Matter 필터 매개변수 검증
    const matterUrl = new URL(config.matterFilterUrl);
    const params = matterUrl.searchParams;
    
    if (!params.has('p_type[]') || params.get('p_type[]') !== '14') {
      errors.push('Matter filter URL must include Matter product type (p_type[]=14)');
    }

    return {
      isValid: errors.length === 0,
      errors
    };
  }
}
```

#### 성능 매개변수 검증
```typescript
export class PerformanceValidator {
  static validatePerformanceSettings(config: CrawlerConfig): ValidationResult {
    const errors: string[] = [];

    // 동시성 한계 검증
    if (config.maxConcurrentTasks > 50) {
      errors.push('Max concurrent tasks should not exceed 50 to prevent server overload');
    }

    // 요청 지연 검증
    if (config.minRequestDelayMs < 50) {
      errors.push('Minimum request delay should be at least 50ms to respect server limits');
    }

    // 배치 크기 검증
    if (config.batchSize > 100) {
      errors.push('Batch size should not exceed 100 to prevent memory issues');
    }

    // 타임아웃 균형 검증
    if (config.pageTimeoutMs < config.productDetailTimeoutMs) {
      errors.push('Page timeout should not be less than product detail timeout');
    }

    return {
      isValid: errors.length === 0,
      errors
    };
  }
}
```

### 2. 보안 고려사항

#### 민감 정보 처리
```typescript
export class ConfigSecurity {
  // 로그 출력 시 민감 정보 마스킹
  static sanitizeConfigForLogging(config: CrawlerConfig): any {
    const sanitized = { ...config };
    
    // 사용자 에이전트 마스킹 (개인정보 포함 가능)
    if (sanitized.customUserAgent) {
      sanitized.customUserAgent = '***MASKED***';
    }
    
    // 파일 경로 마스킹 (시스템 정보 노출 방지)
    if (sanitized.lastExcelExportPath) {
      sanitized.lastExcelExportPath = sanitized.lastExcelExportPath
        .replace(/\/Users\/[^\/]+/, '/Users/***')
        .replace(/C:\\Users\\[^\\]+/, 'C:\\Users\\***');
    }
    
    return sanitized;
  }

  // 설정 파일 권한 검증
  static validateConfigFilePermissions(configPath: string): boolean {
    try {
      const stats = fs.statSync(configPath);
      const mode = stats.mode;
      
      // 읽기/쓰기 권한만 허용 (실행 권한 제거)
      const allowedMode = parseInt('600', 8); // -rw-------
      
      return (mode & parseInt('777', 8)) <= allowedMode;
    } catch {
      return false;
    }
  }
}
```

#### 설정 파일 암호화 (향후 구현)
```typescript
export class ConfigEncryption {
  // 민감한 설정 항목 암호화
  static encryptSensitiveConfig(config: CrawlerConfig, key: string): string {
    const sensitiveFields = {
      customUserAgent: config.customUserAgent,
      lastExcelExportPath: config.lastExcelExportPath
    };

    const encrypted = encrypt(JSON.stringify(sensitiveFields), key);
    return encrypted;
  }

  // 암호화된 설정 복호화
  static decryptSensitiveConfig(encryptedData: string, key: string): any {
    try {
      const decrypted = decrypt(encryptedData, key);
      return JSON.parse(decrypted);
    } catch {
      return {};
    }
  }
}
```

---

## 향후 Rust/Tauri 전환 계획

### 1. Rust 설정 관리 아키텍처

#### Rust 구조 설계
```rust
// src-tauri/src/config/mod.rs
pub mod manager;
pub mod validation;
pub mod persistence;
pub mod environment;

// 설정 구조체 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    // Core crawler settings
    pub page_range_limit: u32,
    pub product_list_retry_count: u32,
    pub product_detail_retry_count: u32,
    pub products_per_page: u32,
    pub auto_add_to_local_db: bool,
    pub auto_status_check: bool,
    pub crawler_type: CrawlerType,

    // URLs and endpoints
    pub base_url: String,
    pub matter_filter_url: String,

    // Performance settings
    pub page_timeout_ms: u64,
    pub product_detail_timeout_ms: u64,
    pub max_concurrent_tasks: u32,
    pub request_delay_ms: u64,

    // Batch processing
    pub batch_size: u32,
    pub batch_delay_ms: u64,
    pub enable_batch_processing: bool,

    // Logging configuration
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlerType {
    Reqwest,
    Playwright,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub enable_stack_trace: bool,
    pub enable_timestamp: bool,
    pub components: HashMap<String, LogLevel>,
}
```

#### Rust 설정 관리자
```rust
// src-tauri/src/config/manager.rs
use serde_json;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::api::path::app_data_dir;

pub struct ConfigManager {
    config: Arc<RwLock<CrawlerConfig>>,
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self, ConfigError> {
        let config_path = Self::get_config_path()?;
        let config = Self::load_or_create_config(&config_path)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
        })
    }

    pub fn get_config(&self) -> Result<CrawlerConfig, ConfigError> {
        let config = self.config.read()
            .map_err(|_| ConfigError::LockError)?;
        Ok(config.clone())
    }

    pub fn update_config(&self, updates: PartialCrawlerConfig) -> Result<(), ConfigError> {
        let mut config = self.config.write()
            .map_err(|_| ConfigError::LockError)?;

        // 검증
        let updated_config = merge_config(&*config, updates)?;
        validate_config(&updated_config)?;

        // 업데이트 및 저장
        *config = updated_config;
        self.save_config(&*config)?;

        Ok(())
    }

    fn get_config_path() -> Result<PathBuf, ConfigError> {
        let app_data = app_data_dir("crawlMatterCertis")
            .ok_or(ConfigError::PathError)?;
        
        std::fs::create_dir_all(&app_data)
            .map_err(ConfigError::IoError)?;

        Ok(app_data.join("crawler-config.json"))
    }

    fn load_or_create_config(path: &PathBuf) -> Result<CrawlerConfig, ConfigError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(ConfigError::IoError)?;
            
            let mut config: CrawlerConfig = serde_json::from_str(&content)
                .map_err(ConfigError::SerializationError)?;

            // 기본값과 병합
            config = merge_with_defaults(config);
            
            Ok(config)
        } else {
            let default_config = CrawlerConfig::default();
            let _ = std::fs::write(path, serde_json::to_string_pretty(&default_config)?);
            Ok(default_config)
        }
    }
}
```

### 2. Tauri 명령어 통합

#### 설정 관련 Tauri 명령어
```rust
// src-tauri/src/commands/config.rs
use tauri::State;
use crate::config::ConfigManager;

#[tauri::command]
pub async fn get_config(
    config_manager: State<'_, ConfigManager>
) -> Result<CrawlerConfig, String> {
    config_manager.get_config()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_config(
    config_manager: State<'_, ConfigManager>,
    updates: serde_json::Value
) -> Result<(), String> {
    let partial_config: PartialCrawlerConfig = serde_json::from_value(updates)
        .map_err(|e| format!("Invalid config format: {}", e))?;

    config_manager.update_config(partial_config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_config(
    config_manager: State<'_, ConfigManager>
) -> Result<CrawlerConfig, String> {
    config_manager.reset_to_defaults()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_config(
    config: CrawlerConfig
) -> Result<ValidationResult, String> {
    crate::config::validation::validate_config(&config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config_path(
    config_manager: State<'_, ConfigManager>
) -> Result<String, String> {
    Ok(config_manager.get_config_path().display().to_string())
}
```

#### 환경 감지 명령어
```rust
// src-tauri/src/commands/environment.rs
#[tauri::command]
pub async fn get_environment_info() -> EnvironmentInfo {
    EnvironmentInfo {
        is_development: cfg!(debug_assertions),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        config_dir: get_config_directory(),
        data_dir: get_data_directory(),
    }
}

#[derive(Serialize)]
pub struct EnvironmentInfo {
    pub is_development: bool,
    pub os: String,
    pub arch: String,
    pub app_version: String,
    pub config_dir: String,
    pub data_dir: String,
}
```

### 3. SolidJS 프론트엔드 통합

#### SolidJS 설정 관리 Store
```typescript
// src/stores/configStore.ts
import { invoke } from '@tauri-apps/api/tauri';
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';

interface ConfigStore {
  config: CrawlerConfig | null;
  isLoading: boolean;
  error: string | null;
  isDirty: boolean;
  lastSaved: Date | null;
}

export function createConfigStore() {
  const [store, setStore] = createStore<ConfigStore>({
    config: null,
    isLoading: false,
    error: null,
    isDirty: false,
    lastSaved: null
  });

  const loadConfig = async () => {
    setStore('isLoading', true);
    setStore('error', null);
    
    try {
      const config = await invoke<CrawlerConfig>('get_config');
      setStore({
        config,
        isLoading: false,
        isDirty: false
      });
    } catch (error) {
      setStore({
        isLoading: false,
        error: error as string
      });
    }
  };

  const updateConfig = async (updates: Partial<CrawlerConfig>) => {
    if (!store.config) return;

    setStore('isLoading', true);
    
    try {
      await invoke('update_config', { updates });
      await loadConfig(); // 최신 상태로 다시 로드
      
      setStore({
        isDirty: false,
        lastSaved: new Date()
      });
    } catch (error) {
      setStore({
        isLoading: false,
        error: error as string
      });
    }
  };

  const resetConfig = async () => {
    setStore('isLoading', true);
    
    try {
      const config = await invoke<CrawlerConfig>('reset_config');
      setStore({
        config,
        isLoading: false,
        isDirty: false,
        lastSaved: new Date()
      });
    } catch (error) {
      setStore({
        isLoading: false,
        error: error as string
      });
    }
  };

  return {
    store,
    loadConfig,
    updateConfig,
    resetConfig,
    setDirty: (dirty: boolean) => setStore('isDirty', dirty),
    clearError: () => setStore('error', null)
  };
}
```

### 4. 설정 파일 마이그레이션

#### TypeScript → Rust 변환기
```rust
// src-tauri/src/migration/config_migrator.rs
use serde_json::Value;

pub struct ConfigMigrator;

impl ConfigMigrator {
    pub fn migrate_from_electron(electron_config_path: &Path) -> Result<CrawlerConfig, MigrationError> {
        let content = std::fs::read_to_string(electron_config_path)?;
        let electron_config: Value = serde_json::from_str(&content)?;

        let migrated_config = Self::convert_electron_to_rust_config(electron_config)?;
        Self::validate_migrated_config(&migrated_config)?;

        Ok(migrated_config)
    }

    fn convert_electron_to_rust_config(electron_config: Value) -> Result<CrawlerConfig, MigrationError> {
        let config = CrawlerConfig {
            page_range_limit: electron_config["pageRangeLimit"].as_u64().unwrap_or(10) as u32,
            product_list_retry_count: electron_config["productListRetryCount"].as_u64().unwrap_or(9) as u32,
            base_url: electron_config["baseUrl"].as_str().unwrap_or("https://csa-iot.org/csa-iot_products/").to_string(),
            crawler_type: match electron_config["crawlerType"].as_str() {
                Some("axios") => CrawlerType::Reqwest,
                Some("playwright") => CrawlerType::Playwright,
                _ => CrawlerType::Reqwest,
            },
            // ... 다른 필드들 변환
        };

        Ok(config)
    }
}
```

---

## 마이그레이션 전략

### 1. 단계별 마이그레이션 계획

#### Phase 1: 기반 구조 구축 (2주)
- [ ] Rust 프로젝트 설정 및 기본 구조 생성
- [ ] Tauri 프로젝트 초기화
- [ ] 기본 설정 구조체 정의
- [ ] SolidJS 프론트엔드 설정

#### Phase 2: 설정 관리 시스템 구현 (3주)
- [ ] Rust ConfigManager 구현
- [ ] 설정 검증 시스템 구현
- [ ] Tauri 명령어 구현
- [ ] SolidJS 설정 스토어 구현

#### Phase 3: 기존 설정 마이그레이션 (1주)
- [ ] TypeScript → Rust 설정 변환기 구현
- [ ] 기존 설정 파일 자동 감지 및 변환
- [ ] 마이그레이션 검증 시스템

#### Phase 4: 통합 테스트 및 최적화 (2주)
- [ ] 설정 시스템 통합 테스트
- [ ] 성능 테스트 및 최적화
- [ ] 에러 처리 및 복구 시스템
- [ ] 문서화 완료

### 2. 호환성 보장 전략

#### 설정 스키마 호환성
```rust
// 버전별 설정 스키마 관리
#[derive(Serialize, Deserialize)]
pub struct ConfigV1 {
    // 초기 버전 설정
}

#[derive(Serialize, Deserialize)]
pub struct ConfigV2 {
    // 개선된 버전 설정
}

pub enum ConfigVersion {
    V1(ConfigV1),
    V2(ConfigV2),
}

impl ConfigManager {
    fn migrate_config_version(old_config: ConfigVersion) -> Result<CrawlerConfig, MigrationError> {
        match old_config {
            ConfigVersion::V1(v1) => Self::migrate_v1_to_v2(v1),
            ConfigVersion::V2(v2) => Self::migrate_v2_to_current(v2),
        }
    }
}
```

#### 백워드 호환성 보장
```rust
// 기존 Electron 설정 파일 자동 감지 및 변환
pub fn detect_and_migrate_electron_config() -> Result<Option<CrawlerConfig>, MigrationError> {
    let possible_paths = vec![
        get_electron_user_config_path(),
        get_electron_project_config_path(),
    ];

    for path in possible_paths {
        if path.exists() {
            println!("Found Electron config at: {:?}", path);
            let migrated = ConfigMigrator::migrate_from_electron(&path)?;
            
            // 기존 파일 백업
            Self::backup_electron_config(&path)?;
            
            return Ok(Some(migrated));
        }
    }

    Ok(None)
}
```

### 3. 설정 관리 개선사항

#### 성능 최적화
- **설정 캐싱**: 메모리 기반 설정 캐시로 빠른 액세스
- **지연 로딩**: 필요한 설정만 선택적 로딩
- **변경 감지**: 파일 시스템 감시로 자동 설정 갱신

#### 보안 강화
- **설정 암호화**: 민감한 설정 항목 암호화 저장
- **권한 관리**: 설정 파일 접근 권한 엄격 관리
- **변경 추적**: 설정 변경 이력 로깅

#### 사용자 경험 개선
- **설정 유효성 실시간 검증**: 입력 시 즉시 피드백
- **설정 가져오기/내보내기**: JSON/TOML 형식 지원
- **설정 프리셋**: 일반적인 사용 사례별 미리 정의된 설정

#### 설정 잠금 및 상태 관리 강화
- **동적 설정 잠금**: 크롤링 중 설정 변경 방지 시스템
- **상태 기반 UI 제어**: 세션 상태에 따른 설정 UI 활성화/비활성화
- **친절한 사용자 안내**: 설정 변경 불가 상황 명확한 안내
- **안전한 작업 중지**: 사용자 선택으로 크롤링 중지 후 설정 변경
- **실시간 상태 동기화**: Frontend/Backend 설정 상태 일치 보장

### 4. 예상 이점

#### 성능 개선
- **메모리 사용량**: 70% 감소 (Rust의 효율적 메모리 관리)
- **설정 로딩 속도**: 5배 향상 (네이티브 바이너리)
- **파일 I/O 성능**: 3배 향상 (Rust의 효율적 I/O)

#### 안정성 향상
- **타입 안전성**: 컴파일 타임 설정 검증
- **메모리 안전성**: Rust의 소유권 시스템
- **동시성 안전성**: 안전한 멀티스레드 설정 접근

#### 개발 효율성
- **코드 중복 제거**: 단일 설정 소스
- **유지보수성**: 명확한 설정 아키텍처
- **확장성**: 새로운 설정 항목 쉬운 추가

이 문서는 현재 프로젝트의 설정 관리 시스템을 완전히 분석하고, 향후 Rust/Tauri 전환을 위한 구체적인 실행 계획을 제시합니다. 각 단계별 구현 가이드와 예상 이점을 통해 체계적인 마이그레이션이 가능할 것입니다.
