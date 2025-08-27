use crate::application::AppState;
use serde_json::json;
use sqlx::Row;
use tauri::State;
use tracing::info;

/// Return aggregated analytics for product_details to power charts.
#[tauri::command]
pub async fn get_product_details_analytics(
    state: State<'_, AppState>,
    start_date: Option<String>,
    end_date: Option<String>,
    manufacturers: Option<Vec<String>>,
    spec_versions: Option<Vec<String>>,
    transport_interfaces: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let pool = state
        .get_database_pool()
        .await
        .map_err(|e| format!("DB pool unavailable: {e}"))?;

    // (kept placeholder for potential reuse)

    // Build shared WHERE clause and a helper to bind values in the same order for each query
    fn build_where_clause(
        start_date: &Option<String>,
        end_date: &Option<String>,
        manufacturers: &Option<Vec<String>>,
        spec_versions: &Option<Vec<String>>,
        transport_interfaces: &Option<Vec<String>>,
    ) -> (String, Vec<String>) {
        let mut where_sql = String::from("WHERE 1=1");
        let mut params: Vec<String> = Vec::new();

        if let (Some(s), Some(e)) = (start_date, end_date) {
            where_sql.push_str(" AND substr(certification_date,1,10) >= ? AND substr(certification_date,1,10) <= ?");
            params.push(s.clone());
            params.push(e.clone());
        }

        if let Some(list) = manufacturers {
            if !list.is_empty() {
                where_sql.push_str(" AND manufacturer IN (");
                where_sql.push_str(&vec!["?"; list.len()].join(","));
                where_sql.push(')');
                params.extend(list.iter().cloned());
            }
        }

        if let Some(list) = spec_versions {
            if !list.is_empty() {
                where_sql.push_str(" AND specification_version IN (");
                where_sql.push_str(&vec!["?"; list.len()].join(","));
                where_sql.push(')');
                params.extend(list.iter().cloned());
            }
        }

        if let Some(list) = transport_interfaces {
            if !list.is_empty() {
                where_sql.push_str(" AND transport_interface IN (");
                where_sql.push_str(&vec!["?"; list.len()].join(","));
                where_sql.push(')');
                params.extend(list.iter().cloned());
            }
        }

        (where_sql, params)
    }

    fn bind_all<'a>(
        mut q: sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>>,
        params: &'a [String],
    ) -> sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
        for p in params {
            q = q.bind(p);
        }
        q
    }

    let (where_sql, where_params) = build_where_clause(&start_date, &end_date, &manufacturers, &spec_versions, &transport_interfaces);

    // Totals
    let total_details: i64 = {
        let sql = format!("SELECT COUNT(*) FROM product_details {}", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params {
            q = q.bind(p);
        }
        q.fetch_one(&pool).await.unwrap_or(0)
    };

    // (removed old group_by helpers; using filter-aware helpers below)

    // Time series by certification_date (stored as TEXT yyyy-mm-dd or similar). Use substr to date-only.
    let cert_by_date = {
        let sql = format!(
            r#"SELECT substr(certification_date,1,10) AS d, COUNT(*) AS c
               FROM product_details
               {} AND certification_date IS NOT NULL AND length(certification_date) >= 10
               GROUP BY substr(certification_date,1,10)
               ORDER BY d ASC"#,
            where_sql
        );
        let q = sqlx::query(&sql);
    bind_all(q, &where_params)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    }
    .into_iter()
    .map(|r| json!({
        "date": r.try_get::<Option<String>, _>("d").ok().flatten(),
        "count": r.get::<i64, _>("c")
    }))
    .collect::<Vec<_>>();

    // created_at / updated_at daily time series
    let created_daily = {
        let sql = format!(
            r#"SELECT date(created_at) AS d, COUNT(*) AS c
               FROM product_details {} GROUP BY date(created_at) ORDER BY d ASC"#,
            where_sql
        );
        let q = sqlx::query(&sql);
    bind_all(q, &where_params)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    }
    .into_iter()
    .map(|r| json!({
        "date": r.try_get::<Option<String>, _>("d").ok().flatten(),
        "count": r.get::<i64, _>("c")
    }))
    .collect::<Vec<_>>();

    let updated_daily = {
        let sql = format!(
            r#"SELECT date(updated_at) AS d, COUNT(*) AS c
               FROM product_details {} GROUP BY date(updated_at) ORDER BY d ASC"#,
            where_sql
        );
        let q = sqlx::query(&sql);
    bind_all(q, &where_params)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    }
    .into_iter()
    .map(|r| json!({
        "date": r.try_get::<Option<String>, _>("d").ok().flatten(),
        "count": r.get::<i64, _>("c")
    }))
    .collect::<Vec<_>>();

    // Page distribution and index heatmap
    let page_distribution = {
        let sql = format!(
            r#"SELECT page_id AS p, COUNT(*) AS c FROM product_details
               {} AND page_id IS NOT NULL GROUP BY page_id ORDER BY p ASC"#,
            where_sql
        );
        let q = sqlx::query(&sql);
    bind_all(q, &where_params)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    }
    .into_iter()
    .map(|r| json!({
        "page_id": r.try_get::<Option<i64>, _>("p").ok().flatten(),
        "count": r.get::<i64, _>("c")
    }))
    .collect::<Vec<_>>();

    let index_heatmap = {
        let sql = format!(
            r#"SELECT page_id AS p, index_in_page AS i, COUNT(*) AS c FROM product_details
               {} AND page_id IS NOT NULL AND index_in_page IS NOT NULL
               GROUP BY page_id, index_in_page ORDER BY p ASC, i ASC"#,
            where_sql
        );
        let q = sqlx::query(&sql);
        bind_all(q, &where_params)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    }
    .into_iter()
    .map(|r| json!({
        "page_id": r.try_get::<Option<i64>, _>("p").ok().flatten(),
        "index_in_page": r.try_get::<Option<i64>, _>("i").ok().flatten(),
        "count": r.get::<i64, _>("c")
    }))
    .collect::<Vec<_>>();

    // Completeness metrics (non-null counts)
    let manufacturer_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND manufacturer IS NOT NULL AND manufacturer <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let model_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND model IS NOT NULL AND model <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let certificate_id_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND certificate_id IS NOT NULL AND certificate_id <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let device_type_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND device_type IS NOT NULL AND device_type <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let certification_date_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND certification_date IS NOT NULL AND certification_date <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let software_version_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND software_version IS NOT NULL AND software_version <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let hardware_version_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND hardware_version IS NOT NULL AND hardware_version <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };
    let spec_version_filled = {
        let sql = format!("SELECT COUNT(*) FROM product_details {} AND specification_version IS NOT NULL AND specification_version <> ''", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for p in &where_params { q = q.bind(p); }
        q.fetch_one(&pool).await.unwrap_or(0)
    };

    // Grouped distributions
    // Group-by helpers with filters
    async fn group_by_with_filters(
        pool: &sqlx::SqlitePool,
        column: &str,
        where_sql: &str,
        where_params: &Vec<String>,
        limit: Option<i64>,
    ) -> Result<Vec<(Option<String>, i64)>, sqlx::Error> {
        let mut sql = format!(
            "SELECT {col} AS k, COUNT(*) AS c FROM product_details {where_sql} GROUP BY {col} ORDER BY c DESC",
            col = column,
            where_sql = where_sql
        );
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }
        let q = sqlx::query(&sql);
    let rows = bind_all(q, where_params).fetch_all(pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let v: Option<String> = r.try_get::<Option<String>, _>("k").ok().flatten();
                (v, r.get::<i64, _>("c"))
            })
            .collect())
    }

    async fn group_by_i32_with_filters(
        pool: &sqlx::SqlitePool,
        column: &str,
        where_sql: &str,
        where_params: &Vec<String>,
        limit: Option<i64>,
    ) -> Result<Vec<(Option<i64>, i64)>, sqlx::Error> {
        let mut sql = format!(
            "SELECT {col} AS k, COUNT(*) AS c FROM product_details {where_sql} GROUP BY {col} ORDER BY c DESC",
            col = column,
            where_sql = where_sql
        );
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }
        let q = sqlx::query(&sql);
    let rows = bind_all(q, where_params).fetch_all(pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let v: Option<i64> = r.try_get::<Option<i64>, _>("k").ok().flatten();
                (v, r.get::<i64, _>("c"))
            })
            .collect())
    }

    let manufacturers = group_by_with_filters(&pool, "manufacturer", &where_sql, &where_params, Some(100)).await.unwrap_or_default();
    let device_types = group_by_with_filters(&pool, "device_type", &where_sql, &where_params, Some(100)).await.unwrap_or_default();
    let spec_versions_dist = group_by_with_filters(&pool, "specification_version", &where_sql, &where_params, None).await.unwrap_or_default();
    let transport_interfaces_dist = group_by_with_filters(&pool, "transport_interface", &where_sql, &where_params, None).await.unwrap_or_default();
    let tis_trp_tested = group_by_with_filters(&pool, "tis_trp_tested", &where_sql, &where_params, None).await.unwrap_or_default();
    let vids = group_by_i32_with_filters(&pool, "vid", &where_sql, &where_params, Some(50)).await.unwrap_or_default();

    let resp = json!({
        "total_details": total_details,
        "distributions": {
            "manufacturers": manufacturers.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>(),
            "device_types": device_types.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>(),
            "spec_versions": spec_versions_dist.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>(),
            "transport_interfaces": transport_interfaces_dist.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>(),
            "tis_trp_tested": tis_trp_tested.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>(),
            "vids": vids.into_iter().map(|(k,c)| json!({"key": k, "count": c})).collect::<Vec<_>>()
        },
        "time_series": {
            "cert_by_date": cert_by_date,
            "created_daily": created_daily,
            "updated_daily": updated_daily
        },
        "page_coverage": {
            "by_page": page_distribution,
            "index_heatmap": index_heatmap
        },
        "completeness": {
            "manufacturer_filled": manufacturer_filled,
            "model_filled": model_filled,
            "certificate_id_filled": certificate_id_filled,
            "device_type_filled": device_type_filled,
            "certification_date_filled": certification_date_filled,
            "software_version_filled": software_version_filled,
            "hardware_version_filled": hardware_version_filled,
            "spec_version_filled": spec_version_filled
        }
    });

    info!("✅ product_details analytics generated: total={} entries", total_details);
    Ok(resp)
}
