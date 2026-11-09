//! Render the compiled Vega example corpus and report timings, optionally
//! writing a CPU flamegraph.
//!
//! ```sh
//! cargo run --release -p vega-svg --example profile_corpus -- \
//!     /tmp/opencode/vega-lite/examples/compiled \
//!     --base node_modules/vega-datasets \
//!     --flamegraph /tmp/opencode/vega-svg-flame.svg
//! ```
//!
//! Env vars: `CORPUS_ITERS` (default 1), `CORPUS_LIMIT`, `CORPUS_FILTER`.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

fn main() {
    let mut corpus = PathBuf::from("/tmp/opencode/vega-lite/examples/compiled");
    let mut base = PathBuf::from("node_modules/vega-datasets");
    let mut flamegraph = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base" => base = PathBuf::from(args.next().expect("--base needs a value")),
            "--flamegraph" => {
                flamegraph = Some(PathBuf::from(
                    args.next().expect("--flamegraph needs a value"),
                ))
            }
            other => corpus = PathBuf::from(other),
        }
    }

    let mut specs: Vec<(String, Value)> = Vec::new();
    let mut names: Vec<PathBuf> = fs::read_dir(&corpus)
        .expect("read corpus")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    names.sort();

    let filter = env::var("CORPUS_FILTER").unwrap_or_default();
    let limit: Option<usize> = env::var("CORPUS_LIMIT").ok().and_then(|v| v.parse().ok());
    for path in names {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .trim_end_matches(".vg.json")
            .to_string();
        if !filter.is_empty() && !name.contains(&filter) {
            continue;
        }
        if let Some(limit) = limit {
            if specs.len() >= limit {
                break;
            }
        }
        let text = fs::read_to_string(&path).expect("read spec");
        let spec: Value = serde_json::from_str(&text).expect("parse spec");
        specs.push((name, spec));
    }

    let iters: usize = env::var("CORPUS_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let opts = vega_svg::RenderOptions {
        data_base: base,
        ..Default::default()
    };

    let guard = flamegraph.as_ref().map(|_| {
        pprof::ProfilerGuardBuilder::default()
            .frequency(997)
            .blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build()
            .expect("start profiler")
    });

    let mut timings: Vec<(String, f64)> = Vec::new();
    let mut errors = 0usize;
    let wall = Instant::now();
    for _ in 0..iters {
        for (name, spec) in &specs {
            if std::env::var_os("CORPUS_PROGRESS").is_some() {
                eprintln!("rendering {name}");
            }
            let t = Instant::now();
            match vega_svg::vega_to_svg_with_options(spec, &opts) {
                Ok(_) => {}
                Err(err) => {
                    errors += 1;
                    if iters == 1 {
                        eprintln!("error {name}: {err}");
                    }
                }
            }
            if iters == 1 {
                timings.push((name.clone(), t.elapsed().as_secs_f64() * 1000.0));
            }
        }
    }
    let total = wall.elapsed().as_secs_f64();
    println!(
        "rendered {} specs x{} in {:.3}s ({:.2} ms/spec), {} errors",
        specs.len(),
        iters,
        total,
        total * 1000.0 / (specs.len() * iters) as f64,
        errors
    );

    if iters == 1 {
        timings.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!("slowest:");
        for (name, ms) in timings.iter().take(20) {
            println!("  {ms:9.2} ms  {name}");
        }
    }

    if let Some(path) = flamegraph {
        let report = guard.unwrap().report().build().expect("build report");
        if let Ok(top) = env::var("CORPUS_TOP") {
            let top: usize = top.parse().unwrap_or(30);
            print_top(&report, top);
        }
        let out = fs::File::create(&path).expect("create flamegraph");
        report.flamegraph(out).expect("write flamegraph");
        println!("flamegraph: {}", path.display());
    }
}

fn print_top(report: &pprof::Report, top: usize) {
    use std::collections::HashMap;

    let mut inclusive: HashMap<String, i64> = HashMap::new();
    let mut exclusive: HashMap<String, i64> = HashMap::new();
    let mut total: i64 = 0;
    for (frames, count) in &report.data {
        let count = *count as i64;
        total += count;
        let mut seen: Vec<String> = Vec::new();
        for frame in frames.frames.iter().rev() {
            for symbol in frame.iter().rev() {
                let name = symbol.name();
                if !seen.contains(&name) {
                    seen.push(name);
                }
            }
        }
        for name in seen {
            *inclusive.entry(name).or_default() += count;
        }
    }
    let mut edges: HashMap<(String, String), i64> = HashMap::new();
    for (frames, count) in &report.data {
        let count = *count as i64;
        let mut stack: Vec<String> = Vec::new();
        for frame in frames.frames.iter().rev() {
            for symbol in frame.iter().rev() {
                stack.push(symbol.name());
            }
        }
        for pair in stack.windows(2) {
            *edges
                .entry((pair[0].clone(), pair[1].clone()))
                .or_default() += count;
        }
        if let Some(leaf) = stack.last() {
            *exclusive.entry(leaf.clone()).or_default() += count;
        }
    }
    if std::env::var_os("CORPUS_EDGES").is_some() {
        let mut e: Vec<_> = edges.into_iter().collect();
        e.sort_by_key(|(_, c)| -*c);
        println!("hot edges (caller -> callee):");
        for ((a, b), c) in e.iter().take(top.min(40)) {
            println!(
                "  {:6.2}%  {} -> {}",
                100.0 * *c as f64 / total as f64,
                a,
                b
            );
        }
    }
    let mut inc: Vec<_> = inclusive.into_iter().collect();
    inc.sort_by_key(|(_, c)| -*c);
    println!("inclusive:");
    for (name, count) in inc.iter().take(top) {
        println!(
            "  {:6.2}%  {}",
            100.0 * *count as f64 / total as f64,
            name
        );
    }
    let mut exc: Vec<_> = exclusive.into_iter().collect();
    exc.sort_by_key(|(_, c)| -*c);
    println!("exclusive:");
    for (name, count) in exc.iter().take(top) {
        println!(
            "  {:6.2}%  {}",
            100.0 * *count as f64 / total as f64,
            name
        );
    }
}
