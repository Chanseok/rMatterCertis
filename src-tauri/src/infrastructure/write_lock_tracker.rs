use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct ActiveWriteTxInfo {
    pub id: u64,
    pub started_at: DateTime<Utc>,
    pub label: &'static str,
    pub note: &'static str,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE: Lazy<Mutex<Vec<ActiveWriteTxInfo>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub struct WriteTxGuard {
    id: u64,
}

impl WriteTxGuard {
    pub fn id(&self) -> u64 { self.id }
}

impl Drop for WriteTxGuard {
    fn drop(&mut self) {
        let mut guard = ACTIVE.lock().unwrap();
        guard.retain(|info| info.id != self.id);
    }
}

pub fn register(label: &'static str, note: &'static str) -> WriteTxGuard {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let info = ActiveWriteTxInfo { id, started_at: Utc::now(), label, note };
    let mut guard = ACTIVE.lock().unwrap();
    guard.push(info);
    WriteTxGuard { id }
}

pub fn snapshot() -> Vec<ActiveWriteTxInfo> {
    ACTIVE.lock().unwrap().clone()
}
