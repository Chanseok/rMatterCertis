//! DataConsistencyChecker
//! DB의 products 테이블에 저장된 page_id, index_in_page 값이
//! 현재 사이트 구조( total_pages, last page 제품 수 )와 일치하는지 검증.
//!
//! 검증 방식:
//! 1) 사이트 상태(StatusChecker)로 total_pages, products_on_last_page 획득
//! 2) 모든 products (page_id, index_in_page, page_id NULL 제외) 조회
//! 3) 역산(reverse_calculate)을 통해 (물리적 페이지, 물리적 인덱스) -> 다시 정방향 calculate 후 동일성 확인
//! 4) 불일치/경계 오류/범위 초과 등을 집계
//! 5) JSON 형태 요약 반환 (명세: DataConsistencyReport)
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use tracing::info;

use crate::domain::pagination::CanonicalPageIdCalculator;
use sqlx::Row; // trait for row.get
use crate::domain::services::StatusChecker;
use crate::infrastructure::IntegratedProductRepository;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InconsistentRecord {
    pub url: String,
    pub stored_page_id: i32,
    pub stored_index_in_page: i32,
    pub recomputed_page_id: i32,
    pub recomputed_index_in_page: i32,
    pub physical_page: u32,
    pub physical_index: usize,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataConsistencyReport {
    pub total_checked: u64,
    pub valid: u64,
    pub invalid: u64,
    pub skipped_null_page_id: u64,
    pub total_pages_site: u32,
    pub products_on_last_page_site: u32,
    pub sample_inconsistencies: Vec<InconsistentRecord>,
    pub max_samples: usize,
}

pub struct DataConsistencyChecker {
    status_checker: Arc<dyn StatusChecker>,
    product_repo: Arc<IntegratedProductRepository>,
    max_samples: usize,
}

impl DataConsistencyChecker {
    pub fn new(status_checker: Arc<dyn StatusChecker>, product_repo: Arc<IntegratedProductRepository>) -> Self {
        Self { status_checker, product_repo, max_samples: 50 }
    }

    pub fn with_max_samples(mut self, samples: usize) -> Self { self.max_samples = samples; self }

    pub async fn run_check(&self) -> Result<DataConsistencyReport> {
        // 1. 사이트 상태 획득
        let site_status = self.status_checker.check_site_status().await?;
    let total_pages = if site_status.total_pages == 0 { 1 } else { site_status.total_pages };
    let products_on_last_page = site_status.products_on_last_page; // u32
    let calculator = CanonicalPageIdCalculator::new(total_pages, products_on_last_page as usize);

        // 2. products 테이블에서 필요한 필드 조회
        // NOTE: compile-time query macros (query!) require offline DB schema; fallback to dynamic query + manual extraction
        let rows = sqlx::query(
            "SELECT url, page_id, index_in_page FROM products"
        )
        .fetch_all(&*self.product_repo.pool())
        .await?;

        let mut valid = 0u64;
        let mut invalid = 0u64;
        let mut skipped_null = 0u64;
        let mut samples: Vec<InconsistentRecord> = Vec::new();

        for row in rows {
            let url: String = row.get("url");
            let stored_page_id: Option<i32> = row.get::<Option<i32>, _>("page_id");
            let stored_index_in_page: Option<i32> = row.get::<Option<i32>, _>("index_in_page");
            let Some(stored_page_id) = stored_page_id else { skipped_null += 1; continue; };
            let Some(stored_index_in_page) = stored_index_in_page else { skipped_null += 1; continue; };
            // 역방향 계산: (page_id, index_in_page) -> (physical_page, physical_index)
            match calculator.reverse_calculate(stored_page_id, stored_index_in_page) {
                Some((phys_page, phys_index)) => {
                    // 정방향 재계산
                    let recalc = calculator.calculate(phys_page, phys_index);
                    if recalc.page_id == stored_page_id && recalc.index_in_page == stored_index_in_page {
                        valid += 1;
                    } else {
                        invalid += 1;
                        if samples.len() < self.max_samples {
                            samples.push(InconsistentRecord {
                                url: url.clone(),
                                stored_page_id,
                                stored_index_in_page,
                                recomputed_page_id: recalc.page_id,
                                recomputed_index_in_page: recalc.index_in_page,
                                physical_page: phys_page,
                                physical_index: phys_index,
                                reason: "Recalculation mismatch".into(),
                            });
                        }
                    }
                }
                None => {
                    invalid += 1;
                    if samples.len() < self.max_samples {
                        samples.push(InconsistentRecord {
                            url,
                            stored_page_id,
                            stored_index_in_page,
                            recomputed_page_id: -1,
                            recomputed_index_in_page: -1,
                            physical_page: 0,
                            physical_index: 0,
                            reason: "Reverse calculation failed (out of range)".into(),
                        });
                    }
                }
            }
        }

        let report = DataConsistencyReport {
            total_checked: valid + invalid + skipped_null,
            valid,
            invalid,
            skipped_null_page_id: skipped_null,
            total_pages_site: total_pages,
            products_on_last_page_site: products_on_last_page,
            sample_inconsistencies: samples,
            max_samples: self.max_samples,
        };
        info!("🧪 DataConsistencyChecker report: total={} valid={} invalid={} skipped_null={}", report.total_checked, report.valid, report.invalid, report.skipped_null_page_id);
        Ok(report)
    }
}
