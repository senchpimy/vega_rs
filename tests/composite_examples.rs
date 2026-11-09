//! Exact-output tests for composite-mark (`boxplot`, `errorbar`, `errorband`)
//! examples.
//!
//! A subset of the upstream specs/compiled pairs is vendored under
//! `tests/fixtures/examples`; the rest are read from a Vega-Lite checkout via
//! `VEGA_LITE_REPO` (falling back to `/tmp/opencode/vega-lite`). Examples whose
//! checkout is unavailable are skipped.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn repo_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = env::var("VEGA_LITE_REPO") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"));
    candidates.push(PathBuf::from("/tmp/opencode/vega-lite"));
    candidates
}

fn find_pair(name: &str) -> Option<(PathBuf, PathBuf)> {
    for repo in repo_candidates() {
        let spec = repo.join("examples/specs").join(format!("{name}.vl.json"));
        let compiled = repo
            .join("examples/compiled")
            .join(format!("{name}.vg.json"));
        if spec.exists() && compiled.exists() {
            return Some((spec, compiled));
        }
    }
    None
}

fn first_diff(actual: &Value, expected: &Value, path: String) -> String {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for key in a.keys() {
                if !e.contains_key(key) {
                    return format!("{path}.{key}: unexpected key in actual");
                }
                let diff = first_diff(&a[key], &e[key], format!("{path}.{key}"));
                if !diff.is_empty() {
                    return diff;
                }
            }
            for key in e.keys() {
                if !a.contains_key(key) {
                    return format!("{path}.{key}: missing in actual (expected {:?})", e[key]);
                }
            }
            String::new()
        }
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return format!("{path}: array length {} != {}", a.len(), e.len());
            }
            for (i, (av, ev)) in a.iter().zip(e.iter()).enumerate() {
                let diff = first_diff(av, ev, format!("{path}[{i}]"));
                if !diff.is_empty() {
                    return diff;
                }
            }
            String::new()
        }
        (a, e) => {
            if a == e {
                String::new()
            } else {
                format!("{path}: {a} != {e}")
            }
        }
    }
}

fn compile_example(name: &str) {
    let Some((spec_path, vg_path)) = find_pair(name) else {
        eprintln!("skipping {name}: no Vega-Lite checkout or fixture available");
        return;
    };
    let spec: Value = serde_json::from_str(
        &fs::read_to_string(&spec_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", spec_path.display())),
    )
    .expect("parse vl spec");
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&vg_path).unwrap_or_else(|e| panic!("read {}: {e}", vg_path.display())),
    )
    .expect("parse vg spec");

    let result = vega_lite::compile::compile::compile(&spec)
        .unwrap_or_else(|e| panic!("compile {name} failed: {e}"));
    if result.spec != expected {
        let diff = first_diff(&result.spec, &expected, "$".to_string());
        let actual = serde_json::to_string_pretty(&result.spec).unwrap();
        let expected_pretty = serde_json::to_string_pretty(&expected).unwrap();
        panic!(
            "example {name} does not match\nfirst difference: {diff}\n\n--- actual ---\n{actual}\n\n--- expected ---\n{expected_pretty}"
        );
    }
}

macro_rules! composite_example_test {
    ($fn:ident, $name:expr) => {
        #[test]
        fn $fn() {
            compile_example($name);
        }
    };
}

// Vendored fixtures (always run).
composite_example_test!(boxplot_minmax_2d_horizontal, "boxplot_minmax_2D_horizontal");
composite_example_test!(
    boxplot_minmax_2d_horizontal_custom_midtick_color,
    "boxplot_minmax_2D_horizontal_custom_midtick_color"
);
composite_example_test!(boxplot_minmax_2d_vertical, "boxplot_minmax_2D_vertical");
composite_example_test!(boxplot_preaggregated, "boxplot_preaggregated");
composite_example_test!(
    errorband_2d_vertical_borders,
    "errorband_2d_vertical_borders"
);
composite_example_test!(errorbar_2d_vertical_ticks, "errorbar_2d_vertical_ticks");
composite_example_test!(errorbar_aggregate, "errorbar_aggregate");
composite_example_test!(errorbar_tooltip, "errorbar_tooltip");
composite_example_test!(
    layer_point_errorbar_2d_horizontal_ci,
    "layer_point_errorbar_2d_horizontal_ci"
);
composite_example_test!(
    layer_scatter_errorband_1d_stdev,
    "layer_scatter_errorband_1d_stdev"
);

// Remaining exact matches (need a Vega-Lite checkout).
composite_example_test!(
    boxplot_preaggregated_with_height,
    "boxplot_preaggregated_with_height"
);
composite_example_test!(errorband_tooltip, "errorband_tooltip");
composite_example_test!(
    errorbar_horizontal_aggregate,
    "errorbar_horizontal_aggregate"
);
composite_example_test!(
    layer_point_errorbar_1d_horizontal,
    "layer_point_errorbar_1d_horizontal"
);
composite_example_test!(
    layer_point_errorbar_1d_vertical,
    "layer_point_errorbar_1d_vertical"
);
composite_example_test!(
    layer_point_errorbar_2d_horizontal,
    "layer_point_errorbar_2d_horizontal"
);
composite_example_test!(
    layer_point_errorbar_2d_horizontal_iqr,
    "layer_point_errorbar_2d_horizontal_iqr"
);
composite_example_test!(
    layer_point_errorbar_2d_horizontal_stdev,
    "layer_point_errorbar_2d_horizontal_stdev"
);
composite_example_test!(
    layer_point_errorbar_2d_vertical,
    "layer_point_errorbar_2d_vertical"
);
composite_example_test!(layer_point_errorbar_ci, "layer_point_errorbar_ci");
composite_example_test!(
    layer_point_errorbar_pre_aggregated_asymmetric_error,
    "layer_point_errorbar_pre_aggregated_asymmetric_error"
);
composite_example_test!(
    layer_point_errorbar_pre_aggregated_symmetric_error,
    "layer_point_errorbar_pre_aggregated_symmetric_error"
);
composite_example_test!(
    layer_point_errorbar_pre_aggregated_upper_lower,
    "layer_point_errorbar_pre_aggregated_upper_lower"
);
composite_example_test!(layer_point_errorbar_stdev, "layer_point_errorbar_stdev");
composite_example_test!(
    layer_scatter_errorband_1d_stdev_global_mean,
    "layer_scatter_errorband_1D_stdev_global_mean"
);
