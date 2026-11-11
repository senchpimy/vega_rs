#!/usr/bin/env node
// Diagnostic visual-issue harness for the Rust Vega/Vega-Lite renderer.
//
// Unlike scripts/compare_upstream_svg.mjs (which only reports counts and
// geometry multiset deltas), this harness extracts semantic mark geometry
// (bars, symbols, lines, axis text, grid lines) from both the upstream Vega
// SVG and the Rust SVG and emits concrete issue codes explaining *why* a
// chart looks wrong: zero-size bars, labels inside instead of above bars,
// missing line breaks at invalid data, reversed scales/stacks, overlapping
// symbols, extra grid lines, clipped axes, wrong colors, wrong time labels,
// missing marks, etc.
//
// Usage:
//   node scripts/compare_visual_issues.mjs [--name <substr>] [--limit <n>]
//        [--json <path>] [--verbose] [--examples <dir>]
//
// The fixture list defaults to the visual-bug corpus; unknown/typo names are
// fuzzy-matched (aliases such as `bar_binned_yearmonth` -> `bar_binnedyearmonth`
// and `test_scatterplot_colored` -> `text_scatterplot_colored` are built in).
//
// Writes /tmp/opencode/visual_issues.json and SVGs under
// /tmp/opencode/visual_harness/{upstream,rust}/.

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
const OUT_DIR = '/tmp/opencode/visual_harness';
const DEFAULT_JSON = '/tmp/opencode/visual_issues.json';
const LOCAL_DATASETS = path.join(REPO_ROOT, 'node_modules/vega-datasets');
const TMP_DATASETS = '/tmp/opencode/node_modules/vega-datasets';

const EPS = 0.05;
const BAR_NEAR = 3;
const TEXT_NEAR = 4;

// The corpus named by AGENT_VISUAL_HARNESS.md. `aliases` cover upstream
// filenames that differ from the names used in bug reports.
const FIXTURES = [
  {name: 'bar_argmax', aliases: ['bar_argmax']},
  {name: 'bar_axis_space_saving', aliases: ['bar_axis_space_saving']},
  {name: 'bar_binned_yearmonth_grouped', aliases: ['bar_binned_yearmonth_grouped']},
  {name: 'bar_binned_yearmonth_label', aliases: ['bar_binned_yearmonth_label']},
  {name: 'bar_binned_yearmonth', aliases: ['bar_binnedyearmonth', 'bar_binned_yearmonth']},
  {name: 'bar_corner_radius_end', aliases: ['bar_corner_radius_end']},
  {name: 'bar_custom_time_domain', aliases: ['bar_custom_time_domain']},
  {
    name: 'bar_diverging_stack_population_pyramid',
    aliases: ['bar_diverging_stack_population_pyramid']
  },
  {name: 'bar_fit', aliases: ['bar_fit']},
  {name: 'bar_grouped_thin', aliases: ['bar_grouped_thin']},
  {name: 'bar_heatlane', aliases: ['bar_heatlane']},
  {name: 'bar_layered_weather', aliases: ['bar_layered_weather']},
  {name: 'bar_params', aliases: ['bar_params']},
  {name: 'bar_qq_stack_horizontal', aliases: ['bar_qq_stack_horizontal']},
  {
    name: 'bar_simple_binned_timeunit_special_chars',
    aliases: ['bar_simple_binned_timeunit_special_chars']
  },
  {name: 'boxplot_1D_invalid', aliases: ['boxplot_1D_invalid']},
  {name: 'circle_binned_maxbins_2', aliases: ['circle_binned_maxbins_2']},
  {name: 'circle_binned_maxbins_5', aliases: ['circle_binned_maxbins_5', 'circle_binned_maxbins2', 'circle_binned_maxbins5']},
  {name: 'circle_scale_quantize', aliases: ['circle_scale_quantize']},
  {name: 'circle_wilkinson_dotplot', aliases: ['circle_wilkinson_dotplot']},
  {name: 'test_invalid_default', aliases: ['test_invalid_default']},
  {name: 'test_invalid_filter', aliases: ['test_invalid_filter']},
  {name: 'test_subobject', aliases: ['test_subobject']},
  {name: 'test_subobject_nested', aliases: ['test_subobject_nested']},
  {name: 'test_subobject_missing', aliases: ['test_subobject_missing']},
  {name: 'test_scatterplot_colored', aliases: ['text_scatterplot_colored', 'test_scatterplot_colored']},
  {name: 'time_parse_binnedutc', aliases: ['time_parse_binnedutc']},
  {name: 'time_parse_utc_format', aliases: ['time_parse_utc_format']},
  {name: 'trail_color', aliases: ['trail_color']}
];

// Second diagnostic batch (AGENT_VISUAL_HARNESS2.md).
const BATCH2_FIXTURES = [
  {name: 'bar_qq_stack', aliases: ['bar_qq_stack', 'bar_qq_stack_horizontal']},
  {name: 'bar_qq_stack_horizontal', aliases: ['bar_qq_stack_horizontal', 'bar_qq_stack']},
  {name: 'circle_scale_quantile', aliases: ['circle_scale_quantile']},
  {name: 'circle_scale_threshold', aliases: ['circle_scale_threshold']},
  {name: 'concat_population_pyramid', aliases: ['concat_population_pyramid']},
  {name: 'embedded_csv', aliases: ['embedded_csv']},
  {name: 'facet_bullet', aliases: ['facet_bullet']},
  {name: 'facet_column_facet_row_point_future', aliases: ['facet_column_facet_row_point_future']},
  {name: 'facet_custom_header', aliases: ['facet_custom_header']},
  {name: 'geo_choropleth', aliases: ['geo_choropleth']},
  {name: 'geo_circle', aliases: ['geo_circle']},
  // "broadly geo_*": cover the remaining upstream geo fixtures as well.
  {name: 'geo_constant_value', aliases: ['geo_constant_value']},
  {name: 'geo_custom_projection', aliases: ['geo_custom_projection']},
  {name: 'geo_graticule_object', aliases: ['geo_graticule_object']},
  {name: 'geo_graticule', aliases: ['geo_graticule']},
  {name: 'geo_layer_line_london', aliases: ['geo_layer_line_london']},
  {name: 'geo_layer', aliases: ['geo_layer']},
  {name: 'geo_line', aliases: ['geo_line']},
  {name: 'geo_params_projections', aliases: ['geo_params_projections']},
  {name: 'geo_point', aliases: ['geo_point']},
  {name: 'geo_repeat', aliases: ['geo_repeat']},
  {name: 'geo_rule', aliases: ['geo_rule']},
  {name: 'geo_sphere', aliases: ['geo_sphere']},
  {name: 'geo_text', aliases: ['geo_text']},
  {name: 'geo_trellis', aliases: ['geo_trellis']},
  {name: 'histogram_log', aliases: ['histogram_log']},
  {name: 'histogram_ordinal_sort', aliases: ['histogram_ordinal_sort']},
  {name: 'histogram_reverse', aliases: ['histogram_reverse']},
  {name: 'image_scatter_no_size', aliases: ['image_scatter_no_size']},
  {name: 'layer_falkensee', aliases: ['layer_falkensee']},
  {name: 'layer_histogram_global_mean', aliases: ['layer_histogram_global_mean']},
  {name: 'layer_line_window', aliases: ['layer_line_window']},
  {name: 'layer_point_line_loess', aliases: ['layer_point_line_loess']},
  {name: 'line_color_binned', aliases: ['line_color_binned']},
  {name: 'line_concat_facet', aliases: ['line_concat_facet']},
  {name: 'line_encoding_impute_keyvals', aliases: ['line_encoding_impute_keyvals']},
  {
    name: 'line_encoding_impute_keyvals_sequence',
    aliases: ['line_encoding_impute_keyvals_sequence']
  },
  {name: 'line_narrow_2year_span', aliases: ['line_narrow_2year_span']}
];

// Third diagnostic batch (AGENT_VISUAL_HARNESS3.md).
const BATCH3_FIXTURES = [
  {name: 'ternary', aliases: ['ternary']},
  {name: 'test_aggregate_nested', aliases: ['test_aggregate_nested']},
  {name: 'test_invalid_color_show', aliases: ['test_invalid_color_show']},
  {name: 'test_trellis_cross_sort_missing', aliases: ['test_trellis_cross_sort_missing']},
  {name: 'waterfall_chart', aliases: ['waterfall_chart']},
  {name: 'window_rank', aliases: ['window_rank']},
  {name: 'trail_comet', aliases: ['trail_comet']},
  {name: 'trellis_cross_sort_array', aliases: ['trellis_cross_sort_array']},
  {name: 'stacked_bar_weather', aliases: ['stacked_bar_weather']},
  {name: 'line_impute_frame', aliases: ['line_impute_frame']},
  {name: 'geo_constant_value', aliases: ['geo_constant_value']},
  {name: 'interactive_geo_facet_species', aliases: ['interactive_geo_facet_species']}
];

function usage() {
  return `Usage: node scripts/compare_visual_issues.mjs [options]

Options:
  --name <text>        Only diagnose fixtures whose name/alias contains <text>
  --limit <n>          Diagnose at most n fixtures
  --batch2             Use the AGENT_VISUAL_HARNESS2 fixture corpus
  --batch3             Use the AGENT_VISUAL_HARNESS3 fixture corpus
  --render-upstream    Re-render upstream from compiled .vg.json instead of
                       using the checked-in compiled .svg
  --examples <dir>     Vega-Lite checkout root (default ${DEFAULT_EXAMPLES})
  --json <file>        Report path (default ${DEFAULT_JSON}; batch2/3 use
                       /tmp/opencode/visual_issues_batch{2,3}.json)
  --verbose, -v        Print per-issue element detail
  -h, --help           Show this help`;
}

function parseArgs(argv) {
  const opts = {
    name: null,
    limit: Infinity,
    examples: DEFAULT_EXAMPLES,
    json: DEFAULT_JSON,
    batch2: false,
    batch3: false,
    renderUpstream: false,
    verbose: false,
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
    else if (arg === '--json') opts.json = path.resolve(take(arg));
    else if (arg.startsWith('--json=')) opts.json = path.resolve(arg.slice('--json='.length));
    else if (arg === '--batch2') opts.batch2 = true;
    else if (arg === '--batch3') opts.batch3 = true;
    else if (arg === '--render-upstream') opts.renderUpstream = true;
    else if (arg === '--verbose' || arg === '-v') opts.verbose = true;
    else throw new Error(`Unknown argument: ${arg}`);
  }
  if (opts.batch2 && opts.json === DEFAULT_JSON) opts.json = '/tmp/opencode/visual_issues_batch2.json';
  if (opts.batch3 && opts.json === DEFAULT_JSON) opts.json = '/tmp/opencode/visual_issues_batch3.json';
  return opts;
}

function firstLine(value) {
  const text = String(value ?? '').trim();
  return text.split(/\r?\n/).find((line) => line.trim() !== '')?.trim() || 'unknown error';
}

function errorMessage(err) {
  return firstLine(err && err.stack ? err.stack : err);
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
  try {
    await execFileAsync('cargo', ['build', '-q', '-p', 'vega-svg', '--bin', 'vega-svg'], {
      cwd: REPO_ROOT,
      timeout: 300_000,
      maxBuffer: 16 * 1024 * 1024
    });
    return {ok: true};
  } catch (err) {
    return {ok: false, reason: `cargo build failed: ${firstLine(err.stderr || err.message)}`};
  }
}

async function renderRust(file, datasets) {
  const args = [
    'run', '-q', '-p', 'vega-svg', '--bin', 'vega-svg', '--',
    '--data-dir', datasets ? path.relative(REPO_ROOT, datasets) : REPO_ROOT,
    '--vega-lite',
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

// ---------------------------------------------------------------------------
// SVG parsing with transform stack and semantic mark extraction.

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

function splitNumbers(text) {
  return String(text ?? '').match(/[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/g) || [];
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

function matrixScale(m) {
  return Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2])) || 1;
}

function round(value, digits = 3) {
  if (!Number.isFinite(value)) return 0;
  const scale = 10 ** digits;
  const rounded = Math.round(value * scale) / scale;
  return Object.is(rounded, -0) ? 0 : rounded;
}

function bboxOf(points) {
  if (!points.length) return null;
  let x1 = Infinity;
  let y1 = Infinity;
  let x2 = -Infinity;
  let y2 = -Infinity;
  for (const [x, y] of points) {
    if (x < x1) x1 = x;
    if (y < y1) y1 = y;
    if (x > x2) x2 = x;
    if (y > y2) y2 = y;
  }
  return {x1, y1, x2, y2, width: x2 - x1, height: y2 - y1};
}

function unionBbox(a, b) {
  if (!a) return b;
  if (!b) return a;
  return {
    x1: Math.min(a.x1, b.x1),
    y1: Math.min(a.y1, b.y1),
    x2: Math.max(a.x2, b.x2),
    y2: Math.max(a.y2, b.y2),
    width: Math.max(a.x2, b.x2) - Math.min(a.x1, b.x1),
    height: Math.max(a.y2, b.y2) - Math.min(a.y1, b.y1)
  };
}

// Parse an SVG path into subpaths, absolute points, arcs and a bbox.
function parsePath(d, matrix, options = {}) {
  const compact = options.compact === true;
  const tokens = String(d ?? '').match(/[a-zA-Z]|[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/g) || [];
  const subpaths = [];
  const points = [];
  const arcs = [];
  let subpath = null;
  let command = '';
  let cx = 0;
  let cy = 0;
  let sx = 0;
  let sy = 0;
  let i = 0;
  let drawCommands = 0;
  let pointCount = 0;
  let bx1 = Infinity;
  let by1 = Infinity;
  let bx2 = -Infinity;
  let by2 = -Infinity;
  const addBBox = (x, y) => {
    if (x < bx1) bx1 = x;
    if (y < by1) by1 = y;
    if (x > bx2) bx2 = x;
    if (y > by2) by2 = y;
  };

  const addPoint = (x, y) => {
    const p = applyMatrix(matrix, x, y);
    pointCount++;
    addBBox(p[0], p[1]);
    if (!compact) points.push(p);
    if (subpath) {
      if (!compact) subpath.points.push(p);
      subpath.pointCount = (subpath.pointCount || 1) + 1;
    }
    return p;
  };
  const startSubpath = (x, y) => {
    cx = x;
    cy = y;
    sx = x;
    sy = y;
    subpath = {start: applyMatrix(matrix, x, y), points: compact ? null : [], pointCount: 0, closed: false};
    subpaths.push(subpath);
    addPoint(x, y);
  };

  while (i < tokens.length) {
    let token = tokens[i];
    if (/[a-zA-Z]/.test(token)) {
      command = token;
      i++;
      if (command === 'Z' || command === 'z') {
        if (subpath) {
          subpath.closed = true;
          addPoint(sx, sy);
        }
        continue;
      }
      if (command === 'M' || command === 'm') {
        const rel = command === 'm';
        let first = true;
        while (i + 1 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
          const x = Number(tokens[i]) + (rel ? cx : 0);
          const y = Number(tokens[i + 1]) + (rel ? cy : 0);
          i += 2;
          if (first) {
            startSubpath(x, y);
            first = false;
          } else {
            addPoint(x, y);
            cx = x;
            cy = y;
            drawCommands++;
          }
        }
        continue;
      }
      continue;
    }
    if (!command) {
      i++;
      continue;
    }
    const rel = command === command.toLowerCase();
    const upper = command.toUpperCase();
    const ox = rel ? cx : 0;
    const oy = rel ? cy : 0;
    if (upper === 'L') {
      while (i + 1 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const x = Number(tokens[i]) + ox;
        const y = Number(tokens[i + 1]) + oy;
        i += 2;
        addPoint(x, y);
        cx = x;
        cy = y;
        drawCommands++;
      }
    } else if (upper === 'H') {
      while (i < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const x = Number(tokens[i]) + ox;
        i++;
        addPoint(x, cy);
        cx = x;
        drawCommands++;
      }
    } else if (upper === 'V') {
      while (i < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const y = Number(tokens[i]) + (rel ? cy : 0);
        i++;
        addPoint(cx, y);
        cy = y;
        drawCommands++;
      }
    } else if (upper === 'C') {
      while (i + 5 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const x1 = Number(tokens[i]) + ox;
        const y1 = Number(tokens[i + 1]) + oy;
        const x2 = Number(tokens[i + 2]) + ox;
        const y2 = Number(tokens[i + 3]) + oy;
        const x = Number(tokens[i + 4]) + ox;
        const y = Number(tokens[i + 5]) + oy;
        i += 6;
        addPoint(x1, y1);
        addPoint(x2, y2);
        addPoint(x, y);
        cx = x;
        cy = y;
        drawCommands++;
      }
    } else if (upper === 'S' || upper === 'Q') {
      const step = upper === 'S' ? 4 : 4;
      while (i + step - 1 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const x1 = Number(tokens[i]) + ox;
        const y1 = Number(tokens[i + 1]) + oy;
        const x = Number(tokens[i + 2]) + ox;
        const y = Number(tokens[i + 3]) + oy;
        i += step;
        addPoint(x1, y1);
        addPoint(x, y);
        cx = x;
        cy = y;
        drawCommands++;
      }
    } else if (upper === 'T') {
      while (i + 1 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const x = Number(tokens[i]) + ox;
        const y = Number(tokens[i + 1]) + oy;
        i += 2;
        addPoint(x, y);
        cx = x;
        cy = y;
        drawCommands++;
      }
    } else if (upper === 'A') {
      while (i + 6 < tokens.length && !/[a-zA-Z]/.test(tokens[i])) {
        const rx = Math.abs(Number(tokens[i]));
        const ry = Math.abs(Number(tokens[i + 1]));
        const rot = Number(tokens[i + 2]);
        const large = Number(tokens[i + 3]);
        const sweep = Number(tokens[i + 4]);
        const x = Number(tokens[i + 5]) + ox;
        const y = Number(tokens[i + 6]) + oy;
        i += 7;
        arcs.push({rx, ry, rot, large, sweep, x1: cx, y1: cy, x2: x, y2: y});
        addPoint(x, y);
        // Include the full arc extent; a 90-degree-ish upper bound is enough
        // for bbox/clipping diagnostics.
        const scale = matrixScale(matrix);
        const r = Math.max(rx, ry) * scale;
        const center = applyMatrix(matrix, (cx + x) / 2, (cy + y) / 2);
        addBBox(center[0] - r, center[1] - r);
        addBBox(center[0] + r, center[1] + r);
        cx = x;
        cy = y;
        drawCommands++;
      }
    } else {
      i++;
    }
  }

  const bbox = pointCount > 0 ? {x1: bx1, y1: by1, x2: bx2, y2: by2, width: bx2 - bx1, height: by2 - by1} : null;
  return {
    d: compact ? '' : String(d ?? ''),
    subpaths,
    points,
    pointCount,
    arcs,
    bbox,
    drawCommands,
    moveOnly: drawCommands === 0 && subpaths.length > 0,
    hasArc: arcs.length > 0
  };
}

// A symbol path is the Vega circle/symbol pattern centered on the local origin:
//   M r,0 A r,r,0,1,1,-r,0 A r,r,0,1,1,r,0
function symbolFromPath(parsed, matrix) {
  if (!parsed.hasArc) return null;
  const circleArcs = parsed.arcs.filter((arc) => Math.abs(arc.rx - arc.ry) < 1e-6);
  if (circleArcs.length < 2) return null;
  if (parsed.subpaths.length !== 1) return null;
  const start = parsed.subpaths[0].start;
  const center = applyMatrix(matrix, 0, 0);
  const r = Math.hypot(start[0] - center[0], start[1] - center[1]);
  if (!(r > 0)) return null;
  // Every arc endpoint must lie on the circle centered on the local origin;
  // this rejects pie/arc marks whose radii are large but whose center is not.
  const tolerance = Math.max(0.5, r * 0.03);
  for (const arc of circleArcs) {
    const from = applyMatrix(matrix, arc.x1, arc.y1);
    const to = applyMatrix(matrix, arc.x2, arc.y2);
    if (Math.abs(Math.hypot(from[0] - center[0], from[1] - center[1]) - r) > tolerance) return null;
    if (Math.abs(Math.hypot(to[0] - center[0], to[1] - center[1]) - r) > tolerance) return null;
  }
  return {
    cx: center[0],
    cy: center[1],
    r,
    largeFlags: circleArcs.map((arc) => arc.large),
    sweepFlags: circleArcs.map((arc) => arc.sweep)
  };
}

const TAG_RE = /<(\/?)([a-zA-Z][\w:-]*)((?:"[^"]*"|'[^']*'|[^>"'])*)>|<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<[!?][^>]*>/g;

function parseSvg(svg) {
  const compact = svg.length > 2_000_000;
  const rootMatch = svg.match(/<svg\b[^>]*>/i);
  if (!rootMatch) throw new Error('no <svg> root element');
  const root = rootMatch[0];
  const attrsRoot = parseAttrs(root);
  const dims = {
    width: Number(String(attrsRoot.width ?? '').replace(/px$/i, '')) || 0,
    height: Number(String(attrsRoot.height ?? '').replace(/px$/i, '')) || 0,
    viewBox: normalizeSpace(attrsRoot.viewBox ?? '') || null
  };
  if (dims.viewBox) {
    const parts = dims.viewBox.split(/[\s,]+/).map(Number);
    if (parts.length === 4) {
      dims.width = dims.width || parts[2];
      dims.height = dims.height || parts[3];
    }
  }

  const paths = [];
  const rects = [];
  const lines = [];
  const texts = [];
  const circles = [];
  const images = [];
  const markGroups = [];
  const facetCells = [];
  const facetHeaders = [];
  let roundedClipPaths = 0;
  let clipRefs = 0;

  const stack = [];
  let matrix = IDENTITY;
  let fill = null;
  let stroke = null;
  let groupClass = '';
  let depth = 0;
  let text = null;

  const flushText = () => {
    if (!text) return;
    const content = normalizeSpace(decodeEntities(text.buffer.replace(/<[^>]*>/g, ' ')));
    if (content !== '') {
      const x = Number(text.attrs.x ?? 0) + Number(text.attrs.dx ?? 0);
      const y = Number(text.attrs.y ?? 0) + Number(text.attrs.dy ?? 0);
      const pos = applyMatrix(text.matrix, x, y);
      const fontSize = Number(String(text.attrs['font-size'] ?? '').replace(/px$/i, '')) || 10;
      texts.push({
        index: texts.length,
        content,
        x: pos[0],
        y: pos[1],
        anchor: text.attrs['text-anchor'] || 'start',
        fontSize,
        fill: text.attrs.fill ?? fill,
        className: text.className,
        matrix: text.matrix,
        bbox: {
          x1: pos[0] - (text.attrs['text-anchor'] === 'middle' ? 30 : text.attrs['text-anchor'] === 'end' ? 60 : 0),
          y1: pos[1] - fontSize,
          x2: pos[0] + (text.attrs['text-anchor'] === 'middle' ? 30 : text.attrs['text-anchor'] === 'end' ? 0 : 60),
          y2: pos[1] + 2
        }
      });
    }
    text = null;
  };

  let lastIndex = 0;
  for (const match of svg.matchAll(TAG_RE)) {
    if (text && match.index > lastIndex) text.buffer += svg.slice(lastIndex, match.index);
    lastIndex = match.index + match[0].length;
    if (match[0].startsWith('<!--') || match[0].startsWith('<!') || match[0].startsWith('<?')) continue;
    const closing = match[1] === '/';
    const name = (match[2] || '').toLowerCase();
    const rawAttrs = match[3] || '';
    if (!name) continue;
    if (text && name === 'tspan') text.buffer += ' ';
    const selfClosing = /\/\s*$/.test(rawAttrs);
    const attrs = parseAttrs(rawAttrs);

    if (closing) {
      if (name === 'text') flushText();
      depth--;
      const entry = stack.pop();
      if (entry) {
        matrix = entry.matrix;
        fill = entry.fill;
        stroke = entry.stroke;
        groupClass = entry.className;
      } else {
        matrix = IDENTITY;
      }
      continue;
    }

    const tagMatrix = multiply(matrix, parseTransform(attrs.transform));
    const className = attrs.class ?? groupClass;
    if (attrs['clip-path']) clipRefs++;
    const tagFill = attrs.fill ?? fill;
    const tagStroke = attrs.stroke ?? stroke;
    const style = attrs.style ? parseAttrs(attrs.style.replace(/;/g, ' ')) : {};
    const effectiveFill = style.fill ?? tagFill;
    const effectiveStroke = style.stroke ?? tagStroke;

    if (name === 'text') {
      if (text) flushText();
      text = {attrs, matrix: tagMatrix, buffer: '', className};
    } else if (name === 'path') {
      const parsed = parsePath(attrs.d ?? '', tagMatrix, {compact});
      const symbol = symbolFromPath(parsed, tagMatrix);
      const item = {
        index: paths.length,
        parsed,
        bbox: parsed.bbox,
        fill: normalizeColor(effectiveFill),
        stroke: normalizeColor(effectiveStroke),
        className,
        matrix: tagMatrix,
        opacity: attrs.opacity == null ? 1 : Number(attrs.opacity),
        ariaRole: (attrs['aria-roledescription'] ?? '').toLowerCase(),
        ariaLabel: attrs['aria-label'] ?? null,
        symbol
      };
      paths.push(item);
      if (markGroups.length && !isMarkChrome(className)) markGroups[markGroups.length - 1].count++;
      if (item.className.toLowerCase().includes('background') && /[cC]/.test(attrs.d ?? '')) {
        roundedClipPaths++;
      }
      if (symbol) {
        circles.push({
          index: circles.length,
          pathIndex: paths.length - 1,
          ...symbol,
          fill: item.fill,
          className
        });
      }
    } else if (name === 'rect') {
      const x = Number(attrs.x ?? 0);
      const y = Number(attrs.y ?? 0);
      const w = Number(attrs.width ?? 0);
      const h = Number(attrs.height ?? 0);
      const p1 = applyMatrix(tagMatrix, x, y);
      const p2 = applyMatrix(tagMatrix, x + w, y + h);
      rects.push({
        index: rects.length,
        x: Math.min(p1[0], p2[0]),
        y: Math.min(p1[1], p2[1]),
        w: Math.abs(p2[0] - p1[0]),
        h: Math.abs(p2[1] - p1[1]),
        fill: normalizeColor(effectiveFill),
        className
      });
    } else if (name === 'line') {
      const p1 = applyMatrix(tagMatrix, Number(attrs.x1 ?? 0), Number(attrs.y1 ?? 0));
      const p2 = applyMatrix(tagMatrix, Number(attrs.x2 ?? 0), Number(attrs.y2 ?? 0));
      lines.push({
        index: lines.length,
        x1: p1[0],
        y1: p1[1],
        x2: p2[0],
        y2: p2[1],
        stroke: normalizeColor(effectiveStroke),
        className
      });
    } else if (name === 'circle' || name === 'ellipse') {
      const c = applyMatrix(tagMatrix, Number(attrs.cx ?? 0), Number(attrs.cy ?? 0));
      const r = Number(attrs.r ?? attrs.rx ?? attrs.ry ?? 0) * matrixScale(tagMatrix);
      circles.push({
        index: circles.length,
        pathIndex: -1,
        cx: c[0],
        cy: c[1],
        r,
        fill: normalizeColor(effectiveFill),
        className
      });
    } else if (name === 'image') {
      const x = Number(attrs.x ?? 0);
      const y = Number(attrs.y ?? 0);
      const w = Number(attrs.width ?? 0);
      const h = Number(attrs.height ?? 0);
      const p = applyMatrix(tagMatrix, x, y);
      const href = attrs.href ?? attrs['xlink:href'] ?? null;
      images.push({
        index: images.length,
        x: p[0],
        y: p[1],
        w: w * matrixScale(tagMatrix),
        h: h * matrixScale(tagMatrix),
        href,
        className,
        opacity: attrs.opacity == null ? 1 : Number(attrs.opacity)
      });
    }

    if (name === 'g' && attrs.class) {
      const own = attrs.class;
      if (own.includes('role-mark') && !own.includes('role-legend')) {
        markGroups.push({name: normalizeMarkClass(own), count: 0});
      }
      if (own.includes('cell')) facetCells.push(own);
      if (own.includes('header')) facetHeaders.push(own);
    }

    if (!selfClosing) {
      stack.push({name, matrix, fill, stroke, className: className});
      matrix = tagMatrix;
      fill = effectiveFill ?? null;
      stroke = effectiveStroke ?? null;
      groupClass = className;
      depth++;
    } else if (name === 'text') {
      flushText();
    }
  }
  flushText();

  // Semantic grouping. Paths that come from mark groups are classified by
  // their parent group's class (`mark-rect role-mark ...`).
  const bars = [];
  const lineMarks = [];
  const areaMarks = [];
  const otherPaths = [];
  const shapes = [];
  const symbolShapes = [];
  for (const item of paths) {
    const cls = item.className.toLowerCase();
    const isBackground = cls.includes('background') || cls.includes('foreground');
    const parsed = item.parsed;
    if (isBackground || item.opacity === 0) continue;
    if (cls.includes('role-legend')) continue;
    const symbol = symbolFromPath(parsed, item.matrix);
    if (symbol) {
      otherPaths.push({...item, kind: 'symbol', symbol});
      continue;
    }
    if (parsed.moveOnly) {
      otherPaths.push({...item, kind: 'empty'});
      continue;
    }
    const bbox = parsed.bbox;
    if (!bbox) continue;

    // Mark groups carry the authoritative mark type. Fall back to shape
    // heuristics for specs/renders that omit the group class.
    const markRect = cls.includes('mark-rect') || item.ariaRole === 'bar' || item.ariaRole === 'rect';
    const markArea = cls.includes('mark-area') || cls.includes('mark-arc');
    const markLine = cls.includes('mark-line') || cls.includes('mark-trail');
    const markShape = cls.includes('mark-shape') || cls.includes('mark-geoshape') || cls.includes('mark-path');
    const markSymbol = cls.includes('mark-symbol') || item.ariaRole === 'square' || item.ariaRole === 'circle';

    if (markShape) {
      shapes.push({
        index: item.index,
        fill: item.fill,
        stroke: item.stroke,
        bbox,
        subpaths: parsed.subpaths.length,
        points: parsed.pointCount ?? parsed.points.length,
        moveOnly: parsed.moveOnly,
        path: parsed,
        className: cls
      });
    } else if (markSymbol && !symbol) {
      symbolShapes.push({
        index: item.index,
        fill: item.fill,
        stroke: item.stroke,
        bbox,
        ariaRole: item.ariaRole,
        path: parsed,
        className: cls
      });
    } else if (markRect || (item.fill && item.fill !== 'none' && parsed.subpaths.length <= 2 && !parsed.hasArc && !markArea && !markLine)) {
      // Rect-like filled path (bars, heatmap cells, boxplot boxes).
      bars.push({
        index: item.index,
        x: bbox.x1,
        y: bbox.y1,
        w: bbox.width,
        h: bbox.height,
        fill: item.fill,
        className: cls,
        subpaths: parsed.subpaths.length,
        path: parsed,
        bbox,
        curved: parsed.arcs.length > 0 || /[cqs]/i.test(item.parsed.d)
      });
    } else if (markLine) {
      lineMarks.push({
        index: item.index,
        stroke: item.stroke,
        fill: item.fill,
        bbox,
        subpaths: parsed.subpaths.length,
        subpathShapes: parsed.subpaths.map((s) => s.pointCount ?? (s.points ? s.points.length : 0)),
        path: parsed,
        className: cls
      });
    } else if (item.fill && item.fill !== 'none') {
      areaMarks.push({
        index: item.index,
        fill: item.fill,
        bbox,
        subpaths: parsed.subpaths.length,
        path: parsed,
        className: cls
      });
    } else {
      lineMarks.push({
        index: item.index,
        stroke: item.stroke,
        bbox,
        subpaths: parsed.subpaths.length,
        subpathShapes: parsed.subpaths.map((s) => s.pointCount ?? (s.points ? s.points.length : 0)),
        path: parsed,
        className: cls
      });
    }
  }
  // Plain <rect> marks (non-background) are bars too. Skip the root white
  // canvas rect and any rect that lacks a mark class.
  for (const rect of rects) {
    const cls = rect.className.toLowerCase();
    if (!cls.includes('mark-rect')) continue;
    bars.push({
      index: 100000 + rect.index,
      x: rect.x,
      y: rect.y,
      w: rect.w,
      h: rect.h,
      fill: rect.fill,
      className: cls,
      subpaths: 1,
      rect: true,
      curved: false,
      bbox: {x1: rect.x, y1: rect.y, x2: rect.x + rect.w, y2: rect.y + rect.h, width: rect.w, height: rect.h}
    });
  }

  const gridLines = lines.filter((line) => {
    const cls = line.className.toLowerCase();
    return cls.includes('grid') || line.stroke === '#ddd' || line.stroke === '#dddddd';
  });
  const tickLines = lines.filter((line) => {
    const cls = line.className.toLowerCase();
    return !cls.includes('grid') && (cls.includes('tick') || line.stroke === '#888' || line.stroke === '#888888');
  });
  const domainLines = lines.filter((line) => !gridLines.includes(line) && !tickLines.includes(line));
  const rules = domainLines.filter((line) => {
    const cls = line.className.toLowerCase();
    return cls.includes('mark-rule') && !cls.includes('axis');
  });

  const plot = findPlot(paths, dims);
  const markCircles = circles.filter((circle) => !String(circle.className ?? '').toLowerCase().includes('role-legend'));
  const legendCircles = circles.filter((circle) => String(circle.className ?? '').toLowerCase().includes('role-legend'));
  const markLabels = paths
    .filter((p) => p.ariaLabel && !p.className.toLowerCase().includes('background') && !p.className.toLowerCase().includes('role-legend'))
    .map((p) => p.ariaLabel);

  return {
    dims,
    paths,
    rects,
    lines,
    texts,
    circles: markCircles,
    legendCircles,
    markLabels,
    images,
    markGroups,
    facetCells,
    facetHeaders,
    shapes,
    symbolShapes,
    roundedClipPaths,
    clipRefs,
    bars,
    lineMarks,
    areaMarks,
    otherPaths,
    gridLines,
    tickLines,
    domainLines,
    rules,
    plot,
    counts: {
      path: paths.length,
      rect: rects.length,
      line: lines.length,
      text: texts.length,
      circle: markCircles.length,
      legendCircle: legendCircles.length,
      bar: bars.length,
      lineMark: lineMarks.length,
      areaMark: areaMarks.length,
      shape: shapes.length,
      symbolShape: symbolShapes.length,
      image: images.length,
      rule: rules.length,
      markGroup: markGroups.length,
      facetCell: facetCells.length,
      facetHeader: facetHeaders.length
    }
  };
}

function findPlot(paths, dims) {
  // The frame background is the first non-degenerate path in the root group,
  // typically `M0.5,0.5h<w>v<h>h-<w>Z` with class background.
  for (const item of paths) {
    const cls = item.className.toLowerCase();
    if (!cls.includes('background')) continue;
    const b = item.parsed.bbox;
    if (b && b.width > 1 && b.height > 1) {
      return {x1: b.x1, y1: b.y1, x2: b.x2, y2: b.y2, width: b.width, height: b.height};
    }
  }
  return {x1: 0, y1: 0, x2: dims.width, y2: dims.height, width: dims.width, height: dims.height};
}

function isMarkChrome(className) {
  const cls = String(className ?? '').toLowerCase();
  return cls.includes('background') || cls.includes('foreground') || cls.includes('role-legend');
}

function normalizeMarkClass(value) {
  const cls = String(value ?? '').toLowerCase();
  const layerMatch = cls.match(/(?:child_)?layer_(\d+)_marks/);
  if (layerMatch) return `layer_${layerMatch[1]}`;
  for (const token of ['geoshape', 'shape', 'image', 'symbol', 'point', 'line', 'trail', 'area', 'rect', 'bar', 'rule', 'text', 'arc']) {
    if (new RegExp(`(^|[^a-z])${token}([^a-z]|$)`).test(cls.replace(/role-mark/g, ' '))) return token;
  }
  return cls.replace(/\s+/g, ' ').trim();
}

function normalizeColor(value) {
  if (value == null) return null;
  const v = String(value).trim().toLowerCase();
  if (v === '' || v === 'none' || v === 'transparent') return null;
  return v.replace(/\s+/g, ' ');
}

// ---------------------------------------------------------------------------
// Matching helpers.

function center(mark) {
  return [mark.x + mark.w / 2, mark.y + mark.h / 2];
}

function distance(a, b) {
  return Math.hypot(a[0] - b[0], a[1] - b[1]);
}

function sortedBars(bars) {
  // Reading order: top-to-bottom then left-to-right. Used only when upstream
  // and rust counts match, so the pairing follows the mark order.
  return [...bars].sort((a, b) => (a.y - b.y) || (a.x - b.x));
}

function matchBars(up, rust) {
  const matched = [];
  const usedRust = new Set();
  for (const u of up) {
    const uc = center(u);
    let best = -1;
    let bestDist = Infinity;
    for (let j = 0; j < rust.length; j++) {
      if (usedRust.has(j)) continue;
      const d = distance(uc, center(rust[j]));
      if (d < bestDist) {
        bestDist = d;
        best = j;
      }
    }
    if (best >= 0 && bestDist <= BAR_NEAR) {
      usedRust.add(best);
      matched.push({up: u, rust: rust[best], dist: bestDist});
    }
  }
  return {matched, usedRust};
}

function pairByOrder(up, rust) {
  const a = sortedBars(up);
  const b = sortedBars(rust);
  const n = Math.min(a.length, b.length);
  const pairs = [];
  for (let i = 0; i < n; i++) pairs.push({up: a[i], rust: b[i]});
  return pairs;
}

function median(values) {
  if (!values.length) return 0;
  const sorted = [...values].sort((x, y) => x - y);
  return sorted[Math.floor(sorted.length / 2)];
}

function quantized(value, step = 1) {
  return Math.round(value / step) * step;
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

function pct(value, total) {
  return total === 0 ? 0 : value / total;
}

// ---------------------------------------------------------------------------
// Issue detection.

function issue(code, detail, extra = {}) {
  return {code, detail, ...extra};
}

function detectIssues(up, rust, name) {
  const issues = [];
  const add = (code, detail, extra) => issues.push(issue(code, detail, extra));

  if (!rust) return issues;

  if (up.counts.path === 0 && rust.counts.path === 0) {
    add('NO_ELEMENTS', 'both renders contain no paths');
    return issues;
  }
  if (rust.paths.length === 0 && up.paths.length > 0) {
    add('NO_ELEMENTS', `rust SVG has 0 paths, upstream has ${up.paths.length}`);
    return issues;
  }

  if (up.dims.width !== rust.dims.width || up.dims.height !== rust.dims.height) {
    add('DIMS_MISMATCH', `upstream ${up.dims.width}x${up.dims.height} vs rust ${rust.dims.width}x${rust.dims.height}`, {
      upstream: `${up.dims.width}x${up.dims.height}`,
      rust: `${rust.dims.width}x${rust.dims.height}`
    });
  }

  // --- zero-size / collapsed marks ---------------------------------------
  detectClipRadius(up, rust, add);
  for (const bar of rust.bars) {
    if (bar.w < 0.01 || bar.h < 0.01) {
      add('ZERO_SIZE_MARK', `bar #${bar.index} has size ${round(bar.w, 2)}x${round(bar.h, 2)} at (${round(bar.x, 1)},${round(bar.y, 1)})`, {
        index: bar.index,
        element: 'path',
        rust: `${round(bar.w, 2)}x${round(bar.h, 2)} @ ${round(bar.x, 1)},${round(bar.y, 1)}`
      });
      break;
    }
  }
  for (const item of rust.paths) {
    const cls = item.className.toLowerCase();
    if (cls.includes('background') || cls.includes('foreground')) continue;
    if (item.symbol) continue;
    if (item.parsed.moveOnly) {
      add('PATH_NO_SEGMENTS', `path #${item.index} contains only a move command "${truncate(item.parsed.d, 60)}"`, {
        index: item.index,
        element: 'path',
        rust: truncate(item.parsed.d, 120)
      });
      break;
    }
    if (item.parsed.bbox && item.parsed.bbox.width < 0.01 && item.parsed.bbox.height < 0.01 && item.parsed.subpaths.length === 1) {
      add('COLLAPSED_RECT', `path #${item.index} collapses to a point at (${round(item.parsed.bbox.x1, 1)},${round(item.parsed.bbox.y1, 1)})`, {
        index: item.index,
        element: 'path',
        rust: truncate(item.parsed.d, 120)
      });
      break;
    }
  }

  // --- mark totals --------------------------------------------------------
  const barDelta = up.bars.length - rust.bars.length;
  if (barDelta > 0) {
    const {missing} = multisetDelta(up.bars.map((b) => barKey(b)), rust.bars.map((b) => barKey(b)));
    add('BAR_MISSING', `${barDelta} bar(s) missing (${up.bars.length} upstream vs ${rust.bars.length} rust); first ${missing[0] ?? 'unknown'}`, {
      index: 0,
      element: 'path',
      upstream: up.bars.length,
      rust: rust.bars.length
    });
  } else if (barDelta < 0) {
    add('BAR_EXTRA', `${-barDelta} extra bar(s) (${up.bars.length} upstream vs ${rust.bars.length} rust)`, {
      index: 0,
      element: 'path',
      upstream: up.bars.length,
      rust: rust.bars.length
    });
  }

  // --- bar geometry: thickness, position, baseline, stacking -------------
  const zipBars = pairByOrder(up.bars, rust.bars);
  if (up.bars.length === rust.bars.length && zipBars.length > 0) {
    const dxs = [];
    const dys = [];
    const thickness = [];
    for (const {up: u, rust: r} of zipBars) {
      dxs.push(center(r)[0] - center(u)[0]);
      dys.push(center(r)[1] - center(u)[1]);
      const horizontal = u.w > u.h;
      const uSpan = horizontal ? u.h : u.w;
      const rSpan = horizontal ? r.h : r.w;
      if (uSpan > 1 && Math.abs(rSpan - uSpan) > Math.max(0.25, uSpan * 0.06)) {
        thickness.push({u, r, uSpan, rSpan});
      }
    }
    const dx = median(dxs);
    const dy = median(dys);
    const consistentX = pct(dxs.filter((v) => Math.abs(v - dx) < 2).length, dxs.length);
    const consistentY = pct(dys.filter((v) => Math.abs(v - dy) < 2).length, dys.length);
    if (Math.abs(dx) > 1 && consistentX > 0.7) {
      add('BAR_POSITION_SHIFT', `all bars shifted horizontally by ${round(dx, 2)}px`, {rust: round(dx, 2), upstream: 0});
    }
    if (Math.abs(dy) > 1 && consistentY > 0.7) {
      add('BAR_POSITION_SHIFT', `all bars shifted vertically by ${round(dy, 2)}px`, {rust: round(dy, 2), upstream: 0});
    }
    if (thickness.length > 0) {
      const first = thickness[0];
      add('BAR_THICKNESS', `${thickness.length}/${zipBars.length} bars have a different thickness; first #${first.r.index} upstream ${round(first.uSpan, 2)}px vs rust ${round(first.rSpan, 2)}px`, {
        index: first.r.index,
        element: 'path',
        upstream: round(first.uSpan, 2),
        rust: round(first.rSpan, 2)
      });
    }

    // Bars of the same size whose centers moved reveal a scale-range or
    // stack-baseline bug even when the shift is not uniform.
    const moved = zipBars.filter(({up: u, rust: r}) =>
      Math.abs(u.w - r.w) <= 0.6 &&
      Math.abs(u.h - r.h) <= 0.6 &&
      distance(center(u), center(r)) > 1.2
    );
    if (moved.length > Math.max(1, zipBars.length * 0.3) &&
        !issues.some((it) => it.code === 'BAR_POSITION_SHIFT')) {
      const first = moved[0];
      add('BAR_POSITION_SHIFT', `${moved.length}/${zipBars.length} same-size bars are displaced (e.g. ${round(distance(center(first.up), center(first.rust)), 1)}px)`, {
        index: first.rust.index,
        element: 'path',
        upstream: `${round(center(first.up)[0], 1)},${round(center(first.up)[1], 1)}`,
        rust: `${round(center(first.rust)[0], 1)},${round(center(first.rust)[1], 1)}`
      });
    }

    // Baseline / origin offset: compare the bar edge that should sit on the
    // zero baseline (bottom for vertical, left for horizontal bars).
    const vertical = up.bars.filter((b) => b.h >= b.w);
    const horizontal = up.bars.filter((b) => b.w > b.h);
    if (vertical.length > 2) {
      const upBottom = median(vertical.map((b) => b.y + b.h));
      const rustBottom = median(rust.bars.filter((b) => b.h >= b.w).map((b) => b.y + b.h));
      if (Math.abs(rustBottom - upBottom) > 0.75 && Math.abs(dy) <= 1) {
        add('BAR_BASELINE_OFFSET', `vertical bar baseline ${round(rustBottom, 2)} vs upstream ${round(upBottom, 2)}`, {
          upstream: round(upBottom, 2),
          rust: round(rustBottom, 2)
        });
      }
    }
    if (horizontal.length > 2) {
      const upLeft = median(horizontal.map((b) => b.x));
      const rustLeft = median(rust.bars.filter((b) => b.w > b.h).map((b) => b.x));
      if (Math.abs(rustLeft - upLeft) > 0.75 && Math.abs(dx) <= 1) {
        add('BAR_BASELINE_OFFSET', `horizontal bar origin ${round(rustLeft, 2)} vs upstream ${round(upLeft, 2)}`, {
          upstream: round(upLeft, 2),
          rust: round(rustLeft, 2)
        });
      }
    }

    detectStackReversal(up.bars, rust.bars, add);
    detectMirror(up.bars, rust.bars, add, 'BAR');
    detectBarSeriesMirror(up.bars, rust.bars, add);
    detectCornerRadius(up.bars, rust.bars, add);
    detectTouchingBars(up.bars, rust.bars, add);
  }

  // --- colors -------------------------------------------------------------
  const upFills = up.bars.map((b) => b.fill).filter(Boolean);
  const rustFills = rust.bars.map((b) => b.fill).filter(Boolean);
  if (upFills.length > 0 && rustFills.length > 0) {
    const {missing, extra} = multisetDelta(upFills, rustFills);
    if (missing.length || extra.length) {
      add('COLOR_MISMATCH', `bar fills differ; missing ${truncate(missing.join(', '), 60) || '-'} extra ${truncate(extra.join(', '), 60) || '-'}`, {
        index: 0,
        element: 'path',
        upstream: [...new Set(upFills)].sort().join(','),
        rust: [...new Set(rustFills)].sort().join(',')
      });
    } else if (zipBars.length === up.bars.length && up.bars.length === rust.bars.length) {
      const mismatches = zipBars.filter(({up: u, rust: r}) => u.fill !== r.fill);
      if (mismatches.length > Math.max(1, up.bars.length * 0.2)) {
        const first = mismatches[0];
        add('COLOR_MAPPING', `${mismatches.length}/${up.bars.length} bars use the wrong color; first #${first.rust.index} upstream ${first.up.fill} vs rust ${first.rust.fill}`, {
          index: first.rust.index,
          element: 'path',
          upstream: first.up.fill,
          rust: first.rust.fill
        });
      }
    }
  }

  // --- grid lines ---------------------------------------------------------
  compareLineSet(up.gridLines.filter(isVertical), rust.gridLines.filter(isVertical), 'GRID_LINE_MISSING', 'GRID_LINE_EXTRA', 'vertical grid line', add);
  compareLineSet(up.gridLines.filter(isHorizontal), rust.gridLines.filter(isHorizontal), 'GRID_LINE_MISSING', 'GRID_LINE_EXTRA', 'horizontal grid line', add);
  compareLineSet(up.tickLines, rust.tickLines, 'AXIS_TICK_MISSING', 'AXIS_TICK_EXTRA', 'axis tick', add);

  // --- text: missing / null / position / inside-vs-above ------------------
  detectTextIssues(up, rust, add);
  detectTitleIssues(up, rust, add);

  // --- line marks / invalid-data breaks -----------------------------------
  detectLineIssues(up, rust, add);

  // --- symbols / circles --------------------------------------------------
  detectSymbolIssues(up, rust, add);

  // --- batch 2: geo, images, facets, layers, histogram scale/order -------
  detectBatch2Issues(up, rust, name, add);
  detectBatch3Issues(up, rust, name, add);

  // --- area marks (invalid/filter charts) ---------------------------------
  const upTrail = up.areaMarks.filter((mark) => mark.className.includes('mark-trail')).length;
  const rustTrailLines = rust.lineMarks.filter((mark) => mark.className.includes('mark-trail')).length;
  if (upTrail > 0 && rustTrailLines > 0) {
    add('TRAIL_FILL_MISSING', `${rustTrailLines} trail mark(s) are stroked as thin lines instead of filled trail areas (upstream ${upTrail} filled trail mark(s))`, {
      element: 'path',
      upstream: `filled trail x${upTrail}`,
      rust: `stroked trail x${rustTrailLines}`
    });
  } else if (up.areaMarks.length !== rust.areaMarks.length) {
    add(
      up.areaMarks.length > rust.areaMarks.length ? 'AREA_MISSING' : 'AREA_EXTRA',
      `${Math.abs(up.areaMarks.length - rust.areaMarks.length)} area mark(s) differ (${up.areaMarks.length} upstream vs ${rust.areaMarks.length} rust)`,
      {element: 'path', upstream: up.areaMarks.length, rust: rust.areaMarks.length}
    );
  }

  // --- clipping / out-of-bounds ------------------------------------------
  detectClipping(up, rust, add);

  // --- axis label formatting / parsed time --------------------------------
  detectAxisLabelMismatch(up, rust, add);

  // --- fallback count signal when nothing semantic fired -----------------
  if (issues.length === 0) {
    for (const [kind, count] of Object.entries({
      path: up.counts.path - rust.counts.path,
      text: up.counts.text - rust.counts.text,
      line: up.counts.line - rust.counts.line
    })) {
      if (count !== 0) {
        add('COUNT_DIFF', `${kind} count ${up.counts[kind]} upstream vs ${rust.counts[kind]} rust`, {
          upstream: up.counts[kind],
          rust: rust.counts[kind]
        });
      }
    }
  }

  return issues;
}

function barKey(bar) {
  return `${round(bar.x, 1)},${round(bar.y, 1)},${round(bar.w, 1)},${round(bar.h, 1)}`;
}

function isVertical(line) {
  return Math.abs(line.x1 - line.x2) < 0.01 && Math.abs(line.y1 - line.y2) > 0.01;
}

function isHorizontal(line) {
  return Math.abs(line.y1 - line.y2) < 0.01 && Math.abs(line.x1 - line.x2) > 0.01;
}

function lineCoord(line, vertical) {
  return vertical ? line.x1 : line.y1;
}

function compareLineSet(upList, rustList, missingCode, extraCode, label, add) {
  if (upList.length === rustList.length) {
    // Same count but shifted positions is still a real problem for grid lines.
    const upCoords = upList.map((line) => lineCoord(line, isVertical(line))).sort((a, b) => a - b);
    const rustCoords = rustList.map((line) => lineCoord(line, isVertical(line))).sort((a, b) => a - b);
    let mismatches = 0;
    for (let i = 0; i < upCoords.length; i++) {
      if (Math.abs(upCoords[i] - rustCoords[i]) > 0.75) mismatches++;
    }
    if (mismatches > 0) {
      add('AXIS_LINE_POSITION', `${mismatches}/${upCoords.length} ${label}s are misplaced (e.g. ${round(upCoords[0], 2)} vs ${round(rustCoords[0], 2)})`, {
        element: 'line',
        upstream: round(upCoords[0], 2),
        rust: round(rustCoords[0], 2)
      });
    }
    return;
  }
  if (upList.length > rustList.length) {
    const upCoords = new Set(upList.map((line) => round(lineCoord(line, isVertical(line)), 2)));
    const rustCoords = new Set(rustList.map((line) => round(lineCoord(line, isVertical(line)), 2)));
    const missing = [...upCoords].filter((c) => !rustCoords.has(c));
    add(missingCode, `${upList.length - rustList.length} ${label}(s) missing (${upList.length} upstream vs ${rustList.length} rust); first ${missing[0] ?? '?'}, e.g. at ${round(missing[0] ?? upCoords.values().next().value, 2)}`, {
      element: 'line',
      upstream: upList.length,
      rust: rustList.length,
      index: 0
    });
  } else {
    const upCoords = new Set(upList.map((line) => round(lineCoord(line, isVertical(line)), 2)));
    const rustCoords = new Set(rustList.map((line) => round(lineCoord(line, isVertical(line)), 2)));
    const extra = [...rustCoords].filter((c) => !upCoords.has(c));
    add(extraCode, `${rustList.length - upList.length} extra ${label}(s) (${upList.length} upstream vs ${rustList.length} rust); first extra at ${round(extra[0] ?? 0, 2)}`, {
      element: 'line',
      upstream: upList.length,
      rust: rustList.length,
      index: 0
    });
  }
}

function textsByContent(texts) {
  const map = new Map();
  for (const text of texts) {
    const key = text.content;
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(text);
  }
  return map;
}

function insideBar(text, bars) {
  for (let i = 0; i < bars.length; i++) {
    const b = bars[i];
    if (text.x >= b.x - 0.5 && text.x <= b.x + b.w + 0.5 && text.y >= b.y - 0.5 && text.y <= b.y + b.h + 0.5) {
      return i;
    }
  }
  return -1;
}

function relationToBars(text, bars) {
  const inside = insideBar(text, bars);
  if (inside >= 0) return {kind: 'inside', bar: inside};
  // "above" means the text baseline sits just above the top edge of a bar
  // whose horizontal span contains the text.
  let best = -1;
  let bestGap = Infinity;
  for (let i = 0; i < bars.length; i++) {
    const b = bars[i];
    if (text.x < b.x - 2 || text.x > b.x + b.w + 2) continue;
    const gap = b.y - text.y; // positive when text is above the bar top
    if (gap >= -1 && gap < 24 && gap < bestGap) {
      bestGap = gap;
      best = i;
    }
  }
  if (best >= 0) return {kind: 'above', bar: best, gap: bestGap};
  return {kind: 'outside', bar: -1};
}

function detectTextIssues(up, rust, add) {
  const upMap = textsByContent(up.texts);
  const rustMap = textsByContent(rust.texts);

  for (const text of rust.texts) {
    if (/^null$/i.test(text.content) && !up.texts.some((t) => /^null$/i.test(t.content))) {
      add('NULL_TEXT', `text #${text.index} renders the literal "${text.content}" at (${round(text.x, 1)},${round(text.y, 1)})`, {
        index: text.index,
        element: 'text',
        rust: text.content
      });
      break;
    }
  }

  const upContents = [...upMap.keys()];
  const rustContents = [...rustMap.keys()];
  const upSet = new Set(upContents);
  const rustSet = new Set(rustContents);
  const missing = upContents.filter((c) => !rustSet.has(c));
  const extra = rustContents.filter((c) => !upSet.has(c));
  if (missing.length) {
    add('TEXT_MISSING', `${missing.length} label(s) missing; first "${truncate(missing[0], 40)}"`, {
      element: 'text',
      upstream: truncate(missing[0], 60)
    });
  }
  if (extra.length) {
    add('TEXT_EXTRA', `${extra.length} extra label(s); first "${truncate(extra[0], 40)}"`, {
      element: 'text',
      rust: truncate(extra[0], 60)
    });
  }

  // Position / inside-vs-above for texts present in both renders.
  const insideDiffs = [];
  const aboveDiffs = [];
  const shifts = [];
  for (const [content, upList] of upMap) {
    const rustList = rustMap.get(content);
    if (!rustList) continue;
    const count = Math.min(upList.length, rustList.length);
    for (let i = 0; i < count; i++) {
      const u = upList[i];
      const r = rustList[i];
      if (u.className.toLowerCase().includes('role-legend') || r.className.toLowerCase().includes('role-legend')) continue;
      const upRel = relationToBars(u, up.bars);
      const rustRel = relationToBars(r, rust.bars);
      if (upRel.kind !== rustRel.kind && upRel.kind !== 'outside' && rustRel.kind !== 'outside') {
        if (rustRel.kind === 'inside') insideDiffs.push({u, r});
        else if (rustRel.kind === 'above') aboveDiffs.push({u, r});
      }
      const d = Math.hypot(r.x - u.x, r.y - u.y);
      if (d > TEXT_NEAR) shifts.push({u, r, d});
    }
  }
  if (insideDiffs.length) {
    const first = insideDiffs[0];
    add('TEXT_INSIDE_BAR', `${insideDiffs.length} label(s) sit inside a bar instead of above it; first "${truncate(first.r.content, 30)}" at (${round(first.r.x, 1)},${round(first.r.y, 1)})`, {
      index: first.r.index,
      element: 'text',
      rust: `${round(first.r.x, 1)},${round(first.r.y, 1)}`,
      upstream: `${round(first.u.x, 1)},${round(first.u.y, 1)}`
    });
  }
  if (aboveDiffs.length) {
    const first = aboveDiffs[0];
    add('TEXT_ABOVE_BAR', `${aboveDiffs.length} label(s) sit above a bar instead of inside it; first "${truncate(first.r.content, 30)}"`, {
      index: first.r.index,
      element: 'text',
      rust: `${round(first.r.x, 1)},${round(first.r.y, 1)}`,
      upstream: `${round(first.u.x, 1)},${round(first.u.y, 1)}`
    });
  }
  if (shifts.length && missing.length === 0 && extra.length === 0) {
    const first = shifts[0];
    add('TEXT_POSITION_SHIFT', `${shifts.length} label(s) moved >${TEXT_NEAR}px; first "${truncate(first.r.content, 30)}" moved ${round(first.d, 1)}px`, {
      index: first.r.index,
      element: 'text',
      upstream: `${round(first.u.x, 1)},${round(first.u.y, 1)}`,
      rust: `${round(first.r.x, 1)},${round(first.r.y, 1)}`
    });
  }

  // Generic overlap count; title/weather labels are the common offender.
  const upOverlaps = countTextOverlaps(up.texts);
  const rustOverlaps = countTextOverlaps(rust.texts);
  if (rustOverlaps.length > upOverlaps.length + 1) {
    const first = rustOverlaps[0];
    add('TEXT_OVERLAP', `${rustOverlaps.length} pairs of texts overlap (upstream ${upOverlaps.length}); first "${truncate(first.a.content, 24)}" / "${truncate(first.b.content, 24)}"`, {
      index: first.a.index,
      element: 'text',
      rust: `${first.a.content} / ${first.b.content}`,
      upstream: upOverlaps.length
    });
  }
}

function countTextOverlaps(texts) {
  const overlaps = [];
  for (let i = 0; i < texts.length; i++) {
    const a = texts[i];
    if (a.content === '') continue;
    if (a.className.toLowerCase().includes('role-legend')) continue;
    for (let j = i + 1; j < texts.length; j++) {
      const b = texts[j];
      if (b.content === '' || a.content === b.content) continue;
      if (b.className.toLowerCase().includes('role-legend')) continue;
      const xOverlap = Math.min(a.bbox.x2, b.bbox.x2) - Math.max(a.bbox.x1, b.bbox.x1);
      const yOverlap = Math.min(a.bbox.y2, b.bbox.y2) - Math.max(a.bbox.y1, b.bbox.y1);
      if (xOverlap > 2 && yOverlap > 2) overlaps.push({a, b});
    }
  }
  return overlaps;
}

function detectTitleIssues(up, rust, add) {
  const titleOf = (svg) => {
    const candidates = svg.texts.filter((t) => t.fontSize >= 12);
    if (!candidates.length) return null;
    return [...candidates].sort((a, b) => (b.fontSize - a.fontSize) || (a.y - b.y))[0];
  };
  const upTitle = titleOf(up);
  const rustTitle = titleOf(rust);
  if (upTitle && !rustTitle) {
    add('TITLE_MISSING', `upstream title "${truncate(upTitle.content, 50)}" has no matching title text`, {
      element: 'text',
      upstream: truncate(upTitle.content, 60)
    });
    return;
  }
  if (!upTitle || !rustTitle) return;
  const norm = (value) => String(value).replace(/\s+/g, '');
  if (norm(upTitle.content) !== norm(rustTitle.content) && !rust.texts.some((t) => norm(t.content) === norm(upTitle.content))) {
    add('TITLE_MISMATCH', `title "${truncate(rustTitle.content, 50)}" vs upstream "${truncate(upTitle.content, 50)}"`, {
      index: rustTitle.index,
      element: 'text',
      upstream: truncate(upTitle.content, 60),
      rust: truncate(rustTitle.content, 60)
    });
  }
  // Title overlapping other text (e.g. weekday labels painted over it).
  const overlaps = countTextOverlaps([rustTitle, ...rust.texts.filter((t) => t !== rustTitle && t.content !== rustTitle.content)]);
  const titleOverlaps = overlaps.filter((p) => p.a === rustTitle || p.b === rustTitle);
  if (titleOverlaps.length > 0) {
    const other = titleOverlaps[0].a === rustTitle ? titleOverlaps[0].b : titleOverlaps[0].a;
    add('TITLE_OVERLAP', `title "${truncate(rustTitle.content, 30)}" overlaps text "${truncate(other.content, 30)}"`, {
      index: other.index,
      element: 'text',
      rust: `${rustTitle.content} / ${other.content}`
    });
  }
}

function detectLineIssues(up, rust, add) {
  const upLines = up.lineMarks.filter((line) => !line.className.includes('axis'));
  const rustLines = rust.lineMarks.filter((line) => !line.className.includes('axis'));

  const collapsed = rustLines.filter((line) => line.bbox && (line.bbox.width < 0.05 || line.bbox.height < 0.05));
  const upExpanded = upLines.filter((line) => line.bbox && line.bbox.width > 1 && line.bbox.height > 1);
  if (collapsed.length > 0 && upExpanded.length > 0) {
    const first = collapsed[0];
    add('LINE_COLLAPSED', `${collapsed.length} line path(s) collapse to a flat segment; first #${first.index} bbox ${round(first.bbox.width, 2)}x${round(first.bbox.height, 2)} at (${round(first.bbox.x1, 1)},${round(first.bbox.y1, 1)})`, {
      index: first.index,
      element: 'path',
      upstream: `${upExpanded.length} expanded line(s)`,
      rust: `${round(first.bbox.width, 2)}x${round(first.bbox.height, 2)}`
    });
  }

  if (upLines.length > 0 && rustLines.length === 0) {
    const upPoints = upLines.reduce((sum, line) => sum + line.path.pointCount ?? line.path.points.length, 0);
    if (upPoints > 2) {
      add('LINE_MISSING', `upstream has ${upLines.length} line/trail path(s) with ${upPoints} points, rust has none`, {
        element: 'path',
        upstream: upLines.length,
        rust: 0
      });
    }
    return;
  }
  if (upLines.length !== rustLines.length) {
    const code = upLines.length > rustLines.length ? 'LINE_MISSING' : 'LINE_EXTRA';
    add(code, `${Math.abs(upLines.length - rustLines.length)} line path(s) differ (${upLines.length} upstream vs ${rustLines.length} rust)`, {
      element: 'path',
      upstream: upLines.length,
      rust: rustLines.length
    });
  }

  // Compare subpath structure, which reveals missing breaks at invalid data.
  const upByColor = groupBy(upLines, (line) => line.stroke ?? 'none');
  const rustByColor = groupBy(rustLines, (line) => line.stroke ?? 'none');
  const upTotal = upLines.reduce((sum, line) => sum + line.subpaths, 0);
  const rustTotal = rustLines.reduce((sum, line) => sum + line.subpaths, 0);
  if (upLines.length === rustLines.length && upLines.length > 0) {
    const pairs = zipSortedLines(upLines, rustLines);
    const breaks = pairs.filter(({up: u, rust: r}) => r.subpaths < u.subpaths);
    if (breaks.length) {
      const first = breaks[0];
      add('PATH_NO_BREAK', `${breaks.length} line path(s) have fewer subpaths (no break at invalid data); first #${first.rust.index} upstream ${first.up.subpaths} vs rust ${first.rust.subpaths} subpaths`, {
        index: first.rust.index,
        element: 'path',
        upstream: first.up.subpaths,
        rust: first.rust.subpaths
      });
    }
    const extraBreaks = pairs.filter(({up: u, rust: r}) => r.subpaths > u.subpaths);
    if (extraBreaks.length && breaks.length === 0) {
      const first = extraBreaks[0];
      add('PATH_EXTRA_BREAK', `${extraBreaks.length} line path(s) have extra subpaths; first #${first.rust.index} upstream ${first.up.subpaths} vs rust ${first.rust.subpaths}`, {
        index: first.rust.index,
        element: 'path',
        upstream: first.up.subpaths,
        rust: first.rust.subpaths
      });
    }
  }
  if (upLines.length > 0 && rustTotal < upTotal && upLines.length !== rustLines.length) {
    add('PATH_NO_BREAK', `rust line paths have ${rustTotal} subpaths vs upstream ${upTotal} (missing breaks)`, {
      element: 'path',
      upstream: upTotal,
      rust: rustTotal
    });
  }
}

function zipSortedLines(up, rust) {
  const a = [...up].sort((x, y) => (x.bbox?.y1 ?? 0) - (y.bbox?.y1 ?? 0));
  const b = [...rust].sort((x, y) => (x.bbox?.y1 ?? 0) - (y.bbox?.y1 ?? 0));
  const pairs = [];
  for (let i = 0; i < Math.min(a.length, b.length); i++) pairs.push({up: a[i], rust: b[i]});
  return pairs;
}

function groupBy(list, keyFn) {
  const map = new Map();
  for (const item of list) {
    const key = keyFn(item);
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(item);
  }
  return map;
}

function detectSymbolIssues(up, rust, add) {
  const upCircles = up.circles;
  const rustCircles = rust.circles;

  if (upCircles.length > 0 && rustCircles.length === 0) {
    // A quantize/point chart rendered as bars instead of circles.
    if (rust.bars.length > 0) {
      add('WRONG_MARK_TYPE', `upstream draws ${upCircles.length} symbol(s), rust draws ${rust.bars.length} bar(s) instead`, {
        element: 'path',
        upstream: `circle x${upCircles.length}`,
        rust: `bar x${rust.bars.length}`
      });
    } else {
      add('SYMBOL_MISSING', `upstream draws ${upCircles.length} symbol(s), rust draws none`, {
        element: 'path',
        upstream: upCircles.length,
        rust: 0
      });
    }
    return;
  }
  if (upCircles.length !== rustCircles.length) {
    const code = upCircles.length > rustCircles.length ? 'SYMBOL_MISSING' : 'SYMBOL_EXTRA';
    add(code, `${Math.abs(upCircles.length - rustCircles.length)} symbol(s) differ (${upCircles.length} upstream vs ${rustCircles.length} rust)`, {
      element: 'path',
      upstream: upCircles.length,
      rust: rustCircles.length
    });
  }

  // Radius multiset; catches quantize size scales replaced by one size.
  const upRadii = upCircles.map((c) => round(c.r, 1));
  const rustRadii = rustCircles.map((c) => round(c.r, 1));
  if (upRadii.length && rustRadii.length && upRadii.length === rustRadii.length) {
    const upUnique = [...new Set(upRadii)].sort((a, b) => a - b);
    const rustUnique = [...new Set(rustRadii)].sort((a, b) => a - b);
    const same = upRadii.length === rustRadii.length &&
      [...upRadii].sort((a, b) => a - b).every((r, i) => Math.abs(r - [...rustRadii].sort((a, b) => a - b)[i]) <= 0.25);
    if (!same) {
      add('SYMBOL_SIZE_MISMATCH', `radii differ; upstream [${upUnique.join(', ')}] vs rust [${rustUnique.join(', ')}]`, {
        element: 'path',
        upstream: upUnique.join(','),
        rust: rustUnique.join(',')
      });
    }
  }

  // Arc flags.
  if (upCircles.length === rustCircles.length && upCircles.length > 0) {
    const flags = [];
    for (let i = 0; i < upCircles.length; i++) {
      const u = upCircles[i];
      const r = rustCircles[i];
      if (u.largeFlags.join(',') !== r.largeFlags.join(',') || u.sweepFlags.join(',') !== r.sweepFlags.join(',')) {
        flags.push({u, r});
      }
    }
    if (flags.length) {
      add('ARC_FLAG_MISMATCH', `${flags.length} symbol(s) have different large-arc/sweep flags; first #${flags[0].r.index}`, {
        index: flags[0].r.index,
        element: 'path',
        upstream: `large=${flags[0].u.largeFlags.join(',')} sweep=${flags[0].u.sweepFlags.join(',')}`,
        rust: `large=${flags[0].r.largeFlags.join(',')} sweep=${flags[0].r.sweepFlags.join(',')}`
      });
    }
  }

  // Overlap: symbols are too large or too close. Compare against upstream so
  // legitimately dense dot plots do not get flagged.
  const upOverlaps = countSymbolOverlaps(upCircles);
  const rustOverlaps = countSymbolOverlaps(rustCircles);
  if (rustOverlaps.length > upOverlaps.length) {
    const first = rustOverlaps[0];
    add('SYMBOL_OVERLAP', `${rustOverlaps.length} overlapping symbol pair(s) vs upstream ${upOverlaps.length}; first #${first.a.index}/#${first.b.index} distance ${round(first.d, 2)} < r1+r2 ${round(first.limit, 2)}`, {
      index: first.a.index,
      element: 'path',
      upstream: upOverlaps.length,
      rust: rustOverlaps.length
    });
  }

  if (upCircles.length === rustCircles.length && upCircles.length >= 5) {
    detectPointMirror(upCircles, rustCircles, add, 'SYMBOL');
  }
}

function detectPointMirror(up, rust, add, prefix) {
  const mid = (values) => (Math.min(...values) + Math.max(...values)) / 2;
  const midX = mid(up.map((c) => c.cx));
  const midY = mid(up.map((c) => c.cy));
  const mirrorX = up.filter((c, i) => Math.abs(rust[i].cx - (2 * midX - c.cx)) < 2).length / up.length;
  const mirrorY = up.filter((c, i) => Math.abs(rust[i].cy - (2 * midY - c.cy)) < 2).length / up.length;
  const directX = up.filter((c, i) => Math.abs(rust[i].cx - c.cx) < 2).length / up.length;
  const directY = up.filter((c, i) => Math.abs(rust[i].cy - c.cy) < 2).length / up.length;
  if (mirrorX > 0.8 && directX < 0.2) {
    add(`${prefix}_AXIS_REVERSED`, `${Math.round(mirrorX * 100)}% of symbols mirrored along x (x scale reversed)`, {
      element: 'path',
      upstream: 'ascending',
      rust: 'descending'
    });
  } else if (mirrorY > 0.8 && directY < 0.2) {
    add(`${prefix}_AXIS_REVERSED`, `${Math.round(mirrorY * 100)}% of symbols mirrored along y (y scale reversed)`, {
      element: 'path',
      upstream: 'ascending',
      rust: 'descending'
    });
  }
}

function countSymbolOverlaps(circles) {
  if (circles.length > 3000) return [];
  const overlaps = [];
  for (let i = 0; i < circles.length; i++) {
    const a = circles[i];
    for (let j = i + 1; j < circles.length; j++) {
      const b = circles[j];
      const d = Math.hypot(a.cx - b.cx, a.cy - b.cy);
      const limit = a.r + b.r;
      if (d < limit - 0.5) overlaps.push({a, b, d, limit});
    }
  }
  return overlaps;
}

function detectStackReversal(up, rust, add) {
  const orientation = dominantOrientation(up);
  if (!orientation) return;
  const keyOf = (bar) => (orientation === 'vertical' ? quantized(bar.x, 1) : quantized(bar.y, 1));
  const spanOf = (bar) => (orientation === 'vertical' ? bar.h : bar.w);
  const perpOf = (bar) => (orientation === 'vertical' ? bar.y : bar.x);

  const upGroups = groupBy(up, keyOf);
  const rustGroups = groupBy(rust, keyOf);
  const reversals = [];
  for (const [key, upGroup] of upGroups) {
    const rustGroup = rustGroups.get(key);
    if (!rustGroup || upGroup.length < 2 || rustGroup.length !== upGroup.length) continue;
    const upSpans = [...upGroup].sort((a, b) => perpOf(a) - perpOf(b)).map((b) => round(spanOf(b), 1));
    const rustSpans = [...rustGroup].sort((a, b) => perpOf(a) - perpOf(b)).map((b) => round(spanOf(b), 1));
    if (upSpans.join(',') === [...rustSpans].reverse().join(',') && upSpans.join(',') !== rustSpans.join(',')) {
      reversals.push({key, upSpans, rustSpans});
    }
  }
  if (reversals.length > Math.max(1, upGroups.size * 0.3)) {
    add('STACK_REVERSED', `${reversals.length} stacked group(s) have reversed order; first at ${round(reversals[0].key, 1)} upstream [${reversals[0].upSpans.join(', ')}] vs rust [${reversals[0].rustSpans.join(', ')}]`, {
      element: 'path',
      upstream: reversals[0].upSpans.join(','),
      rust: reversals[0].rustSpans.join(',')
    });
  }
}

function dominantOrientation(bars) {
  let vertical = 0;
  let horizontal = 0;
  for (const b of bars) {
    if (b.h >= b.w) vertical++;
    else horizontal++;
  }
  if (vertical === 0 && horizontal === 0) return null;
  return vertical >= horizontal ? 'vertical' : 'horizontal';
}

function detectBarSeriesMirror(up, rust, add) {
  if (up.length < 4 || rust.length !== up.length) return;
  if (dominantOrientation(up) !== 'horizontal') return;
  const horizontal = true;
  const plotMid = (bars) => {
    const centers = bars.map((b) => (horizontal ? b.x + b.w / 2 : b.y + b.h / 2));
    return (Math.min(...centers) + Math.max(...centers)) / 2;
  };
  const upMid = plotMid(up);
  const rustMid = plotMid(rust);
  const sideOf = (bar, mid) => ((horizontal ? bar.x + bar.w / 2 : bar.y + bar.h / 2) < mid ? 'a' : 'b');
  const seq = (bars, mid) => {
    const groups = new Map([['a', []], ['b', []]]);
    for (const bar of bars) groups.get(sideOf(bar, mid)).push(bar);
    const out = {};
    for (const [key, list] of groups) {
      out[key] = list
        .sort((p, q) => (horizontal ? p.y - q.y : p.x - q.x))
        .map((b) => (horizontal ? b.w : b.h));
    }
    return out;
  };
  const upSeq = seq(up, upMid);
  const rustSeq = seq(rust, rustMid);
  for (const key of ['a', 'b']) {
    const a = upSeq[key];
    const b = rustSeq[key];
    if (a.length < 2 || a.length !== b.length) continue;
    const span = Math.max(...a) - Math.min(...a);
    if (span < 3) continue;
    const reversed = [...b].reverse();
    const close = a.filter((v, i) => Math.abs(v - reversed[i]) <= Math.max(0.5, span * 0.05)).length;
    const same = a.filter((v, i) => Math.abs(v - b[i]) <= Math.max(0.5, span * 0.05)).length;
    if (close / a.length > 0.8 && same / a.length <= 0.8) {
      add('BAR_AXIS_REVERSED', `bar width profile is vertically mirrored on the ${key === 'a' ? 'left' : 'right'} side (${Math.round((close / a.length) * 100)}% reversed)`, {
        element: 'path',
        upstream: truncate(a.slice(0, 5).map((v) => round(v, 1)).join(','), 80),
        rust: truncate(b.slice(0, 5).map((v) => round(v, 1)).join(','), 80)
      });
      return;
    }
  }
}

function detectCornerRadius(up, rust, add) {
  const upCurved = up.filter((b) => b.curved).length;
  const rustCurved = rust.filter((b) => b.curved).length;
  if (upCurved > 0 && rustCurved < upCurved) {
    add('CORNER_RADIUS_MISSING', `${upCurved - rustCurved}/${upCurved} rounded bar(s) rendered with square corners`, {
      element: 'path',
      upstream: `rounded x${upCurved}`,
      rust: `rounded x${rustCurved}`
    });
  }
}

function detectClipRadius(up, rust, add) {
  if (up.roundedClipPaths > 0 && rust.roundedClipPaths < up.roundedClipPaths) {
    add('CORNER_RADIUS_MISSING', `upstream clips ${up.roundedClipPaths} bar(s) to a rounded shape; rust has ${rust.roundedClipPaths} rounded clip(s) (square bars)`, {
      element: 'clipPath',
      upstream: up.roundedClipPaths,
      rust: rust.roundedClipPaths
    });
  }
}

function detectTouchingBars(up, rust, add) {
  const gapsOf = (bars) => {
    const orientation = dominantOrientation(bars);
    if (!orientation) return [];
    const vertical = orientation === 'vertical';
    const sorted = [...bars].filter((b) => (vertical ? b.h >= b.w : b.w > b.h))
      .sort((a, b) => (vertical ? a.x - b.x : a.y - b.y));
    const gaps = [];
    for (let i = 0; i < sorted.length - 1; i++) {
      const a = sorted[i];
      const b = sorted[i + 1];
      if (vertical) {
        const overlap = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
        if (overlap < Math.min(a.h, b.h) * 0.5) continue;
        gaps.push(b.x - (a.x + a.w));
      } else {
        const overlap = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
        if (overlap < Math.min(a.w, b.w) * 0.5) continue;
        gaps.push(b.y - (a.y + a.h));
      }
    }
    return gaps;
  };
  const upGaps = gapsOf(up);
  const rustGaps = gapsOf(rust);
  const touching = (gaps) => gaps.filter((g) => g > -1 && g < 0.05).length;
  if (rustGaps.length && upGaps.length && touching(rustGaps) > touching(upGaps)) {
    add('BARS_TOUCHING', `${touching(rustGaps)} adjacent bar pair(s) touch with no gap (upstream ${touching(upGaps)})`, {
      element: 'path',
      upstream: touching(upGaps),
      rust: touching(rustGaps)
    });
  }
}

function detectMirror(up, rust, add, prefix) {
  if (up.length !== rust.length || up.length < 3) return;
  const pairs = [];
  // Match in reading order to avoid nearest-neighbour ambiguity with mirrored
  // axes, where every mark's nearest neighbor can be itself.
  const a = [...up].sort((x, y) => (x.x - y.x) || (x.y - y.y));
  const b = [...rust].sort((x, y) => (x.x - y.x) || (x.y - y.y));
  for (let i = 0; i < a.length; i++) pairs.push({u: a[i], r: b[i]});
  const xs = pairs.map(({u, r}) => [center(u)[0], center(r)[0]]);
  const ys = pairs.map(({u, r}) => [center(u)[1], center(r)[1]]);
  const sized = pairs.filter(({u, r}) => Math.abs(u.w - r.w) <= 1 && Math.abs(u.h - r.h) <= 1).length / pairs.length;
  if (sized < 0.8) return;
  const midX = (Math.min(...xs.map((p) => p[0])) + Math.max(...xs.map((p) => p[0]))) / 2;
  const midY = (Math.min(...ys.map((p) => p[0])) + Math.max(...ys.map((p) => p[0]))) / 2;
  const mirrorX = xs.filter(([ux, rx]) => Math.abs(rx - (2 * midX - ux)) < 2).length / xs.length;
  const mirrorY = ys.filter(([uy, ry]) => Math.abs(ry - (2 * midY - uy)) < 2).length / ys.length;
  const directX = xs.filter(([ux, rx]) => Math.abs(rx - ux) < 2).length / xs.length;
  const directY = ys.filter(([uy, ry]) => Math.abs(ry - uy) < 2).length / ys.length;
  if (process.env.DEBUG_MIRROR) console.error('mirror', prefix, mirrorX, mirrorY, directX, directY, midX, midY);
  if (mirrorX > 0.8 && directX < 0.2) {
    add(`${prefix}_AXIS_REVERSED`, `${Math.round(mirrorX * 100)}% of marks are mirrored along x (x scale reversed)`, {
      element: 'path',
      upstream: 'ascending',
      rust: 'descending'
    });
  } else if (mirrorY > 0.8 && directY < 0.2) {
    add(`${prefix}_AXIS_REVERSED`, `${Math.round(mirrorY * 100)}% of marks are mirrored along y (y scale reversed)`, {
      element: 'path',
      upstream: 'ascending',
      rust: 'descending'
    });
  }
}

const TIME_LABEL_RE = /^(\d{1,2}:\d{2}(:\d{2})?(\s*[AP]M)?|\d{1,4}[-/]\d{1,2}|\d{4}|[A-Z][a-z]{2}( \d{1,2})?)$/;

function markKindCounts(svg) {
  const kinds = [];
  if (svg.bars.length) kinds.push(`bar:${svg.bars.length}`);
  if (svg.lineMarks.length) kinds.push(`line:${svg.lineMarks.length}`);
  if (svg.areaMarks.length) kinds.push(`area:${svg.areaMarks.length}`);
  const symbols = svg.circles.length + (svg.symbolShapes ? svg.symbolShapes.length : 0);
  if (symbols) kinds.push(`symbol:${symbols}`);
  if (svg.shapes.length) kinds.push(`shape:${svg.shapes.length}`);
  if (svg.images.length) kinds.push(`image:${svg.images.length}`);
  if (svg.rules.length) kinds.push(`rule:${svg.rules.length}`);
  return kinds;
}

function unionMarkBbox(marks) {
  let box = null;
  for (const mark of marks) {
    if (mark.bbox) box = unionBbox(box, mark.bbox);
  }
  return box;
}

function bboxDiff(a, b) {
  if (!a || !b) return Infinity;
  return Math.max(Math.abs(a.x1 - b.x1), Math.abs(a.y1 - b.y1), Math.abs(a.x2 - b.x2), Math.abs(a.y2 - b.y2));
}

function detectGeoIssues(up, rust, name, add) {
  const isGeo = /^geo_/.test(name) || up.shapes.length > 0;
  if (!isGeo) return;
  if (up.shapes.length > 0 && rust.shapes.length === 0) {
    add('GEO_GEOMETRY_MISSING', `upstream renders ${up.shapes.length} geoshape path(s), rust renders none`, {
      element: 'path',
      upstream: up.shapes.length,
      rust: 0
    });
  } else if (up.shapes.length > 0 && rust.shapes.length > 0) {
    const upValid = up.shapes.filter((s) => !s.moveOnly && s.bbox && s.bbox.width > 0.5 && s.bbox.height > 0.5);
    const rustValid = rust.shapes.filter((s) => !s.moveOnly && s.bbox && s.bbox.width > 0.5 && s.bbox.height > 0.5);
    const invalid = rust.shapes.filter((s) => s.moveOnly || !s.bbox || s.bbox.width < 0.5 || s.bbox.height < 0.5);
    if (invalid.length > 0 && upValid.length > 0) {
      add('GEO_PATH_INVALID', `${invalid.length}/${rust.shapes.length} geoshape path(s) have no area; first #${invalid[0].index} is ${invalid[0].moveOnly ? 'move-only' : 'empty bbox'}`, {
        index: invalid[0].index,
        element: 'path',
        rust: truncate(invalid[0].path.d, 100)
      });
    }
    const upPoints = up.shapes.reduce((sum, s) => sum + s.points, 0);
    const rustPoints = rust.shapes.reduce((sum, s) => sum + s.points, 0);
    if (upPoints > 20 && rustPoints < upPoints * 0.5) {
      add('GEO_GEOMETRY_MISSING', `geoshape detail collapsed: ${rustPoints} path points vs upstream ${upPoints}`, {
        element: 'path',
        upstream: upPoints,
        rust: rustPoints
      });
    }
    const upBox = unionMarkBbox(upValid);
    const rustBox = unionMarkBbox(rustValid);
    if (upBox && rustBox && bboxDiff(upBox, rustBox) > Math.max(2, upBox.width * 0.02, upBox.height * 0.02)) {
      add('GEO_PROJECTION_MISMATCH', `geoshape extent differs: rust ${round(rustBox.width, 1)}x${round(rustBox.height, 1)} at (${round(rustBox.x1, 1)},${round(rustBox.y1, 1)}) vs upstream ${round(upBox.width, 1)}x${round(upBox.height, 1)} at (${round(upBox.x1, 1)},${round(upBox.y1, 1)})`, {
        element: 'path',
        upstream: `${round(upBox.width, 1)}x${round(upBox.height, 1)}`,
        rust: `${round(rustBox.width, 1)}x${round(rustBox.height, 1)}`
      });
    }
  }

  // Symbol-based geo layers (geo_circle): compare projected point extent.
  if (up.circles.length > 0 && rust.circles.length > 0 && /^geo_/.test(name)) {
    const nonZero = (circles) => circles.filter((c) => Math.abs(c.cx) > 0.5 || Math.abs(c.cy) > 0.5).length;
    const upNZ = nonZero(up.circles);
    const rustNZ = nonZero(rust.circles);
    if (upNZ > 10 && rustNZ < upNZ * 0.5) {
      add('GEO_PROJECTION_MISMATCH', `projection places ${rustNZ}/${rust.circles.length} symbols at a real coordinate; upstream ${upNZ}/${up.circles.length} (most points collapse to 0,0)`, {
        element: 'path',
        upstream: upNZ,
        rust: rustNZ
      });
    }
    const upBox = unionBbox(up.circles.reduce((box, c) => unionBbox(box, {x1: c.cx, y1: c.cy, x2: c.cx, y2: c.cy, width: 0, height: 0}), null));
    const rustBox = unionBbox(rust.circles.reduce((box, c) => unionBbox(box, {x1: c.cx, y1: c.cy, x2: c.cx, y2: c.cy, width: 0, height: 0}), null));
    if (upBox && rustBox && bboxDiff(upBox, rustBox) > Math.max(3, upBox.width * 0.03, upBox.height * 0.03)) {
      add('GEO_PROJECTION_MISMATCH', `projected symbol extent differs: rust x[${round(rustBox.x1, 1)},${round(rustBox.x2, 1)}] y[${round(rustBox.y1, 1)},${round(rustBox.y2, 1)}] vs upstream x[${round(upBox.x1, 1)},${round(upBox.x2, 1)}] y[${round(upBox.y1, 1)},${round(upBox.y2, 1)}]`, {
        element: 'path',
        upstream: `${round(upBox.x1, 1)}..${round(upBox.x2, 1)}`,
        rust: `${round(rustBox.x1, 1)}..${round(rustBox.x2, 1)}`
      });
    }
  }
}

function detectImageIssues(up, rust, add) {
  if (up.images.length === 0) return;
  if (rust.images.length === 0) {
    const other = rust.circles.length ? `symbols x${rust.circles.length}` : rust.bars.length ? `bars x${rust.bars.length}` : 'nothing';
    add('IMAGE_MISSING', `upstream renders ${up.images.length} image mark(s), rust renders ${other}`, {
      element: 'image',
      upstream: `image x${up.images.length}`,
      rust: other
    });
    return;
  }
  if (rust.images.length !== up.images.length) {
    add(rust.images.length < up.images.length ? 'IMAGE_MISSING' : 'IMAGE_EXTRA', `${Math.abs(up.images.length - rust.images.length)} image mark(s) differ (${up.images.length} upstream vs ${rust.images.length} rust)`, {
      element: 'image',
      upstream: up.images.length,
      rust: rust.images.length
    });
  }
  const n = Math.min(up.images.length, rust.images.length);
  for (let i = 0; i < n; i++) {
    const u = up.images[i];
    const r = rust.images[i];
    if (!r.href) {
      add('IMAGE_HREF_MISSING', `image #${i} has no href/url`, {index: i, element: 'image', rust: 'null', upstream: u.href});
      return;
    }
    if ((u.w > 0 || u.h > 0) && (r.w <= 0.01 || r.h <= 0.01)) {
      add('IMAGE_ZERO_SIZE', `image #${i} renders at ${round(r.w, 1)}x${round(r.h, 1)} (upstream ${round(u.w, 1)}x${round(u.h, 1)})`, {
        index: i,
        element: 'image',
        upstream: `${round(u.w, 1)}x${round(u.h, 1)}`,
        rust: `${round(r.w, 1)}x${round(r.h, 1)}`
      });
      return;
    }
    if (Math.abs(u.w - r.w) > 1.5 || Math.abs(u.h - r.h) > 1.5) {
      add('IMAGE_SIZE_MISMATCH', `image #${i} size ${round(r.w, 1)}x${round(r.h, 1)} vs upstream ${round(u.w, 1)}x${round(u.h, 1)}`, {
        index: i,
        element: 'image',
        upstream: `${round(u.w, 1)}x${round(u.h, 1)}`,
        rust: `${round(r.w, 1)}x${round(r.h, 1)}`
      });
      return;
    }
    if (u.href && r.href && !u.href.endsWith(r.href.replace(/^\.\//, ''))) {
      add('IMAGE_HREF_MISMATCH', `image #${i} href "${truncate(r.href, 50)}" vs upstream "${truncate(u.href, 50)}"`, {
        index: i,
        element: 'image',
        upstream: truncate(u.href, 80),
        rust: truncate(r.href, 80)
      });
      return;
    }
  }
}

function detectLayerIssues(up, rust, name, add) {
  if (!/^(layer_|facet_|concat_|line_concat_facet)/.test(name)) return;
  const upKinds = markKindCounts(up);
  const rustKinds = markKindCounts(rust);
  const countMap = (kinds) => {
    const map = new Map();
    for (const kind of kinds) {
      const [type, count] = kind.split(':');
      map.set(type, Number(count));
    }
    return map;
  };
  const upMap = countMap(upKinds);
  const rustMap = countMap(rustKinds);
  for (const [type, count] of upMap) {
    if (!rustMap.has(type)) {
      add('LAYER_MARK_MISSING', `layer mark type "${type}" missing (upstream ${count} mark(s))`, {
        element: type,
        upstream: `${type} x${count}`,
        rust: 'none'
      });
    }
  }
  for (const [type, count] of rustMap) {
    if (!upMap.has(type) && type !== 'area') {
      add('LAYER_MARK_EXTRA', `extra layer mark type "${type}" (rust ${count} mark(s))`, {
        element: type,
        upstream: 'none',
        rust: `${type} x${count}`
      });
    }
  }
  if (up.rules.length > 0 && rust.rules.length > 0 && up.rules.length === rust.rules.length) {
    const upRules = [...up.rules].sort((a, b) => a.x1 - b.x1);
    const rustRules = [...rust.rules].sort((a, b) => a.x1 - b.x1);
    const moved = upRules.filter((r, i) => Math.abs(r.x1 - rustRules[i].x1) > 1 || Math.abs(r.y1 - rustRules[i].y1) > 1).length;
    if (moved > 0) {
      add('LAYER_RULE_MISMATCH', `${moved}/${upRules.length} rule layer(s) misplaced; first at x=${round(rustRules[0].x1, 1)} vs upstream ${round(upRules[0].x1, 1)}`, {
        element: 'line',
        upstream: `${round(upRules[0].x1, 1)},${round(upRules[0].y1, 1)}`,
        rust: `${round(rustRules[0].x1, 1)},${round(rustRules[0].y1, 1)}`
      });
    }
  }
}

function detectFacetIssues(up, rust, name, add) {
  const isFacet = /^facet_|^concat_|^line_concat/.test(name) || up.facetCells.length >= 2 || up.facetHeaders.length > 0;
  if (!isFacet) return;
  const titles = (svg) => svg.texts
    .filter((t) => t.className.toLowerCase().includes('title'))
    .map((t) => t.content)
    .filter((c) => c !== '' && !/^[\d.,%−-]+$/.test(c));
  const upTitles = titles(up);
  const rustTitles = titles(rust);
  const norm = (value) => String(value).replace(/\s+/g, ' ').trim();
  if (upTitles.length >= 2 && rustTitles.length >= 2) {
    const same = upTitles.length === rustTitles.length && upTitles.every((t, i) => norm(t) === norm(rustTitles[i]));
    if (!same) {
      const first = upTitles.findIndex((t, i) => norm(t) !== norm(rustTitles[i] ?? ''));
      add('FACET_ORDER_MISMATCH', `facet/header order differs at #${first}: upstream "${truncate(upTitles[first] ?? '?', 40)}" vs rust "${truncate(rustTitles[first] ?? '?', 40)}"; upstream order [${truncate(upTitles.join(', '), 120)}]`, {
        index: first,
        element: 'text',
        upstream: truncate(upTitles.join(', '), 180),
        rust: truncate(rustTitles.join(', '), 180)
      });
    }
  }
  if (up.facetCells.length > 0 && rust.facetCells.length === 0) {
    add('FACET_LAYOUT_MISMATCH', `upstream has ${up.facetCells.length} facet cell(s), rust has none`, {
      element: 'g',
      upstream: up.facetCells.length,
      rust: 0
    });
  } else if (up.facetCells.length > 0 && up.facetCells.length !== rust.facetCells.length) {
    add('FACET_LAYOUT_MISMATCH', `${Math.abs(up.facetCells.length - rust.facetCells.length)} facet cell(s) differ (${up.facetCells.length} upstream vs ${rust.facetCells.length} rust)`, {
      element: 'g',
      upstream: up.facetCells.length,
      rust: rust.facetCells.length
    });
  }
  if (up.facetHeaders.length > 0 && rust.facetHeaders.length === 0) {
    add('FACET_HEADER_MISSING', `upstream has ${up.facetHeaders.length} facet header group(s), rust has none`, {
      element: 'g',
      upstream: up.facetHeaders.length,
      rust: 0
    });
  }
}

function detectHistogramIssues(up, rust, name, add) {
  if (!/^histogram_/.test(name)) return;
  if (up.bars.length > 0 && rust.bars.length > 0 && up.bars.length === rust.bars.length) {
    const a = [...up.bars].sort((p, q) => p.x - q.x);
    const b = [...rust.bars].sort((p, q) => p.x - q.x);
    const upHeights = a.map((bar) => round(bar.h, 1));
    const rustHeights = b.map((bar) => round(bar.h, 1));
    const upSorted = [...upHeights].sort((x, y) => x - y);
    const rustSorted = [...rustHeights].sort((x, y) => x - y);
    const sameMultiset = upSorted.length === rustSorted.length && upSorted.every((v, i) => Math.abs(v - rustSorted[i]) <= Math.max(0.5, Math.abs(v) * 0.02));
    if (sameMultiset && upHeights.join(',') !== rustHeights.join(',')) {
      const first = upHeights.findIndex((v, i) => Math.abs(v - rustHeights[i]) > 0.5);
      add('BAR_ORDER_MISMATCH', `histogram bars are in the wrong category order; first difference at sorted-x #${first}: upstream height ${upHeights[first]} vs rust ${rustHeights[first]}`, {
        index: b[first]?.index ?? 0,
        element: 'path',
        upstream: upHeights.join(','),
        rust: rustHeights.join(',')
      });
    }
  }
}

function detectScaleTypeIssues(up, rust, name, add) {
  const numeric = (svg) => svg.texts
    .filter((t) => t.className.toLowerCase().includes('axis-label') || /^-?[\d,.]+$/.test(t.content))
    .map((t) => Number(t.content.replace(/[,%]/g, '')))
    .filter((n) => Number.isFinite(n) && n > 0);
  const upNums = [...new Set(numeric(up))].sort((a, b) => a - b);
  const rustNums = [...new Set(numeric(rust))].sort((a, b) => a - b);
  const isLog = (values) => {
    const powers = values.filter((v) => {
      const l = Math.log10(v);
      return Math.abs(l - Math.round(l)) < 0.01;
    });
    return powers.length >= 5;
  };
  if (isLog(upNums) && !isLog(rustNums)) {
    add('SCALE_TYPE_MISMATCH', `upstream axis uses a log scale (${truncate(upNums.join(', '), 60)}); rust ticks are not logarithmic (${truncate(rustNums.join(', '), 60)})`, {
      element: 'text',
      upstream: truncate(upNums.join(','), 80),
      rust: truncate(rustNums.join(','), 80)
    });
  }
  // Quantile/threshold colour/size scales: compare symbol colour sets.
  if (/^circle_scale_/.test(name) && up.circles.length && rust.circles.length) {
    if (up.legendCircles.length > rust.legendCircles.length) {
      add('SYMBOL_MISSING', `threshold/quantile legend has ${rust.legendCircles.length} symbol(s) vs upstream ${up.legendCircles.length}`, {
        element: 'path',
        upstream: up.legendCircles.length,
        rust: rust.legendCircles.length
      });
    }
    const upColors = up.circles.map((c) => c.fill).filter(Boolean);
    const rustColors = rust.circles.map((c) => c.fill).filter(Boolean);
    const {missing, extra} = multisetDelta(upColors, rustColors);
    if (missing.length || extra.length) {
      add('SYMBOL_COLOR_MISMATCH', `symbol colours differ; missing ${truncate(missing.join(', '), 60) || '-'} extra ${truncate(extra.join(', '), 60) || '-'}`, {
        element: 'path',
        upstream: truncate([...new Set(upColors)].join(','), 100),
        rust: truncate([...new Set(rustColors)].join(','), 100)
      });
    }
  }
}

function detectLineColorIssues(up, rust, add) {
  const upColors = up.lineMarks.filter((l) => l.stroke).map((l) => l.stroke);
  const rustColors = rust.lineMarks.filter((l) => l.stroke).map((l) => l.stroke);
  if (upColors.length > 0) {
    const {missing, extra} = multisetDelta(upColors, rustColors);
    if (missing.length || extra.length) {
      add('LINE_COLOR_MISMATCH', `line strokes differ; missing ${truncate([...new Set(missing)].join(', '), 70) || '-'} extra ${truncate([...new Set(extra)].join(', '), 70) || '-'}`, {
        element: 'path',
        upstream: truncate([...new Set(upColors)].join(','), 120),
        rust: truncate([...new Set(rustColors)].join(','), 120)
      });
    }
  }
  const upFills = up.lineMarks.filter((l) => l.fill).map((l) => l.fill);
  const rustFills = rust.lineMarks.filter((l) => l.fill).map((l) => l.fill);
  if (upFills.length > 0) {
    const {missing, extra} = multisetDelta(upFills, rustFills);
    if (missing.length || extra.length) {
      add('LINE_FILL_MISMATCH', `filled line/area marks lost colors; missing ${truncate([...new Set(missing)].join(', '), 70) || '-'} extra ${truncate([...new Set(extra)].join(', '), 70) || '-'}`, {
        element: 'path',
        upstream: truncate([...new Set(upFills)].join(','), 120),
        rust: truncate([...new Set(rustFills)].join(','), 120)
      });
    }
  }
}

function detectAxisTickOrder(up, rust, add, name = '') {
  if (/^(facet_|concat_|geo_)/.test(name)) return;
  const labels = (svg) => svg.texts.filter((t) => t.className.toLowerCase().includes('axis-label') && /^-?[\d,.]+$/.test(t.content));
  const upLabels = labels(up);
  const rustLabels = labels(rust);
  if (upLabels.length < 3 || rustLabels.length < 3) return;
  const groups = (list) => {
    const horizontal = list.filter((t) => t.y > up.plot.y2);
    const vertical = list.filter((t) => t.x < up.plot.x1);
    return {horizontal, vertical};
  };
  const upGroups = groups(upLabels);
  const rustGroups = groups(rustLabels);
  for (const key of ['horizontal', 'vertical']) {
    const a = upGroups[key];
    const b = rustGroups[key];
    if (a.length < 3 || b.length < 3 || a.length !== b.length) continue;
    const aVals = [...a].sort((p, q) => (key === 'horizontal' ? p.x - q.x : p.y - q.y)).map((t) => t.content);
    const bVals = [...b].sort((p, q) => (key === 'horizontal' ? p.x - q.x : p.y - q.y)).map((t) => t.content);
    const setA = [...aVals].sort().join(',');
    const setB = [...bVals].sort().join(',');
    if (setA === setB && aVals.join(',') !== bVals.join(',')) {
      add('AXIS_REVERSED', `${key} axis tick order is reversed: upstream [${truncate(aVals.join(', '), 70)}] vs rust [${truncate(bVals.join(', '), 70)}]`, {
        element: 'text',
        upstream: truncate(aVals.join(','), 100),
        rust: truncate(bVals.join(','), 100)
      });
    }
  }
}

function detectSymbolShapeIssues(up, rust, add) {
  if (up.symbolShapes.length === 0) return;
  if (rust.symbolShapes.length === 0) {
    add('SYMBOL_MISSING', `upstream renders ${up.symbolShapes.length} non-circular symbol mark(s), rust renders none`, {
      element: 'path',
      upstream: up.symbolShapes.length,
      rust: 0
    });
    return;
  }
  const upColors = up.symbolShapes.map((s) => s.fill).filter(Boolean);
  const rustColors = rust.symbolShapes.map((s) => s.fill).filter(Boolean);
  const {missing, extra} = multisetDelta(upColors, rustColors);
  if (missing.length || extra.length) {
    add('SYMBOL_COLOR_MISMATCH', `non-circular symbol fills differ; missing ${truncate([...new Set(missing)].join(', '), 60) || '-'} extra ${truncate([...new Set(extra)].join(', '), 60) || '-'}`, {
      element: 'path',
      upstream: truncate([...new Set(upColors)].join(','), 100),
      rust: truncate([...new Set(rustColors)].join(','), 100)
    });
  }
}

function detectLineGeometryIssues(up, rust, add) {
  const upLines = up.lineMarks.filter((l) => !l.className.includes('axis'));
  const rustLines = rust.lineMarks.filter((l) => !l.className.includes('axis'));
  if (upLines.length === 0 || upLines.length !== rustLines.length) return;
  const a = [...upLines].sort((p, q) => (p.bbox?.y1 ?? 0) - (q.bbox?.y1 ?? 0));
  const b = [...rustLines].sort((p, q) => (p.bbox?.y1 ?? 0) - (q.bbox?.y1 ?? 0));
  for (let i = 0; i < a.length; i++) {
    const u = a[i];
    const r = b[i];
    if (!u.bbox || !r.bbox) continue;
    const upPoints = u.path.pointCount ?? u.path.points.length;
    const rustPoints = r.path.pointCount ?? r.path.points.length;
    if (upPoints !== rustPoints) {
      add('LINE_GEOMETRY_MISMATCH', `line #${r.index} has ${rustPoints} points vs upstream ${upPoints} (missing/extra data points or imputed values)`, {
        index: r.index,
        element: 'path',
        upstream: upPoints,
        rust: rustPoints
      });
      return;
    }
    const diff = bboxDiff(u.bbox, r.bbox);
    if (diff > Math.max(1.5, u.bbox.width * 0.02, u.bbox.height * 0.02)) {
      add('LINE_GEOMETRY_MISMATCH', `line #${r.index} extent differs by ${round(diff, 1)}px: rust (${round(r.bbox.x1, 1)},${round(r.bbox.y1, 1)})-(${round(r.bbox.x2, 1)},${round(r.bbox.y2, 1)}) vs upstream (${round(u.bbox.x1, 1)},${round(u.bbox.y1, 1)})-(${round(u.bbox.x2, 1)},${round(u.bbox.y2, 1)})`, {
        index: r.index,
        element: 'path',
        upstream: `${round(u.bbox.width, 1)}x${round(u.bbox.height, 1)}`,
        rust: `${round(r.bbox.width, 1)}x${round(r.bbox.height, 1)}`
      });
      return;
    }
  }
}

function detectLegendIssues(up, rust, add) {
  const labels = (svg) => svg.texts.filter((t) => t.className.toLowerCase().includes('legend-label')).map((t) => t.content);
  const upLabels = labels(up);
  const rustLabels = labels(rust);
  if (upLabels.length === 0) return;
  const {missing, extra} = multisetDelta(upLabels, rustLabels);
  if (missing.length || extra.length) {
    add('LEGEND_LABEL_MISMATCH', `legend labels differ; missing ${truncate([...new Set(missing)].join(', '), 80) || '-'} extra ${truncate([...new Set(extra)].join(', '), 80) || '-'}`, {
      element: 'text',
      upstream: truncate(upLabels.join(','), 160),
      rust: truncate(rustLabels.join(','), 160)
    });
  }
}

function detectBarGroups(up, rust, name, add) {
  if (up.bars.length === 0 || rust.bars.length === 0) return;
  const horizontal = dominantOrientation(up.bars) === 'horizontal';
  const groups = (bars) => new Set(bars.map((b) => round(horizontal ? b.y : b.x, 1))).size;
  const upGroups = groups(up.bars);
  const rustGroups = groups(rust.bars);
  if (rustGroups < upGroups) {
    add('BAR_GROUP_MISSING', `${upGroups - rustGroups} category group(s) missing (${upGroups} upstream vs ${rustGroups} rust)`, {
      element: 'path',
      upstream: upGroups,
      rust: rustGroups
    });
  }
  if (/waterfall/.test(name) && up.bars.length === rust.bars.length) {
    const a = [...up.bars].sort((p, q) => p.x - q.x);
    const b = [...rust.bars].sort((p, q) => p.x - q.x);
    const wrong = a.filter((bar, i) => Math.abs(bar.y - b[i].y) > 2 || Math.abs(bar.h - b[i].h) > 2).length;
    if (wrong > 0) {
      const first = a.findIndex((bar, i) => Math.abs(bar.y - b[i].y) > 2 || Math.abs(bar.h - b[i].h) > 2);
      add('WATERFALL_SEGMENT_MISMATCH', `${wrong}/${a.length} waterfall segments have wrong span; first #${b[first].index} rust y=${round(b[first].y, 1)} h=${round(b[first].h, 1)} vs upstream y=${round(a[first].y, 1)} h=${round(a[first].h, 1)}`, {
        index: b[first].index,
        element: 'path',
        upstream: `${round(a[first].y, 1)}/${round(a[first].h, 1)}`,
        rust: `${round(b[first].y, 1)}/${round(b[first].h, 1)}`
      });
    }
    const expected = ['Begin', 'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec', 'End'];
    const missing = expected.filter((label) => up.texts.some((t) => t.content === label) && !rust.texts.some((t) => t.content === label));
    if (missing.length) {
      add('WATERFALL_LABEL_MISMATCH', `${missing.length} waterfall month/step label(s) missing; first "${missing[0]}"`, {
        element: 'text',
        upstream: truncate(missing.join(','), 80),
        rust: 'missing'
      });
    }
  }
}

function titleOfSvg(svg) {
  const candidates = svg.texts.filter((t) => t.className.toLowerCase().includes('title-text') || t.fontSize >= 12);
  if (!candidates.length) return null;
  return [...candidates].sort((a, b) => (b.fontSize - a.fontSize) || (a.y - b.y))[0];
}

function detectTitlePosition(up, rust, add) {
  const u = titleOfSvg(up);
  const r = titleOfSvg(rust);
  if (!u || !r) return;
  if (Math.hypot(u.x - r.x, u.y - r.y) > 2 || u.anchor !== r.anchor) {
    add('TITLE_POSITION_MISMATCH', `title at (${round(r.x, 1)},${round(r.y, 1)}) anchor=${r.anchor} vs upstream (${round(u.x, 1)},${round(u.y, 1)}) anchor=${u.anchor}`, {
      element: 'text',
      upstream: `${round(u.x, 1)},${round(u.y, 1)} ${u.anchor}`,
      rust: `${round(r.x, 1)},${round(r.y, 1)} ${r.anchor}`
    });
  }
}

function detectTrellisOrder(up, rust, name, add) {
  if (!/trellis|facet|concat|interactive_geo/.test(name)) return;
  const labels = (svg) => svg.texts
    .filter((t) => t.className.toLowerCase().includes('title-text') && t.content !== '' && !/^[\d.,%−-]+$/.test(t.content))
    .map((t) => ({content: t.content, x: round(t.x, 1), y: round(t.y, 1)}));
  const a = labels(up);
  const b = labels(rust);
  if (a.length === 0 || b.length === 0) return;
  const moved = [];
  for (const item of a) {
    const other = b.find((candidate) => candidate.content === item.content);
    if (other && (Math.abs(other.x - item.x) > 2 || Math.abs(other.y - item.y) > 2)) moved.push({item, other});
  }
  if (moved.length > 0) {
    const first = moved[0];
    add('TRELLIS_ORDER_MISMATCH', `${moved.length} facet header/cell label(s) moved; "${first.item.content}" at (${first.other.x},${first.other.y}) vs upstream (${first.item.x},${first.item.y})`, {
      element: 'text',
      upstream: `${first.item.x},${first.item.y}`,
      rust: `${first.other.x},${first.other.y}`
    });
  }
}

function detectWindowRank(up, rust, name, add) {
  if (!/window_rank/.test(name)) return;
  const a = up.lineMarks.filter((l) => !l.className.includes('axis'));
  const b = rust.lineMarks.filter((l) => !l.className.includes('axis'));
  if (a.length === 0 || a.length !== b.length) return;
  for (let i = 0; i < a.length; i++) {
    const upPoints = a[i].path.pointCount ?? a[i].path.points.length;
    const rustPoints = b[i].path.pointCount ?? b[i].path.points.length;
    if (upPoints !== rustPoints || bboxDiff(a[i].bbox, b[i].bbox) > 2) {
      add('WINDOW_RANK_MISMATCH', `rank line #${b[i].index} geometry differs (${rustPoints} vs ${upPoints} points, bbox difference ${round(bboxDiff(a[i].bbox, b[i].bbox), 1)}px)`, {
        index: b[i].index,
        element: 'path',
        upstream: `${upPoints} pts`,
        rust: `${rustPoints} pts`
      });
      return;
    }
  }
}

function detectImputeIssues(up, rust, name, add) {
  if (!/impute/.test(name)) return;
  const a = up.lineMarks.filter((l) => !l.className.includes('axis'));
  const b = rust.lineMarks.filter((l) => !l.className.includes('axis'));
  if (a.length === 0 || a.length !== b.length) return;
  for (let i = 0; i < a.length; i++) {
    const upPoints = a[i].path.pointCount ?? a[i].path.points.length;
    const rustPoints = b[i].path.pointCount ?? b[i].path.points.length;
    if (upPoints !== rustPoints) {
      add('IMPUTE_MISSING_POINTS', `imputed line #${b[i].index} has ${rustPoints} points vs upstream ${upPoints}; frame/keyvals rows are not filled`, {
        index: b[i].index,
        element: 'path',
        upstream: upPoints,
        rust: rustPoints
      });
      return;
    }
  }
}

function detectGeoConstantValue(up, rust, name, add) {
  if (!/geo_constant_value/.test(name)) return;
  const stats = (svg) => {
    const map = new Map();
    for (const shape of svg.symbolShapes) {
      const key = shape.fill ?? 'none';
      if (!map.has(key)) map.set(key, {count: 0, xs: [], ys: []});
      const entry = map.get(key);
      entry.count++;
      if (shape.bbox) {
        entry.xs.push(shape.bbox.x1, shape.bbox.x2);
        entry.ys.push(shape.bbox.y1, shape.bbox.y2);
      }
    }
    return map;
  };
  const a = stats(up);
  const b = stats(rust);
  if (process.env.DEBUG_GEO) console.error('geo const spans', [...a].map(([k,v])=>[k,Math.max(...v.xs)-Math.min(...v.xs),Math.max(...v.ys)-Math.min(...v.ys)]), [...b].map(([k,v])=>[k,Math.max(...v.xs)-Math.min(...v.xs),Math.max(...v.ys)-Math.min(...v.ys)]));
  for (const [color, upStat] of a) {
    const rustStat = b.get(color);
    if (!rustStat) {
      add('GEO_CONSTANT_LINE_MISSING', `constant-value layer "${color}" is missing entirely`, {
        element: 'path',
        upstream: `${color} x${upStat.count}`,
        rust: 'missing'
      });
      return;
    }
    const upSpanX = Math.max(...upStat.xs) - Math.min(...upStat.xs);
    const upSpanY = Math.max(...upStat.ys) - Math.min(...upStat.ys);
    const rustSpanX = rustStat.xs.length ? Math.max(...rustStat.xs) - Math.min(...rustStat.xs) : 0;
    const rustSpanY = rustStat.ys.length ? Math.max(...rustStat.ys) - Math.min(...rustStat.ys) : 0;
    if (Math.max(upSpanX, upSpanY) > 10 && Math.max(rustSpanX, rustSpanY) < Math.max(2, Math.max(upSpanX, upSpanY) * 0.1)) {
      add('GEO_CONSTANT_LINE_MISSING', `constant-value layer "${color}" collapses to a single point (span ${round(rustSpanX, 1)}x${round(rustSpanY, 1)} vs upstream ${round(upSpanX, 1)}x${round(upSpanY, 1)})`, {
        element: 'path',
        upstream: `${round(upSpanX, 1)}x${round(upSpanY, 1)}`,
        rust: `${round(rustSpanX, 1)}x${round(rustSpanY, 1)}`
      });
      return;
    }
  }
}

function detectInteractiveGeoFacet(up, rust, name, add) {
  if (!/interactive_geo_facet/.test(name)) return;
  const cells = (svg) => svg.markGroups.filter((g) => g.name === 'shape' || g.name === 'geoshape').length;
  if (cells(up) !== cells(rust)) {
    add('FACET_LAYOUT_MISMATCH', `interactive geo facet has ${cells(rust)} geoshape cell(s) vs upstream ${cells(up)}`, {
      element: 'g',
      upstream: cells(up),
      rust: cells(rust)
    });
  }
}

function detectStackCategories(up, rust, name, add) {
  if (!/stacked_bar/.test(name) || up.bars.length === 0 || rust.bars.length === 0) return;
  const horizontal = dominantOrientation(up.bars) === 'horizontal';
  const groups = (bars) => {
    const map = new Map();
    for (const bar of bars) {
      const key = round(horizontal ? bar.y : bar.x, 1);
      if (!map.has(key)) map.set(key, new Set());
      map.get(key).add(bar.fill ?? 'none');
    }
    return map;
  };
  const a = groups(up.bars);
  const b = groups(rust.bars);
  for (const [key, fills] of a) {
    const rustFills = b.get(key);
    if (!rustFills) continue; // month/category missing is BAR_GROUP_MISSING
    const missing = [...fills].filter((fill) => !rustFills.has(fill));
    if (missing.length) {
      add('STACK_CATEGORY_MISSING', `stacked group at ${key} is missing color categories: ${truncate(missing.join(', '), 80)}`, {
        element: 'path',
        upstream: truncate([...fills].join(','), 100),
        rust: truncate([...rustFills].join(','), 100)
      });
      return;
    }
  }
}

function detectTrellisCellOrder(up, rust, name, add) {
  if (!/trellis|facet/.test(name)) return;
  const a = up.markLabels ?? [];
  const b = rust.markLabels ?? [];
  if (a.length === 0 || b.length === 0) return;
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) {
      add('TRELLIS_CELL_ORDER_MISMATCH', `facet cell mark order differs at #${i}: rust "${truncate(b[i], 60)}" vs upstream "${truncate(a[i], 60)}"`, {
        index: i,
        element: 'path',
        upstream: truncate(a[i], 100),
        rust: truncate(b[i], 100)
      });
      return;
    }
  }
}

function detectFacetGroupSizes(up, rust, name, add) {
  if (!/trellis|facet|interactive_geo/.test(name)) return;
  const groups = (svg) => svg.markGroups.filter((g) => g.count > 0);
  const a = groups(up);
  const b = groups(rust);
  if (a.length === 0 || a.length !== b.length) return;
  for (let i = 0; i < a.length; i++) {
    if (a[i].count !== b[i].count && Math.abs(a[i].count - b[i].count) > Math.max(1, a[i].count * 0.02)) {
      add('FACET_CELL_GEOMETRY_MISMATCH', `facet cell "${b[i].name}" has ${b[i].count} mark(s) vs upstream ${a[i].count}`, {
        index: i,
        element: 'path',
        upstream: a[i].count,
        rust: b[i].count
      });
      return;
    }
  }
}

function detectSymbolColors(up, rust, name, add) {
  if (!/test_invalid_color/.test(name)) return;
  const a = up.circles.map((c) => c.fill).filter(Boolean);
  const b = rust.circles.map((c) => c.fill).filter(Boolean);
  if (a.length === 0) return;
  const {missing, extra} = multisetDelta(a, b);
  if (missing.length || extra.length) {
    add('COLOR_SCALE_MISMATCH', `point colors differ; missing ${truncate([...new Set(missing)].join(', '), 70) || '-'} extra ${truncate([...new Set(extra)].join(', '), 70) || '-'}`, {
      element: 'path',
      upstream: truncate([...new Set(a)].join(','), 120),
      rust: truncate([...new Set(b)].join(','), 120)
    });
  }
}

function detectBatch3Issues(up, rust, name, add) {
  detectLegendIssues(up, rust, add);
  detectSymbolColors(up, rust, name, add);
  detectBarGroups(up, rust, name, add);
  detectStackCategories(up, rust, name, add);
  detectTrellisCellOrder(up, rust, name, add);
  detectFacetGroupSizes(up, rust, name, add);
  detectTitlePosition(up, rust, add);
  detectTrellisOrder(up, rust, name, add);
  detectWindowRank(up, rust, name, add);
  detectImputeIssues(up, rust, name, add);
  detectGeoConstantValue(up, rust, name, add);
  detectInteractiveGeoFacet(up, rust, name, add);
}

function detectBatch2Issues(up, rust, name, add) {
  detectGeoIssues(up, rust, name, add);
  detectImageIssues(up, rust, add);
  detectSymbolShapeIssues(up, rust, add);
  detectLineGeometryIssues(up, rust, add);
  detectLayerIssues(up, rust, name, add);
  detectFacetIssues(up, rust, name, add);
  detectHistogramIssues(up, rust, name, add);
  detectScaleTypeIssues(up, rust, name, add);
  detectLineColorIssues(up, rust, add);
  detectAxisTickOrder(up, rust, add, name);
}


function detectAxisLabelMismatch(up, rust, add) {
  const numeric = (texts) => texts
    .map((t) => t.content)
    .filter((c) => /^-?[\d,]+(\.\d+)?$/.test(c) && /\d/.test(c))
    .map((c) => Number(c.replace(/,/g, '')))
    .filter((n) => Number.isFinite(n));
  const upNums = numeric(up.texts);
  const rustNums = numeric(rust.texts);
  if (upNums.length >= 2 && rustNums.length >= 2) {
    const range = (values) => Math.max(...values) - Math.min(...values);
    const upRange = range(upNums);
    const rustRange = range(rustNums);
    const ratio = upRange > 0 && rustRange > 0 ? Math.max(upRange / rustRange, rustRange / upRange) : 1;
    if (ratio > 2) {
      add('AXIS_DOMAIN_MISMATCH', `numeric axis range ${truncate(rustNums.slice(0, 3).join(', '), 50)} (span ${round(rustRange, 2)}) vs upstream span ${round(upRange, 2)}`, {
        element: 'text',
        upstream: `${round(Math.min(...upNums), 2)}..${round(Math.max(...upNums), 2)}`,
        rust: `${round(Math.min(...rustNums), 2)}..${round(Math.max(...rustNums), 2)}`
      });
    }
  }

  const upLabels = up.texts.filter((t) => TIME_LABEL_RE.test(t.content));
  const rustLabels = rust.texts.filter((t) => TIME_LABEL_RE.test(t.content));
  if (upLabels.length === 0 && rustLabels.length === 0) return;
  const upContents = new Set(upLabels.map((t) => t.content));
  const rustContents = new Set(rustLabels.map((t) => t.content));
  const missing = [...upContents].filter((c) => !rustContents.has(c));
  const extra = [...rustContents].filter((c) => !upContents.has(c));
  if (upLabels.length === rustLabels.length && upLabels.length > 0 && (missing.length || extra.length)) {
    add('TIME_PARSE_MISMATCH', `time/axis labels differ; upstream [${truncate([...upContents].slice(0, 6).join(', '), 80)}] vs rust [${truncate([...rustContents].slice(0, 6).join(', '), 80)}]`, {
      element: 'text',
      upstream: truncate([...upContents].slice(0, 6).join(', '), 100),
      rust: truncate([...rustContents].slice(0, 6).join(', '), 100)
    });
  }
}

function detectClipping(up, rust, add) {
  const dims = rust.dims;
  const outside = (x1, y1, x2, y2, d) =>
    x1 < -0.6 || y1 < -0.6 || x2 > d.width + 0.6 || y2 > d.height + 0.6;

  const collect = (svg) => {
    const found = [];
    for (const bar of svg.bars) {
      if (bar.bbox && outside(bar.bbox.x1, bar.bbox.y1, bar.bbox.x2, bar.bbox.y2, svg.dims)) {
        found.push({kind: 'bar', index: bar.index, bbox: bar.bbox});
      }
    }
    for (const line of svg.lineMarks) {
      if (line.bbox && outside(line.bbox.x1, line.bbox.y1, line.bbox.x2, line.bbox.y2, svg.dims)) {
        found.push({kind: 'line', index: line.index, bbox: line.bbox});
      }
    }
    for (const area of svg.areaMarks) {
      if (area.bbox && outside(area.bbox.x1, area.bbox.y1, area.bbox.x2, area.bbox.y2, svg.dims)) {
        found.push({kind: 'area', index: area.index, bbox: area.bbox});
      }
    }
    for (const line of svg.lines) {
      if (outside(line.x1, line.y1, line.x2, line.y2, svg.dims)) {
        found.push({
          kind: line.className.includes('grid') ? 'grid-line' : 'line',
          index: line.index,
          bbox: {x1: line.x1, y1: line.y1, x2: line.x2, y2: line.y2}
        });
      }
    }
    for (const text of svg.texts) {
      if (outside(text.x, text.y, text.x, text.y, svg.dims)) {
        found.push({kind: 'text', index: text.index, bbox: {x1: text.x, y1: text.y, x2: text.x, y2: text.y}});
      }
    }
    return found;
  };

  const out = collect(rust);
  const upOut = collect(up);
  if (out.length > upOut.length) {
    const first = out[0];
    const b = first.bbox;
    add('AXIS_CLIPPED', `${out.length} element(s) fall outside the ${dims.width}x${dims.height} viewBox (upstream ${upOut.length}); first ${first.kind} #${first.index} at (${round(b.x1, 1)},${round(b.y1, 1)})`, {
      index: first.index,
      element: first.kind,
      rust: `${round(b.x1, 1)},${round(b.y1, 1)}`,
      upstream: `${round(up.plot.x1, 1)},${round(up.plot.y1, 1)}`
    });
  }

  // Plot frame itself out of bounds / different plot size.
  if (rust.plot) {
    const p = rust.plot;
    if (p.x1 < -0.6 || p.y1 < -0.6 || p.x2 > dims.width + 0.6 || p.y2 > dims.height + 0.6) {
      add('PLOT_CLIPPED', `plot frame ${round(p.x1, 1)},${round(p.y1, 1)} - ${round(p.x2, 1)},${round(p.y2, 1)} exceeds viewBox ${dims.width}x${dims.height}`, {
        element: 'path',
        rust: `${round(p.width, 1)}x${round(p.height, 1)}`
      });
    } else if (up.plot && (Math.abs(p.width - up.plot.width) > 1 || Math.abs(p.height - up.plot.height) > 1)) {
      add('PLOT_SIZE_MISMATCH', `plot ${round(p.width, 1)}x${round(p.height, 1)} vs upstream ${round(up.plot.width, 1)}x${round(up.plot.height, 1)}`, {
        element: 'path',
        upstream: `${round(up.plot.width, 1)}x${round(up.plot.height, 1)}`,
        rust: `${round(p.width, 1)}x${round(p.height, 1)}`
      });
    }
  }
}

function truncate(text, max = 100) {
  const value = String(text ?? '');
  return value.length <= max ? value : `${value.slice(0, max - 1)}…`;
}

// ---------------------------------------------------------------------------
// Fixture resolution (aliases + fuzzy matching).

function normalizeName(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9]/g, '');
}

function levenshtein(a, b) {
  const m = a.length;
  const n = b.length;
  if (!m) return n;
  if (!n) return m;
  let prev = Array.from({length: n + 1}, (_, j) => j);
  for (let i = 1; i <= m; i++) {
    const cur = [i];
    for (let j = 1; j <= n; j++) {
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
    }
    prev = cur;
  }
  return prev[n];
}

async function resolveFixtures(opts) {
  const specsDir = path.join(opts.examples, 'specs');
  const compiledDir = path.join(opts.examples, 'compiled');
  const available = new Set(
    (await readdir(specsDir))
      .filter((f) => f.endsWith('.vl.json'))
      .map((f) => f.slice(0, -'.vl.json'.length))
      .filter((name) => existsSync(path.join(compiledDir, `${name}.vg.json`)))
  );
  const allNames = [...available].sort();
  const corpus = opts.batch3 ? BATCH3_FIXTURES : opts.batch2 ? BATCH2_FIXTURES : FIXTURES;
  const wanted = opts.name
    ? corpus.filter((f) => f.name.includes(opts.name) || f.aliases.some((a) => a.includes(opts.name)) || f.aliases.some((a) => normalizeName(a).includes(normalizeName(opts.name))))
    : corpus;

  const resolved = [];
  for (const fixture of wanted) {
    let match = fixture.aliases.find((alias) => available.has(alias));
    if (!match) {
      const normalizedAliases = [...fixture.aliases, fixture.name].map(normalizeName);
      let best = null;
      let bestScore = 4;
      for (const candidate of allNames) {
        const norm = normalizeName(candidate);
        for (const alias of normalizedAliases) {
          let score;
          if (norm === alias) score = 0;
          else if (norm.includes(alias) || alias.includes(norm)) score = 1;
          else score = levenshtein(norm, alias) / Math.max(norm.length, alias.length);
          if (score < bestScore) {
            bestScore = score;
            best = candidate;
          }
        }
      }
      if (best && bestScore < 0.45) match = best;
    }
    resolved.push({...fixture, resolved: match});
  }
  return {resolved, available, specsDir, compiledDir, allNames};
}

// ---------------------------------------------------------------------------
// Main.

function summarize(svg) {
  return {
    dims: svg.dims,
    counts: svg.counts,
    roundedClipPaths: svg.roundedClipPaths,
    clipRefs: svg.clipRefs,
    plot: svg.plot && {
      x1: round(svg.plot.x1, 1),
      y1: round(svg.plot.y1, 1),
      width: round(svg.plot.width, 1),
      height: round(svg.plot.height, 1)
    }
  };
}

async function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (err) {
    console.error(`compare_visual_issues: ${errorMessage(err)}`);
    console.error(usage());
    process.exitCode = 2;
    return;
  }
  if (opts.help) {
    console.log(usage());
    return;
  }

  await mkdir(path.join(OUT_DIR, 'upstream'), {recursive: true});
  await mkdir(path.join(OUT_DIR, 'rust'), {recursive: true});

  let resolved;
  let specsDir;
  let compiledDir;
  try {
    ({resolved, specsDir, compiledDir} = await resolveFixtures(opts));
  } catch (err) {
    console.error(`compare_visual_issues: cannot read examples: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }
  resolved = resolved.slice(0, opts.limit);
  if (resolved.length === 0) {
    console.error('compare_visual_issues: no fixtures matched');
    process.exitCode = 1;
    return;
  }

  let vegaInfo;
  try {
    vegaInfo = await loadVega();
  } catch (err) {
    console.error(`compare_visual_issues: ${errorMessage(err)}`);
    process.exitCode = 1;
    return;
  }
  const datasets = datasetRoot();
  const rustBuild = await probeRust();
  if (!rustBuild.ok) {
    console.error(`compare_visual_issues: Rust renderer unavailable (${rustBuild.reason})`);
    process.exitCode = 1;
    return;
  }

  const results = [];
  for (const fixture of resolved) {
    const name = fixture.resolved ?? fixture.name;
    const result = {
      name: fixture.name,
      resolvedName: fixture.resolved,
      aliasUsed: fixture.resolved && fixture.resolved !== fixture.name,
      specFile: null,
      compiledFile: null,
      status: 'error',
      error: null,
      upstream: null,
      rust: null,
      issues: [],
      findings: []
    };

    if (!name) {
      result.error = `no upstream spec found for "${fixture.name}" (aliases: ${fixture.aliases.join(', ')})`;
      result.findings.push(result.error);
      results.push(result);
      console.log(`[MISSING] ${fixture.name} | ${result.error}`);
      continue;
    }

    const specPath = path.relative(REPO_ROOT, path.join(specsDir, `${name}.vl.json`));
    result.specFile = specPath;
    result.compiledFile = path.relative(REPO_ROOT, path.join(compiledDir, `${name}.vg.json`));

    let upSvg;
    let upParsed;
    try {
      const compiledSvgPath = path.join(compiledDir, `${name}.svg`);
      if (!opts.renderUpstream && existsSync(compiledSvgPath)) {
        upSvg = await readFile(compiledSvgPath, 'utf8');
        result.upstreamSource = 'compiled-svg';
      } else {
        const compiled = JSON.parse(await readFile(path.join(compiledDir, `${name}.vg.json`), 'utf8'));
        upSvg = await renderUpstream(vegaInfo.vega, compiled, datasets);
        result.upstreamSource = 'rendered';
      }
      await writeFile(path.join(OUT_DIR, 'upstream', `${fixture.name}.svg`), upSvg);
      if (process.env.DEBUG_MEM) console.error('mem after read', fixture.name, Math.round(process.memoryUsage().heapUsed / 1e6));
      upParsed = parseSvg(upSvg);
      if (process.env.DEBUG_MEM) console.error('mem after up parse', Math.round(process.memoryUsage().heapUsed / 1e6), upParsed.counts);
      result.upstream = summarize(upParsed);
    } catch (err) {
      result.error = `upstream failed: ${errorMessage(err)}`;
      result.findings.push(result.error);
      results.push(result);
      console.log(`[ERROR] ${fixture.name} | ${result.error}`);
      continue;
    }

    const rendered = await renderRust(specPath, datasets);
    if (!rendered.ok) {
      result.error = `rust failed: ${rendered.reason}`;
      const dataError = /could not load|invalid JSON|invalid csv|csv|data url|parse.*data|No such file|failed to read/i.test(rendered.reason);
      result.issues = [{
        code: dataError ? 'DATA_LOAD_ERROR' : 'RENDER_FAILED',
        detail: rendered.reason,
        element: 'data',
        rust: rendered.reason
      }];
      result.status = 'diagnosed';
      result.findings = result.issues.map((it) => `${it.code}: ${it.detail}`);
      results.push(result);
      console.log(`[ISSUES] ${fixture.name} | ${result.issues[0].code} | ${truncate(rendered.reason, 150)}`);
      continue;
    }
    let rustParsed;
    try {
      await writeFile(path.join(OUT_DIR, 'rust', `${fixture.name}.svg`), rendered.svg);
      rustParsed = parseSvg(rendered.svg);
      if (process.env.DEBUG_MEM) console.error('mem after rust parse', fixture.name, Math.round(process.memoryUsage().heapUsed / 1e6), rustParsed.counts);
      result.rust = summarize(rustParsed);
    } catch (err) {
      result.error = `rust SVG parse failed: ${errorMessage(err)}`;
      result.findings.push(result.error);
      results.push(result);
      console.log(`[ERROR] ${fixture.name} | ${result.error}`);
      continue;
    }

    result.status = 'diagnosed';
    result.issues = detectIssues(upParsed, rustParsed, fixture.name);
    result.findings = result.issues.map((it) => `${it.code}: ${it.detail}`);
    results.push(result);

    const up = result.upstream;
    const rs = result.rust;
    const codes = result.issues.length
      ? result.issues.map((it) => (it.index != null ? `${it.code}#${it.index}` : it.code)).join(' ')
      : 'OK';
    const first = result.issues[0]?.detail ?? 'no visual issues detected';
    console.log(
      `[${result.issues.length ? 'ISSUES' : 'OK'}] ${fixture.name}${fixture.resolved && fixture.resolved !== fixture.name ? ` (${fixture.resolved})` : ''} | ` +
      `bars ${up.counts.bar}/${rs.counts.bar} symbols ${up.counts.circle}/${rs.counts.circle} ` +
      `lines ${up.counts.lineMark}/${rs.counts.lineMark} texts ${up.counts.text}/${rs.counts.text} | ` +
      `${codes} | ${truncate(first, 150)}`
    );
    if (opts.verbose) {
      for (const it of result.issues) {
        console.log(`    - ${it.code}: ${it.detail}`);
        if (it.upstream != null || it.rust != null) {
          console.log(`      upstream=${truncate(String(it.upstream ?? '-'), 100)} rust=${truncate(String(it.rust ?? '-'), 100)}`);
        }
      }
    }
  }

  const codes = new Map();
  for (const result of results) {
    for (const it of result.issues) codes.set(it.code, (codes.get(it.code) || 0) + 1);
  }
  const summary = {
    total: results.length,
    diagnosed: results.filter((r) => r.status === 'diagnosed').length,
    withIssues: results.filter((r) => r.issues.length > 0).length,
    clean: results.filter((r) => r.status === 'diagnosed' && r.issues.length === 0).length,
    errors: results.filter((r) => r.status === 'error').length,
    issueCodes: Object.fromEntries([...codes].sort())
  };

  console.log('');
  console.log(
    `Summary: ${summary.diagnosed}/${summary.total} diagnosed, ${summary.withIssues} with issues, ` +
    `${summary.clean} clean, ${summary.errors} errors`
  );
  if (Object.keys(summary.issueCodes).length) {
    console.log(`Issue codes: ${Object.entries(summary.issueCodes).map(([code, n]) => `${code}×${n}`).join(', ')}`);
  }
  console.log(`Upstream vega: ${vegaInfo.from}`);
  console.log(`SVGs: ${OUT_DIR}`);

  const report = {
    generatedAt: new Date().toISOString(),
    repoRoot: REPO_ROOT,
    examples: opts.examples,
    outDir: OUT_DIR,
    datasets,
    upstreamVega: vegaInfo.from,
    options: {...opts, limit: Number.isFinite(opts.limit) ? opts.limit : null},
    summary,
    results
  };
  await writeFile(opts.json, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Report: ${opts.json}`);
  if (summary.errors > 0) process.exitCode = 1;
}

main().catch((err) => {
  console.error(`compare_visual_issues: unexpected failure: ${errorMessage(err)}`);
  process.exitCode = 1;
});
