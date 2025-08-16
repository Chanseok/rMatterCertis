//! Atomic task events for real-time UI visualization
//!
//! This module defines lightweight, high-frequency events that are emitted
//! immediately when individual tasks change state, enabling real-time
//! animation and detailed progress tracking in the UI.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for individual tasks
pub type TaskId = Uuid;

/// Lightweight event for immediate task state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AtomicTaskEvent {
    TaskStarted {
        task_id: TaskId,
        task_type: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskCompleted {
        task_id: TaskId,
        task_type: String,
        duration_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskFailed {
        task_id: TaskId,
        task_type: String,
        error_message: String,
        retry_count: u32,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskRetrying {
        task_id: TaskId,
        task_type: String,
        retry_count: u32,
        delay_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

impl AtomicTaskEvent {
    /// Get the event name for Tauri emission
    pub fn event_name() -> &'static str {
        "atomic-task-update"
    }

    /// Get the task ID from any variant
    pub fn task_id(&self) -> TaskId {
        match self {
            AtomicTaskEvent::TaskStarted { task_id, .. } => *task_id,
            AtomicTaskEvent::TaskCompleted { task_id, .. } => *task_id,
            AtomicTaskEvent::TaskFailed { task_id, .. } => *task_id,
            AtomicTaskEvent::TaskRetrying { task_id, .. } => *task_id,
        }
    }

    /// Get the task type from any variant
    pub fn task_type(&self) -> &str {
        match self {
            AtomicTaskEvent::TaskStarted { task_type, .. } => task_type,
            AtomicTaskEvent::TaskCompleted { task_type, .. } => task_type,
            AtomicTaskEvent::TaskFailed { task_type, .. } => task_type,
            AtomicTaskEvent::TaskRetrying { task_type, .. } => task_type,
        }
    }

    /// Create a TaskStarted event
    pub fn started(task_id: TaskId, task_type: String) -> Self {
        Self::TaskStarted {
            task_id,
            task_type,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a TaskCompleted event
    pub fn completed(task_id: TaskId, task_type: String, duration_ms: u64) -> Self {
        Self::TaskCompleted {
            task_id,
            task_type,
            duration_ms,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a TaskFailed event
    pub fn failed(
        task_id: TaskId,
        task_type: String,
        error_message: String,
        retry_count: u32,
    ) -> Self {
        Self::TaskFailed {
            task_id,
            task_type,
            error_message,
            retry_count,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a TaskRetrying event
    pub fn retrying(task_id: TaskId, task_type: String, retry_count: u32, delay_ms: u64) -> Self {
        Self::TaskRetrying {
            task_id,
            task_type,
            retry_count,
            delay_ms,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Statistics for atomic task events (for performance monitoring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicEventStats {
    pub events_emitted: u64,
    pub events_per_second: f64,
    pub last_emission_time: chrono::DateTime<chrono::Utc>,
    pub event_type_counts: std::collections::HashMap<String, u64>,
}

impl Default for AtomicEventStats {
    fn default() -> Self {
        Self {
            events_emitted: 0,
            events_per_second: 0.0,
            last_emission_time: chrono::Utc::now(),
            event_type_counts: std::collections::HashMap::new(),
        }
    }
}

impl AtomicEventStats {
    /// Record a new event emission
    pub fn record_emission(&mut self, event: &AtomicTaskEvent) {
        let now = chrono::Utc::now();
        let time_diff = (now - self.last_emission_time).num_milliseconds() as f64 / 1000.0;

        self.events_emitted += 1;
        self.last_emission_time = now;

        // Update events per second (exponential moving average)
        if time_diff > 0.0 {
            let current_rate = 1.0 / time_diff;
            self.events_per_second = 0.1 * current_rate + 0.9 * self.events_per_second;
        }

        // Update event type counts
        let event_type = match event {
            AtomicTaskEvent::TaskStarted { .. } => "started",
            AtomicTaskEvent::TaskCompleted { .. } => "completed",
            AtomicTaskEvent::TaskFailed { .. } => "failed",
            AtomicTaskEvent::TaskRetrying { .. } => "retrying",
        };

        *self
            .event_type_counts
            .entry(event_type.to_string())
            .or_insert(0) += 1;
    }
}
