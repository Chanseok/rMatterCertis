//! 중앙집중식 데이터베이스 경로 관리
//!
//! Modern Rust 2024 가이드에 따라 데이터베이스 경로를 한 곳에서만 관리하여
//! "엉뚱한 경로를 잡는 문제"를 영구적으로 해결
//!
//! 핵심 원칙:
//! 1. 단일 책임: 데이터베이스 경로 관리만 담당
//! 2. 불변성: 한번 설정된 경로는 변경되지 않음
//! 3. 예측 가능성: 항상 동일한 경로 생성 로직
//! 4. 에러 안전성: 경로 생성 실패 시 안전한 폴백

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{debug, info};

/// 전역 데이터베이스 경로 관리자 (싱글톤)
static DATABASE_PATH_MANAGER: OnceLock<DatabasePathManager> = OnceLock::new();

/// 데이터베이스 경로 관리자 (단순화: 메인 DB만 사용)
#[derive(Debug, Clone)]
pub struct DatabasePathManager {
    /// 기본 데이터베이스 파일 경로 (절대 경로)
    pub main_database_path: PathBuf,
    /// 데이터베이스 디렉토리 경로
    pub database_directory: PathBuf,
}

impl DatabasePathManager {
    /// 새로운 경로 관리자 생성 (내부 용도)
    fn new() -> Result<Self> {
        let app_data_dir = Self::get_app_data_directory()?;
        let database_directory = app_data_dir.join("database");

        Ok(Self {
            main_database_path: database_directory.join("matter_certis.db"),
            database_directory,
        })
    }

    /// 앱 데이터 디렉토리 결정 (Modern Rust 2024 방식)
    pub fn get_app_data_directory() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Data directory not found"))?
            .join("matter-certis-v2");

        // Create directory if it doesn't exist
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("Failed to create directory: {}", data_dir.display()))?;
        }

        Ok(data_dir)
    }

    /// 전역 인스턴스 초기화 (앱 시작 시 한 번만 호출)
    pub fn initialize() -> Result<()> {
        let manager = Self::new()?;
        DATABASE_PATH_MANAGER
            .set(manager)
            .map_err(|_| anyhow::anyhow!("DatabasePathManager가 이미 초기화되었습니다"))?;
        Ok(())
    }

    /// 전역 인스턴스 가져오기
    pub fn global() -> &'static DatabasePathManager {
        DATABASE_PATH_MANAGER
            .get()
            .expect("DatabasePathManager가 초기화되지 않았습니다. initialize()를 먼저 호출하세요")
    }

    /// 메인 데이터베이스 URL 반환 (SQLx 형식)
    pub fn get_main_database_url(&self) -> String {
        format!("sqlite:{}", self.main_database_path.display())
    }

    /// 필요한 디렉토리 생성 (메인 DB만)
    pub async fn ensure_directories_exist(&self) -> Result<()> {
        // 메인 데이터베이스 디렉토리
        if let Some(parent) = self.main_database_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("메인 데이터베이스 디렉토리 생성 실패")?;
        }

        Ok(())
    }

    /// 데이터베이스 파일이 존재하는지 확인
    pub fn database_exists(&self) -> bool {
        self.main_database_path.exists()
    }

    /// 데이터베이스 파일 생성 (존재하지 않는 경우)
    pub async fn ensure_database_file_exists(&self) -> Result<()> {
        if !self.database_exists() {
            // 디렉토리부터 생성
            self.ensure_directories_exist().await?;

            // 빈 파일 생성
            tokio::fs::File::create(&self.main_database_path)
                .await
                .context("데이터베이스 파일 생성 실패")?;

            tracing::info!(
                "✅ 새 데이터베이스 파일 생성: {}",
                self.main_database_path.display()
            );
        }

        Ok(())
    }

    /// 데이터베이스 파일이 쓰기 가능한지 확인
    pub fn is_database_writable(&self) -> bool {
        if !self.database_exists() {
            return false;
        }

        // 실제 쓰기 테스트
        std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.main_database_path)
            .is_ok()
    }

    /// 완전한 데이터베이스 초기화 (경로 + 파일 + 권한)
    pub async fn full_initialization(&self) -> Result<()> {
        let concise_all = std::env::var("MC_CONCISE_ALL")
            .ok()
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true);
        let concise = concise_all
            || std::env::var("MC_CONCISE_STARTUP")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        if concise {
            debug!("🔧 데이터베이스 경로 전체 초기화 시작...");
        } else {
            info!("🔧 데이터베이스 경로 전체 초기화 시작...");
        }

        // 1. 디렉토리 생성
        self.ensure_directories_exist()
            .await
            .context("디렉토리 생성 단계 실패")?;

        // 2. 파일 생성
        self.ensure_database_file_exists()
            .await
            .context("파일 생성 단계 실패")?;

        // 3. 권한 확인
        if !self.is_database_writable() {
            anyhow::bail!(
                "데이터베이스 파일에 쓰기 권한이 없습니다: {}",
                self.main_database_path.display()
            );
        }

        if concise {
            debug!("✅ 데이터베이스 경로 전체 초기화 완료");
            info!("📁 메인 DB: {}", self.main_database_path.display());
        } else {
            info!("✅ 데이터베이스 경로 전체 초기화 완료");
            info!("📁 메인 DB: {}", self.main_database_path.display());
        }

        Ok(())
    }
}

/// 편의 함수들 - 전역에서 쉽게 사용할 수 있도록

/// 메인 데이터베이스 URL 가져오기 (가장 자주 사용)
pub fn get_main_database_url() -> String {
    DatabasePathManager::global().get_main_database_url()
}

/// 데이터베이스 전체 초기화 (앱 시작 시 호출)
pub async fn initialize_database_paths() -> Result<()> {
    // 1. 경로 관리자 초기화
    DatabasePathManager::initialize().context("DatabasePathManager 초기화 실패")?;

    // 2. 실제 파일 시스템 초기화
    DatabasePathManager::global()
        .full_initialization()
        .await
        .context("데이터베이스 파일 시스템 초기화 실패")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_database_path_consistency() {
        let manager = DatabasePathManager::new().expect("Manager 생성 실패");

        // 같은 URL이 반복 호출 시에도 동일한지 확인
        let url1 = manager.get_main_database_url();
        let url2 = manager.get_main_database_url();
        assert_eq!(url1, url2, "데이터베이스 URL이 일관성이 없습니다");

        // URL 형식이 올바른지 확인
        assert!(
            url1.starts_with("sqlite:"),
            "잘못된 SQLite URL 형식: {}",
            url1
        );
    }

    #[tokio::test]
    async fn test_directory_creation() {
        let temp_dir = TempDir::new().expect("임시 디렉토리 생성 실패");
        let test_path = temp_dir.path().join("test_db").join("test.db");

        // 존재하지 않는 경로
        assert!(!test_path.exists());

        // 디렉토리 생성 테스트 (실제 코드 로직 모방)
        if let Some(parent) = test_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .expect("디렉토리 생성 실패");
        }

        assert!(
            test_path.parent().unwrap().exists(),
            "디렉토리가 생성되지 않았습니다"
        );
    }
}
