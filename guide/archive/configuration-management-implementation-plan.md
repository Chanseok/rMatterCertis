# Matter Certis v2 - 설정 관리 구현 계획

> 현재 구현 상황을 고려한 설정 관리 시스템의 단계별 구현 계획

## 📋 목차

1. [현재 구현 상황 분석](#현재-구현-상황-분석)
2. [핵심 구현 목표](#핵심-구현-목표)
3. [단계별 구현 계획](#단계별-구현-계획)
4. [우선순위 기반 구현 순서](#우선순위-기반-구현-순서)
5. [각 단계별 상세 구현](#각-단계별-상세-구현)

## 현재 구현 상황 분석

### ✅ 이미 완성된 부분

#### 1. Backend 기반 구조
- ✅ **SessionManager**: 메모리 기반 세션 상태 관리 완료
- ✅ **DatabaseConnection**: 안정적인 DB 연결 및 마이그레이션
- ✅ **Tauri Commands**: 기본적인 backend-frontend 통신 구조
- ✅ **Repository Pattern**: 데이터 액세스 계층 완성

#### 2. Frontend 기본 구조
- ✅ **SolidJS Components**: Settings UI 컴포넌트 구현
- ✅ **Toast/Modal System**: 사용자 피드백 시스템
- ✅ **Config Types**: TypeScript 타입 정의

#### 3. 설정 스키마 및 문서화
- ✅ **완전한 설정 스키마 정의**: 모든 설정 항목 명세
- ✅ **환경별 설정 전략**: 개발/프로덕션/테스트 환경 구분
- ✅ **설정 우선순위 체계**: 기본값 → 파일 → 환경변수

### 🔄 부분적으로 구현된 부분

#### 1. 설정 잠금 메커니즘
- 🔄 **SessionConfigManager 설계**: 잠금 로직은 설계되었으나 실제 구현 필요
- 🔄 **UI 상태 연동**: 크롤링 상태에 따른 UI 제어 로직 필요

#### 2. Tauri Commands
- 🔄 **기본 commands**: 일부 구현되었으나 설정 관련 commands 추가 필요
- 🔄 **상태 동기화**: Frontend/Backend 상태 일치 보장 로직 필요

### ❌ 미구현 부분

#### 1. 설정 관리 Core System
- ❌ **ConfigManager (Rust)**: 설정 파일 읽기/쓰기/검증
- ❌ **설정 잠금 시스템**: 크롤링 중 설정 변경 방지
- ❌ **실시간 상태 동기화**: 설정 변경 시 즉시 반영

#### 2. 사용자 경험 기능
- ❌ **설정 변경 불가 시 안내**: 친절한 메시지 및 대안 제시
- ❌ **안전한 작업 중지**: 크롤링 중지 후 설정 변경 플로우
- ❌ **설정 프리셋 시스템**: 미리 정의된 설정 조합

## 핵심 구현 목표

### 🎯 1차 목표 (핵심 기능)
1. **설정 파일 기반 관리**: JSON 파일 읽기/쓰기/검증
2. **크롤링 중 설정 잠금**: 작업 중 설정 변경 방지
3. **Frontend/Backend 동기화**: 단일 진실 소스 보장

### 🎯 2차 목표 (사용자 경험)
1. **친절한 사용자 안내**: 설정 변경 불가 상황 명확한 피드백
2. **안전한 작업 제어**: 크롤링 중지 및 재시작 플로우
3. **실시간 상태 반영**: 설정 변경 즉시 UI 업데이트

### 🎯 3차 목표 (고급 기능)
1. **설정 프리셋**: 개발/프로덕션/테스트용 미리 정의된 설정
2. **설정 가져오기/내보내기**: JSON 파일 기반 설정 공유
3. **변경 이력 추적**: 설정 변경 내역 로깅

## 단계별 구현 계획

### Phase 1: 핵심 설정 관리 시스템 (1-2주)
**목표**: 기본적인 설정 파일 관리 및 Tauri 연동

#### 구현 항목
1. **Rust ConfigManager 구현**
   - 설정 파일 읽기/쓰기
   - 기본값 처리 및 검증
   - 환경별 설정 병합

2. **Tauri Commands 확장**
   - `get_config()`: 현재 설정 조회
   - `update_config()`: 설정 업데이트
   - `reset_config()`: 기본값으로 재설정

3. **Frontend Store 연동**
   - SolidJS 설정 스토어 구현
   - 실시간 설정 상태 관리
   - 변경 감지 및 더티 플래그

### Phase 2: 설정 잠금 시스템 (1주)
**목표**: 크롤링 중 설정 변경 방지 및 상태 기반 UI 제어

#### 구현 항목
1. **설정 잠금 로직**
   - SessionManager와 연동한 잠금 상태 확인
   - 활성 세션 존재 시 설정 변경 차단

2. **UI 상태 제어**
   - 크롤링 중 설정 폼 비활성화
   - 잠금 상태 시각적 표시

3. **사용자 피드백**
   - 설정 변경 불가 알림
   - 대안 행동 안내 (작업 중지 옵션)

### Phase 3: 사용자 경험 개선 (1주)
**목표**: 친절한 안내 및 안전한 작업 제어

#### 구현 항목
1. **작업 중지 플로우**
   - 현재 크롤링 안전하게 중지
   - 설정 변경 후 재시작 옵션

2. **향상된 피드백**
   - 단계별 진행 상황 표시
   - 설정 변경 사유 및 영향 설명

3. **설정 검증 강화**
   - 실시간 유효성 검사
   - 설정 충돌 사전 감지

### Phase 4: 고급 기능 (1-2주)
**목표**: 설정 프리셋 및 편의 기능

#### 구현 항목
1. **설정 프리셋**
   - 환경별 미리 정의된 설정
   - 원클릭 설정 적용

2. **설정 관리 도구**
   - 설정 가져오기/내보내기
   - 설정 비교 및 차이점 표시

3. **모니터링 및 로깅**
   - 설정 변경 이력
   - 성능 지표 연동

## 우선순위 기반 구현 순서

### 🔥 최우선 (즉시 시작)
1. **ConfigManager 기본 구현**
2. **핵심 Tauri Commands**
3. **Frontend Store 연동**

### ⚡ 높은 우선순위 (1주 내)
1. **설정 잠금 시스템**
2. **UI 상태 제어**
3. **기본 사용자 피드백**

### 📈 중간 우선순위 (2-3주 내)
1. **작업 중지 플로우**
2. **설정 검증 강화**
3. **에러 처리 개선**

### 🌟 낮은 우선순위 (여유 시)
1. **설정 프리셋 시스템**
2. **가져오기/내보내기**
3. **변경 이력 추적**

## 각 단계별 상세 구현

> **중요**: 모든 새로운 코드는 mod.rs 파일을 사용하지 않고 명시적인 파일명으로 구성합니다.

### Phase 1: ConfigManager 구현

#### 1.1 Rust ConfigManager 기본 구조 (mod.rs 사용 금지)
```rust
// src-tauri/src/lib.rs에 모듈 추가
pub mod infrastructure {
    pub mod config_manager;        // 새 파일
    pub mod config_types;          // 새 파일  
    pub mod config_validation;     // 새 파일
    // 기존 모듈들...
    pub mod crawler;
    pub mod database_connection;
    pub mod http_client;
    pub mod repositories;
}

// src-tauri/src/infrastructure/config_manager.rs (새 파일)
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::infrastructure::config_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    pub http: HttpConfig,
    pub database: DatabaseConfig,
    pub crawling: CrawlingConfig,
    pub ui: UiConfig,
}

pub struct ConfigManager {
    config: Arc<RwLock<CrawlerConfig>>,
    file_path: String,
    is_locked: Arc<RwLock<bool>>,
}

impl ConfigManager {
    pub async fn new(file_path: String) -> Result<Self, ConfigError> {
        let config = Self::load_from_file(&file_path)?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            file_path,
            is_locked: Arc::new(RwLock::new(false)),
        })
    }
    
    pub async fn get_config(&self) -> CrawlerConfig {
        self.config.read().await.clone()
    }
    
    pub async fn update_config(&self, partial: PartialCrawlerConfig) -> Result<(), ConfigError> {
        if *self.is_locked.read().await {
            return Err(ConfigError::ConfigLocked);
        }
        
        let mut config = self.config.write().await;
        // Apply partial updates and save to file
        Ok(())
    }
    
    pub async fn lock(&self) {
        *self.is_locked.write().await = true;
    }
    
    pub async fn unlock(&self) {
        *self.is_locked.write().await = false;
    }
}

// src-tauri/src/infrastructure/config_types.rs (새 파일)
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub user_agent: String,
    pub rate_limit_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    pub max_pages: u32,
    pub concurrent_requests: u32,
    pub delay_ms: u64,
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub auto_refresh_interval: u64,
    pub max_log_entries: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration is locked during crawling")]
    ConfigLocked,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("File operation failed: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
```

#### 1.2 Tauri Commands 확장 (명시적 구조)
```rust
// src-tauri/src/commands.rs에 추가 (mod.rs 사용 안 함)
use crate::infrastructure::config_manager::ConfigManager;
use crate::infrastructure::config_types::{CrawlerConfig, PartialCrawlerConfig, ConfigError};

#[tauri::command]
pub async fn get_config(
    config_manager: tauri::State<'_, ConfigManager>
) -> Result<CrawlerConfig, String> {
    config_manager.get_config().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_config(
    config_manager: tauri::State<'_, ConfigManager>,
    updates: serde_json::Value
) -> Result<(), String> {
    let partial_config: PartialCrawlerConfig = 
        serde_json::from_value(updates)
        .map_err(|e| format!("Invalid config format: {}", e))?;

    config_manager.update_config(partial_config).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_config_locked(
    config_manager: tauri::State<'_, ConfigManager>
) -> Result<bool, String> {
    Ok(config_manager.is_locked().await)
}

#[tauri::command]
pub async fn lock_config_for_crawling(
    config_manager: tauri::State<'_, ConfigManager>
) -> Result<(), String> {
    config_manager.lock().await;
    Ok(())
}

#[tauri::command]
pub async fn unlock_config_after_crawling(
    config_manager: tauri::State<'_, ConfigManager>
) -> Result<(), String> {
    config_manager.unlock().await;
    Ok(())
}
```

#### 1.3 Frontend Store 구현 (타입 안전성 강화)
```typescript
// src/stores/config_store.ts (새 파일, mod 방식 사용 안 함)
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import { invoke } from '@tauri-apps/api/tauri';

// 타입 정의
interface CrawlerConfig {
  http: HttpConfig;
  database: DatabaseConfig;
  crawling: CrawlingConfig;
  ui: UiConfig;
}

interface ConfigStore {
  config: CrawlerConfig | null;
  isLoading: boolean;
  error: string | null;
  isDirty: boolean;
  isLocked: boolean;
  lastSaved: Date | null;
}

export function createConfigStore() {
  const [store, setStore] = createStore<ConfigStore>({
    config: null,
    isLoading: false,
    error: null,
    isDirty: false,
    isLocked: false,
    lastSaved: null
  });

  const [lockReason, setLockReason] = createSignal<string | null>(null);

  const loadConfig = async () => {
    setStore('isLoading', true);
    setStore('error', null);
    
    try {
      const config = await invoke<CrawlerConfig>('get_config');
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

  const updateConfig = async (updates: Partial<CrawlerConfig>) => {
    if (store.isLocked) {
      throw new Error('설정이 잠겨있어 변경할 수 없습니다.');
    }

    try {
      await invoke('update_config', { updates });
      setStore('isDirty', false);
      setStore('lastSaved', new Date());
      await loadConfig(); // 최신 상태로 다시 로드
    } catch (error) {
      throw new Error(`설정 업데이트 실패: ${error}`);
    }
  };

  const checkLockStatus = async () => {
    try {
      const isLocked = await invoke<boolean>('is_config_locked');
      const reason = isLocked ? 
        await invoke<string | null>('get_lock_reason') : null;
      
      setStore('isLocked', isLocked);
      setLockReason(reason);
    } catch (error) {
      console.error('Lock status check failed:', error);
    }
  };

  // 정기적으로 잠금 상태 확인
  setInterval(checkLockStatus, 2000);

  return { 
    store, 
    loadConfig, 
    updateConfig, 
    checkLockStatus,
    lockReason
  };
}
```

### Phase 2: 설정 잠금 시스템

#### 2.1 잠금 상태 확인 Command
```rust
#[tauri::command]
pub async fn is_config_locked(
    session_manager: State<'_, SessionManager>
) -> Result<bool, String> {
    let active_sessions = session_manager.get_active_sessions().await;
    Ok(!active_sessions.is_empty())
}

#[tauri::command]
pub async fn get_lock_reason(
    session_manager: State<'_, SessionManager>
) -> Result<Option<String>, String> {
    let active_sessions = session_manager.get_active_sessions().await;
    if active_sessions.is_empty() {
        Ok(None)
    } else {
        let session_info = active_sessions.into_iter()
            .map(|s| format!("세션 {}: {} ({}페이지)", 
                s.session_id, s.status, s.current_page))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Some(format!("진행 중인 크롤링: {}", session_info)))
    }
}
```

#### 2.2 UI 잠금 상태 표시
```tsx
// src/components/features/settings/Settings.tsx
const Settings = () => {
  const { store, loadConfig, updateConfig } = useConfigStore();
  const [lockInfo, setLockInfo] = createSignal<string | null>(null);

  // 정기적으로 잠금 상태 확인
  createEffect(() => {
    const checkLockStatus = async () => {
      const isLocked = await invoke<boolean>('is_config_locked');
      const reason = await invoke<string | null>('get_lock_reason');
      setStore('isLocked', isLocked);
      setLockInfo(reason);
    };

    const interval = setInterval(checkLockStatus, 2000);
    return () => clearInterval(interval);
  });

  return (
    <div class="settings-section">
      <Show when={store.isLocked}>
        <div class="lock-banner">
          <Icon name="lock" />
          <span>설정이 잠겨있습니다: {lockInfo()}</span>
          <Button onClick={handleStopCrawling}>
            크롤링 중지하고 설정 변경
          </Button>
        </div>
      </Show>
      
      <form class="settings-form">
        <fieldset disabled={store.isLocked}>
          {/* 모든 설정 입력 필드 */}
        </fieldset>
      </form>
    </div>
  );
};
```

### Phase 3: 작업 중지 플로우

#### 3.1 안전한 크롤링 중지
```rust
#[tauri::command]
pub async fn stop_all_sessions_for_config(
    session_manager: State<'_, SessionManager>
) -> Result<Vec<String>, String> {
    let active_sessions = session_manager.get_active_sessions().await;
    let mut stopped_sessions = Vec::new();

    for session in active_sessions {
        session_manager.set_status(&session.session_id, SessionStatus::Stopped)
            .await
            .map_err(|e| e.to_string())?;
        stopped_sessions.push(session.session_id);
    }

    Ok(stopped_sessions)
}
```

#### 3.2 사용자 확인 플로우
```tsx
const handleStopCrawling = async () => {
  const result = await showConfirmDialog({
    title: "크롤링 중지",
    message: "진행 중인 모든 크롤링을 중지하고 설정을 변경하시겠습니까?",
    confirmText: "중지하고 설정 변경",
    cancelText: "취소"
  });

  if (result) {
    try {
      const stoppedSessions = await invoke<string[]>('stop_all_sessions_for_config');
      setToast({
        type: 'success',
        message: `${stoppedSessions.length}개 세션이 중지되었습니다. 이제 설정을 변경할 수 있습니다.`
      });
      // 잠금 상태 새로고침
      await checkLockStatus();
    } catch (error) {
      setToast({
        type: 'error',
        message: `크롤링 중지 실패: ${error}`
      });
    }
  }
};
```

## 구현 시작점

### 즉시 시작할 수 있는 작업 (mod.rs 없는 구조)

1. **ConfigManager 기본 구조 생성**
   - `src-tauri/src/infrastructure/config_manager.rs` 생성
   - `src-tauri/src/infrastructure/config_types.rs` 생성  
   - `src-tauri/src/infrastructure/config_validation.rs` 생성
   - `src-tauri/src/lib.rs`에 명시적 모듈 경로 추가

2. **설정 파일 스키마 정의**
   - `config/default.json` 기본 설정 파일 생성
   - `config/development.json` 개발 환경 설정
   - `config/production.json` 프로덕션 설정

3. **Tauri Commands 확장**
   - 기존 `src-tauri/src/commands.rs`에 설정 관련 함수 추가
   - State 관리 구조 설정
   - `src-tauri/src/main.rs`에 ConfigManager 등록

4. **Frontend Store 생성**
   - `src/stores/config_store.ts` 새 파일 생성
   - 타입 정의 및 실시간 상태 동기화
   - 잠금 상태 모니터링 로직

### 파일 구조 예시 (mod.rs 사용 금지)
```
src-tauri/src/
├── lib.rs                              # 명시적 모듈 경로 정의
├── main.rs                            # ConfigManager 등록
├── commands.rs                        # 설정 관련 Tauri commands
├── infrastructure/
│   ├── config_manager.rs              # 핵심 설정 관리 로직
│   ├── config_types.rs                # 설정 구조체 정의
│   ├── config_validation.rs           # 설정 검증 로직
│   ├── crawler.rs                     # 기존 크롤링 엔진
│   ├── database_connection.rs         # 기존 DB 연결
│   └── ...                           # 기타 인프라 컴포넌트
└── config/
    ├── default.json                   # 기본 설정
    ├── development.json               # 개발 환경
    └── production.json                # 프로덕션 환경

src/
├── stores/
│   ├── config_store.ts               # 설정 상태 관리
│   └── index.ts                      # 스토어 exports
└── components/
    └── features/
        └── settings/
            ├── settings_form.tsx      # 설정 UI 컴포넌트
            ├── lock_indicator.tsx     # 잠금 상태 표시
            └── validation_display.tsx # 검증 결과 표시
```

이 계획을 통해 mod.rs 없이 명확하고 유지보수하기 쉬운 구조로 설정 관리 시스템을 구축할 수 있습니다.
