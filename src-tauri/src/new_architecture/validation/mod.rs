//! Validation (MI-2) Skeleton
//! Provides event enums and coordinator stub for page_id/index_in_page integrity validation.

use serde::Serialize;
use tracing::info;

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ValidationEvent {
    Started {
        scan_depth: u32,
        last_physical_page: u32,
    },
    PageFetchStarted {
        physical_page: u32,
    },
    PageFetchDone {
        physical_page: u32,
        raw_count: usize,
        ms_fetch: u128,
        ms_parse: u128,
    },
    AssignmentProgress {
        assigned_total: usize,
        last_page_id: u32,
    },
    DiffReady {
        total: usize,
        missing_in_db: usize,
        orphan_in_db: usize,
        coord_mismatch: usize,
    },
    Completed {
        duration_ms: u128,
        remaining_mismatch: usize,
    },
    Warning {
        code: String,
        detail: String,
    },
}

pub struct ValidationCoordinator {
    scan_depth: u32,
}

impl ValidationCoordinator {
    pub fn new(scan_depth: u32) -> Self {
        Self { scan_depth }
    }

    pub async fn run(&self) {
        let start = std::time::Instant::now();
        info!("[VALIDATION] Started scan_depth={}", self.scan_depth);
        // TODO(MI-2): integrate with site state + page fetch actors
        info!(
            "[VALIDATION] Completed in {} ms",
            start.elapsed().as_millis()
        );
    }
}

// Future: emit events via EventEmitter (MI-2 integration)
