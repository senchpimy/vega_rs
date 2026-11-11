//! Benchmarks for rendering compiled Vega specifications to SVG.
//!
//! The corpus is `tests/fixtures/examples/compiled/*.vg.json`, i.e. the
//! upstream-compiled output bundled with the workspace. Datasets are resolved
//! from `node_modules/vega-datasets`; specs whose data cannot be loaded are
//! skipped.
//!
//! Run with:
//!
//! ```sh
//! cargo bench -p vega-svg
//! cargo bench -p vega-svg -- render/corpus
//! cargo bench -p vega-svg -- --quick
//! ```

use std::hint::black_box;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::Value;
use vega_svg::RenderOptions;

struct Spec {
    name: String,
    text: String,
    value: Value,
}

const REPRESENTATIVE: &[&str] = &[
    "bar",
    "line",
    "point_colorramp_size",
    "rect_heatmap",
    "stacked_bar_normalize",
    "trellis_barley",
    "concat_bar_layer_circle",
    "interactive_seattle_weather",
    "geo_choropleth",
    "arc_radial",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vega-svg lives in the workspace")
        .to_path_buf()
}

fn corpus_dir() -> PathBuf {
    workspace_root().join("tests/fixtures/examples/compiled")
}

fn options() -> RenderOptions {
    RenderOptions {
        data_base: workspace_root().join("node_modules/vega-datasets"),
        ..Default::default()
    }
}

fn read_specs(dir: &Path) -> Vec<Spec> {
    let opts = options();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".vg.json"))
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            let mut ok = false;
            let _ = catch_unwind(AssertUnwindSafe(|| {
                ok = vega_svg::vega_to_svg_with_options(&value, &opts).is_ok();
            }));
            if !ok {
                return None;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .trim_end_matches(".vg.json")
                .to_string();
            Some(Spec { name, text, value })
        })
        .collect()
}

fn corpus() -> &'static Vec<Spec> {
    static CORPUS: OnceLock<Vec<Spec>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let specs = read_specs(&corpus_dir());
        let size: usize = specs.iter().map(|spec| spec.text.len()).sum();
        eprintln!(
            "render corpus: {} specs, {:.1} KiB",
            specs.len(),
            size as f64 / 1024.0
        );
        specs
    })
}

fn representative() -> Vec<&'static Spec> {
    let specs = corpus();
    REPRESENTATIVE
        .iter()
        .filter_map(|name| specs.iter().find(|spec| spec.name == *name))
        .collect()
}

/// End-to-end rendering of the whole compiled corpus.
fn render_corpus(c: &mut Criterion) {
    let specs = corpus();
    let opts = options();
    let mut group = c.benchmark_group("render/corpus");
    group.throughput(Throughput::Elements(specs.len() as u64));
    group.sample_size(10);
    group.bench_function("specs", |b| {
        b.iter(|| {
            for spec in specs {
                let svg = vega_svg::vega_to_svg_with_options(black_box(&spec.value), &opts);
                let _ = black_box(svg.map(|svg| svg.len()));
            }
        });
    });
    group.finish();
}

/// Per-spec rendering latency for representative charts.
fn render_specs(c: &mut Criterion) {
    let opts = options();
    let mut group = c.benchmark_group("render/spec");
    for spec in representative() {
        group.throughput(Throughput::Bytes(spec.text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(&spec.name),
            &spec.value,
            |b, value| {
                b.iter(|| {
                    let svg = vega_svg::vega_to_svg_with_options(black_box(value), &opts);
                    let _ = black_box(svg.map(|svg| svg.len()));
                });
            },
        );
    }
    group.finish();
}

/// Compile + render from a Vega-Lite spec in one call.
fn render_vega_lite(c: &mut Criterion) {
    let specs_dir = workspace_root().join("tests/fixtures/examples/specs");
    let opts = options();
    let mut group = c.benchmark_group("render/vega-lite");
    for name in REPRESENTATIVE {
        let path = specs_dir.join(format!("{name}.vl.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if vega_svg::vega_lite_to_svg_with_options(&value, &opts).is_err() {
            continue;
        }
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &value, |b, spec| {
            b.iter(|| {
                let svg = vega_svg::vega_lite_to_svg_with_options(black_box(spec), &opts);
                let _ = black_box(svg.map(|svg| svg.len()));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, render_corpus, render_specs, render_vega_lite);
criterion_main!(benches);
