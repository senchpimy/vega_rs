//! Full-corpus smoke scan (ignored by default): compiles every upstream
//! example that has a compiled Vega output and reports how many match exactly.
//!
//! Run with:
//!   cargo test --test examples_scan -- --ignored --nocapture

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn repo() -> PathBuf {
    if let Ok(path) = env::var("VEGA_LITE_REPO") {
        let path = PathBuf::from(path);
        if path.join("examples/specs").is_dir() {
            return path;
        }
    }
    let upstream = PathBuf::from("/tmp/opencode/vega-lite");
    if upstream.join("examples/specs").is_dir() {
        return upstream;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
#[ignore = "full 633-example scan"]
fn scan_all_examples() {
    let repo = repo();
    let specs = repo.join("examples/specs");
    let compiled = repo.join("examples/compiled");
    let mut names: Vec<String> = fs::read_dir(&specs)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".vl.json"))
        .collect();
    names.sort();

    let mut total = 0usize;
    let mut matched = 0usize;
    let mut failed = 0usize;
    let mut panicked = 0usize;
    let mut matches: Vec<String> = Vec::new();

    for file in &names {
        let name = file.trim_end_matches(".vl.json");
        let vg_path = compiled.join(format!("{name}.vg.json"));
        if !vg_path.exists() {
            continue;
        }
        total += 1;
        let spec: Value = match fs::read_to_string(specs.join(file))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
        {
            Some(spec) => spec,
            None => {
                failed += 1;
                continue;
            }
        };
        let expected: Value = match fs::read_to_string(&vg_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
        {
            Some(value) => value,
            None => {
                failed += 1;
                continue;
            }
        };
        let result = std::panic::catch_unwind(|| {
            vega_lite::compile::compile::compile(&spec).ok()
        });
        match result {
            Ok(Some(result)) if result.spec == expected => {
                matched += 1;
                matches.push(name.to_string());
            }
            Ok(_) => {
                failed += 1;
                println!("FAIL {name}");
            }
            Err(_) => {
                panicked += 1;
                println!("PANIC {name}");
            }
        }
    }

    println!("exact matches: {matched}/{total} (failed: {failed}, panicked: {panicked})");
    for name in &matches {
        println!("MATCH {name}");
    }
}
