#!/usr/bin/env node
/**
 * Diagnose a canonical page_id by fetching contributing physical pages
 * and mapping each extracted URL to (page_id, index_in_page).
 *
 * Usage:
 *   node scripts/diagnose_canonical_page.mjs <page_id>
 *
 * Output includes: total_pages, items_on_last_page, contributing physical pages,
 * present indices, missing indices, and the URLs per index.
 */

const BASE = 'https://csa-iot.org';
const NEWEST_URL = 'https://csa-iot.org/csa-iot_products/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver';
const PAGINATED_TMPL = 'https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver';
const PER_PAGE = 12;

function toAbsolute(url) {
  if (!url) return url;
  if (url.startsWith('http://') || url.startsWith('https://')) return url;
  if (url.startsWith('//')) return 'https:' + url;
  if (url.startsWith('/')) return BASE + url;
  return BASE + '/' + url.replace(/^\.?\/?/, '');
}

function extractProductUrls(html) {
  const hrefRe = /<a\s+[^>]*href=["']([^"']+)["'][^>]*>/gi;
  const seen = new Set();
  const urls = [];
  let m;
  while ((m = hrefRe.exec(html)) !== null) {
    const raw = m[1];
    const abs = toAbsolute(raw);
    if (!abs) continue;
    if (abs.includes('/csa_product/') && !abs.includes('/csa-iot_products/')) {
      if (!seen.has(abs)) {
        seen.add(abs);
        urls.push(abs);
      }
    }
  }
  return urls;
}

function extractTotalPages(html) {
  // Look for /page/NNN/
  const re = /\/page\/(\d+)\//g;
  let max = 1;
  let m;
  while ((m = re.exec(html)) !== null) {
    const n = parseInt(m[1], 10);
    if (Number.isFinite(n)) max = Math.max(max, n);
  }
  // Fallback via page= param if present (not typical for CSA)
  const re2 = /[?&]page=(\d+)/g;
  while ((m = re2.exec(html)) !== null) {
    const n = parseInt(m[1], 10);
    if (Number.isFinite(n)) max = Math.max(max, n);
  }
  return max;
}

async function fetchText(url) {
  const res = await fetch(url, {
    headers: {
      'user-agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0 Safari/537.36',
      'accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
      'accept-language': 'en-US,en;q=0.9'
    }
  });
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  return await res.text();
}

function mapToCanonical(total_pages, items_on_last_page, physical_page, index_in_physical) {
  // Mirror app logic
  const total_products = (total_pages - 1) * PER_PAGE + items_on_last_page;
  const index_from_newest = (physical_page - 1) * PER_PAGE + index_in_physical;
  const index_from_oldest = (total_products - 1) - index_from_newest;
  const page_id = Math.floor(index_from_oldest / PER_PAGE);
  const index_in_page = index_from_oldest % PER_PAGE;
  return { page_id, index_in_page };
}

async function run(targetPageId) {
  // 1) Discover meta
  const newestHtml = await fetchText(NEWEST_URL);
  const total_pages = extractTotalPages(newestHtml) || 1;
  const oldestUrl = PAGINATED_TMPL.replace('{}', String(total_pages));
  const oldestHtml = await fetchText(oldestUrl);
  const items_on_last_page = extractProductUrls(oldestHtml).length;

  // 2) Derive contributing physical pages
  const phys = total_pages - targetPageId; // "current physical page number" heuristic from app
  const candidates = [];
  if (phys >= 1) candidates.push(phys);
  if (phys > 1) candidates.push(phys - 1); // newer neighbor

  // 3) Fetch candidates and map
  const present = new Map(); // index_in_page -> url
  for (const p of candidates) {
    const url = PAGINATED_TMPL.replace('{}', String(p));
    try {
      const html = await fetchText(url);
      const urls = extractProductUrls(html);
      urls.forEach((u, i) => {
        const { page_id, index_in_page } = mapToCanonical(total_pages, items_on_last_page, p, i);
        if (page_id === targetPageId) {
          if (!present.has(index_in_page)) present.set(index_in_page, u);
        }
      });
      await new Promise((r) => setTimeout(r, 200));
    } catch (e) {
      console.error(`Fetch failed for page ${p}: ${e.message}`);
    }
  }

  // 4) Report
  const indices = Array.from(present.keys()).sort((a, b) => a - b);
  const missing = [];
  for (let i = 0; i < PER_PAGE; i++) if (!present.has(i)) missing.push(i);

  console.log(`meta: total_pages=${total_pages} items_on_last_page=${items_on_last_page}`);
  console.log(`target page_id=${targetPageId} => physical=${phys} (candidates: ${candidates.join(',')})`);
  console.log(`present count=${indices.length}, indices=[${indices.join(',')}]`);
  console.log(`missing count=${missing.length}, indices=[${missing.join(',')}]`);
  indices.forEach((i) => console.log(`  idx ${String(i).padStart(2,' ')} -> ${present.get(i)}`));
}

const arg = process.argv[2];
if (!arg) {
  console.error('Usage: node scripts/diagnose_canonical_page.mjs <page_id>');
  process.exit(1);
}
const pageId = parseInt(arg, 10);
if (!Number.isFinite(pageId)) {
  console.error('Invalid page_id');
  process.exit(1);
}
run(pageId).catch((e) => {
  console.error(e);
  process.exit(1);
});
