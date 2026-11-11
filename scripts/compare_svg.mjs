#!/usr/bin/env node
// Side-by-side quality check of this repo's Rust Vega renderer against
// upstream Vega 6, using the compiled specs in tests/fixtures/examples/compiled.
//
// Usage:
//   node scripts/compare_svg.mjs [--limit <n>] [--name <substring>]
//
// Upstream SVG references are written to /tmp/opencode/ref_svg/ and a machine
// readable report to /tmp/opencode/svg_compare_report.json.

import {existsSync} from 'node:fs';
import {execFile} from 'node:child_process';
import {mkdir, readFile, readdir, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, '..');
const FIXTURES_DIR = path.join(REPO_ROOT, 'tests/fixtures/examples/compiled');
const REF_SVG_DIR = '/tmp/opencode/ref_svg';
const REPORT_PATH = '/tmp/opencode/svg_compare_report.json';
const LOCAL_DATASETS = path.join(REPO_ROOT, 'node_modules/vega-datasets');
const TMP_DATASETS = '/tmp/opencode/node_modules/vega-datasets';
const COUNT_TAGS = ['path', 'rect', 'line', 'text', 'g'];

function usage() {
  return `Usage: node scripts/compare_svg.mjs [options]

Options:
  --limit <n>       Compare at most n fixtures
  --name <text>     Only compare fixtures whose filename contains <text>
  -h, --help        Show this help`;
}

function parseArgs(argv) {
  const opts = {limit: Infinity, name: null, help: false};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '-h' || arg === '--help') {
      opts.help = true;
    } else if (arg === '--limit') {
      const value = argv[++i];
      if (value === undefined || !/^\d+$/.test(value) || Number(value) < 1) {
        throw new Error('--limit requires a positive integer');
      }
      opts.limit = Number(value);
    } else if (arg.startsWith('--limit=')) {
      const value = arg.slice('--limit='.length);
      if (!/^\d+$/.test(value) || Number(value) < 1) {
        throw new Error('--limit requires a positive integer');
      }
      opts.limit = Number(value);
    } else if (arg === '--name') {
      const value = argv[++i];
      if (value === undefined || value === '') {
        throw new Error('--name requires a non-empty substring');
      }
      opts.name = value;
    } else if (arg.startsWith('--name=')) {
      const value = arg.slice('--name='.length);
      if (value === '') throw new Error('--name requires a non-empty substring');
      opts.name = value;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return opts;
}

async function loadVega() {
  const candidates = [
    '/tmp/opencode/node_modules/vega/index.js',
    path.join(REPO_ROOT, 'node_modules/vega/index.js'),
    'vega'
  ];
  const errors = [];
  for (const candidate of candidates) {
    if (candidate !== 'vega' && !existsSync(candidate)) continue;
    try {
      const href = candidate === 'vega' ? candidate : pathToFileURL(candidate).href;
      const vega = await import(href);
      return {vega, from: candidate};
    } catch (err) {
      errors.push(`${candidate}: ${errorMessage(err)}`);
    }
  }
  throw new Error(`Unable to load upstream vega (${errors.join('; ') || 'not found'})`);
}

function datasetRoot() {
  if (existsSync(path.join(LOCAL_DATASETS, 'data'))) return LOCAL_DATASETS;
  if (existsSync(path.join(TMP_DATASETS, 'data'))) return TMP_DATASETS;
  return null;
}

// Deep-clone a spec and turn relative vega-datasets URLs (data/foo.json) into
// absolute filesystem paths so the Node loader can read them.
function prepareSpec(spec, datasets) {
  const copy = JSON.parse(JSON.stringify(spec));
  if (!datasets) return copy;

  const visit = (node) => {
    if (Array.isArray(node)) {
      for (const item of node) visit(item);
      return;
    }
    if (!node || typeof node !== 'object') return;
    for (const [key, value] of Object.entries(node)) {
      if (key === 'url' && typeof value === 'string' && value.startsWith('data/')) {
        node[key] = path.join(datasets, value);
      } else {
        visit(value);
      }
    }
  };

  visit(copy);
  return copy;
}

async function renderUpstream(vega, spec, datasets) {
  const view = new vega.View(vega.parse(prepareSpec(spec, datasets)), {renderer: 'none'});
  try {
    await view.runAsync();
    return await view.toSVG();
  } finally {
    view.finalize();
  }
}

async function probeRust() {
  const binary = path.join(REPO_ROOT, 'target/debug/vega-svg');
  const args = ['build', '-q', '-p', 'vega-svg', '--bin', 'vega-svg'];
  try {
    // A build also covers a missing binary whose source has been added later.
    await execFileAsync('cargo', args, {
      cwd: REPO_ROOT,
      timeout: 180_000,
      maxBuffer: 16 * 1024 * 1024
    });
    return {ok: true, binary};
  } catch (err) {
    return {
      ok: false,
      reason: `cargo build failed: ${firstLine(err.stderr || err.message)}`
    };
  }
}

async function renderRust(fixtureRel, datasets) {
  const args = [
    'run', '-q', '-p', 'vega-svg', '--bin', 'vega-svg', '--',
    '--data-dir', path.relative(REPO_ROOT, datasets),
    fixtureRel
  ];
  try {
    const {stdout} = await execFileAsync('cargo', args, {
      cwd: REPO_ROOT,
      timeout: 180_000,
      maxBuffer: 64 * 1024 * 1024
    });
    return {ok: true, svg: stdout};
  } catch (err) {
    return {
      ok: false,
      reason: firstLine(err.stderr || err.message)
    };
  }
}

function firstLine(value) {
  const text = String(value ?? '').trim();
  return text.split(/\r?\n/).find((line) => line.trim() !== '')?.trim() || 'unknown error';
}

function errorMessage(err) {
  return firstLine(err && err.stack ? err.stack : err);
}

function attr(tag, name) {
  const re = new RegExp(`\\b${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`, 'i');
  const match = tag.match(re);
  if (!match) return null;
  return decodeEntities(match[1] ?? match[2] ?? '');
}

function decodeEntities(text) {
  return text
    .replace(/&#x([0-9a-f]+);/gi, (_, hex) => String.fromCodePoint(parseInt(hex, 16)))
    .replace(/&#(\d+);/g, (_, dec) => String.fromCodePoint(Number(dec)))
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
}

function normalizeSpace(text) {
  return text.replace(/\s+/g, ' ').trim();
}

function normalizeDimension(value) {
  if (value == null) return null;
  const trimmed = value.trim().replace(/px$/i, '');
  const number = Number(trimmed);
  return Number.isFinite(number) ? String(number) : trimmed;
}

function normalizeViewBox(value) {
  if (value == null) return null;
  return normalizeSpace(value);
}

function parseSvg(svg) {
  const rootMatch = svg.match(/<svg\b[^>]*>/i);
  if (!rootMatch) throw new Error('no <svg> root element');

  const root = rootMatch[0];
  const dims = {
    width: normalizeDimension(attr(root, 'width')),
    height: normalizeDimension(attr(root, 'height')),
    viewBox: normalizeViewBox(attr(root, 'viewBox'))
  };

  const counts = {};
  for (const tag of COUNT_TAGS) {
    counts[tag] = (svg.match(new RegExp(`<${tag}\\b`, 'g')) || []).length;
  }

  const text = [];
  const textRe = /<text\b[^>]*>([\s\S]*?)<\/text>/gi;
  for (const match of svg.matchAll(textRe)) {
    const content = normalizeSpace(decodeEntities(match[1].replace(/<[^>]*>/g, ' ')));
    if (content !== '') text.push(content);
  }

  const fills = new Set();
  for (const match of svg.matchAll(/\bfill\s*=\s*(?:"([^"]*)"|'([^']*)')/gi)) {
    addFill(fills, match[1] ?? match[2]);
  }
  for (const match of svg.matchAll(/\bfill\s*:\s*([^;"']+)/gi)) {
    addFill(fills, match[1]);
  }

  return {
    dims,
    counts,
    text,
    sortedText: [...text].sort(),
    fills: [...fills].sort(),
    markupLength: svg.length
  };
}

function addFill(fills, raw) {
  if (raw == null) return;
  const value = raw.trim().toLowerCase();
  if (value === '' || value === 'none' || value === 'transparent') return;
  fills.add(value);
}

function sameDims(a, b) {
  return a.width === b.width && a.height === b.height && a.viewBox === b.viewBox;
}

function sameSorted(a, b) {
  return a.length === b.length && a.every((value, i) => value === b[i]);
}

function dimLabel(dims) {
  const size = `${dims.width ?? '?'}x${dims.height ?? '?'}`;
  return dims.viewBox == null ? size : `${size} [${dims.viewBox}]`;
}

function countLabel(counts) {
  return COUNT_TAGS.map((tag) => `${tag} ${counts[tag]}`).join(' ');
}

function compare(up, rust) {
  const diffs = [];

  if (!sameDims(up.dims, rust.dims)) {
    const keys = ['width', 'height', 'viewBox'].filter((key) => up.dims[key] !== rust.dims[key]);
    diffs.push(
      `dimensions (${keys.map((key) => `${key} ${up.dims[key] ?? 'null'} vs ${rust.dims[key] ?? 'null'}`).join(', ')})`
    );
  }

  for (const tag of COUNT_TAGS) {
    if (up.counts[tag] !== rust.counts[tag]) {
      diffs.push(`<${tag}> count ${up.counts[tag]} vs ${rust.counts[tag]}`);
    }
  }

  if (!sameSorted(up.sortedText, rust.sortedText)) {
    const upSet = new Set(up.sortedText);
    const rustSet = new Set(rust.sortedText);
    const missing = up.sortedText.filter((item) => !rustSet.has(item));
    const extra = rust.sortedText.filter((item) => !upSet.has(item));
    const detail = [
      missing.length ? `missing "${truncate(missing[0])}"` : '',
      extra.length ? `extra "${truncate(extra[0])}"` : ''
    ].filter(Boolean).join(', ');
    diffs.push(`text content (${up.text.length} vs ${rust.text.length}${detail ? `; ${detail}` : ''})`);
  }

  if (!sameSorted(up.fills, rust.fills)) {
    const upSet = new Set(up.fills);
    const rustSet = new Set(rust.fills);
    const missing = up.fills.filter((item) => !rustSet.has(item));
    const extra = rust.fills.filter((item) => !upSet.has(item));
    const detail = [
      missing.length ? `missing ${truncate(missing.join(', '))}` : '',
      extra.length ? `extra ${truncate(extra.join(', '))}` : ''
    ].filter(Boolean).join('; ');
    diffs.push(`fill colors${detail ? ` (${detail})` : ''}`);
  }

  return diffs;
}

function truncate(text, max = 50) {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}

function oneLine(text) {
  return String(text ?? '').replace(/\s*\r?\n\s*/g, ' ').trim();
}

async function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (err) {
    console.error(`compare_svg: ${errorMessage(err)}`);
    console.error(usage());
    process.exitCode = 2;
    return;
  }

  if (opts.help) {
    console.log(usage());
    return;
  }

  await mkdir(REF_SVG_DIR, {recursive: true});

  let entries;
  try {
    entries = (await readdir(FIXTURES_DIR))
      .filter((name) => name.endsWith('.vg.json'))
      .filter((name) => !opts.name || name.includes(opts.name))
      .sort();
  } catch (err) {
    console.error(`compare_svg: cannot read ${FIXTURES_DIR}: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }

  const selected = entries.slice(0, opts.limit);
  if (selected.length === 0) {
    console.error('compare_svg: no fixtures matched the filters');
    process.exitCode = 1;
    return;
  }

  let vegaInfo;
  try {
    vegaInfo = await loadVega();
  } catch (err) {
    console.error(`compare_svg: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }

  const datasets = datasetRoot();
  if (!datasets) {
    console.warn('compare_svg: warning: node_modules/vega-datasets not found; relative data URLs may fail');
  }

  const rust = await probeRust();
  if (!rust.ok) {
    console.warn(`compare_svg: Rust renderer unavailable (${rust.reason}); comparing upstream only`);
  }

  const results = [];
  let upstreamRendered = 0;
  let rustRendered = 0;

  for (const filename of selected) {
    const base = filename.slice(0, -'.vg.json'.length);
    const absFile = path.join(FIXTURES_DIR, filename);
    const relFile = path.relative(REPO_ROOT, absFile);
    const result = {
      name: base,
      file: relFile,
      status: 'error',
      upstream: null,
      rust: null,
      diffs: [],
      firstDifference: null,
      error: null
    };

    let upSvg = null;
    try {
      const spec = JSON.parse(await readFile(absFile, 'utf8'));
      upSvg = await renderUpstream(vegaInfo.vega, spec, datasets);
      await writeFile(path.join(REF_SVG_DIR, `${base}.svg`), upSvg);
      result.upstream = parseSvg(upSvg);
      upstreamRendered++;
    } catch (err) {
      result.error = `upstream failed: ${errorMessage(err)}`;
      result.firstDifference = result.error;
    }

    if (upSvg != null && rust.ok) {
      const rendered = await renderRust(relFile, datasets);
      if (rendered.ok) {
        try {
          result.rust = parseSvg(rendered.svg);
          rustRendered++;
        } catch (err) {
          result.error = `rust SVG parse failed: ${errorMessage(err)}`;
        }
      } else {
        result.error = `rust failed: ${rendered.reason}`;
      }
    } else if (upSvg != null && !rust.ok) {
      result.error = `rust unavailable: ${rust.reason}`;
    }

    if (result.upstream && result.rust) {
      result.diffs = compare(result.upstream, result.rust);
      result.status = result.diffs.length === 0 ? 'pass' : 'fail';
      result.firstDifference = result.diffs[0] || null;
    } else if (result.status !== 'error') {
      result.status = 'error';
    }

    if (!result.firstDifference && result.error) result.firstDifference = result.error;
    results.push(result);

    const up = result.upstream;
    const rs = result.rust;
    const dims = up
      ? `dims up=${dimLabel(up.dims)}${rs ? ` rust=${dimLabel(rs.dims)}` : ''}`
      : 'dims up=?';
    const counts = up
      ? `counts${rs ? ' up/rust' : ''} ${COUNT_TAGS.map((tag) => `${tag} ${up.counts[tag]}${rs ? `/${rs.counts[tag]}` : ''}`).join(' ')}`
      : 'counts ?';
    const textInfo = up && rs ? `text ${up.text.length}/${rs.text.length}` : up ? `text ${up.text.length}` : 'text ?';
    const fillInfo = up && rs ? `fills ${up.fills.length}/${rs.fills.length}` : up ? `fills ${up.fills.length}` : 'fills ?';
    const detail = result.status === 'pass'
      ? ''
      : ` | first diff: ${oneLine(result.firstDifference || result.error || 'unknown')}`;
    console.log(
      `[${result.status.toUpperCase()}] ${filename} | ${dims} | ${counts} | ${textInfo} | ${fillInfo}${detail}`
    );
  }

  const compared = results.filter((result) => result.status !== 'error');
  const dimensions = compared.filter((result) => sameDims(result.upstream.dims, result.rust.dims));
  const texts = compared.filter((result) => sameSorted(result.upstream.sortedText, result.rust.sortedText));
  const counts = compared.filter((result) =>
    COUNT_TAGS.every((tag) => result.upstream.counts[tag] === result.rust.counts[tag])
  );
  const fills = compared.filter((result) => sameSorted(result.upstream.fills, result.rust.fills));
  const passed = results.filter((result) => result.status === 'pass');

  const summary = {
    total: selected.length,
    fixturesAvailable: entries.length,
    upstreamRendered,
    rustRendered,
    compared: compared.length,
    passed: passed.length,
    failed: results.filter((result) => result.status === 'fail').length,
    errors: results.filter((result) => result.status === 'error').length,
    dimensionMatches: dimensions.length,
    countMatches: counts.length,
    textMatches: texts.length,
    fillMatches: fills.length
  };

  console.log('');
  console.log(
    `Summary: ${summary.compared}/${summary.total} rendered (upstream ${summary.upstreamRendered}, rust ${summary.rustRendered}), ` +
    `${summary.dimensionMatches} dimension matches, ${summary.textMatches} text matches, ` +
    `${summary.countMatches} count matches, ${summary.fillMatches} fill matches, ` +
    `${summary.passed} passed, ${summary.failed} failed, ${summary.errors} errors`
  );
  if (datasets) console.log(`Datasets: ${datasets}`);
  console.log(`Upstream vega: ${vegaInfo.from}`);
  console.log(`Reference SVGs: ${REF_SVG_DIR}`);

  const report = {
    generatedAt: new Date().toISOString(),
    repoRoot: REPO_ROOT,
    fixturesDir: FIXTURES_DIR,
    refSvgDir: REF_SVG_DIR,
    datasets,
    upstreamVega: vegaInfo.from,
    options: {
      limit: Number.isFinite(opts.limit) ? opts.limit : null,
      name: opts.name
    },
    rustAvailable: rust.ok,
    rustError: rust.ok ? null : rust.reason,
    summary,
    results
  };

  await writeFile(REPORT_PATH, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Report: ${REPORT_PATH}`);

  if (summary.errors > 0) process.exitCode = 1;
}

main().catch((err) => {
  console.error(`compare_svg: unexpected failure: ${errorMessage(err)}`);
  process.exitCode = 1;
});
