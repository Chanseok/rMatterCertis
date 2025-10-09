# 제품 보완 동기화 SessionActor 리팩토링 계획

## 📋 현재 문제점

### 1. 구조적 불일치
- **빠른 동기화**: SessionActor 사용 ✅
- **스마트 동기화**: SessionActor 사용 ✅
- **제품 보완 동기화**: 직접 구현 ❌ (일관성 부족)

### 2. 중지 불가능
- CancellationToken을 전달하지만 실제로 체크하지 않음
- 프론트엔드의 중지 버튼이 작동하지 않음

### 3. 코드 중복
- 배치 처리 로직이 수동으로 구현됨
- 이벤트 발행 로직이 수동으로 구현됨
- StageActor의 기능을 재구현함

## 🎯 리팩토링 목표

### 1. SessionActor 기반 구현
```rust
pub async fn start_complement_crawl(
    app: tauri::AppHandle,
) -> Result<ComplementCrawlResult, String> {
    // 1. 누락된 제품 URL 목록 조회
    let missing_urls = query_missing_products(&app).await?;
    
    // 2. ExecutionPlan 생성 (ProductDetailsCollection 단계만)
    let execution_plan = create_complement_execution_plan(missing_urls);
    
    // 3. SessionActor 실행
    let (session_id, _) = bootstrap_and_spawn_session(&app, execution_plan, app_config).await?;
    
    // 4. 세션 ID 반환 (백그라운드 실행)
    Ok(ComplementCrawlResult {
        session_id,
        urls_targeted: missing_urls.len() as u32,
        status: "started".to_string(),
        ...
    })
}
```

### 2. 전용 ExecutionPlan 생성
```rust
fn create_complement_execution_plan(
    missing_urls: Vec<ProductUrl>
) -> ExecutionPlan {
    ExecutionPlan {
        session_id: format!("complement-{}", Utc::now().timestamp()),
        crawling_ranges: vec![], // URL 기반이므로 범위 없음
        product_urls: Some(missing_urls), // 직접 URL 목록 제공
        stages: vec![
            StageConfig::ProductDetailsCollection, // 상세 정보 수집만
            StageConfig::DataValidation,           // 검증
            StageConfig::DataPersistence,          // DB 저장
        ],
        batch_size: 20,
        list_only: false,
        ...
    }
}
```

### 3. URL 기반 크롤링 지원
StageActor의 `ProductDetailsCollection` 로직 확장:
```rust
// stage_actor.rs의 execute_product_details_collection() 수정
if let Some(product_urls) = &self.execution_plan.product_urls {
    // URL 목록이 직접 제공된 경우 (제품 보완 동기화)
    self.collect_from_url_list(product_urls).await?;
} else {
    // 기존 로직: DB에서 페이지 범위로 조회
    self.collect_from_page_ranges().await?;
}
```

## 📝 구현 단계

### Phase 1: ExecutionPlan 확장 (30분) ✅ 완료 (Commit 81daf03)
- [x] `ExecutionPlan`에 `product_urls: Option<Vec<ProductUrl>>` 필드 추가
- [x] `query_missing_products()` 헬퍼 함수 작성 및 분리
- [x] `start_complement_crawl()` SessionActor 기반으로 리팩토링
- [x] 기존 200+ 라인 → 70 라인으로 단순화
- [x] 모든 ExecutionPlan 초기화 지점에 `product_urls: None` 추가

### Phase 2: SessionActor URL 기반 크롤링 지원 (1시간) ✅ 완료 (Commit f8583e6)
- [x] `session_actor.rs`의 `run_preplanned_batches()` 수정
  - `product_urls` 필드 체크
  - URL 목록이 있으면 ListPageCrawling 건너뛰고 바로 ProductDetailCrawling
  - 없으면 기존 로직 (페이지 범위 기반 크롤링)
- [x] ProductDetails 구조 올바르게 사용 (products, source_urls, extraction_stats)
- [x] 기존 로직과 호환성 유지 (빠른/스마트 동기화 영향 없음)
- [x] StageActor 재사용으로 이벤트 발행 및 메트릭 일관성 확보

### Phase 3: 프론트엔드 연동 (30분) ⏳ 진행 예정
- [ ] `handleComplementCrawl()` 수정
  - 세션 ID 저장: `setCurrentSessionId(result.sessionId)`
  - 백그라운드 실행 안내 메시지 표시
  - 중지 버튼 활성화
- [ ] `actor-session-completed` 이벤트로 완료 처리

### Phase 4: 테스트 및 검증 (30분) ⏳ 진행 예정
- [ ] 제품 보완 동기화 실행 테스트
- [ ] 중지 버튼 작동 확인
- [ ] 이벤트 발행 확인
- [ ] DB 저장 확인

### Phase 5: 문서화 (15분) ⏳ 진행 예정
- [ ] 아키텍처 다이어그램 업데이트
- [ ] URL 기반 vs 범위 기반 크롤링 모드 문서화
- [ ] 이 계획서의 완료 상태 업데이트

## 🔧 코드 변경 사항

### 1. ExecutionPlan 구조체 수정
```rust
// src-tauri/src/crawl_engine/execution_plan.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub session_id: String,
    pub crawling_ranges: Vec<CrawlingRange>,
    
    // 🆕 URL 직접 지정 (제품 보완 동기화용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_urls: Option<Vec<ProductUrl>>,
    
    pub stages: Vec<StageConfig>,
    // ... 기존 필드들
}
```

### 2. stage_actor.rs 수정
```rust
async fn execute_product_details_collection(&mut self) -> Result<(), String> {
    let stage_name = "ProductDetailsCollection";
    self.emit_stage_started(stage_name).await;
    
    // 🆕 URL 목록이 직접 제공된 경우
    let product_urls = if let Some(urls) = &self.execution_plan.product_urls {
        info!("📋 Using provided product URLs ({} products)", urls.len());
        urls.clone()
    } else {
        // 기존 로직: DB에서 페이지 범위로 조회
        info!("📋 Querying products from page ranges");
        self.query_products_from_ranges().await?
    };
    
    // 공통 크롤링 로직
    self.collect_and_emit_details(product_urls).await?;
    
    self.emit_stage_completed(stage_name).await;
    Ok(())
}
```

### 3. shallow_sync_commands.rs 리팩토링
```rust
#[tauri::command]
pub async fn start_complement_crawl(
    app: tauri::AppHandle,
) -> Result<ComplementCrawlResult, String> {
    info!("🔧 Starting complement crawl (SessionActor-based)");
    
    // 1. 누락 제품 조회
    let missing_products = query_missing_products(&app).await?;
    
    if missing_products.is_empty() {
        return Ok(ComplementCrawlResult {
            session_id: String::new(),
            urls_targeted: 0,
            urls_completed: 0,
            urls_failed: 0,
            duration_ms: 0,
            status: "no_missing_products".to_string(),
        });
    }
    
    // 2. ExecutionPlan 생성
    let app_state = app.state::<AppState>();
    let config = app_state.config.read().await.clone();
    
    let execution_plan = ExecutionPlan {
        session_id: format!("complement-{}", Utc::now().timestamp_millis()),
        crawling_ranges: vec![],
        product_urls: Some(missing_products.clone()),
        stages: vec![
            StageConfig::ProductDetailsCollection,
            StageConfig::DataValidation,
            StageConfig::DataPersistence,
        ],
        batch_size: 20,
        list_only: false,
        estimated_total_products: missing_products.len() as u32,
        // ... 기타 필드
    };
    
    // 3. SessionActor 실행
    let (session_id, _) = crate::commands::crawling::actor_system::bootstrap_and_spawn_session(
        &app,
        execution_plan,
        config,
    ).await?;
    
    info!("✅ Complement crawl session started: {}", session_id);
    
    Ok(ComplementCrawlResult {
        session_id,
        urls_targeted: missing_products.len() as u32,
        urls_completed: 0,
        urls_failed: 0,
        duration_ms: 0,
        status: "started".to_string(),
    })
}

// 🆕 누락 제품 쿼리 함수 분리
async fn query_missing_products(app: &tauri::AppHandle) -> Result<Vec<ProductUrl>, String> {
    let app_state = app.state::<AppState>();
    let pool = app_state.get_database_pool().await?;
    
    let query = r"
        SELECT pd.url, pd.page_id, pd.index_in_page
        FROM product_details pd
        INNER JOIN products p ON pd.url = p.url
        WHERE pd.certification_date IS NULL
           OR pd.transport_interface IS NULL
           OR pd.primary_device_type_ids IS NULL
           OR pd.primary_device_type_ids = ''
           OR pd.primary_device_type_ids = '[]'
        ORDER BY pd.page_id DESC, pd.index_in_page ASC
    ";
    
    let rows = sqlx::query(query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to query missing products: {}", e))?;
    
    let product_urls = rows.into_iter().map(|row| ProductUrl {
        url: row.get("url"),
        page_id: row.get("page_id"),
        index_in_page: row.get("index_in_page"),
    }).collect();
    
    Ok(product_urls)
}
```

### 4. 프론트엔드 수정
```typescript
// src/components/tabs/CrawlingEngineTabSimple.tsx
const handleComplementCrawl = async () => {
    if (isRunning() || isSyncing()) {
      addLog("⚠️ 이미 크롤링이 진행 중입니다.");
      return;
    }

    setIsRunning(true);
    setIsSyncing(true);
    setStatusMessage("🔧 제품 보완 동기화 진행 중...");
    addLog("🔧 제품 보완 동기화 시작");

    try {
      const result = await tauriApi.startComplementCrawl();
      
      // 🆕 세션 ID 저장
      if (result.sessionId || result.session_id) {
        const sessionId = result.sessionId || result.session_id;
        setCurrentSessionId(sessionId);
        console.log("📝 Complement crawl session ID saved:", sessionId);
      }
      
      if (result.urlsTargeted === 0) {
        addLog(`✨ 보완이 필요한 제품이 없습니다.`);
        setIsRunning(false);
        setIsSyncing(false);
      } else {
        addLog(`🔄 ${result.urlsTargeted}개 제품 재크롤링 시작 (백그라운드)`);
        addLog(`💡 진행 상황은 실시간으로 표시됩니다. (중지 버튼으로 중단 가능)`);
        setStatusMessage(`🔧 제품 보완 중... (${result.urlsTargeted}개)`);
      }
    } catch (error) {
      console.error("제품 보완 동기화 실패:", error);
      addLog(`❌ 제품 보완 동기화 실패: ${error}`);
      setStatusMessage("❌ 제품 보완 동기화 실패");
      setIsRunning(false);
      setIsSyncing(false);
    }
    
    // 완료는 actor-session-completed 이벤트에서 처리
};
```

## ✅ 기대 효과

### 1. 아키텍처 일관성
- 모든 크롤링 기능이 SessionActor 사용
- 단일 진입점: `bootstrap_and_spawn_session()`
- 유지보수 용이

### 2. 기능 개선
- ✅ 중지 가능
- ✅ 실시간 진행률 표시
- ✅ 재시도 로직 자동 적용
- ✅ 이벤트 자동 발행

### 3. 코드 간결화
- 200+ 라인 → 50 라인으로 축소
- 배치 처리 로직 제거 (StageActor가 처리)
- 이벤트 발행 로직 제거 (자동 발행)

## 📊 영향 범위

### 변경 필요
- ✏️ `execution_plan.rs`: product_urls 필드 추가
- ✏️ `stage_actor.rs`: URL 기반 크롤링 지원
- ✏️ `shallow_sync_commands.rs`: 리팩토링
- ✏️ `CrawlingEngineTabSimple.tsx`: 세션 ID 저장

### 영향 없음
- ✅ 빠른 동기화
- ✅ 스마트 동기화
- ✅ 수동 크롤링
- ✅ 전체 크롤링

## 🚀 다음 단계

1. **Phase 1 시작**: ExecutionPlan에 product_urls 필드 추가
2. **단위 테스트**: URL 기반 크롤링 로직 검증
3. **통합 테스트**: 전체 플로우 확인
4. **문서 업데이트**: 아키텍처 다이어그램 갱신

---

**작성일**: 2025-10-09  
**예상 소요 시간**: 3시간  
**우선순위**: 높음 (중지 기능 필수)
