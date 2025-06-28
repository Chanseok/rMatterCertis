# Phase 2: 백엔드 도메인 구현 - 세부 실행 계획

## 📊 현재 상황 분석

# Phase 2: 백엔드 도메인 구현 - 세부 실행 계획 (업데이트됨)

## 📊 현재 상황 분석

### ✅ 완료된 사항 (Phase 1 + Phase 2 일부)
- ✅ 프로젝트 초기화 및 최적화 완료
- ✅ 기본 데이터베이스 연결 구현
- ✅ 빌드 성능 최적화 (90% 향상)
- ✅ 테스트 환경 구축 (단위 테스트 + CLI + UI)
- ✅ **모든 mod.rs 파일 제거 완료** (Rust 2024 모던 컨벤션)
- ✅ **Repository 패턴 기초 구조 완성**
- ✅ **도메인 엔티티 정의 완료** (Product, Vendor, CrawlingSession)
- ✅ **Repository trait 정의 완료** (확장된 메서드 포함)
- ✅ **Repository 구현체 기본 틀 완성**

### 🔄 현재 진행 중
- Repository 테스트 오류 수정 (DB 권한, 외래키 제약)
- Use Cases 구현 준비

### 🎯 Phase 2 남은 목표 (1주)
- Repository 테스트 안정화
- Use Cases 비즈니스 로직 구현
- Tauri Commands 확장
- 에러 처리 및 로깅 시스템

---

## 📅 Week 2.1: Repository 안정화 및 Use Cases 구현 (남은 4일)

### ✅ 이미 완료된 작업

**모던 Rust 모듈 구조 완성:**
```
src/
├── lib.rs (루트 모듈)
├── main.rs
├── commands.rs
├── domain.rs ← mod.rs 제거
├── domain/
│   ├── entities.rs
│   ├── repositories.rs ← 확장된 trait 정의
│   └── services.rs
├── application.rs ← mod.rs 제거
├── application/
│   ├── dto.rs
│   └── use_cases.rs
├── infrastructure.rs ← mod.rs 제거
├── infrastructure/
│   ├── repositories.rs ← 통합된 구현체
│   ├── database_connection.rs
│   ├── config.rs
│   ├── database.rs
│   └── http.rs
└── bin/
    └── test_db.rs
```

**Repository Pattern 기초 완성:**
```rust
// ✅ 완료: trait 정의 (src/domain/repositories.rs)
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

// ✅ 완료: 기본 구현체 (src/infrastructure/repositories.rs)
pub struct SqliteVendorRepository { pool: SqlitePool }
pub struct SqliteProductRepository { pool: SqlitePool }
impl VendorRepository for SqliteVendorRepository { /* 모든 메서드 구현 */ }
impl ProductRepository for SqliteProductRepository { /* 모든 메서드 구현 */ }
```

### Day 6 (다음 작업): Repository 테스트 수정

#### 🎯 목표
- 현재 실패하는 테스트 오류 해결
- 안정적인 테스트 환경 구축
- CI/CD 준비
impl VendorRepository for SqliteVendorRepository {
    async fn create(&self, vendor: &Vendor) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO vendors (id, name, base_url, selector_config, is_active, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            vendor.id.to_string(),
            vendor.name,
            vendor.base_url,
            vendor.selector_config,
            vendor.is_active,
            vendor.created_at.to_rfc3339(),
            vendor.updated_at.to_rfc3339()
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    async fn find_by_id(&self, id: &Uuid) -> Result<Option<Vendor>> {
        let row = sqlx::query!(
            "SELECT * FROM vendors WHERE id = ?1",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let vendor = Vendor {
                    id: Uuid::parse_str(&r.id)?,
                    name: r.name,
                    base_url: r.base_url,
                    selector_config: r.selector_config,
                    is_active: r.is_active,
                    created_at: chrono::DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&chrono::Utc),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&r.updated_at)?.with_timezone(&chrono::Utc),
                };
                Ok(Some(vendor))
            }
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Vendor>> {
        // 구현 로직
    }

    async fn update(&self, vendor: &Vendor) -> Result<()> {
        // 구현 로직
    }

    async fn delete(&self, id: &Uuid) -> Result<()> {
        // 구현 로직
    }
}
```

**2. 테스트 작성 (TDD 방식)**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database_connection::DatabaseConnection;
    use tempfile::tempdir;
    use chrono::Utc;

    #[tokio::test]
    async fn test_vendor_repository_crud() -> Result<()> {
        // Given: 테스트 데이터베이스 설정
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite:{}", db_path.to_string_lossy());
        
        let db = DatabaseConnection::new(&database_url).await?;
        db.migrate().await?;
        
        let repo = SqliteVendorRepository::new(db.pool().clone());
        
        // When: Vendor 생성
        let vendor = Vendor {
            id: Uuid::new_v4(),
            name: "Test Vendor".to_string(),
            base_url: "https://example.com".to_string(),
            selector_config: "{}".to_string(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Then: CRUD 연산 테스트
        repo.create(&vendor).await?;
        let found = repo.find_by_id(&vendor.id).await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, vendor.name);
        
        Ok(())
    }
}
```

**3. 예상 소요시간**: 6시간
**4. 성공 기준**: 모든 Repository 테스트 통과

### Day 7 (2일차): Use Cases 비즈니스 로직 구현

#### 🎯 목표
- Vendor 관리 Use Cases 구현
- Product 관리 Use Cases 구현
- 에러 처리 및 검증 로직

#### 📋 구체적 작업

**1. Vendor Use Cases 구현**
```rust
// src-tauri/src/application/use_cases/vendor_use_cases.rs
use crate::domain::{entities::Vendor, repositories::VendorRepository};
use crate::application::dto::{CreateVendorDto, UpdateVendorDto, VendorResponseDto};
use anyhow::{Result, anyhow};
use uuid::Uuid;
use chrono::Utc;

pub struct VendorUseCases<T: VendorRepository> {
    vendor_repository: T,
}

impl<T: VendorRepository> VendorUseCases<T> {
    pub fn new(vendor_repository: T) -> Self {
        Self { vendor_repository }
    }

    pub async fn create_vendor(&self, dto: CreateVendorDto) -> Result<VendorResponseDto> {
        // 입력 검증
        if dto.name.trim().is_empty() {
            return Err(anyhow!("Vendor name cannot be empty"));
        }
        
        if !dto.base_url.starts_with("http") {
            return Err(anyhow!("Invalid URL format"));
        }

        // 도메인 엔티티 생성
        let vendor = Vendor {
            id: Uuid::new_v4(),
            name: dto.name.trim().to_string(),
            base_url: dto.base_url,
            selector_config: dto.selector_config.unwrap_or_default(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 저장
        self.vendor_repository.create(&vendor).await?;
        
        Ok(VendorResponseDto::from(vendor))
    }

    pub async fn get_all_vendors(&self) -> Result<Vec<VendorResponseDto>> {
        let vendors = self.vendor_repository.find_all().await?;
        Ok(vendors.into_iter().map(VendorResponseDto::from).collect())
    }

    pub async fn update_vendor(&self, id: Uuid, dto: UpdateVendorDto) -> Result<VendorResponseDto> {
        // 기존 vendor 조회
        let mut vendor = self.vendor_repository.find_by_id(&id).await?
            .ok_or_else(|| anyhow!("Vendor not found"))?;

        // 업데이트
        if let Some(name) = dto.name {
            vendor.name = name;
        }
        if let Some(base_url) = dto.base_url {
            vendor.base_url = base_url;
        }
        vendor.updated_at = Utc::now();

        self.vendor_repository.update(&vendor).await?;
        Ok(VendorResponseDto::from(vendor))
    }

    pub async fn delete_vendor(&self, id: Uuid) -> Result<()> {
        self.vendor_repository.delete(&id).await?;
        Ok(())
    }
}
```

**2. DTO 구현**
```rust
// src-tauri/src/application/dto.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::domain::entities::{Vendor, Product};

#[derive(Debug, Deserialize)]
pub struct CreateVendorDto {
    pub name: String,
    pub base_url: String,
    pub selector_config: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendorDto {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub selector_config: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct VendorResponseDto {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub selector_config: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Vendor> for VendorResponseDto {
    fn from(vendor: Vendor) -> Self {
        Self {
            id: vendor.id.to_string(),
            name: vendor.name,
            base_url: vendor.base_url,
            selector_config: vendor.selector_config,
            is_active: vendor.is_active,
            created_at: vendor.created_at.to_rfc3339(),
            updated_at: vendor.updated_at.to_rfc3339(),
        }
    }
}
```

**3. 예상 소요시간**: 7시간
**4. 성공 기준**: Use Cases 단위 테스트 통과

### Day 8 (3일차): Tauri Commands 확장

#### 🎯 목표
- Vendor 관리 Tauri Commands 구현
- Product 관리 Tauri Commands 구현
- 에러 응답 표준화

#### 📋 구체적 작업

**1. Vendor Commands 구현**
```rust
// src-tauri/src/commands/vendor_commands.rs
use crate::application::{
    use_cases::VendorUseCases,
    dto::{CreateVendorDto, UpdateVendorDto, VendorResponseDto}
};
use crate::infrastructure::{
    database_connection::DatabaseConnection,
    vendor_repository::SqliteVendorRepository
};
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn create_vendor(
    db: State<'_, DatabaseConnection>,
    dto: CreateVendorDto
) -> Result<VendorResponseDto, String> {
    let repo = SqliteVendorRepository::new(db.pool().clone());
    let use_cases = VendorUseCases::new(repo);
    
    match use_cases.create_vendor(dto).await {
        Ok(vendor) => Ok(vendor),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn get_all_vendors(
    db: State<'_, DatabaseConnection>
) -> Result<Vec<VendorResponseDto>, String> {
    let repo = SqliteVendorRepository::new(db.pool().clone());
    let use_cases = VendorUseCases::new(repo);
    
    match use_cases.get_all_vendors().await {
        Ok(vendors) => Ok(vendors),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn update_vendor(
    db: State<'_, DatabaseConnection>,
    id: String,
    dto: UpdateVendorDto
) -> Result<VendorResponseDto, String> {
    let vendor_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid UUID: {}", e))?;
        
    let repo = SqliteVendorRepository::new(db.pool().clone());
    let use_cases = VendorUseCases::new(repo);
    
    match use_cases.update_vendor(vendor_id, dto).await {
        Ok(vendor) => Ok(vendor),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn delete_vendor(
    db: State<'_, DatabaseConnection>,
    id: String
) -> Result<(), String> {
    let vendor_id = Uuid::parse_str(&id)
        .map_err(|e| format!("Invalid UUID: {}", e))?;
        
    let repo = SqliteVendorRepository::new(db.pool().clone());
    let use_cases = VendorUseCases::new(repo);
    
    match use_cases.delete_vendor(vendor_id).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
```

**2. main.rs에 Commands 등록**
```rust
// src-tauri/src/main.rs
use crate::commands::vendor_commands::*;
use crate::infrastructure::database_connection::DatabaseConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 데이터베이스 초기화
    let db = DatabaseConnection::new("sqlite:data/matter_certis.db").await?;
    db.migrate().await?;

    tauri::Builder::default()
        .manage(db)
        .invoke_handler(tauri::generate_handler![
            test_database_connection,
            get_database_info,
            create_vendor,
            get_all_vendors,
            update_vendor,
            delete_vendor
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
```

**3. 예상 소요시간**: 6시간
**4. 성공 기준**: Tauri Commands 통합 테스트 통과

### Day 9 (4일차): 통합 테스트 및 프론트엔드 연동

#### 🎯 목표
- 전체 플로우 통합 테스트
- 프론트엔드에서 API 호출 테스트
- 에러 시나리오 검증

#### 📋 구체적 작업

**1. 통합 테스트 구현**
```rust
// tests/integration/vendor_integration_test.rs
use rMatterCertis::*;
use tauri::test::{MockRuntime, mock_app};

#[tokio::test]
async fn test_vendor_full_workflow() {
    // Given: 앱 초기화
    let app = mock_app().await;
    
    // When: Vendor 생성
    let create_dto = CreateVendorDto {
        name: "Test Vendor".to_string(),
        base_url: "https://example.com".to_string(),
        selector_config: None,
    };
    
    let created = app.invoke("create_vendor", create_dto).await.unwrap();
    
    // Then: 생성된 Vendor 확인
    let vendors = app.invoke("get_all_vendors", ()).await.unwrap();
    assert_eq!(vendors.len(), 1);
    
    // When: Vendor 업데이트
    let update_dto = UpdateVendorDto {
        name: Some("Updated Vendor".to_string()),
        base_url: None,
        selector_config: None,
        is_active: None,
    };
    
    app.invoke("update_vendor", (created.id, update_dto)).await.unwrap();
    
    // Then: 업데이트 확인
    let updated_vendors = app.invoke("get_all_vendors", ()).await.unwrap();
    assert_eq!(updated_vendors[0].name, "Updated Vendor");
}
```

**2. 프론트엔드 테스트 UI 확장**
```tsx
// src/App.tsx
import { invoke } from "@tauri-apps/api/tauri";
import { createSignal, onMount } from "solid-js";

interface Vendor {
  id: string;
  name: string;
  base_url: string;
  is_active: boolean;
}

function App() {
  const [vendors, setVendors] = createSignal<Vendor[]>([]);
  const [status, setStatus] = createSignal<string>("");

  onMount(async () => {
    await loadVendors();
  });

  const loadVendors = async () => {
    try {
      const result = await invoke<Vendor[]>("get_all_vendors");
      setVendors(result);
      setStatus("✅ Vendors loaded");
    } catch (error) {
      setStatus(`❌ Error: ${error}`);
    }
  };

  const createVendor = async () => {
    try {
      await invoke("create_vendor", {
        dto: {
          name: "Test Vendor",
          base_url: "https://example.com",
          selector_config: "{}"
        }
      });
      await loadVendors();
      setStatus("✅ Vendor created");
    } catch (error) {
      setStatus(`❌ Error: ${error}`);
    }
  };

  return (
    <div class="container">
      <h1>rMatterCertis - Vendor Management</h1>
      
      <div class="controls">
        <button onClick={createVendor}>Create Test Vendor</button>
        <button onClick={loadVendors}>Reload Vendors</button>
      </div>
      
      <div class="status">
        <p>{status()}</p>
      </div>
      
      <div class="vendors">
        <h2>Vendors ({vendors().length})</h2>
        <For each={vendors()}>
          {(vendor) => (
            <div class="vendor-item">
              <h3>{vendor.name}</h3>
              <p>URL: {vendor.base_url}</p>
              <p>Active: {vendor.is_active ? "Yes" : "No"}</p>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
```

**3. 예상 소요시간**: 6시간
**4. 성공 기준**: 전체 플로우 테스트 통과

---

## 📅 Week 2.2: Product 도메인 및 고급 기능 (3일)

### Day 10-12: Product 관리 구현
- Product Repository 구현
- Product Use Cases 구현  
- Product Tauri Commands 구현
- Vendor-Product 관계 관리

---

## 🎯 Phase 2 완료 기준

### 기능적 요구사항
- [x] Vendor CRUD 완전 구현
- [x] Product CRUD 완전 구현
- [x] Repository 패턴 구현
- [x] Use Cases 비즈니스 로직
- [x] Tauri Commands API

### 비기능적 요구사항
- [x] 단위 테스트 커버리지 80% 이상
- [x] 통합 테스트 시나리오 구현
- [x] 에러 처리 표준화
- [x] 로깅 시스템 구축

### 성능 요구사항
- [x] 빌드 시간 5초 이하 유지
- [x] 데이터베이스 연산 100ms 이하
- [x] UI 응답 속도 500ms 이하

---

## 🚀 다음 단계 준비

Phase 2 완료 후 Phase 3 크롤링 엔진 구현을 위한 사전 준비:
- HTTP 클라이언트 구성
- HTML 파싱 라이브러리 검증
- 비동기 처리 패턴 설계
- 크롤링 설정 스키마 정의
