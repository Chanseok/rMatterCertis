// Session management for OneShot Actor crawling system
// Provides thread-safe session state management with pause/resume functionality

#[derive(Debug, Clone)]
pub enum CrawlingSessionStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Error(String),
}

#[derive(Debug)]
pub struct CrawlingSessionManager {
    status: std::sync::RwLock<CrawlingSessionStatus>,
    session_id: std::sync::RwLock<String>,
}

impl CrawlingSessionManager {
    pub fn new() -> Self {
        Self {
            status: std::sync::RwLock::new(CrawlingSessionStatus::Idle),
            session_id: std::sync::RwLock::new(String::new()),
        }
    }

    pub fn start_session(&self) -> Result<String, String> {
        let mut status = self.status.write().map_err(|_| "Lock error")?;
        
        match &*status {
            CrawlingSessionStatus::Running => {
                return Err("Session already running".to_string());
            }
            _ => {}
        }

        let session_id = format!("session_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_secs());
        
        *status = CrawlingSessionStatus::Running;
        
        let mut session_id_guard = self.session_id.write().map_err(|_| "Lock error")?;
        *session_id_guard = session_id.clone();
        
        Ok(session_id)
    }

    pub fn pause_session(&self) -> Result<(), String> {
        let mut status = self.status.write().map_err(|_| "Lock error")?;
        
        match &*status {
            CrawlingSessionStatus::Running => {
                *status = CrawlingSessionStatus::Paused;
                Ok(())
            }
            _ => Err("Session not running".to_string())
        }
    }

    pub fn resume_session(&self) -> Result<(), String> {
        let mut status = self.status.write().map_err(|_| "Lock error")?;
        
        match &*status {
            CrawlingSessionStatus::Paused => {
                *status = CrawlingSessionStatus::Running;
                Ok(())
            }
            _ => Err("Session not paused".to_string())
        }
    }

    pub fn stop_session(&self) -> Result<(), String> {
        let mut status = self.status.write().map_err(|_| "Lock error")?;
        
        match &*status {
            CrawlingSessionStatus::Running | CrawlingSessionStatus::Paused => {
                *status = CrawlingSessionStatus::Completed;
                Ok(())
            }
            _ => Err("No active session to stop".to_string())
        }
    }

    pub fn get_status(&self) -> Result<CrawlingSessionStatus, String> {
        let status = self.status.read().map_err(|_| "Lock error")?;
        Ok(status.clone())
    }

    pub fn get_session_id(&self) -> Result<String, String> {
        let session_id = self.session_id.read().map_err(|_| "Lock error")?;
        Ok(session_id.clone())
    }
}
