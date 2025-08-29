//! 채널 타입 정의
//! 
//! Actor 간 통신을 위한 삼중 채널 시스템을 정의합니다:
//! - Control Channel: Actor 간 명령 전달
//! - Data Channel: 일회성 결과 전달  
//! - Event Channel: 상태 변화 브로드캐스트

use tokio::sync::{mpsc, oneshot, broadcast};
use serde::{Serialize, Deserialize};
use ts_rs::TS;

/// 제어 채널: Actor 간 명령 전달용
/// 
/// 명령과 제어 신호를 전달하기 위한 다중 생산자, 단일 소비자 채널입니다.
/// 버퍼링을 통해 백프레셔를 관리할 수 있습니다.
pub type ControlChannel<T> = mpsc::Sender<T>;
pub type ControlReceiver<T> = mpsc::Receiver<T>;

/// 데이터 채널: 일회성 결과 전달용
/// 
/// 작업 결과나 응답을 전달하기 위한 단일 사용 채널입니다.
/// 요청-응답 패턴에 적합합니다.
pub type DataChannel<T> = oneshot::Sender<T>;
pub type DataReceiver<T> = oneshot::Receiver<T>;

/// 이벤트 채널: 상태 변화 브로드캐스트용
/// 
/// 시스템 이벤트를 여러 구독자에게 동시에 전달하기 위한 채널입니다.
/// 이벤트 기반 아키텍처의 핵심 구성 요소입니다.
pub type EventChannel<T> = broadcast::Sender<T>;
pub type EventReceiver<T> = broadcast::Receiver<T>;

/// 채널 생성 및 관리를 위한 팩토리
/// 
/// 다양한 타입의 채널을 생성하고 설정을 관리합니다.
/// 함수형 프로그래밍 원칙을 따라 상태를 갖지 않습니다.
pub struct ChannelFactory;

impl ChannelFactory {
    /// 제어 채널을 생성합니다.
    /// 
    /// # Arguments
    /// * `buffer_size` - 채널 버퍼 크기
    /// 
    /// # Returns
    /// * `(ControlChannel<T>, ControlReceiver<T>)` - 송신단과 수신단 튜플
    #[must_use]
    pub fn create_control_channel<T>(buffer_size: usize) -> (ControlChannel<T>, ControlReceiver<T>) {
        mpsc::channel(buffer_size)
    }
    
    /// 데이터 채널을 생성합니다.
    /// 
    /// # Returns
    /// * `(DataChannel<T>, DataReceiver<T>)` - 송신단과 수신단 튜플
    #[must_use]
    pub fn create_data_channel<T>() -> (DataChannel<T>, DataReceiver<T>) {
        oneshot::channel()
    }
    
    /// 이벤트 채널을 생성합니다.
    /// 
    /// # Arguments
    /// * `buffer_size` - 채널 버퍼 크기
    /// 
    /// # Returns
    /// * `EventChannel<T>` - 브로드캐스트 송신단
    #[must_use]
    pub fn create_event_channel<T: Clone>(buffer_size: usize) -> EventChannel<T> {
        broadcast::channel(buffer_size).0
    }
    
    /// 표준 버퍼 크기로 제어 채널을 생성합니다.
    /// 
    /// # Returns
    /// * `(ControlChannel<T>, ControlReceiver<T>)` - 송신단과 수신단 튜플
    #[must_use]
    pub fn create_default_control_channel<T>() -> (ControlChannel<T>, ControlReceiver<T>) {
        Self::create_control_channel(DEFAULT_CONTROL_BUFFER_SIZE)
    }
    
    /// 표준 버퍼 크기로 이벤트 채널을 생성합니다.
    /// 
    /// # Returns
    /// * `EventChannel<T>` - 브로드캐스트 송신단
    #[must_use]
    pub fn create_default_event_channel<T>() -> EventChannel<T> {
        Self::create_event_channel(DEFAULT_EVENT_BUFFER_SIZE)
    }
}

/// 채널 설정 상수
const DEFAULT_CONTROL_BUFFER_SIZE: usize = 100;
const DEFAULT_EVENT_BUFFER_SIZE: usize = 1000;

/// 채널 통계 정보
/// 
/// 채널의 성능 모니터링을 위한 메트릭을 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChannelMetrics {
    /// 채널 타입
    pub channel_type: ChannelType,
    
    /// 송신된 메시지 수
    pub messages_sent: u64,
    
    /// 수신된 메시지 수  
    pub messages_received: u64,
    
    /// 드롭된 메시지 수 (브로드캐스트 채널의 경우)
    pub messages_dropped: u64,
    
    /// 현재 큐 크기
    pub current_queue_size: usize,
    
    /// 최대 큐 크기
    pub max_queue_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ChannelType {
    Control,
    Data,
    Event,
}

/// 채널 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("채널이 닫혔습니다")]
    ChannelClosed,
    
    #[error("메시지 전송 실패: {0}")]
    SendFailed(String),
    
    #[error("메시지 수신 실패: {0}")]
    ReceiveFailed(String),
    
    #[error("채널 버퍼가 가득 참")]
    BufferFull,
    
    #[error("타임아웃 발생")]
    Timeout,
}

/// 채널 유틸리티 함수들
pub mod utils {
    use super::*;
    use tokio::time::{timeout, Duration};
    
    /// 타임아웃과 함께 메시지를 전송합니다.
    /// 
    /// # Arguments
    /// * `channel` - 송신 채널
    /// * `message` - 전송할 메시지
    /// * `timeout_duration` - 타임아웃 시간
    /// 
    /// # Returns
    /// * `Ok(())` - 전송 성공
    /// * `Err(ChannelError)` - 전송 실패 또는 타임아웃
    pub async fn send_with_timeout<T>(
        channel: &ControlChannel<T>,
        message: T,
        timeout_duration: Duration,
    ) -> Result<(), ChannelError> {
        timeout(timeout_duration, channel.send(message))
            .await
            .map_err(|_| ChannelError::Timeout)?
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;
        
        Ok(())
    }
    
    /// 타임아웃과 함께 메시지를 수신합니다.
    /// 
    /// # Arguments
    /// * `receiver` - 수신 채널
    /// * `timeout_duration` - 타임아웃 시간
    /// 
    /// # Returns
    /// * `Ok(T)` - 수신 성공
    /// * `Err(ChannelError)` - 수신 실패 또는 타임아웃
    pub async fn recv_with_timeout<T>(
        receiver: &mut ControlReceiver<T>,
        timeout_duration: Duration,
    ) -> Result<T, ChannelError> {
        timeout(timeout_duration, receiver.recv())
            .await
            .map_err(|_| ChannelError::Timeout)?
            .ok_or(ChannelError::ChannelClosed)
    }
    
    /// 이벤트 채널에서 특정 타입의 이벤트만 필터링합니다.
    /// 
    /// # Arguments
    /// * `receiver` - 이벤트 수신 채널
    /// * `filter` - 필터링 함수
    /// 
    /// # Returns
    /// * `Option<T>` - 필터 조건에 맞는 이벤트 또는 None
    pub async fn filter_event<T, F>(
        receiver: &mut EventReceiver<T>,
        filter: F,
    ) -> Option<T>
    where
        T: Clone,
        F: Fn(&T) -> bool,
    {
        while let Ok(event) = receiver.recv().await {
            if filter(&event) {
                return Some(event);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_control_channel_creation() {
        let (tx, mut rx) = ChannelFactory::create_control_channel::<String>(10);
        
        tx.send("test message".to_string()).await.unwrap();
        let received = rx.recv().await.unwrap();
        
        assert_eq!(received, "test message");
    }

    #[tokio::test]
    async fn test_data_channel_creation() {
        let (tx, rx) = ChannelFactory::create_data_channel::<i32>();
        
        tx.send(42).unwrap();
        let received = rx.await.unwrap();
        
        assert_eq!(received, 42);
    }

    #[tokio::test]
    async fn test_event_channel_creation() {
        let tx = ChannelFactory::create_event_channel::<String>(10);
        let mut rx1 = tx.subscribe();
        let mut rx2 = tx.subscribe();
        
        tx.send("broadcast message".to_string()).unwrap();
        
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        
        assert_eq!(received1, "broadcast message");
        assert_eq!(received2, "broadcast message");
    }

    #[tokio::test]
    async fn test_default_channel_creation() {
        let (tx, mut rx) = ChannelFactory::create_default_control_channel::<String>();
        
        tx.send("default test".to_string()).await.unwrap();
        let received = rx.recv().await.unwrap();
        
        assert_eq!(received, "default test");
    }

    #[tokio::test]
    async fn test_send_with_timeout() {
        let (tx, _rx) = ChannelFactory::create_control_channel::<String>(1);
        
        // 첫 번째 메시지는 성공
        let result = utils::send_with_timeout(
            &tx,
            "message1".to_string(),
            Duration::from_millis(100),
        ).await;
        assert!(result.is_ok());
        
        // 버퍼가 가득 찬 상태에서 타임아웃 테스트
        let result = utils::send_with_timeout(
            &tx,
            "message2".to_string(),
            Duration::from_millis(10),
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_recv_with_timeout() {
        let (_tx, mut rx) = ChannelFactory::create_control_channel::<String>(10);
        
        // 빈 채널에서 타임아웃 테스트
        let result = utils::recv_with_timeout(&mut rx, Duration::from_millis(10)).await;
        assert!(result.is_err());
    }
}

// ⚠️ Phase 2 호환성: 기존 코드가 찾는 타입들 re-export
pub use crate::crawl_engine::actors::types::{
    StageType, StageItem, StageResult, ActorCommand, AppEvent, 
    BatchConfig, CrawlingConfig, SessionSummary
};
