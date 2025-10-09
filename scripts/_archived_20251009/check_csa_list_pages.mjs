#!/usr/bin/env node
/**
 * Quick verification script to fetch CSA-IoT Matter listing pages and extract product detail URLs.
 * Usage: node scripts/check_csa_list_pages.mjs 327 326
 */

const BASE = 'https://csa-iot.org';
const PAGINATED_TMPL = 'https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver';

const sleep = (ms) => new Promise((res) => setTimeout(res, ms));

function toAbsolute(url) {
  if (!url) return url;
  if (url.startsWith('http://') || url.startsWith('https://')) return url;
  if (url.startsWith('//')) return 'https:' + url;
  if (url.startsWith('/')) return BASE + url;
  return BASE + '/' + url.replace(/^\.?\/?/, '');
}

function extractProductUrls(html) {
  // Simple anchor href extraction without DOM deps.
  // Strategy: collect all href values, then filter to product detail pages only.
  const hrefRe = /<a\s+[^>]*href=["']([^"']+)["'][^>]*>/gi;
  const seen = new Set();
  const urls = [];
  let m;
  while ((m = hrefRe.exec(html)) !== null) {
    const raw = m[1];
    const abs = toAbsolute(raw);
    if (!abs) continue;
    // Only keep product detail pages; avoid listing pages.
    if (abs.includes('/csa_product/') && !abs.includes('/csa-iot_products/')) {
      if (!seen.has(abs)) {
        seen.add(abs);
        urls.push(abs);
      }
    }
  }
  return urls;
}

async function fetchPage(url) {
  const res = await fetch(url, {
    headers: {
      'user-agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0 Safari/537.36',
      'accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
      'accept-language': 'en-US,en;q=0.9'
    },
  });
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  return await res.text();
}

async function run(pages) {
  if (!pages.length) {
    console.error('Usage: node scripts/check_csa_list_pages.mjs <page> [<page> ...]');
    process.exit(1);
  }
  for (const p of pages) {
    const url = PAGINATED_TMPL.replace('{}', String(p));
    const started = Date.now();
    try {
      const html = await fetchPage(url);
      const urls = extractProductUrls(html);
      console.log(`\nPage ${p} => ${urls.length} product URLs`);
      urls.forEach((u, i) => console.log(`${String(i + 1).padStart(2, ' ')}. ${u}`));
      console.log(`Fetched in ${Date.now() - started} ms: ${url}`);
    } catch (e) {
      console.error(`\nPage ${p} failed: ${e.message}`);
    }
    await sleep(300);
  }
}

const argPages = process.argv.slice(2).map((s) => parseInt(s, 10)).filter((n) => Number.isFinite(n));
run(argPages);
