# Frontend-Backend Database Architecture 권고사항

## 현재 상황 분석

### 문제점
1. Frontend에서 직접 database 연결 시도로 인한 "Failed to connect to database" 오류
2. 중앙집중식 DatabasePathManager가 있지만 일부 코드에서 미사용
3. Frontend와 Backend 간 데이터베이스 접근 패턴 불일치

### 해결된 부분
✅ `simple_crawling.rs`에서 중앙집중식 경로 관리자 사용으로 변경
✅ Modern Rust 2024 컴플라이언스 적용

## 권장 아키텍처: Backend-Only CRUD

```
┌─────────────────┐    Tauri Commands    ┌─────────────────┐    SQLite Pool    ┌──────────────┐
│   Frontend      │ ─────────────────► │   Backend       │ ────────────────► │   Database   │
│   (SolidJS)     │                    │   (Rust)        │                   │   (SQLite)   │
│                 │ ◄───────────────── │                 │ ◄──────────────── │              │
│ - UI 상태관리   │    JSON Response   │ - CRUD 전담     │    Query Results  │ - 데이터 저장│
│ - 사용자 인터랙션│                    │ - 비즈니스 로직 │                   │ - 스키마 관리│
└─────────────────┘                    └─────────────────┘                   └──────────────┘
```

### 패턴 1: Backend-Only CRUD (추천)

**Backend 담당:**
- 모든 CREATE, READ, UPDATE, DELETE 작업
- Database connection 관리
- 트랜잭션 관리
- 비즈니스 로직

**Frontend 담당:**
- UI 상태 관리
- 사용자 인터랙션
- Tauri commands 호출
- 응답 데이터 렌더링

**구현 예시:**
```rust
// Backend: Tauri Commands
#[tauri::command]
pub async fn get_products_page(page: u32, size: u32) -> Result<ProductPage, String> {
    let database_url = get_main_database_url()?;
    let pool = SqlitePool::connect(&database_url).await?;
    let repo = IntegratedProductRepository::new(pool);
    
    repo.get_products_paginated(page, size).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_product(product: CreateProductRequest) -> Result<Product, String> {
    // CRUD 작업
}
```

```tsx
// Frontend: Tauri Commands 호출
const loadProducts = async () => {
    try {
        const products = await invoke<ProductPage>('get_products_page', { 
            page: currentPage(), 
            size: 20 
        });
        setProducts(products);
    } catch (error) {
        addLog(`❌ 제품 로딩 실패: ${error}`);
    }
};
```

### 패턴 2: Frontend Read + Backend CRUD (비추천)

현재 상황에서는 다음 이유로 비추천:
- Database connection 에러 발생 중
- 경로 관리 복잡성 증가  
- 보안 취약점 증가

## 구현 액션 플랜

### 즉시 적용 (High Priority)
1. **Frontend의 직접 DB 접근 제거**
   - CrawlingEngineTabSimple.tsx 등에서 database import 제거
   - 모든 데이터 로딩을 Tauri commands로 변경

2. **Backend Commands 확장**
   - 필요한 모든 데이터 조회 명령 추가
   - 크롤링 상태 조회 명령 추가

### 중기 적용 (Medium Priority)
3. **Connection Pool 최적화**
   - AppState에 공유 connection pool 추가
   - 명령별 connection 생성 → 공유 pool 사용

4. **에러 핸들링 통일**
   - Backend에서 구조화된 에러 응답
   - Frontend에서 일관된 에러 처리

## 마이그레이션 체크리스트

### Frontend (src/components/tabs/)
- [ ] CrawlingEngineTabSimple.tsx: 직접 DB 접근 제거
- [ ] CrawlingProgressMonitor.tsx: Tauri commands로 변경
- [ ] StatusTab.tsx: 상태 조회 commands 사용
- [ ] LiveProductionTab.tsx: 제품 데이터 commands 사용

### Backend (src-tauri/src/commands/)
- [x] simple_crawling.rs: 중앙집중식 경로 관리자 적용 완료
- [ ] 제품 데이터 조회 commands 추가
- [ ] 크롤링 상태 조회 commands 추가
- [ ] 시스템 상태 모니터링 commands 추가

## 예상 효과

### 즉시 효과
- ✅ "Failed to connect to database" 오류 해결
- ✅ 중앙집중식 경로 관리 효과 극대화
- ✅ Modern Rust 2024 컴플라이언스 완전 적용

### 장기 효과
- 🚀 성능 향상 (connection pooling)
- 🔒 보안 강화 (Backend-only DB access)
- 🛠️ 유지보수성 향상 (단일 책임 원칙)
- 🐛 디버깅 용이성 (로그 중앙화)

## 결론

**Backend-Only CRUD 패턴**을 채택하여:
1. 모든 데이터베이스 접근을 Backend로 집중
2. Frontend는 Tauri commands를 통한 데이터 요청만 담당
3. 중앙집중식 DatabasePathManager의 효과 극대화

이 패턴은 현재 발생하는 데이터베이스 연결 오류를 근본적으로 해결하고, 
Modern Rust 2024 가이드라인을 완전히 준수하는 아키텍처입니다.
