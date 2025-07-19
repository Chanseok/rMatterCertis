# 다음 구현 단계: OneShot 데이터 채널 완전 통합

## 🚨 즉시 수행해야 할 작업

### 1. SessionActor에서 BatchActor로의 완전한 OneShot 통합

**현재 문제점:**
- `SessionActor`가 `BatchActor`를 직접 호출하고 있음 (`process_pages` 메서드)
- OneShot 채널을 통한 비동기 결과 대기가 구현되지 않음
- `StageResult`가 실제로 전달되지 않음

**구현해야 할 것:**
```rust
// src-tauri/src/new_architecture/actors/session_actor.rs
impl SessionActor {
    async fn spawn_and_wait_for_batch(&mut self, batch_plan: BatchPlan) -> crate::Result<StageResult> {
        // 1. OneShot 데이터 채널 생성
        let (data_tx, data_rx) = oneshot::channel::<StageResult>();
        
        // 2. BatchActor 스폰
        let batch_actor = BatchActor::new(/*...*/);
        let handle = tokio::spawn(async move {
            batch_actor.run(control_rx, data_tx).await
        });
        
        // 3. 명령 전송
        control_tx.send(ActorCommand::ProcessBatch { /*...*/ }).await?;
        
        // 4. 결과 대기 (타임아웃과 함께)
        let result = tokio::time::timeout(
            Duration::from_secs(self.config.system.session_timeout_secs),
            data_rx
        ).await??;
        
        // 5. 결과에 따른 재시도/에러 처리
        self.handle_batch_result(result).await
    }
}
```

### 2. BatchActor에서 StageActor로의 OneShot 통합

**현재 문제점:**
- `BatchActor`가 `StageActor`를 직접 호출
- 실제 `StageResult` 반환이 구현되지 않음

**구현해야 할 것:**
```rust
// src-tauri/src/new_architecture/actors/batch_actor.rs
impl BatchActor {
    async fn execute_stage_with_oneshot(&mut self, stage_type: StageType, items: Vec<StageItem>) -> StageResult {
        // 1. OneShot 생성
        let (stage_data_tx, stage_data_rx) = oneshot::channel::<StageResult>();
        
        // 2. StageActor 스폰
        let stage_actor = StageActor::new(/*...*/);
        let handle = tokio::spawn(async move {
            stage_actor.run(stage_control_rx, stage_data_tx).await
        });
        
        // 3. 스테이지 명령 전송
        stage_control_tx.send(ActorCommand::ExecuteStage { /*...*/ }).await?;
        
        // 4. 결과 대기
        let result = stage_data_rx.await.unwrap_or_else(|_| StageResult::FatalError {
            error: StageError::ChannelError { message: "Stage channel closed".to_string() },
            stage_id: stage_type.to_string(),
            context: "Channel communication failure".to_string(),
        });
        
        // 5. 재시도 정책 적용
        self.apply_retry_policy(result, stage_type).await
    }
}
```

### 3. 재시도 정책 실제 적용

**현재 문제점:**
- `RetryPolicy`와 `RetryCalculator`가 정의되어 있지만 실제로 사용되지 않음
- 설정 기반 재시도가 구현되지 않음

**구현해야 할 것:**
```rust
// src-tauri/src/new_architecture/actors/batch_actor.rs
impl BatchActor {
    async fn apply_retry_policy(&mut self, result: StageResult, stage_type: StageType) -> StageResult {
        match result {
            StageResult::RecoverableError { error, attempts, .. } => {
                let retry_policy = self.get_retry_policy_for_stage(&stage_type);
                
                if RetryCalculator::should_retry(&retry_policy, &error, attempts) {
                    let delay = RetryCalculator::calculate_delay(&retry_policy, attempts);
                    tokio::time::sleep(delay).await;
                    
                    // 재시도 실행
                    return self.execute_stage_with_oneshot(stage_type, items).await;
                }
                
                // 최대 재시도 초과
                StageResult::FatalError {
                    error: StageError::ValidationError {
                        message: format!("Max retries exceeded: {}", error)
                    },
                    stage_id: stage_type.to_string(),
                    context: "Retry exhausted".to_string(),
                }
            }
            other => other,
        }
    }
}
```

## 🔧 구체적인 실행 계획

### Week 1: OneShot 통합 및 재시도 구현

**Day 1-2: SessionActor OneShot 통합**
- [ ] `SessionActor::spawn_and_wait_for_batch` 구현
- [ ] 타임아웃 및 에러 처리 추가
- [ ] 기본 테스트 작성

**Day 3-4: BatchActor OneShot 통합**
- [ ] `BatchActor::execute_stage_with_oneshot` 구현
- [ ] `StageResult` 실제 반환 구현
- [ ] Stage간 데이터 전달 구현

**Day 5-7: 재시도 정책 구현**
- [ ] `RetryCalculator` 실제 사용
- [ ] 설정 기반 재시도 지연 적용
- [ ] 부분 실패 처리 구현

### Week 2: UI 통합 및 테스트

**Day 8-10: 새 아키텍처와 UI 연결**
- [ ] 새 아키텍처에서 기존 이벤트 시스템으로 연결
- [ ] `CrawlingProcessDashboard`에서 새 Actor 시스템 호출
- [ ] 실시간 이벤트 발송 구현

**Day 11-14: 통합 테스트 및 디버깅**
- [ ] 전체 플로우 테스트
- [ ] 성능 최적화
- [ ] 에러 케이스 처리 완성

## 🚦 성공 지표

### 기술적 완성도
- [ ] `cargo test` 모든 테스트 통과
- [ ] OneShot 채널로 모든 Actor간 통신 완료
- [ ] 설정 파일로 재시도 정책 동적 변경 가능
- [ ] UI에서 실시간 진행 상황 확인 가능

### 사용자 경험
- [ ] 크롤링 시작 → 진행 상황 → 완료까지 매끄러운 플로우
- [ ] 오류 발생 시 자동 재시도 확인 가능
- [ ] 부분 실패 시에도 전체 프로세스 지속

이 계획을 따라 구현하면 `re-arch-plan-final.md`의 완전한 production-ready 아키텍처가 완성됩니다.
