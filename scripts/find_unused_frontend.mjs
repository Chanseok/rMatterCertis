#!/usr/bin/env node
/**
 * Simple unused frontend code detector for TS/TSX/JS/JSX without extra deps.
 * - Builds a shallow module graph from import/export statements.
 * - Resolves relative (./, ../) and alias (@/) paths to filesystem.
 * - Starts from entrypoints (src/index.tsx by default, plus optional others) and marks reachable files.
 * - Reports files under src/ with code extensions not reachable.
 *
 * Limitations:
 * - Regex-based import extraction; may miss rare patterns.
 * - Does not evaluate conditional imports; errs on the conservative side.
 * - Does not report unused exports within used files (use ts-prune/knip later for that).
 */

import fs from 'fs';
import path from 'path';
import url from 'url';

const repoRoot = process.cwd();
const SRC_DIR = path.join(repoRoot, 'src');
const CODE_EXTS = ['.ts', '.tsx', '.js', '.jsx'];
const INDEX_BASENAMES = ['index'];
const EXCLUDE_DIRS = new Set([
  'node_modules', '.git', '_archive',
]);

// Additional excludes that are legacy according to tsconfig exclude (best effort)
const EXCLUDE_GLOBS = [
  'src/components/visualization/',
  'src/components/dashboard/',
  'src/components/charts/',
  'src/components/visualizations/',
  'src/components/actor-system/',
  'src/components/CrawlingProcessDashboard.tsx',
  'src/components/HierarchicalEventMonitor.tsx',
  'src/components/dashboard/DomainStatusDashboard.tsx',
  'src/components/dashboard/DomainStatusDashboardDeprecated.tsx',
  'src/_archive/',
  'src/types/src/types/generated/',
];

function isCodeFile(p) {
  return CODE_EXTS.includes(path.extname(p));
}

function readFileSafe(p) {
  try { return fs.readFileSync(p, 'utf8'); } catch { return null; }
}

function listAllCodeFiles(dir) {
  const out = [];
  function walk(d) {
    const ents = fs.readdirSync(d, { withFileTypes: true });
    for (const e of ents) {
      const full = path.join(d, e.name);
      const rel = path.relative(repoRoot, full).replaceAll('\\', '/');
      if (e.isDirectory()) {
        if (EXCLUDE_DIRS.has(e.name)) continue;
        // exclude known globs
        if (EXCLUDE_GLOBS.some(g => rel.startsWith(g))) continue;
        walk(full);
      } else {
        if (isCodeFile(full)) {
          // respect exclude globs
          if (EXCLUDE_GLOBS.some(g => rel.startsWith(g))) continue;
          out.push(full);
        }
      }
    }
  }
  walk(dir);
  return out;
}

const importRe = /(?:import\s+[^'"();]+\s+from\s+['"]([^'";]+)['"])|(?:import\(\s*['"]([^'";]+)['"]\s*\))|(?:export\s+\*\s+from\s+['"]([^'";]+)['"])|(?:export\s+\{[^}]*\}\s+from\s+['"]([^'";]+)['"])/g;

function extractSpecifiers(srcText) {
  const specs = new Set();
  if (!srcText) return specs;
  let m;
  while ((m = importRe.exec(srcText)) !== null) {
    const s = m[1] || m[2] || m[3] || m[4];
    if (!s) continue;
    specs.add(s);
  }
  return specs;
}

function resolveSpecifier(fromFile, spec) {
  if (!spec) return null;
  if (spec.startsWith('@/' )) {
    const p = path.join(SRC_DIR, spec.slice(2));
    return resolveWithExtensions(p);
  }
  if (spec.startsWith('./') || spec.startsWith('../')) {
    const base = path.resolve(path.dirname(fromFile), spec);
    return resolveWithExtensions(base);
  }
  // bare specifier -> external dep; ignore
  return null;
}

function resolveWithExtensions(basePath) {
  // try exact with extension
  if (fs.existsSync(basePath) && fs.statSync(basePath).isFile()) return basePath;
  for (const ext of CODE_EXTS) {
    const p = basePath + ext;
    if (fs.existsSync(p) && fs.statSync(p).isFile()) return p;
  }
  // try index files if base is a dir
  if (fs.existsSync(basePath) && fs.statSync(basePath).isDirectory()) {
    for (const name of INDEX_BASENAMES) {
      for (const ext of CODE_EXTS) {
        const p = path.join(basePath, name + ext);
        if (fs.existsSync(p) && fs.statSync(p).isFile()) return p;
      }
    }
  }
  return null;
}

function buildGraph(files) {
  const graph = new Map(); // file -> Set<depFile>
  for (const f of files) {
    const txt = readFileSafe(f);
    const specs = extractSpecifiers(txt);
    const deps = new Set();
    for (const s of specs) {
      const r = resolveSpecifier(f, s);
      if (r && files.includes(r)) deps.add(r);
    }
    graph.set(f, deps);
  }
  return graph;
}

function reachableFrom(entryFiles, graph) {
  const vis = new Set();
  const q = [...entryFiles.filter(f => f && graph.has(f))];
  while (q.length) {
    const f = q.pop();
    if (vis.has(f)) continue;
    vis.add(f);
    const deps = graph.get(f) || new Set();
    for (const d of deps) {
      if (!vis.has(d)) q.push(d);
    }
  }
  return vis;
}

function findEntrypoints() {
  const candidates = [
    path.join(SRC_DIR, 'index.tsx'),
    path.join(SRC_DIR, 'index.ts'),
    path.join(SRC_DIR, 'main.tsx'),
    path.join(SRC_DIR, 'main.ts'),
  ];
  return candidates.filter(f => fs.existsSync(f));
}

function main() {
  if (!fs.existsSync(SRC_DIR)) {
    console.error('[error] src directory not found');
    process.exit(2);
  }
  const files = listAllCodeFiles(SRC_DIR);
  const graph = buildGraph(files);
  const entry = findEntrypoints();
  const vis = reachableFrom(entry, graph);
  const unused = files.filter(f => !vis.has(f));

  console.log(`[info] Files scanned: ${files.length}`);
  console.log(`[info] Entrypoints: ${entry.map(e => path.relative(repoRoot, e)).join(', ')}`);
  if (unused.length === 0) {
    console.log('[result] No unreachable files found.');
    return;
  }
  console.log('[result] Unreachable (possibly unused) files:');
  for (const f of unused) {
    console.log(' - ' + path.relative(repoRoot, f));
  }
}

main();
