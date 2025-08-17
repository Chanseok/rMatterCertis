use matter_certis_v2_lib as lib; // ensure lib is built
use reqwest::Client;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: check_app_extractor <page> [<page> ...]");
        std::process::exit(1);
    }

    // Build HTTP client similar to app defaults
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0 Safari/537.36")
        .timeout(Duration::from_secs(20))
        .build()?;

    let extractor = lib::infrastructure::html_parser::MatterDataExtractor::new()?;

    // Discover site meta (total_pages and items_on_last_page) similar to app flow
    let newest_url = lib::infrastructure::config::csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string();
    let newest_html = client.get(&newest_url).send().await?.text().await?;
    let total_pages = extractor.extract_total_pages(&newest_html).unwrap_or(1).max(1);
    let oldest_page = total_pages;
    let oldest_url = lib::infrastructure::config::csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
        .replace("{}", &oldest_page.to_string());
    let oldest_html = client.get(&oldest_url).send().await?.text().await?;
    let items_on_last_page = extractor
        .extract_product_urls_from_content(&oldest_html)
        .unwrap_or_default()
        .len();
    let calc = lib::domain::pagination::CanonicalPageIdCalculator::new(
        total_pages,
        items_on_last_page,
    );
    println!(
        "[meta] total_pages={} items_on_last_page={}",
        total_pages, items_on_last_page
    );

    for a in args {
        let page: u32 = a.parse().unwrap_or(1);
        let url = lib::infrastructure::config::csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED
            .replace("{}", &page.to_string());
        let started = std::time::Instant::now();
        match client.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(html_str) => {
                    let urls = extractor
                        .extract_product_urls_from_content(&html_str)
                        .unwrap_or_default();
                    println!("\n[app extractor] Page {} => {} product URLs", page, urls.len());
                    for (i, u) in urls.iter().enumerate() {
                        let pos = calc.calculate(page, i);
                        println!(
                            "{:>2}. {}  => page_id={}, index_in_page={}",
                            i + 1,
                            u,
                            pos.page_id,
                            pos.index_in_page
                        );
                    }
                    println!("Fetched+parsed in {} ms: {}", started.elapsed().as_millis(), url);
                }
                Err(e) => {
                    eprintln!("\n[app extractor] Page {} read failed: {}", page, e);
                }
            },
            Err(e) => {
                eprintln!("\n[app extractor] Page {} HTTP failed: {}", page, e);
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Ok(())
}
