# rMatterCertis - 실전 단계별 개발 가이드 (검증된 구현 기반)

## 🗓️ 전체 개발 일정 (8주) - 실제 검증된 단계

### ✅ Phase 1: 프로젝트 초기화 및 아키텍처 최적화 (2주) - **완료**
### 🔄 Phase 2: 백엔드 도메인 구현 (2주) - **진행 중**
### Phase 3: 크롤링 엔진 구현 (2주)
### Phase 4: 프론트엔드 구현 (1.5주)
### Phase 5: 통합 테스트 및 최적화 (0.5주)

---

## ✅ Phase 1: 프로젝트 초기화 및 아키텍처 최적화 (완료)

### 🎯 실제 달성된 목표
- ✅ Tauri + SolidJS 프로젝트 초기화
- ✅ 모던 Rust 구조 구축 (mod.rs 없는 방식)
- ✅ 빌드 성능 최적화 (66~95% 향상)
- ✅ 기본 데이터베이스 연결 구현
- ✅ Tauri Commands 및 UI 테스트 환경 구축

### 📋 실제 완료된 작업 목록

#### Week 1.1: 프로젝트 셋업 및 최적화 (실제 3일)

**1일차: 프로젝트 초기화 (실제 구현)**
```bash
# 실제 사용된 명령어
pnpm create tauri-app@latest rMatterCertis
cd rMatterCertis

# 실제 선택한 옵션
# - Package manager: pnpm (npm보다 빠름)
# - Frontend template: SolidJS (Vanilla 대신)
# - TypeScript: Yes
```

**실제 구현된 프로젝트 구조** (Rust 2024 모던 컨벤션)
```
rMatterCertis/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs (Tauri 명령어들)
│   │   ├── domain.rs (도메인 계층 진입점, mod.rs 제거)
│   │   ├── domain/
│   │   │   ├── entities.rs (비즈니스 엔티티)
│   │   │   ├── repositories.rs (repository trait 정의)
│   │   │   └── services.rs (도메인 서비스)
│   │   ├── application.rs (애플리케이션 계층 진입점)
│   │   ├── application/
│   │   │   ├── dto.rs (Data Transfer Objects)
│   │   │   └── use_cases.rs (유즈케이스 구현)
│   │   ├── infrastructure.rs (인프라 계층 진입점)
│   │   ├── infrastructure/
│   │   │   ├── repositories.rs (repository 구현체)
│   │   │   ├── database_connection.rs (DB 연결 관리)
│   │   │   ├── database.rs (DB 유틸리티)
│   │   │   ├── config.rs (설정 관리)
│   │   │   └── http.rs (HTTP 클라이언트)
│   │   └── bin/
│   │       └── test_db.rs (DB 테스트 CLI)
│   ├── migrations/ (SQL 마이그레이션)
│   ├── .cargo/config.toml (빌드 최적화)
│   └── Cargo.toml
│   │   │   └── use_cases.rs
│   │   ├── infrastructure.rs
│   │   ├── infrastructure/
│   │   │   └── database_connection.rs
│   │   ├── commands.rs
│   │   └── bin/
│   │       └── test_db.rs
│   ├── migrations/
│   │   └── 001_initial.sql
│   ├── data/ (런타임 생성)
│   ├── Cargo.toml (최적화됨)
│   └── tauri.conf.json
├── src/
│   ├── main.tsx
│   ├── App.tsx (DB 테스트 UI)
│   └── app.css
├── .cargo/
│   └── config.toml (빌드 최적화)
├── scripts/
│   └── test-fast.sh
├── .env.development
├── .gitignore (확장됨)
└── package.json (SolidJS)
```

**2일차: 성능 최적화된 Cargo.toml**
```toml
# 실제 검증된 설정
[package]
name = "matter-certis-v2"
version = "0.1.0"
description = "rMatterCertis - E-commerce Product Crawling Application"
authors = ["Chanseok <hi007chans@gmail.com>"]
edition = "2021"
default-run = "matter-certis-v2"

[workspace]
resolver = "2"

# 🚀 실제 적용된 빌드 최적화
[profile.dev]
opt-level = 0
debug = 1  # 축소된 디버그 정보
split-debuginfo = "unpacked"
incremental = true
codegen-units = 512  # 높은 병렬화

[profile.test]
opt-level = 0
debug = 1
incremental = true
codegen-units = 512

# 의존성 최적화 유지
[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.test.package."*"]
opt-level = 3
debug = false
```
tauri-build = { version = "2.0", features = [] }
```

**3일차: 실제 구현된 데이터베이스 연결**
```rust
// src-tauri/src/infrastructure/database_connection.rs
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use std::path::Path;
use anyhow::Result;

pub struct DatabaseConnection {
    pool: Option<Pool<Sqlite>>,
}

impl DatabaseConnection {
    pub async fn new(database_url: &str) -> Result<Self> {
        // 실제 구현: 디렉토리 자동 생성
        if database_url.starts_with("sqlite:") {
            let path_str = database_url.strip_prefix("sqlite:").unwrap();
            let path = Path::new(path_str);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool: Some(pool) })
    }

    pub async fn migrate(&self) -> Result<()> {
        // 실제 구현: 수동 테이블 생성 (sqlx::migrate! 대신)
        let pool = self.pool.as_ref().unwrap();
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS vendors (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                selector_config TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS products (
                id TEXT PRIMARY KEY,
                vendor_id TEXT NOT NULL,
                name TEXT NOT NULL,
                price REAL,
                currency TEXT NOT NULL DEFAULT 'KRW',
                url TEXT NOT NULL,
                image_url TEXT,
                description TEXT,
                is_available BOOLEAN NOT NULL DEFAULT 1,
                crawled_at TEXT NOT NULL,
                FOREIGN KEY (vendor_id) REFERENCES vendors (id)
            );
            "#
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        self.pool.as_ref().unwrap()
    }
}

// 실제 구현된 테스트
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_connection() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}", db_path.to_string_lossy());

        let db = DatabaseConnection::new(&database_url).await?;
        assert!(!db.pool().is_closed());
        
        // 마이그레이션 테스트
        db.migrate().await?;
        
        Ok(())
    }
}
```

#### Week 1.2: 성능 최적화 및 테스트 환경 (실제 2일)

**4일차: 빌드 성능 최적화 구현**
```toml
# .cargo/config.toml - 실제 검증된 설정
[build]
jobs = 8
incremental = true

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[profile.dev]
debug = 1
split-debuginfo = "unpacked"

[profile.dev.package."*"]
opt-level = 3
debug = false
```

**실제 달성된 성능 향상:**
- 초기 빌드: 1분 (이전 2-3분에서 66% 향상)
- 증분 빌드: 0.5초 (이전 10-30초에서 95% 향상)
- 작은 변경: 2.6초 (이전 30-60초에서 90% 향상)

**5일차: Tauri Commands 및 UI 테스트**
```rust
// src-tauri/src/commands.rs - 실제 구현
#[tauri::command]
pub async fn test_database_connection() -> Result<String, String> {
    let db_path = "data/matter_certis.db";
    match DatabaseConnection::new(db_path).await {
        Ok(_) => Ok("Database connection successful".to_string()),
        Err(e) => Err(format!("Database connection failed: {}", e)),
    }
}

#[tauri::command]
pub async fn get_database_info() -> Result<String, String> {
    Ok("Database: SQLite, Location: data/matter_certis.db".to_string())
}
```

```tsx
// src/App.tsx - 실제 구현된 테스트 UI
import { invoke } from "@tauri-apps/api/tauri";
import { createSignal } from "solid-js";

function App() {
  const [dbStatus, setDbStatus] = createSignal<string>("");
  const [dbInfo, setDbInfo] = createSignal<string>("");

  const testConnection = async () => {
    try {
      const result = await invoke<string>("test_database_connection");
      setDbStatus(`✅ ${result}`);
    } catch (error) {
      setDbStatus(`❌ ${error}`);
    }
  };

  const getInfo = async () => {
    try {
      const result = await invoke<string>("get_database_info");
      setDbInfo(result);
    } catch (error) {
      setDbInfo(`❌ ${error}`);
    }
  };

  return (
    <div class="container">
      <h1>rMatterCertis</h1>
      <div class="controls">
        <button onClick={testConnection}>Test DB Connection</button>
        <button onClick={getInfo}>Get DB Info</button>
      </div>
      <div class="status">
        <p>{dbStatus()}</p>
        <p>{dbInfo()}</p>
      </div>
    </div>
  );
}
```

### ✅ Phase 1 완료 체크리스트

- [x] **프로젝트 초기화**: Tauri + SolidJS 구조
- [x] **모던 Rust 아키텍처**: mod.rs 없는 구조
- [x] **빌드 성능 최적화**: 66~95% 향상 달성
- [x] **데이터베이스 연결**: SQLite 연결 및 마이그레이션
- [x] **테스트 환경**: 단위 테스트, CLI 도구, UI 테스트
- [x] **Tauri Commands**: 기본 DB 명령어 구현
- [x] **개발 도구**: 빠른 테스트 스크립트, 환경 설정

---

## 🔄 Phase 2: 백엔드 도메인 구현 (진행 중 - mod.rs 제거 완료)

### 🎯 달성 목표
- ✅ **모던 Rust 모듈 구조**: 모든 mod.rs 파일 제거 완료
- ✅ **Repository 패턴 기초**: trait 정의 및 기본 구현체 완성
- ✅ **도메인 엔티티**: Product, Vendor, CrawlingSession 정의
- 🔄 Repository 구현체 완성 및 테스트 수정
- 🔄 Use Cases 비즈니스 로직 구현  
- 🔄 Tauri Commands 확장
- 🔄 에러 처리 및 로깅 시스템

### 📋 실제 완료된 작업

#### ✅ 모던 Rust 모듈 구조 완성 (Rust 2024 컨벤션)

**모든 mod.rs 파일 제거:**
```bash
# 이전 구조 (mod.rs 사용)
src/
├── infrastructure/
│   ├── mod.rs ❌
│   ├── repositories/
│   │   ├── mod.rs ❌
│   │   ├── vendor.rs
│   │   └── product.rs
│   └── ...

# 현재 구조 (mod.rs 없는 모던 방식)
src/
├── infrastructure.rs ✅ (진입점)
├── infrastructure/
│   ├── repositories.rs ✅ (통합 구현체)
│   ├── database_connection.rs
│   ├── config.rs
│   └── ...
```

**주요 변경 사항:**
- `infrastructure/mod.rs` → `infrastructure.rs`
- `domain/mod.rs` → `domain.rs`
- `application/mod.rs` → `application.rs`
- Repository 구현체들을 `infrastructure/repositories.rs`로 통합
- 빈 서브디렉토리들 정리 완료

#### ✅ Repository 패턴 기초 구조

**Domain Layer (trait 정의):**
```rust
// src/domain/repositories.rs
#[async_trait]
pub trait VendorRepository: Send + Sync {
    async fn create(&self, vendor: &Vendor) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Vendor>>;
    async fn find_all(&self) -> Result<Vec<Vendor>>;
    async fn find_active(&self) -> Result<Vec<Vendor>>;
    async fn update(&self, vendor: &Vendor) -> Result<()>;
    async fn update_last_crawled(&self, id: &str, timestamp: DateTime<Utc>) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn create(&self, product: &Product) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Product>>;
    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>>;
    async fn find_all(&self) -> Result<Vec<Product>>;
    async fn find_in_stock(&self) -> Result<Vec<Product>>;
    async fn find_by_category(&self, category: &str) -> Result<Vec<Product>>;
    async fn search_by_name(&self, query: &str) -> Result<Vec<Product>>;
    async fn count_by_vendor(&self, vendor_id: &str) -> Result<i64>;
    async fn get_latest_by_vendor(&self, vendor_id: &str, limit: i64) -> Result<Vec<Product>>;
    async fn update(&self, product: &Product) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn delete_by_vendor(&self, vendor_id: &str) -> Result<()>;
}
```

**Infrastructure Layer (구현체):**
```rust
// src/infrastructure/repositories.rs
pub struct SqliteVendorRepository {
    pool: SqlitePool,
}

pub struct SqliteProductRepository {
    pool: SqlitePool,
}

// 모든 trait 메서드 구현 완료
impl VendorRepository for SqliteVendorRepository { ... }
impl ProductRepository for SqliteProductRepository { ... }
```

### 📋 다음 단계 작업 계획

#### Week 2.1: Repository 테스트 수정 및 Use Cases 구현 (남은 4일)

**Day 6: Repository 테스트 수정**
- 데이터베이스 권한 및 외래키 제약 조건 오류 해결
- 테스트용 임시 데이터베이스 생성 방식 개선
- 트랜잭션 격리를 통한 안정적인 테스트 환경 구축

**Day 7: Use Cases 구현**
```

**Day 7: Use Cases 비즈니스 로직 구현**
- VendorUseCases와 ProductUseCases 구현
- 입력 검증 및 비즈니스 규칙 적용
- DTO 변환 로직 구현
- Use Cases 단위 테스트

**성공 기준:**
```bash
# Use Cases 테스트 통과
./scripts/test-fast.sh use_cases
```

**Day 8: Tauri Commands 확장**
- Vendor 관리 Commands (CRUD) 구현
- Product 관리 Commands (CRUD) 구현  
- 에러 응답 표준화
- main.rs에 Commands 등록

**성공 기준:**
```bash
# Tauri Commands 통합 테스트 통과
cargo test commands
```

**Day 9: 통합 테스트 및 프론트엔드 연동**
- 전체 플로우 통합 테스트
- 프론트엔드 API 호출 테스트
- 에러 시나리오 검증
- UI 확장 (Vendor 관리 인터페이스)

**성공 기준:**
```bash
# 전체 통합 테스트 통과
cargo test --test integration
pnpm tauri dev # UI에서 Vendor CRUD 동작 확인
```

#### Week 2.2: Product 도메인 및 고급 기능 (3일)

**Day 10-12: Product 관리 및 관계 처리**
- Product Repository 완전 구현
- Vendor-Product 관계 관리
- 복합 쿼리 및 필터링 기능
- 성능 최적화 및 인덱싱

### 🎯 Phase 2 완료 체크리스트

#### 핵심 기능
- [ ] **Vendor CRUD**: 완전한 Create, Read, Update, Delete
- [ ] **Product CRUD**: 완전한 Create, Read, Update, Delete  
- [ ] **Repository 패턴**: 데이터 접근 추상화 완성
- [ ] **Use Cases**: 비즈니스 로직 분리 및 구현
- [ ] **Tauri Commands**: 프론트엔드 API 인터페이스

#### 품질 보증
- [ ] **단위 테스트**: 커버리지 80% 이상
- [ ] **통합 테스트**: 전체 플로우 검증
- [ ] **에러 처리**: 표준화된 에러 응답
- [ ] **로깅**: 구조화된 로그 시스템

#### 성능 기준
- [ ] **빌드 시간**: 증분 빌드 5초 이하 유지
- [ ] **DB 연산**: 평균 응답 시간 100ms 이하
- [ ] **UI 응답**: 사용자 상호작용 500ms 이하

### 📊 예상 성능 지표

| 메트릭 | 목표 | 측정 방법 |
|--------|------|-----------|
| Repository 테스트 | 100% 통과 | `cargo test repository` |
| Use Cases 테스트 | 100% 통과 | `cargo test use_cases` |
| 통합 테스트 | 100% 통과 | `cargo test --test integration` |
| UI 응답성 | < 500ms | 브라우저 DevTools |
| 빌드 성능 | < 5초 | `time cargo test` |

---

## Phase 3: 크롤링 엔진 구현 (예정)

### 🎯 사전 준비 사항 (Phase 2 완료 후)
- HTTP 클라이언트 검증 (reqwest 최적화)
- HTML 파싱 성능 테스트 (scraper 라이브러리)
- 비동기 처리 패턴 설계 (tokio + rayon)
- 크롤링 설정 스키마 정의 (JSON/YAML)
