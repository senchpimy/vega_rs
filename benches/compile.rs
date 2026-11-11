//! Micro-benchmarks for the Vega-Lite compiler.
//!
//! The corpus is the set of `.vl.json` examples shipped in
//! `tests/fixtures/examples/specs`. Point `VEGA_LITE_BENCH_CORPUS` at the
//! `examples/specs` directory of a full upstream Vega-Lite checkout to
//! benchmark against every upstream example instead.
//!
//! Run with:
//!
//! ```sh
//! cargo bench                      # all groups
//! cargo bench -- compile/corpus    # one group
//! cargo bench -- --quick           # fast, noisy run (Criterion 0.8+)
//! VEGA_LITE_BENCH_CORPUS=/tmp/opencode/vega-lite/examples/specs cargo bench
//! ```

use std::hint::black_box;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::Value;

struct Spec {
    name: String,
    text: String,
    value: Value,
}

/// Specifications that exercise distinct compiler paths (unit marks, layering,
/// faceting, concatenation, repetition, selections and geo projections).
const REPRESENTATIVE: &[&str] = &[
    "bar",
    "bar_grouped",
    "line",
    "area",
    "point_2d_aggregate",
    "histogram_ordinal",
    "stacked_bar_normalize",
    "layer_bar_annotations",
    "facet_custom",
    "trellis_barley",
    "concat_bar_layer_circle",
    "repeat_splom",
    "interactive_seattle_weather",
    "geo_choropleth",
    "boxplot_2D_vertical",
    "errorbar_aggregate",
];

fn corpus_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("VEGA_LITE_BENCH_CORPUS") {
        let dir = PathBuf::from(dir);
        if dir.is_dir() {
            return dir;
        }
        eprintln!(
            "warning: VEGA_LITE_BENCH_CORPUS={} is not a directory; using fixtures",
            dir.display()
        );
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/examples/specs")
}

fn read_specs(dir: &Path) -> Vec<Spec> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".vl.json"))
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            // Ignore specs the compiler rejects (invalid examples etc.) so the
            // corpus measures successful compilation only.
            let mut ok = false;
            let _ = catch_unwind(AssertUnwindSafe(|| {
                ok = vega_lite::compile(&value).is_ok();
            }));
            if !ok {
                return None;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .trim_end_matches(".vl.json")
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
            "benchmark corpus: {} specs, {:.1} KiB",
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

/// End-to-end compilation of the whole corpus.
fn compile_corpus(c: &mut Criterion) {
    let specs = corpus();
    let mut group = c.benchmark_group("compile/corpus");
    group.throughput(Throughput::Elements(specs.len() as u64));
    group.sample_size(20);
    group.bench_function("specs", |b| {
        b.iter(|| {
            for spec in specs {
                let result = vega_lite::compile(black_box(&spec.value));
                black_box(result.is_ok());
            }
        });
    });
    group.finish();
}

/// Per-spec compilation latency for representative charts.
fn compile_specs(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile/spec");
    for spec in representative() {
        group.throughput(Throughput::Bytes(spec.text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(&spec.name),
            &spec.value,
            |b, value| {
                b.iter(|| {
                    let result = vega_lite::compile(black_box(value));
                    black_box(result.is_ok());
                });
            },
        );
    }
    group.finish();
}

/// JSON parsing, to separate serde cost from compiler cost.
fn parse_specs(c: &mut Criterion) {
    let mut group = c.benchmark_group("serde/parse");
    for spec in representative() {
        group.throughput(Throughput::Bytes(spec.text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(&spec.name),
            &spec.text,
            |b, text| {
                b.iter(|| {
                    let value: Value = serde_json::from_str(black_box(text)).unwrap();
                    black_box(value);
                });
            },
        );
    }
    group.finish();
}

/// Normalization only, the first half of `compile`.
fn normalize_specs(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize/spec");
    for spec in representative() {
        group.bench_with_input(
            BenchmarkId::from_parameter(&spec.name),
            &spec.value,
            |b, value| {
                b.iter(|| {
                    let normalized = vega_lite::normalize::normalize(black_box(value), None);
                    black_box(normalized.is_ok());
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    compile_corpus,
    compile_specs,
    parse_specs,
    normalize_specs
);
criterion_main!(benches);
