//! Phase 3 regression tests: local-time units, d3-compatible binning, KDE and
//! window frames. The local-time assertions derive the expected year through
//! the expression engine so they hold regardless of the process timezone.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};
use vega_svg::data::{apply_transforms, DataOptions};
use vega_svg::expression::{evaluate, EmptyEnv};

fn run(values: Vec<Value>, transforms: Vec<Value>) -> (Vec<Value>, HashMap<String, Value>) {
    let mut signals = HashMap::new();
    let out = apply_transforms(values, &transforms, &mut signals).expect("apply transforms");
    (out, signals)
}

fn load(data: Value) -> vega_svg::data::DataStore {
    let entries = data.as_array().expect("data array").clone();
    let opts = DataOptions {
        base: Path::new("."),
        extra_dirs: vec![],
    };
    vega_svg::data::load(Some(&entries), &opts, &HashMap::new()).expect("load")
}

fn num(expr: &str) -> f64 {
    evaluate(expr, None, &EmptyEnv)
        .unwrap()
        .as_f64()
        .expect("number")
}

#[test]
fn bin_clamps_values_to_extent_and_bins_the_upper_edge_low() {
    let values = vec![json!({"v": -5}), json!({"v": 100}), json!({"v": 105})];
    let (out, signals) = run(
        values,
        vec![json!({
            "type": "bin", "field": "v", "extent": [0, 100], "maxbins": 10,
            "signal": "b", "as": ["b0", "b1"]
        })],
    );
    let bins = &signals["b"];
    assert_eq!(bins["start"].as_f64(), Some(0.0));
    assert_eq!(bins["stop"].as_f64(), Some(100.0));
    assert_eq!(bins["step"].as_f64(), Some(10.0));
    // Out-of-extent values become ±Infinity (serialized as null in JSON).
    assert!(out[0]["b0"].is_null(), "{:?}", out[0]);
    assert!(out[2]["b0"].is_null(), "{:?}", out[2]);
    // The extent maximum belongs to the last bin, not a new one.
    assert_eq!(out[1]["b0"].as_f64(), Some(90.0));
    assert_eq!(out[1]["b1"].as_f64(), Some(100.0));
}

#[test]
fn bin_uses_d3_step_selection_for_fine_steps() {
    let values = vec![json!({"v": 1.4}), json!({"v": 9.2})];
    let (out, signals) = run(
        values,
        vec![json!({
            "type": "bin", "field": "v", "extent": [1.4, 9.2], "maxbins": 60,
            "signal": "b", "as": ["b0", "b1"]
        })],
    );
    let bins = &signals["b"];
    let step = bins["step"].as_f64().unwrap();
    assert!((step - 0.2).abs() < 1e-9, "step {step}");
    // Extent nice-ing may start slightly below 1.4; the bin is still the first.
    let b0 = out[0]["b0"].as_f64().unwrap();
    assert!(b0 <= 1.4 + 1e-9 && b0 > 1.4 - step, "b0 {b0}");
    assert_eq!(out[1]["b0"].as_f64().unwrap(), 9.2 - step);
}

#[test]
fn kde_shared_extent_samples_a_uniform_curve() {
    let values = vec![
        json!({"g": "a", "v": 1.0}),
        json!({"g": "a", "v": 2.0}),
        json!({"g": "a", "v": 3.0}),
        json!({"g": "b", "v": 5.0}),
    ];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "kde", "field": "v", "groupby": ["g"], "extent": [0, 10],
            "resolve": "shared", "steps": 20, "as": ["value", "density"]
        })],
    );
    assert_eq!(out.len(), 42, "21 samples per group");
    for group in ["a", "b"] {
        let rows: Vec<&Value> = out.iter().filter(|d| d["g"] == json!(group)).collect();
        assert_eq!(rows.len(), 21);
        assert_eq!(rows[0]["value"].as_f64(), Some(0.0));
        assert_eq!(rows[20]["value"].as_f64(), Some(10.0));
        for row in rows {
            let density = row["density"].as_f64().expect("density");
            assert!(density.is_finite() && density >= 0.0, "{density}");
        }
    }
}

#[test]
fn density_normal_samples_pdf_over_extent() {
    let (out, _) = run(
        vec![json!({"x": 0})],
        vec![json!({
            "type": "density", "extent": [-2, 2], "steps": 9, "method": "pdf",
            "distribution": {"function": "normal", "mean": 0, "stdev": 1},
            "as": ["value", "density"]
        })],
    );
    assert_eq!(out.len(), 10);
    assert_eq!(out[0]["value"].as_f64(), Some(-2.0));
    assert_eq!(out[9]["value"].as_f64(), Some(2.0));
    let middle = out[4]["density"].as_f64().unwrap();
    assert!(middle > out[0]["density"].as_f64().unwrap());
    assert!(middle > 0.0 && middle < 1.0);
}

#[test]
fn window_frame_makes_cumulative_sums() {
    let values = vec![json!({"v": 1}), json!({"v": 2}), json!({"v": 3})];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "window", "ops": ["sum"], "fields": ["v"],
            "sort": {"field": ["v"], "order": ["ascending"]},
            "frame": [null, 0], "as": ["cum"]
        })],
    );
    assert_eq!(out[0]["cum"], json!(1));
    assert_eq!(out[1]["cum"], json!(3));
    assert_eq!(out[2]["cum"], json!(6));
}

#[test]
fn timeunit_year_uses_local_time() {
    // ISO date-only strings parse as UTC in JavaScript; the local year may thus
    // be the previous year. The expected value is computed with the expression
    // engine so the test is timezone independent.
    let local_year = num("year(toDate('1970-01-01'))");
    let expected_start = num(&format!("toDate('{}-01-01 00:00:00')", local_year));
    let expected_end = num(&format!("toDate('{}-01-01 00:00:00')", local_year + 1.0));
    let (out, _) = run(
        vec![json!({"d": 0})],
        vec![json!({
            "type": "timeunit", "field": "d", "units": ["year"], "as": ["y0", "y1"]
        })],
    );
    assert_eq!(out[0]["y0"].as_f64(), Some(expected_start));
    assert_eq!(out[0]["y1"].as_f64(), Some(expected_end));
}

#[test]
fn data_format_date_parses_rfc_dates_in_local_time() {
    // `Jan 1 2000` is a non-ISO date, which JavaScript's Date parses locally.
    let store = load(json!([{
        "name": "d",
        "values": [{"d": "Jan 1 2000"}],
        "format": {"parse": {"d": "date"}}
    }]));
    let value = store.get("d").unwrap()[0]["d"].as_f64().unwrap();
    let expected = num("toDate('2000-01-01 00:00:00')");
    assert_eq!(value, expected);
}
