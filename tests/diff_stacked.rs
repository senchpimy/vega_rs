//! Diagnostic: prints the first structural difference for the stacked/bar/circle/histogram/area examples.
//! Run with: cargo test --test diff_stacked -- --ignored --nocapture

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
    PathBuf::from("/tmp/opencode/vega-lite")
}

fn first_diff(actual: &Value, expected: &Value, path: String) -> String {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for key in a.keys() {
                if !e.contains_key(key) {
                    return format!("{path}.{key}: unexpected key in actual = {}", a[key]);
                }
                let diff = first_diff(&a[key], &e[key], format!("{path}.{key}"));
                if !diff.is_empty() {
                    return diff;
                }
            }
            for key in e.keys() {
                if !a.contains_key(key) {
                    return format!("{path}.{key}: missing in actual (expected {})", e[key]);
                }
            }
            String::new()
        }
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return format!(
                    "{path}: array length {} != {}\n  actual={}\n  expected={}",
                    a.len(),
                    e.len(),
                    Value::Array(a.clone()),
                    Value::Array(e.clone())
                );
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

const NAMES: &[&str] = &[
    "bar_argmax_transform",
    "bar_binned_data",
    "bar_binned_yearmonth_grouped_center_band",
    "bar_binned_yearmonth_label_band_center",
    "bar_corner_radius_end",
    "bar_count_minimap",
    "bar_custom_time_domain",
    "bar_default_tooltip_title_null",
    "bar_diverging_stack_population_pyramid",
    "bar_grouped_custom_color_domain",
    "bar_grouped_errorbar",
    "bar_grouped_stacked",
    "bar_grouped_timeunit_yearweek",
    "bar_group_timeunit",
    "bar_layered_transparent",
    "bar_params",
    "bar_params_bound",
    "bar_sort_by_count",
    "bar_temporal_color_config_time_unit_band_position",
    "circle_binned",
    "circle_binned_maxbins_2",
    "circle_binned_maxbins_20",
    "circle_binned_maxbins_5",
    "circle_bubble_health_income",
    "circle_custom_tick_labels",
    "circle_github_punchcard",
    "circle_labelangle_orient_signal",
    "circle_wilkinson_dotplot_stacked",
    "histogram_bin_spacing",
    "histogram_bin_spacing_reverse",
    "histogram_nonlinear",
    "histogram_no_spacing",
    "histogram_ordinal_sort",
    "histogram_rel_freq",
    "stacked_area",
    "stacked_area_binned",
    "stacked_area_normalize",
    "stacked_area_ordinal",
    "stacked_area_overlay",
    "stacked_area_stream",
    "stacked_bar_count",
    "stacked_bar_count_corner_radius_config",
    "stacked_bar_count_corner_radius_mark",
    "stacked_bar_count_corner_radius_mark_x",
    "stacked_bar_count_corner_radius_stroke",
    "stacked_bar_h",
    "stacked_bar_h_custom_color_domain",
    "stacked_bar_h_normalized_labeled",
    "stacked_bar_h_order",
    "stacked_bar_h_order_custom",
    "stacked_bar_normalize",
    "stacked_bar_population",
    "stacked_bar_size",
    "stacked_bar_sum_opacity",
    "stacked_bar_v",
    "stacked_bar_v_ascending",
    "stacked_bar_weather",
];

#[test]
#[ignore = "diagnostic"]
fn diff_stacked() {
    let repo = repo();
    for name in NAMES {
        let spec_path = repo.join("examples/specs").join(format!("{name}.vl.json"));
        let vg_path = repo.join("examples/compiled").join(format!("{name}.vg.json"));
        let spec: Value = match fs::read_to_string(&spec_path) {
            Ok(t) => serde_json::from_str(&t).unwrap(),
            Err(_) => {
                println!("MISSING SPEC {name}");
                continue;
            }
        };
        let expected: Value = match fs::read_to_string(&vg_path) {
            Ok(t) => serde_json::from_str(&t).unwrap(),
            Err(_) => {
                println!("MISSING VG {name}");
                continue;
            }
        };
        let actual = match std::panic::catch_unwind(|| vega_lite::compile::compile::compile(&spec)) {
            Ok(Ok(r)) => r.spec,
            Ok(Err(e)) => {
                println!("=== {name}: COMPILE ERROR {e}");
                continue;
            }
            Err(_) => {
                println!("=== {name}: PANIC");
                continue;
            }
        };
        if let Ok(dir) = env::var("DUMP_ACTUAL") {
            let _ = fs::create_dir_all(&dir);
            fs::write(
                PathBuf::from(&dir).join(format!("{name}.vg.json")),
                serde_json::to_string_pretty(&actual).unwrap(),
            )
            .ok();
        }
        let diff = first_diff(&actual, &expected, "$".into());
        if diff.is_empty() {
            println!("=== {name}: MATCH");
        } else {
            println!("=== {name}: {diff}");
        }
    }
}
