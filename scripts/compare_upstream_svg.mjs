#!/usr/bin/env node
// Strict SVG comparison against the upstream Vega-Lite example corpus.
//
// The upstream reference SVG is rendered by Vega (JS) from the *compiled*
// `examples/compiled/<name>.vg.json`, while the Rust renderer consumes the
// source `examples/specs/<name>.vl.json` (its own in-memory compiler). This
// keeps renderer bugs separate from compiler bugs as much as possible; use
// `--diagnose-compiler` to also render the Rust renderer from the compiled
// Vega spec and report compiler divergence separately.
//
// Usage:
//   node scripts/compare_upstream_svg.mjs [--name <substr>] [--limit <n>]
//        [--strict-geometry] [--diagnose-compiler] [--json <path>]
//
// Default corpus root: /tmp/opencode/vega-lite/examples (override with
// --examples <dir>). A machine-readable report is written to
// /tmp/opencode/upstream_svg_compare_report.json by default.

import {existsSync} from 'node:fs';
import {execFile} from 'node:child_process';
import {mkdir, readFile, readdir, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, '..');
const DEFAULT_EXAMPLES = '/tmp/opencode/vega-lite/examples';
const REF_SVG_DIR = '/tmp/opencode/upstream_ref_svg';
const REPORT_PATH = '/tmp/opencode/upstream_svg_compare_report.json';
const LOCAL_DATASETS = path.join(REPO_ROOT, 'node_modules/vega-datasets');
const TMP_DATASETS = '/tmp/opencode/node_modules/vega-datasets';
const COUNT_TAGS = ['path', 'rect', 'line', 'text', 'g'];
const GEOMETRY_TAGS = ['path', 'rect', 'line'];
const ARC_PRECISION = 3;

function usage() {
  return `Usage: node scripts/compare_upstream_svg.mjs [options]

Options:
  --name <text>          Only compare specs whose filename contains <text>
  --limit <n>            Compare at most n specs
  --examples <dir>       Vega-Lite checkout root (default ${DEFAULT_EXAMPLES})
  --strict-geometry      Include normalized path/rect/line geometry in pass/fail
  --diagnose-compiler    Also render the Rust renderer from compiled vg specs and
                         report compiler divergence separately
  --verbose              Print every unmatched geometry entry
  --tolerance <n>        Numeric geometry tolerance (default 0.0015)
  --json <file>          Report path (default ${REPORT_PATH})
  -h, --help             Show this help`;
}

function parseArgs(argv) {
  const opts = {
    limit: Infinity,
    name: null,
    examples: DEFAULT_EXAMPLES,
    strict: false,
    diagnoseCompiler: false,
    verbose: false,
    tolerance: 0.0015,
    json: REPORT_PATH,
    help: false
  };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const take = (flag) => {
      const value = argv[++i];
      if (value === undefined || value === '') throw new Error(`${flag} requires a value`);
      return value;
    };
    if (arg === '-h' || arg === '--help') opts.help = true;
    else if (arg === '--name') opts.name = take(arg);
    else if (arg.startsWith('--name=')) opts.name = arg.slice('--name='.length);
    else if (arg === '--limit') {
      const value = take(arg);
      if (!/^\d+$/.test(value) || Number(value) < 1) throw new Error('--limit requires a positive integer');
      opts.limit = Number(value);
    } else if (arg.startsWith('--limit=')) {
      const value = arg.slice('--limit='.length);
      if (!/^\d+$/.test(value) || Number(value) < 1) throw new Error('--limit requires a positive integer');
      opts.limit = Number(value);
    } else if (arg === '--examples') opts.examples = path.resolve(take(arg));
    else if (arg.startsWith('--examples=')) opts.examples = path.resolve(arg.slice('--examples='.length));
    else if (arg === '--strict-geometry') opts.strict = true;
    else if (arg === '--diagnose-compiler') opts.diagnoseCompiler = true;
    else if (arg === '--verbose') opts.verbose = true;
    else if (arg === '--tolerance') {
      const value = take(arg);
      const n = Number(value);
      if (!Number.isFinite(n) || n < 0) throw new Error('--tolerance requires a non-negative number');
      opts.tolerance = n;
    } else if (arg.startsWith('--tolerance=')) {
      const n = Number(arg.slice('--tolerance='.length));
      if (!Number.isFinite(n) || n < 0) throw new Error('--tolerance requires a non-negative number');
      opts.tolerance = n;
    }
    else if (arg === '--json') opts.json = path.resolve(take(arg));
    else if (arg.startsWith('--json=')) opts.json = path.resolve(arg.slice('--json='.length));
    else throw new Error(`Unknown argument: ${arg}`);
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
      return {vega: await import(href), from: candidate};
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

// Turn relative vega-datasets URLs (data/foo.json) into absolute paths.
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
  try {
    await execFileAsync('cargo', ['build', '-q', '-p', 'vega-svg', '--bin', 'vega-svg'], {
      cwd: REPO_ROOT,
      timeout: 300_000,
      maxBuffer: 16 * 1024 * 1024
    });
    return {ok: true, binary};
  } catch (err) {
    return {ok: false, reason: `cargo build failed: ${firstLine(err.stderr || err.message)}`};
  }
}

async function renderRust(file, datasets, vegaLite) {
  const args = [
    'run', '-q', '-p', 'vega-svg', '--bin', 'vega-svg', '--',
    '--data-dir', datasets ? path.relative(REPO_ROOT, datasets) : REPO_ROOT,
    ...(vegaLite ? ['--vega-lite'] : []),
    file
  ];
  try {
    const {stdout} = await execFileAsync('cargo', args, {
      cwd: REPO_ROOT,
      timeout: 180_000,
      maxBuffer: 64 * 1024 * 1024
    });
    return {ok: true, svg: stdout};
  } catch (err) {
    return {ok: false, reason: firstLine(err.stderr || err.message)};
  }
}

function firstLine(value) {
  const text = String(value ?? '').trim();
  return text.split(/\r?\n/).find((line) => line.trim() !== '')?.trim() || 'unknown error';
}

function errorMessage(err) {
  return firstLine(err && err.stack ? err.stack : err);
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

// ---------------------------------------------------------------------------
// Minimal SVG walker: tracks the transform matrix of every element so line,
// rect and text geometry can be compared in absolute coordinates.

const TAG_RE = /<(\/?)([a-zA-Z][\w:-]*)((?:"[^"]*"|'[^']*'|[^>"'])*)>|<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<[!?][^>]*>/g;

function attr(tag, name) {
  const re = new RegExp(`(?:^|\\s)${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`, 'i');
  const match = tag.match(re);
  if (!match) return null;
  return decodeEntities(match[1] ?? match[2] ?? '');
}

function parseAttrs(raw) {
  const attrs = {};
  const re = /([\w:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  for (const m of raw.matchAll(re)) attrs[m[1]] = decodeEntities(m[2] ?? m[3] ?? '');
  return attrs;
}

const IDENTITY = [1, 0, 0, 1, 0, 0];

function multiply(a, b) {
  return [
    a[0] * b[0] + a[2] * b[1],
    a[1] * b[0] + a[3] * b[1],
    a[0] * b[2] + a[2] * b[3],
    a[1] * b[2] + a[3] * b[3],
    a[0] * b[4] + a[2] * b[5] + a[4],
    a[1] * b[4] + a[3] * b[5] + a[5]
  ];
}

function parseTransform(text) {
  let m = IDENTITY;
  if (!text) return m;
  for (const match of text.matchAll(/([a-zA-Z]+)\s*\(([^)]*)\)/g)) {
    const nums = splitNumbers(match[2]).map(Number);
    const name = match[1].toLowerCase();
    let t = IDENTITY;
    if (name === 'translate') {
      t = [1, 0, 0, 1, nums[0] || 0, nums[1] || 0];
    } else if (name === 'scale') {
      t = [nums[0] ?? 1, 0, 0, nums[1] ?? nums[0] ?? 1, 0, 0];
    } else if (name === 'rotate') {
      const a = ((nums[0] || 0) * Math.PI) / 180;
      const cx = nums[1] || 0;
      const cy = nums[2] || 0;
      const cos = Math.cos(a);
      const sin = Math.sin(a);
      t = [cos, sin, -sin, cos, cx - cos * cx + sin * cy, cy - sin * cx - cos * cy];
    } else if (name === 'matrix' && nums.length >= 6) {
      t = nums.slice(0, 6);
    } else if (name === 'skewx') {
      t = [1, 0, Math.tan(((nums[0] || 0) * Math.PI) / 180), 1, 0, 0];
    } else if (name === 'skewy') {
      t = [1, Math.tan(((nums[0] || 0) * Math.PI) / 180), 0, 1, 0, 0];
    }
    m = multiply(m, t);
  }
  return m;
}

function applyMatrix(m, x, y) {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

function splitNumbers(text) {
  return String(text ?? '').match(/[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/g) || [];
}

function round(value, digits = ARC_PRECISION) {
  if (!Number.isFinite(value)) return 'NaN';
  const scale = 10 ** digits;
  const rounded = Math.round(value * scale) / scale;
  // Avoid "-0" and trailing .0
  return String(Object.is(rounded, -0) ? 0 : rounded);
}

// Canonical SVG path: commands + numbers rounded to `digits`. Acronyms are
// kept because the arc flags (large-arc, sweep) are part of the command args.
function normalizePath(d, digits = ARC_PRECISION) {
  const tokens = String(d ?? '').match(/[a-zA-Z]|[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/g) || [];
  const out = [];
  let command = '';
  for (const token of tokens) {
    if (/[a-zA-Z]/.test(token)) {
      command = token;
      out.push(token === 'z' ? 'Z' : token);
    } else {
      out.push(round(Number(token), digits));
    }
  }
  return {command, text: out.join(' '), tokens: out};
}

function arcFlags(normalized) {
  // Extract (rx,ry,rot,large,sweep,x,y) segments from absolute/relative arcs.
  const t = normalized.tokens;
  const flags = [];
  let command = '';
  let i = 0;
  while (i < t.length) {
    const token = t[i];
    if (/[a-zA-Z]/.test(token)) {
      command = token;
      i++;
      continue;
    }
    if (command === 'A' || command === 'a') {
      const args = t.slice(i, i + 7).map(Number);
      if (args.length === 7) {
        flags.push(`large=${args[3]} sweep=${args[4]} r=${args[0]},${args[1]}`);
      }
      i += 7;
      continue;
    }
    i++;
  }
  return flags.sort();
}

function parseSvg(svg) {
  const rootMatch = svg.match(/<svg\b[^>]*>/i);
  if (!rootMatch) throw new Error('no <svg> root element');
  const root = rootMatch[0];
  const dims = {
    width: normalizeDimension(attr(root, 'width')),
    height: normalizeDimension(attr(root, 'height')),
    viewBox: normalizeSpace(attr(root, 'viewBox') ?? '') || null
  };

  const counts = {};
  for (const tag of COUNT_TAGS) counts[tag] = (svg.match(new RegExp(`<${tag}\\b`, 'g')) || []).length;

  const paths = [];
  const rects = [];
  const lines = [];
  const texts = [];
  const arcs = [];
  const fills = new Set();

  for (const match of svg.matchAll(/\bfill\s*=\s*(?:"([^"]*)"|'([^']*)')/gi)) {
    addFill(fills, match[1] ?? match[2]);
  }
  for (const match of svg.matchAll(/\bfill\s*:\s*([^;"']+)/gi)) addFill(fills, match[1]);

  const stack = [];
  let matrix = IDENTITY;
  let textDepth = -1;
  let textBuffer = '';
  let textMatrix = IDENTITY;
  let textAttrs = null;
  let textTagDepth = -1;
  let depth = 0;

  const flushText = () => {
    if (textAttrs == null) return;
    const content = normalizeSpace(decodeEntities(textBuffer.replace(/<[^>]*>/g, ' ')));
    const x = Number(textAttrs.x ?? 0) + Number(textAttrs.dx ?? 0);
    const y = Number(textAttrs.y ?? 0) + Number(textAttrs.dy ?? 0);
    const pos = applyMatrix(textMatrix, x, y);
    texts.push({
      content,
      x: round(pos[0]),
      y: round(pos[1]),
      anchor: textAttrs['text-anchor'] || textAttrs.align || null,
      key: `${content}@${round(pos[0])},${round(pos[1])}`
    });
    textAttrs = null;
    textBuffer = '';
    textDepth = -1;
  };

  let lastIndex = 0;
  for (const match of svg.matchAll(TAG_RE)) {
    if (textAttrs != null && match.index > lastIndex) {
      textBuffer += svg.slice(lastIndex, match.index);
    }
    lastIndex = match.index + match[0].length;
    if (match[0].startsWith('<!--') || match[0].startsWith('<!') || match[0].startsWith('<?')) continue;
    const closing = match[1] === '/';
    const name = (match[2] || '').toLowerCase();
    const rawAttrs = match[3] || '';
    if (!name) continue;
    const selfClosing = /\/\s*$/.test(rawAttrs);
    const attrs = parseAttrs(rawAttrs);

    if (closing) {
      depth--;
      const entry = stack.pop();
      matrix = entry ? entry.matrix : IDENTITY;
      if (name === 'text' && textAttrs != null && depth === textTagDepth) flushText();
      continue;
    }

    const tagMatrix = multiply(matrix, parseTransform(attrs.transform));
    if (name === 'text') {
      if (textAttrs != null) flushText();
      textAttrs = attrs;
      textMatrix = tagMatrix;
      textBuffer = '';
      textTagDepth = depth;
    }

    if (name === 'path') {
      const normalized = normalizePath(attrs.d ?? '');
      const localArcs = arcFlags(normalized);
      const m = tagMatrix;
      const matrixKey = m.map((v) => round(v, 4)).join(',');
      // Degenerate paths (only zero-valued move/line commands) carry no
      // visual geometry; their translate-only difference is not meaningful.
      const degenerate = normalized.tokens.every((token) => /[a-zA-Z]/.test(token) || Number(token) === 0);
      paths.push({
        d: normalized.text,
        transform: matrixKey,
        arcs: localArcs,
        degenerate,
        key: degenerate ? 'empty' : `${normalized.text}|${matrixKey}`
      });
      if (!degenerate) for (const arc of localArcs) arcs.push(`${arc}|${matrixKey}`);
    } else if (name === 'rect') {
      const m = tagMatrix;
      const x = Number(attrs.x ?? 0);
      const y = Number(attrs.y ?? 0);
      const w = Number(attrs.width ?? 0);
      const h = Number(attrs.height ?? 0);
      const p1 = applyMatrix(m, x, y);
      const p2 = applyMatrix(m, x + w, y + h);
      const key = `${round(p1[0])},${round(p1[1])},${round(p2[0])},${round(p2[1])}`;
      rects.push({key});
    } else if (name === 'line') {
      const m = tagMatrix;
      const p1 = applyMatrix(m, Number(attrs.x1 ?? 0), Number(attrs.y1 ?? 0));
      const p2 = applyMatrix(m, Number(attrs.x2 ?? 0), Number(attrs.y2 ?? 0));
      const key = `${round(p1[0])},${round(p1[1])},${round(p2[0])},${round(p2[1])}`;
      lines.push({key});
    }

    if (!selfClosing) {
      stack.push({name, matrix});
      matrix = tagMatrix;
      depth++;
    } else if (name === 'text' && textAttrs != null && depth === textTagDepth) {
      flushText();
    }
  }
  flushText();

  const textContents = texts.map((t) => t.content).filter((t) => t !== '');

  return {
    dims,
    counts,
    text: textContents,
    sortedText: [...textContents].sort(),
    textKeys: texts.filter((t) => t.content !== '').map((t) => t.key).sort(),
    fills: [...fills].sort(),
    paths: paths.map((p) => p.key).sort(),
    rects: rects.map((r) => r.key).sort(),
    lines: lines.map((l) => l.key).sort(),
    arcs: arcs.sort(),
    raw: {paths, rects, lines, texts: texts.filter((t) => t.content !== '')}
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

function numericParts(text) {
  const numbers = splitNumbers(text).map(Number);
  const literals = text.replace(/[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/g, '#');
  return {numbers, literals};
}

function keysClose(a, b, tolerance) {
  if (a === b) return true;
  if (tolerance <= 0) return false;
  const pa = numericParts(a);
  const pb = numericParts(b);
  if (pa.literals !== pb.literals || pa.numbers.length !== pb.numbers.length) return false;
  return pa.numbers.every((value, i) => Math.abs(value - pb.numbers[i]) <= tolerance);
}

function firstMultisetDiff(upList, rustList, tolerance = 0) {
  const up = [...upList].sort();
  const rust = [...rustList].sort();
  if (sameSorted(up, rust)) return null;
  if (tolerance <= 0) {
    const limit = Math.max(up.length, rust.length);
    for (let i = 0; i < limit; i++) {
      if (up[i] !== rust[i]) {
        return {
          index: i,
          upstream: up[i] ?? null,
          rust: rust[i] ?? null,
          upstreamCount: up.length,
          rustCount: rust.length
        };
      }
    }
    return null;
  }
  // Tolerant multiset matching: pair each upstream entry with an unused rust
  // entry whose numeric tokens agree within `tolerance`.
  const used = new Array(rust.length).fill(false);
  for (let i = 0; i < up.length; i++) {
    let matched = -1;
    for (let j = 0; j < rust.length; j++) {
      if (!used[j] && keysClose(up[i], rust[j], tolerance)) {
        matched = j;
        break;
      }
    }
    if (matched < 0) {
      const exactIndex = rust.findIndex((value, j) => !used[j] && value === up[i]);
      return {
        index: i,
        upstream: up[i] ?? null,
        rust: exactIndex >= 0 ? rust[exactIndex] : null,
        upstreamCount: up.length,
        rustCount: rust.length
      };
    }
    used[matched] = true;
  }
  const leftover = rust.findIndex((value, j) => !used[j]);
  if (leftover >= 0) {
    return {
      index: up.length,
      upstream: null,
      rust: rust[leftover],
      upstreamCount: up.length,
      rustCount: rust.length
    };
  }
  return null;
}

function truncate(text, max = 120) {
  const value = String(text ?? '');
  return value.length <= max ? value : `${value.slice(0, max - 1)}…`;
}

function multisetDelta(upList, rustList) {
  const counts = new Map();
  for (const v of upList) counts.set(v, (counts.get(v) || 0) + 1);
  for (const v of rustList) counts.set(v, (counts.get(v) || 0) - 1);
  const missing = [];
  const extra = [];
  for (const [v, n] of counts) {
    if (n > 0) for (let i = 0; i < n; i++) missing.push(v);
    else if (n < 0) for (let i = 0; i < -n; i++) extra.push(v);
  }
  return {missing: missing.sort(), extra: extra.sort()};
}

function compare(up, rust, strict, tolerance = 0) {
  const diffs = [];
  const geometry = {
    paths: firstMultisetDiff(up.paths, rust.paths, tolerance),
    rects: firstMultisetDiff(up.rects, rust.rects, tolerance),
    lines: firstMultisetDiff(up.lines, rust.lines, tolerance),
    texts: firstMultisetDiff(up.textKeys, rust.textKeys, tolerance),
    arcs: firstMultisetDiff(up.arcs, rust.arcs, tolerance)
  };
  const geometryMatches =
    !geometry.paths && !geometry.rects && !geometry.lines && !geometry.texts && !geometry.arcs;

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
      missing.length ? `missing "${truncate(missing[0], 50)}"` : '',
      extra.length ? `extra "${truncate(extra[0], 50)}"` : ''
    ].filter(Boolean).join(', ');
    diffs.push(`text content (${up.text.length} vs ${rust.text.length}${detail ? `; ${detail}` : ''})`);
  }

  if (!sameSorted(up.fills, rust.fills)) {
    const upSet = new Set(up.fills);
    const rustSet = new Set(rust.fills);
    const missing = up.fills.filter((item) => !rustSet.has(item));
    const extra = rust.fills.filter((item) => !upSet.has(item));
    const detail = [
      missing.length ? `missing ${truncate(missing.join(', '), 60)}` : '',
      extra.length ? `extra ${truncate(extra.join(', '), 60)}` : ''
    ].filter(Boolean).join('; ');
    diffs.push(`fill colors${detail ? ` (${detail})` : ''}`);
  }

  if (strict) {
    const labels = {paths: 'path d', rects: 'rect geometry', lines: 'line geometry', texts: 'text position', arcs: 'arc flags'};
    const upLists = {paths: up.paths, rects: up.rects, lines: up.lines, texts: up.textKeys, arcs: up.arcs};
    const rustLists = {paths: rust.paths, rects: rust.rects, lines: rust.lines, texts: rust.textKeys, arcs: rust.arcs};
    for (const [kind, diff] of Object.entries(geometry)) {
      if (!diff) continue;
      const detail = [
        diff.upstream == null ? `missing upstream index ${diff.index}` : '',
        diff.rust == null ? `extra rust index ${diff.index}` : '',
        diff.upstream != null && diff.rust != null
          ? `upstream "${truncate(diff.upstream, 90)}" vs rust "${truncate(diff.rust, 90)}"`
          : ''
      ].filter(Boolean).join('; ');
      diffs.push(`${labels[kind]} mismatch at #${diff.index} (${upLists[kind].length} vs ${rustLists[kind].length})${detail ? `: ${detail}` : ''}`);
    }
  }

  return {diffs, geometry, geometryMatches};
}

function countLabel(counts) {
  return COUNT_TAGS.map((tag) => `${tag} ${counts[tag]}`).join(' ');
}

async function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (err) {
    console.error(`compare_upstream_svg: ${errorMessage(err)}`);
    console.error(usage());
    process.exitCode = 2;
    return;
  }
  if (opts.help) {
    console.log(usage());
    return;
  }

  const specsDir = path.join(opts.examples, 'specs');
  const compiledDir = path.join(opts.examples, 'compiled');
  await mkdir(REF_SVG_DIR, {recursive: true});

  let names;
  try {
    names = (await readdir(specsDir))
      .filter((name) => name.endsWith('.vl.json'))
      .map((name) => name.slice(0, -'.vl.json'.length))
      .filter((name) => !opts.name || name.includes(opts.name))
      .filter((name) => existsSync(path.join(compiledDir, `${name}.vg.json`)))
      .sort();
  } catch (err) {
    console.error(`compare_upstream_svg: cannot read ${specsDir}: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }
  names = names.slice(0, opts.limit);
  if (names.length === 0) {
    console.error('compare_upstream_svg: no specs matched the filters');
    process.exitCode = 1;
    return;
  }

  let vegaInfo;
  try {
    vegaInfo = await loadVega();
  } catch (err) {
    console.error(`compare_upstream_svg: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }
  const datasets = datasetRoot();
  const rust = await probeRust();
  if (!rust.ok) {
    console.error(`compare_upstream_svg: Rust renderer unavailable (${rust.reason})`);
    process.exitCode = 1;
    return;
  }

  const results = [];
  for (const name of names) {
    const specFile = path.relative(REPO_ROOT, path.join(specsDir, `${name}.vl.json`));
    const compiledFile = path.relative(REPO_ROOT, path.join(compiledDir, `${name}.vg.json`));
    const result = {
      name,
      specFile,
      compiledFile,
      status: 'error',
      upstream: null,
      rust: null,
      diffs: [],
      firstDifference: null,
      geometryMatches: false,
      geometry: null,
      compilerMatches: null,
      compilerDiffs: [],
      error: null
    };

    let upSvg = null;
    try {
      const compiled = JSON.parse(await readFile(path.join(compiledDir, `${name}.vg.json`), 'utf8'));
      upSvg = await renderUpstream(vegaInfo.vega, compiled, datasets);
      await writeFile(path.join(REF_SVG_DIR, `${name}.svg`), upSvg);
      result.upstream = parseSvg(upSvg);
    } catch (err) {
      result.error = `upstream failed: ${errorMessage(err)}`;
      result.firstDifference = result.error;
      results.push(result);
      console.log(`[ERROR] ${name} | ${result.error}`);
      continue;
    }

    const rendered = await renderRust(specFile, datasets, true);
    if (!rendered.ok) {
      result.error = `rust failed: ${rendered.reason}`;
      result.firstDifference = result.error;
      results.push(result);
      console.log(`[ERROR] ${name} | ${result.error}`);
      continue;
    }
    try {
      result.rust = parseSvg(rendered.svg);
    } catch (err) {
      result.error = `rust SVG parse failed: ${errorMessage(err)}`;
      result.firstDifference = result.error;
      results.push(result);
      console.log(`[ERROR] ${name} | ${result.error}`);
      continue;
    }

    const cmp = compare(result.upstream, result.rust, opts.strict, opts.tolerance);
    result.diffs = cmp.diffs;
    result.geometry = cmp.geometry;
    result.geometryMatches = cmp.geometryMatches;
    result.status = cmp.diffs.length === 0 ? 'pass' : 'fail';
    result.firstDifference = cmp.diffs[0] || null;

    if (opts.diagnoseCompiler) {
      const compiledRust = await renderRust(compiledFile, datasets, false);
      if (!compiledRust.ok) {
        result.compilerDiffs = [`rust compiled-spec render failed: ${compiledRust.reason}`];
        result.compilerMatches = false;
      } else {
        const fromCompiled = parseSvg(compiledRust.svg);
        const cmp2 = compare(result.rust, fromCompiled, opts.strict, opts.tolerance);
        result.compilerDiffs = cmp2.diffs;
        result.compilerMatches = cmp2.diffs.length === 0;
      }
    }

    results.push(result);

    const up = result.upstream;
    const rs = result.rust;
    const detail = result.status === 'pass'
      ? ''
      : ` | first diff: ${oneLine(result.firstDifference || 'unknown')}`;
    const compiler = result.compilerMatches === false ? ' | compiler-output differs' : '';
    console.log(
      `[${result.status.toUpperCase()}] ${name} | dims up=${up.dims.width}x${up.dims.height} rust=${rs.dims.width}x${rs.dims.height}` +
      ` | counts up/rust ${COUNT_TAGS.map((tag) => `${tag} ${up.counts[tag]}/${rs.counts[tag]}`).join(' ')}` +
      ` | text ${up.text.length}/${rs.text.length} | geom ${result.geometryMatches ? 'match' : 'diff'}${detail}${compiler}`
    );
    if (opts.verbose && !result.geometryMatches) {
      const upLists = {paths: up.paths, rects: up.rects, lines: up.lines, texts: up.textKeys, arcs: up.arcs};
      const rustLists = {paths: rs.paths, rects: rs.rects, lines: rs.lines, texts: rs.textKeys, arcs: rs.arcs};
      for (const [kind, list] of Object.entries(upLists)) {
        const {missing, extra} = multisetDelta(list, rustLists[kind]);
        if (missing.length === 0 && extra.length === 0) continue;
        console.log(`    ${kind}: ${missing.length} missing, ${extra.length} extra`);
        for (const v of missing.slice(0, opts.verbose > 1 ? Infinity : 6)) console.log(`      - ${v}`);
        for (const v of extra.slice(0, opts.verbose > 1 ? Infinity : 6)) console.log(`      + ${v}`);
      }
    }
  }

  const compared = results.filter((result) => result.status !== 'error');
  const passed = results.filter((result) => result.status === 'pass');
  const geometryMatches = compared.filter((result) => result.geometryMatches);
  const geometryKinds = ['paths', 'rects', 'lines', 'texts', 'arcs'];
  const summary = {
    total: results.length,
    compared: compared.length,
    passed: passed.length,
    failed: results.filter((result) => result.status === 'fail').length,
    errors: results.filter((result) => result.status === 'error').length,
    geometryMatches: geometryMatches.length,
    geometryKindMatches: Object.fromEntries(
      geometryKinds.map((kind) => [kind, compared.filter((result) => !result.geometry?.[kind]).length])
    ),
    compilerMatches: opts.diagnoseCompiler
      ? compared.filter((result) => result.compilerMatches).length
      : null,
    strict: opts.strict
  };

  console.log('');
  console.log(
    `Summary: ${summary.compared}/${summary.total} compared, ${summary.passed} passed, ` +
    `${summary.failed} failed, ${summary.errors} errors; geometry ${summary.geometryMatches}/${summary.compared} match ` +
    `(${geometryKinds.map((kind) => `${kind} ${summary.geometryKindMatches[kind]}`).join(', ')})` +
    (opts.diagnoseCompiler ? `; compiler output ${summary.compilerMatches}/${summary.compared} match` : '')
  );
  console.log(`Upstream vega: ${vegaInfo.from}`);
  console.log(`Reference SVGs: ${REF_SVG_DIR}`);

  const report = {
    generatedAt: new Date().toISOString(),
    repoRoot: REPO_ROOT,
    examples: opts.examples,
    refSvgDir: REF_SVG_DIR,
    datasets,
    upstreamVega: vegaInfo.from,
    options: {...opts, limit: Number.isFinite(opts.limit) ? opts.limit : null},
    summary,
    results
  };
  await writeFile(opts.json, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Report: ${opts.json}`);

  const failedStrict = opts.strict && (summary.failed > 0 || summary.errors > 0);
  if (summary.errors > 0 || failedStrict) process.exitCode = 1;
}

function oneLine(text) {
  return String(text ?? '').replace(/\s*\r?\n\s*/g, ' ').trim();
}

main().catch((err) => {
  console.error(`compare_upstream_svg: unexpected failure: ${errorMessage(err)}`);
  process.exitCode = 1;
});
