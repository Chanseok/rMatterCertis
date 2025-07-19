# 제안: 크롤링 제어 및 범위 설정 기능 개선안

**문서 목적:** 현재 구현에서 발견된 두 가지 핵심 문제(1. 제어 불능, 2. 정적 범위 설정)의 원인을 진단하고, 이를 해결하여 시스템을 v4.0 아키텍처 설계에 부합하도록 개선하기 위한 구체적인 기술적 방안을 제안합니다.

--- 

## 1. 현상 및 문제 정의

현재 크롤링 시스템은 테스트 결과, 두 가지 심각한 설계-구현 간의 차이점을 보이고 있습니다.

1.  **제어 불능 문제:** UI에서 `중지` 버튼을 눌러도, 백엔드 작업이 즉시 중단되지 않고 한참 후에야 멈추거나 상태가 늦게 반영되어 사용자는 시스템이 멈췄는지 알 수 없습니다.
2.  **정적 범위 문제:** 크롤링 시작 시, 사이트와 로컬 DB 상태를 분석하여 최적의 범위를 동적으로 계산하는 핵심 로직이 동작하지 않고, 항상 `1-50`과 같은 하드코딩된 범위로 작업이 시작됩니다.

이 문제들은 시스템의 안정성과 효율성을 저해하며, 사용자에게 부정적인 경험을 제공하는 시급한 개선 과제입니다.

## 1.1. 코드 분석 결과 (2025.07.13)

제안서 작성 후 실제 코드를 분석한 결과, 문제점들이 정확히 진단되었음을 확인했습니다:

### 1.1.1. 제어 불능 문제 확인사항
- **현재 구현 상태**: `ServiceBasedBatchCrawlingEngine`에는 `stop()` 메서드가 없음
- **CancellationToken 전파**: 일부 구현에서는 `cancellation_token`이 전달되지만, 실제 Worker 단계에서 활용되지 않음
- **stop_crawling 커맨드**: 현재는 단순히 UI 이벤트만 발생시키고 실제 중단은 수행하지 않음

### 1.1.2. 정적 범위 문제 확인사항
- **지능형 범위 계산**: `calculate_intelligent_crawling_range_v4` 함수는 구현되어 있으나, 사용자가 명시적 범위를 제공하지 않는 경우에만 동작
- **하드코딩 문제**: 기본 설정에서 `start_page: 1, end_page: 100` 등의 고정값 사용
- **사이트 분석 로직**: `StatusChecker`, `DatabaseAnalyzer` 등의 서비스는 존재하지만 `start_crawling` 워크플로우에 통합되지 않음

## 2. 근본 원인 심층 분석

### 2.1. '중지' 기능의 지연 원인: 비협조적 작업자와 동기적 대기

코드 분석 결과, 이 문제는 다음과 같은 구조적 원인들로 발생합니다:

-   **1. 엔진 레벨 중단 메커니즘 부재:** 
    - `ServiceBasedBatchCrawlingEngine`에는 `stop()` 메서드가 구현되어 있지 않음
    - 현재 `stop_crawling` 커맨드는 UI 이벤트만 발생시키고 실제 엔진 중단은 수행하지 않음
    - 코드: `crawling_v4.rs:212-238`에서 확인 가능

-   **2. CancellationToken 전파 및 활용 부족:**
    - `BatchCrawlingConfig`에 `cancellation_token`이 포함되어 있지만, 실제 Worker 레벨에서 활용되지 않음
    - 개별 Worker의 `process_task` 메서드에서 `tokio::select!` 패턴을 사용하지 않음
    - 현재는 단순히 `shared_state.is_shutdown_requested()` 체크만 수행

-   **3. 동기적 대기 패턴:**
    - `engine.execute().await`가 완료될 때까지 `start_crawling` 커맨드가 블로킹됨
    - 중간에 중단할 수 있는 메커니즘이 없어 사용자는 완료까지 기다려야 함

### 2.2. '동적 범위 설정' 누락 원인: 워크플로우 통합 부족

코드 분석 결과, 다음과 같은 구현 문제들이 확인됩니다:

-   **1. 기본값 의존성:**
    - `BatchCrawlingConfig::default()`에서 `start_page: 1, end_page: 100` 등의 하드코딩된 값 사용
    - 사용자가 명시적 범위를 제공하지 않으면 기본값이 그대로 사용됨

-   **2. 분석 로직 미통합:**
    - `StatusChecker`, `DatabaseAnalyzer` 등의 서비스는 존재하지만 `start_crawling` 워크플로우에 통합되지 않음
    - `calculate_intelligent_crawling_range_v4` 함수는 구현되어 있으나, 조건부로만 실행됨

-   **3. 사이트 상태 분석 생략:**
    - 현재 총 페이지 수는 하드코딩된 값(481)을 사용
    - 실제 사이트 상태 확인 없이 추정값에 의존

--- 

## 3. 해결 방안 및 구체적인 기술 제안

### 3.1. 제안 1: 즉각적이고 협조적인 중단 메커니즘 구현

**핵심 목표:** 실시간 중단이 가능한 크롤링 엔진으로 개선합니다.

#### 3.1.1. ServiceBasedBatchCrawlingEngine 중단 메커니즘 구현

**실행 방안:**
- `ServiceBasedBatchCrawlingEngine`에 `stop()` 메서드 추가
- 엔진 내부에 `CancellationToken` 저장 및 활용
- `execute()` 메서드를 배경에서 실행하고, 중단 신호 시 즉시 종료

**코드 예시:**
```rust
impl ServiceBasedBatchCrawlingEngine {
    pub fn new(/* ... */, cancellation_token: CancellationToken) -> Self {
        // cancellation_token을 엔진 내부에 저장
    }
    
    pub async fn stop(&self) -> Result<(), String> {
        self.cancellation_token.cancel();
        // 진행 중인 작업들이 중단되도록 처리
        Ok(())
    }
    
    pub async fn execute(&self) -> Result<()> {
        // 각 단계에서 cancellation_token 확인
        tokio::select! {
            result = self.run_stages() => result,
            _ = self.cancellation_token.cancelled() => {
                tracing::info!("Engine execution cancelled");
                Err(anyhow::anyhow!("Execution cancelled"))
            }
        }
    }
}
```

#### 3.1.2. Worker 레벨 협조적 취소 패턴 구현

**실행 방안:**
- 모든 Worker의 `process_task` 메서드에서 `tokio::select!` 패턴 적용
- HTTP 요청 등 장시간 I/O 작업에 취소 신호 통합

**코드 예시:**
```rust
// list_page_fetcher.rs 수정
async fn process_task(&self, task: CrawlingTask, shared_state: Arc<SharedState>) -> Result<TaskResult, WorkerError> {
    // CancellationToken 획득
    let cancellation_token = shared_state.get_cancellation_token();
    
    let html = tokio::select! {
        result = self.http_client.fetch_html_string(&url) => {
            result.map_err(|e| WorkerError::NetworkError(e.to_string()))?
        },
        _ = cancellation_token.cancelled() => {
            tracing::info!("Task cancelled during HTTP fetch for url: {}", url);
            return Err(WorkerError::Cancelled);
        }
    };
    
    // 나머지 처리...
}
```

#### 3.1.3. 즉시 응답하는 stop_crawling 커맨드 구현

**실행 방안:**
- `stop_crawling` 커맨드를 즉시 응답하도록 수정
- 엔진 중단과 UI 상태 업데이트를 분리

**코드 예시:**
```rust
#[tauri::command]
pub async fn stop_crawling(app: AppHandle, state: State<'_, CrawlingEngineState>) -> Result<CrawlingResponse, String> {
    // 1. 즉시 엔진 중단 신호 전송
    let engine_guard = state.engine.read().await;
    if let Some(engine) = engine_guard.as_ref() {
        engine.stop().await?;
    }
    
    // 2. 즉시 UI 상태 업데이트
    app.emit("crawling-stopped", serde_json::json!({
        "status": "stopped",
        "message": "Crawling has been stopped",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))?;
    
    // 3. 즉시 응답 반환
    Ok(CrawlingResponse {
        success: true,
        message: "Crawling stopped successfully".to_string(),
        data: None,
    })
}
```

### 3.2. 제안 2: 지능형 크롤링 범위 설정 로직 활성화

**핵심 목표:** 항상 최적의 범위로 크롤링을 수행하도록 개선합니다.

#### 3.2.1. start_crawling 워크플로우 재설계

**실행 방안:**
- `start_crawling` 커맨드를 4단계 워크플로우로 개선
- 기본적으로 항상 지능형 범위 계산 수행

**코드 예시:**
```rust
#[tauri::command]
pub async fn start_crawling(
    app: AppHandle,
    state: State<'_, CrawlingEngineState>,
    request: StartCrawlingRequest,
) -> Result<CrawlingResponse, String> {
    // Phase 1: 사전 분석
    let status_checker = create_status_checker().await?;
    let site_status = status_checker.check_site_status().await?;
    
    let database_analyzer = create_database_analyzer().await?;
    let db_state = database_analyzer.analyze_current_state().await?;
    
    // Phase 2: 범위 계산 (사용자 요청 무시하고 항상 지능형 계산)
    let range_calculator = create_range_calculator().await?;
    let (start_page, end_page) = range_calculator.calculate_optimal_range(
        &site_status,
        &db_state,
        &request  // 사용자 선호도 참고용
    ).await?;
    
    // Phase 3: 엔진 설정 및 UI 준비
    let config = BatchCrawlingConfig {
        start_page,
        end_page,
        // ... 기타 설정
    };
    
    // UI에 계산된 범위 정보 전송
    app.emit("crawling-range-calculated", serde_json::json!({
        "start_page": start_page,
        "end_page": end_page,
        "total_pages": end_page - start_page + 1,
        "calculation_reason": "Based on site analysis and database state"
    }))?;
    
    // Phase 4: 엔진 실행
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref().ok_or("Engine not initialized")?;
    
    // 백그라운드에서 실행하여 즉시 응답
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        if let Err(e) = engine_clone.execute_with_config(config).await {
            tracing::error!("Crawling execution failed: {}", e);
        }
    });
    
    Ok(CrawlingResponse {
        success: true,
        message: format!("Crawling started: intelligent range {} to {}", start_page, end_page),
        data: Some(serde_json::json!({"start_page": start_page, "end_page": end_page})),
    })
}
```

#### 3.2.2. 범위 계산 로직 개선

**실행 방안:**
- 기존 `calculate_intelligent_crawling_range_v4` 함수 개선
- 항상 실행되도록 보장하고, 사용자 입력은 힌트로만 활용

**코드 예시:**
```rust
async fn calculate_intelligent_crawling_range_v4(
    site_status: &SiteStatus,
    db_state: &DatabaseState,
    user_hint: &StartCrawlingRequest,
) -> Result<(u32, u32), String> {
    // 사용자 입력을 힌트로 참고하되, 최종 결정은 분석 결과에 따라
    let user_preference = UserPreference {
        preferred_start: user_hint.start_page,
        preferred_end: user_hint.end_page,
        max_pages: user_hint.max_products_per_page,
    };
    
    // 실제 사이트 총 페이지 수 확인
    let total_pages = site_status.total_pages;
    
    // 데이터베이스 상태 분석
    let last_crawled_page = db_state.last_crawled_page;
    let missing_ranges = db_state.missing_ranges;
    
    // 지능형 범위 계산 로직
    let (start_page, end_page) = if !missing_ranges.is_empty() {
        // 누락된 범위 우선 처리
        missing_ranges.first().unwrap().clone()
    } else if last_crawled_page > 0 {
        // 마지막 크롤링 이후 신규 데이터 확인
        (last_crawled_page + 1, total_pages)
    } else {
        // 처음 크롤링인 경우
        (1, std::cmp::min(50, total_pages))
    };
    
    tracing::info!("Intelligent range calculated: {} to {} (reason: site_analysis)", start_page, end_page);
    Ok((start_page, end_page))
}
```

## 4. 구현 우선순위 및 영향도 분석

### 4.1. 우선순위 매트릭스

| 개선사항 | 구현 복잡도 | 사용자 영향도 | 시스템 안정성 | 우선순위 |
|----------|-------------|---------------|---------------|----------|
| 즉시 중단 메커니즘 | 중간 | 높음 | 높음 | **1순위** |
| 지능형 범위 계산 | 낮음 | 중간 | 중간 | **2순위** |
| Worker 레벨 협조적 취소 | 높음 | 중간 | 높음 | **3순위** |

### 4.2. 단계별 구현 계획

#### Phase 1: 긴급 수정 (1-2일)
- `ServiceBasedBatchCrawlingEngine`에 `stop()` 메서드 추가
- `stop_crawling` 커맨드 즉시 응답 구현
- 기본 중단 메커니즘 구현

#### Phase 2: 지능형 범위 계산 활성화 (2-3일)
- `start_crawling` 워크플로우 재설계
- 항상 지능형 범위 계산 수행하도록 수정
- UI에 계산된 범위 정보 표시

#### Phase 3: 완전한 협조적 취소 (3-5일)
- 모든 Worker에 `tokio::select!` 패턴 적용
- `CancellationToken` 전파 체계 완성
- 종합적인 테스트 수행

### 4.3. 성공 지표

#### 정량적 지표
- **중단 응답 시간**: 5초 이내 → 1초 이내
- **범위 계산 정확도**: 하드코딩 → 100% 지능형 계산
- **사용자 만족도**: 중단 기능 사용 시 응답성 개선

#### 정성적 지표
- 사용자가 중단 버튼을 눌렀을 때 즉시 피드백 제공
- 크롤링 시작 시 최적의 범위 자동 계산
- 시스템 안정성 및 예측 가능성 향상

## 5. 결론 및 권장사항

### 5.1. 핵심 개선 효과

위에 제안된 두 가지 개선안을 적용함으로써, 현재 구현의 가장 큰 문제점인 '제어 불능'과 '정적 동작'을 해결할 수 있습니다. 이는 시스템의 안정성과 효율성을 크게 향상시키고, 우리가 목표로 하는 v4.0 아키텍처의 사용자 경험을 제공하는 데 필수적인 단계입니다.

### 5.2. 즉시 실행 권장사항

1. **긴급 수정 먼저**: 제어 불능 문제는 사용자 경험에 직접적인 영향을 미치므로 최우선으로 해결
2. **점진적 개선**: 전체 시스템을 한 번에 바꾸지 말고, 단계적으로 개선하여 안정성 확보
3. **철저한 테스트**: 각 단계마다 중단 기능과 범위 계산 로직을 철저히 테스트

### 5.3. 장기적 비전

이 개선안들은 v4.0 아키텍처의 완성도를 높이고, 향후 추가 기능 개발의 기반을 마련합니다. 특히 실시간 제어 메커니즘은 향후 동적 설정 변경, 실시간 모니터링 등의 고급 기능 구현에 필수적입니다.

### 5.4. 다음 단계 실행 계획

1. **즉시 시작**: Phase 1의 긴급 수정 사항부터 구현 시작
2. **코드 리뷰**: 각 단계별 구현 후 코드 리뷰 수행
3. **사용자 테스트**: 개선된 기능에 대한 사용자 테스트 수행
4. **피드백 반영**: 테스트 결과를 바탕으로 추가 개선사항 도출

다음 단계로 이 제안들을 실제 코드에 반영하는 작업을 진행할 것을 강력히 권장합니다.
