use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, warn};

use crate::application::AppState;
use crate::infrastructure::integrated_product_repository::{IntegratedProductRepository, UpsertOutcome};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorSyncResult {
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
    pub api_total: u32,
    pub final_count: u32,
    pub pages: u32,
    pub finished_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CsaVendorItem {
    #[serde(rename = "vendorID")] vendor_id: i32,
    #[serde(rename = "vendorName")] vendor_name: String,
    #[serde(rename = "companyLegalName")] company_legal_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CsaPagination { #[serde(rename = "next_key")] next_key: Option<String>, total: String }

#[derive(Debug, Clone, Deserialize)]
struct CsaVendorsPage { #[serde(rename = "vendorInfo")] vendor_info: Vec<CsaVendorItem>, pagination: CsaPagination }

async fn fetch_csa_page(http: &crate::infrastructure::simple_http_client::HttpClient, key: Option<&str>) -> anyhow::Result<CsaVendorsPage> {
    let base = "https://on.dcl.csa-iot.org/dcl/vendorinfo/vendors";
    let url = if let Some(k) = key { format!("{}?pagination.key={}", base, k) } else { base.to_string() };
    let resp = http.fetch_response_with_policy(&url).await?;
    let text = resp.text().await?;
    let page: CsaVendorsPage = serde_json::from_str(&text)?;
    Ok(page)
}

#[tauri::command]
pub async fn update_vendors_from_csa(state: State<'_, AppState>) -> Result<VendorSyncResult, String> {
    // resources
    let pool = state.get_database_pool().await?;
    let http = state.get_http_client().await?;
    let repo = IntegratedProductRepository::new(pool);

    // first page
    let first = fetch_csa_page(&http, None).await.map_err(|e| format!("CSA fetch failed: {}", e))?;
    let api_total: u32 = first.pagination.total.parse().unwrap_or(0);
    let local_count: u32 = repo.count_vendors().await.map(|c| u32::try_from(c).unwrap_or(u32::MAX)).unwrap_or(0);

    info!("CSA vendors total={}, local_count={}", api_total, local_count);

    let mut inserted = 0u32;
    let mut updated = 0u32;
    let mut skipped = 0u32;
    let mut pages = 0u32;

    // decide whether to proceed based on requirement: only when API total > local
    let mut next_key = if api_total > local_count { first.pagination.next_key.clone() } else { None };

    // process first page if proceeding
    if api_total > local_count {
        for v in first.vendor_info.iter() {
            match repo.upsert_vendor_by_number(v.vendor_id, &v.vendor_name, v.company_legal_name.as_deref()).await {
                Ok(UpsertOutcome { inserted: true, updated: false }) => inserted += 1,
                Ok(UpsertOutcome { inserted: false, updated: true }) => updated += 1,
                Ok(_) => skipped += 1,
                Err(e) => {
                    warn!("Vendor upsert failed for {} ({}): {}", v.vendor_name, v.vendor_id, e);
                }
            }
        }
        pages += 1;

        // iterate remaining pages
        let mut seen_keys = std::collections::HashSet::new();
        while let Some(k) = next_key.clone() {
            if !seen_keys.insert(k.clone()) {
                warn!("Duplicate pagination key detected, breaking: {}", k);
                break;
            }
            let page = match fetch_csa_page(&http, Some(&k)).await {
                Ok(p) => p,
                Err(e) => { warn!("CSA fetch failed on next_key {}: {}", k, e); break; }
            };
            for v in page.vendor_info.iter() {
                match repo.upsert_vendor_by_number(v.vendor_id, &v.vendor_name, v.company_legal_name.as_deref()).await {
                    Ok(UpsertOutcome { inserted: true, updated: false }) => inserted += 1,
                    Ok(UpsertOutcome { inserted: false, updated: true }) => updated += 1,
                    Ok(_) => skipped += 1,
                    Err(e) => warn!("Vendor upsert failed for {} ({}): {}", v.vendor_name, v.vendor_id, e),
                }
            }
            pages += 1;
            next_key = page.pagination.next_key;
        }
    }

    let final_count: u32 = repo.count_vendors().await.map(|c| u32::try_from(c).unwrap_or(u32::MAX)).unwrap_or(0);
    Ok(VendorSyncResult {
        inserted,
        updated,
        skipped,
        api_total,
        final_count,
        pages,
        finished_at: Utc::now().to_rfc3339(),
    })
}
