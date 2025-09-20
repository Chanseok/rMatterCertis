use rMatterCertis::commands::database::core_queries::core_fetch_products_page;
use rMatterCertis::commands::database::core_queries::{core_analytics_query, core_diagnostics_analytics_mapping};
use rMatterCertis::infrastructure::test_db::{build_memory_db};
use sqlx::SqlitePool;

static SCHEMA: &str = include_str!("./min_schema.sql");
static SEED: &str = include_str!("./seed_basic.sql");

async fn setup() -> SqlitePool {
    build_memory_db(SCHEMA, SEED).await.expect("memory db")
}

#[tokio::test]
async fn test_core_fetch_products_page_basic() {
    let pool = setup().await;
    // Insert mirror rows into products to satisfy repository expectations (if needed)
    // Minimal: copy product_details -> products
    sqlx::query("INSERT INTO products(url, model, vendor_name) SELECT url, model, vendor_name FROM product_details")
        .execute(&pool).await.unwrap();

    let (page0, total0) = core_fetch_products_page(&pool, 0, 2).await.expect("page0");
    assert_eq!(total0, 3);
    assert_eq!(page0.len(), 2);

    let (page1, _total1) = core_fetch_products_page(&pool, 1, 2).await.expect("page1");
    assert_eq!(page1.len(), 1);
}

#[tokio::test]
async fn test_core_analytics_query_basic_filter_and_sort() {
    let pool = setup().await;
    sqlx::query("INSERT INTO products(url, model, vendor_name) SELECT url, model, vendor_name FROM product_details")
        .execute(&pool).await.unwrap();
    let page = core_analytics_query(&pool, 0, 50, Some("VendorX"), None).await.expect("analytics");
    assert_eq!(page.total, 2);
    assert!(page.rows.len() <= 2);
}

#[tokio::test]
async fn test_core_diagnostics_basic_metrics() {
    let pool = setup().await;
    sqlx::query("INSERT INTO products(url, model, vendor_name) SELECT url, model, vendor_name FROM product_details")
        .execute(&pool).await.unwrap();
    // Ensure bridge exists (seed inserted already)
    let diag = core_diagnostics_analytics_mapping(&pool).await;
    assert_eq!(diag.product_details_total, 3);
    assert!(diag.bridge_rows >= 3);
    assert!(diag.mapping_coverage_pct > 0.0);
}
