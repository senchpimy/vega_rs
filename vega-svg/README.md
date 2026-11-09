# vega-svg

A Rust renderer for [Vega](https://github.com/vega/vega) specifications,
structured after the upstream packages (`vega-expression`, `vega-transforms`,
`vega-scale`, `vega-scenegraph`, `vega-parser`, `vega-view`).

It provides two entry points:

```rust
use serde_json::json;

// Vega -> SVG
let vega = json!({
    "width": 200,
    "height": 100,
    "data": [{"name": "t", "values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]}],
    "scales": [
        {"name": "x", "type": "band", "domain": {"data": "t", "field": "a"}, "range": "width"},
        {"name": "y", "type": "linear", "domain": {"data": "t", "field": "b"}, "range": [100, 0]}
    ],
    "marks": [{
        "type": "rect",
        "from": {"data": "t"},
        "encode": {"update": {
            "x": {"scale": "x", "field": "a"},
            "width": {"signal": "bandwidth('x')"},
            "y": {"scale": "y", "field": "b"},
            "y2": {"scale": "y", "value": 0}
        }}
    }]
});
let svg = vega_svg::vega_to_svg(&vega).unwrap();

// Vega-Lite -> SVG, without a separate compiler invocation or JSON round-trip.
let vl = json!({
    "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "a", "type": "nominal"},
        "y": {"field": "b", "type": "quantitative"}
    }
});
let svg = vega_svg::vega_lite_to_svg(&vl).unwrap();
```

`RenderOptions` controls where relative `data.url` files are resolved from and
the output scale factor:

```rust
let opts = vega_svg::RenderOptions {
    data_base: "node_modules/vega-datasets".into(),
    ..Default::default()
};
let svg = vega_svg::vega_lite_to_svg_with_options(&vl, &opts).unwrap();
```

## Building and testing

```sh
cargo test -p vega-svg
cargo run -p vega-svg --bin vega-svg -- spec.vg.json -o chart.svg
cargo run -p vega-svg --bin vega-svg -- --vega-lite spec.vl.json -o chart.svg
```

The CLI accepts `--data-dir` (repeatable), `--base`, `--scale`, `--background`,
reads `-` from stdin and writes to stdout unless `-o/--output` is provided.

`scripts/compare_svg.mjs` (run from the workspace root) renders every compiled
example with upstream Vega and with this crate, writing reference SVGs to
`/tmp/opencode/ref_svg` and a comparison report to
`/tmp/opencode/svg_compare_report.json`:

```sh
node scripts/compare_svg.mjs
node scripts/compare_svg.mjs --name trellis --limit 5
```

## Profiling

Two `examples/` harnesses use `pprof` to sample render CPU time:

```sh
# Whole compiled corpus; prints total/per-spec timings and writes a flamegraph.
cargo run --release -p vega-svg --example profile_corpus -- \
    /tmp/opencode/vega-lite/examples/compiled \
    --base node_modules/vega-datasets \
    --flamegraph /tmp/opencode/vega-svg-flamegraph.svg

# One spec; dumps a flamegraph if it is still running after N seconds.
cargo run --release -p vega-svg --example profile_hang -- \
    spec.vg.json /tmp/opencode/out.svg 5
```

`profile_corpus` accepts `CORPUS_ITERS`, `CORPUS_LIMIT`, `CORPUS_FILTER`,
`CORPUS_PROGRESS`, `CORPUS_TOP` (print inclusive/exclusive function tables) and
`CORPUS_EDGES` (print hot caller→callee edges).

Local status after the profiling pass: the 633-example corpus renders in
~5.8 s (~9.2 ms/spec) versus ~10.2 s before, with byte-identical SVG output
(except the two specs that use `Math.random`). Main wins: cached access-path
parsing, non-cloning `datum[...]` evaluation, hash-indexed `lookup` transforms,
linear-time discrete-domain deduplication, and per-group dataset copying
instead of cloning the whole store.

## Supported subset

- Data: inline `values`, local/HTTP-less file loading, JSON/CSV/TSV, `format.parse`.
- Transforms: filter, formula, aggregate, joinaggregate, stack, window, extent,
  bin, timeunit, impute, fold, flatten, project, sequence, pivot, quantile,
  identifier, sample, loess, geojson, geopoint and graticule.
- Expressions: the Vega expression subset (operators, member access, ternaries,
  arrays/objects and the common function library).
- Scales: linear, log, pow, sqrt, time/utc, band, point, ordinal, quantile,
  quantize, threshold, sequential, plus Vega color schemes.
- Marks: rect, rule, symbol, text, line, area, trail, arc, path, shape, image
  and nested group marks, including projected `geoshape` paths.
- Guides: axes (including grids, labels and titles), symbol and gradient
  legends, chart titles, faceted/concat group layout.
- Projections: albers, albersUsa, azimuthal equal-area/equidistant, conic
  conformal/equal-area/equidistant, equalEarth, equirectangular, gnomonic,
  identity, mercator, naturalEarth1, orthographic, stereographic and
  transverseMercator, with fit/extent/clip and adaptive resampling.
- SVG: background, padding, margins for guides, gradients, clipping, links,
  rotation and basic text layout.

Facet groups support partitioned local data, group-local scales, row/column
headers and the concat/trellis layout used by Vega-Lite.

Interactive features (events, selections, signals with `on`, tooltips,
force/wordcloud) are intentionally outside the static rendering scope;
unsupported transforms pass their data through.

## Validation

- Unit and integration tests cover expressions, transforms, scales, axes,
  legends, gradients, text escaping, layout and every spec in
  `tests/fixtures/examples/compiled`.
- The comparison harnesses render every compiled example with upstream Vega and
  this crate. Current local status (247 fixtures): 247/247 render with no
  errors, 197 fully match all checked dimensions/text/counts/fills, 214
  dimension matches, 232 text matches, 230 element-count matches and 233 fill
  matches. Remaining differences are mostly facet/legend layout details.
- `scripts/compare_upstream_svg.mjs` runs the full upstream corpus with strict
  path/line/rect/text geometry comparison: 419/629 currently pass; four specs cannot load remote datasets
  because the loader is local-only.
- `scripts/compare_visual_issues.mjs` diagnoses semantic visual bugs (missing
  or zero-size marks, labels inside bars, reversed stacks/axes, extra grid
  lines, overlapping symbols, missing line breaks at invalid data, etc.).

## Layout

| module | upstream |
| --- | --- |
| `expression` | `vega-expression` |
| `data` | `vega-transforms` |
| `scale` | `vega-scale` |
| `scenegraph` | `vega-scenegraph` |
| `runtime` | `vega-runtime` / `vega-parser` / `vega-view` |
| `svg` | `vega-scenegraph` SVG renderer |
| `format`, `color`, `value` | `vega-format` / `vega-scale` / `vega-util` |
