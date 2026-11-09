//! Integration tests for the public `vega_svg::data` API.
//!
//! These tests exercise the standalone transform pipeline
//! (`apply_transforms`) and the data loader (`load`) with inline
//! specifications. No repository fixture files are needed; the CSV test
//! writes (and removes) a temporary file.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use vega_svg::data::{apply_transforms, DataOptions, DataStore};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run(values: Vec<Value>, transforms: Vec<Value>) -> (Vec<Value>, HashMap<String, Value>) {
    let mut signals = HashMap::new();
    let out = apply_transforms(values, &transforms, &mut signals).expect("apply transforms");
    (out, signals)
}

fn load_spec(data: &Value, base: &std::path::Path) -> DataStore {
    let data = data.as_array().expect("data array").clone();
    let opts = DataOptions {
        base,
        extra_dirs: Vec::new(),
    };
    load(data.as_slice(), &opts)
}

fn load(data: &[Value], opts: &DataOptions) -> DataStore {
    let signals = HashMap::new();
    vega_svg::data::load(Some(&data.to_vec()), opts, &signals).expect("load data")
}

/// Find the first row where `field` equals `value`.
fn find<'a>(rows: &'a [Value], field: &str, value: &Value) -> &'a Value {
    rows.iter()
        .find(|row| row.get(field) == Some(value))
        .unwrap_or_else(|| panic!("no row with {field} == {value}: {rows:?}"))
}

fn approx(actual: &Value, expected: f64) {
    let n = actual
        .as_f64()
        .unwrap_or_else(|| panic!("expected a number, got {actual}"));
    assert!(
        (n - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

// ---------------------------------------------------------------------------
// filter + formula
// ---------------------------------------------------------------------------

#[test]
fn filter_and_formula() {
    let values = vec![json!({"a": 1}), json!({"a": 2}), json!({"a": 3})];
    let transforms = vec![
        json!({"type": "filter", "expr": "datum.a >= 2"}),
        json!({"type": "formula", "expr": "datum.a * 10", "as": "b"}),
    ];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0]["a"], json!(2));
    assert_eq!(out[0]["b"], json!(20));
    assert_eq!(out[1]["a"], json!(3));
    assert_eq!(out[1]["b"], json!(30));
}

// ---------------------------------------------------------------------------
// aggregate
// ---------------------------------------------------------------------------

#[test]
fn aggregate_all_ops() {
    let values = vec![
        json!({"g": "x", "v": 1}),
        json!({"g": "x", "v": 3}),
        json!({"g": "x", "v": 5}),
        json!({"g": "x", "v": 5}),
        json!({"g": "y", "v": 2}),
    ];
    let transforms = vec![json!({
        "type": "aggregate",
        "groupby": ["g"],
        "ops": ["sum", "count", "mean", "min", "max", "median", "distinct"],
        "fields": ["v", null, "v", "v", "v", "v", "v"],
        "as": ["total", "n", "avg", "lo", "hi", "mid", "uniq"]
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 2, "{out:?}");

    let x = find(&out, "g", &json!("x"));
    assert_eq!(x["total"], json!(14));
    assert_eq!(x["n"], json!(4));
    approx(&x["avg"], 3.5);
    assert_eq!(x["lo"], json!(1));
    assert_eq!(x["hi"], json!(5));
    approx(&x["mid"], 4.0);
    assert_eq!(x["uniq"], json!(3));

    let y = find(&out, "g", &json!("y"));
    assert_eq!(y["total"], json!(2));
    assert_eq!(y["n"], json!(1));
    approx(&y["avg"], 2.0);
    assert_eq!(y["lo"], json!(2));
    assert_eq!(y["hi"], json!(2));
    approx(&y["mid"], 2.0);
    assert_eq!(y["uniq"], json!(1));
}

// ---------------------------------------------------------------------------
// joinaggregate
// ---------------------------------------------------------------------------

#[test]
fn joinaggregate_adds_group_totals_to_each_row() {
    let values = vec![
        json!({"g": "a", "v": 1}),
        json!({"g": "a", "v": 2}),
        json!({"g": "b", "v": 10}),
    ];
    let transforms = vec![json!({
        "type": "joinaggregate",
        "groupby": ["g"],
        "ops": ["sum"],
        "fields": ["v"],
        "as": ["group_total"]
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 3, "{out:?}");
    assert_eq!(out[0]["group_total"], json!(3));
    assert_eq!(out[1]["group_total"], json!(3));
    assert_eq!(out[2]["group_total"], json!(10));
}

// ---------------------------------------------------------------------------
// stack
// ---------------------------------------------------------------------------

#[test]
fn stack_offset_zero() {
    let values = vec![json!({"g": "a", "v": 2}), json!({"g": "a", "v": 3})];
    let transforms = vec![json!({
        "type": "stack",
        "groupby": ["g"],
        "field": "v",
        "offset": "zero",
        "as": ["s0", "s1"]
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0]["s0"], json!(0));
    assert_eq!(out[0]["s1"], json!(2));
    assert_eq!(out[1]["s0"], json!(2));
    assert_eq!(out[1]["s1"], json!(5));
}

#[test]
fn stack_offset_normalize() {
    let values = vec![
        json!({"g": "a", "v": 2}),
        json!({"g": "a", "v": 3}),
        json!({"g": "b", "v": 1}),
        json!({"g": "b", "v": 1}),
    ];
    let transforms = vec![json!({
        "type": "stack",
        "groupby": ["g"],
        "field": "v",
        "offset": "normalize",
        "as": ["s0", "s1"]
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 4, "{out:?}");

    // Group "a": 2/5 then 3/5.
    approx(&out[0]["s0"], 0.0);
    approx(&out[0]["s1"], 0.4);
    approx(&out[1]["s0"], 0.4);
    approx(&out[1]["s1"], 1.0);

    // Group "b": two equal halves.
    approx(&out[2]["s0"], 0.0);
    approx(&out[2]["s1"], 0.5);
    approx(&out[3]["s0"], 0.5);
    approx(&out[3]["s1"], 1.0);
}

// ---------------------------------------------------------------------------
// window
// ---------------------------------------------------------------------------

#[test]
fn window_rank_row_number_and_lag() {
    let values = vec![
        json!({"x": 10}),
        json!({"x": 20}),
        json!({"x": 20}),
        json!({"x": 30}),
    ];
    let transforms = vec![json!({
        "type": "window",
        "sort": {"field": "x"},
        "ops": ["row_number", "rank", "lag"],
        "fields": [null, "x", "x"],
        "as": ["rn", "rk", "prev"]
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 4, "{out:?}");
    let expected_rn = [1, 2, 3, 4];
    let expected_rank = [1, 2, 2, 4];
    let expected_lag = [Value::Null, json!(10), json!(20), json!(20)];

    for (i, row) in out.iter().enumerate() {
        assert_eq!(row["rn"], json!(expected_rn[i]), "row {i}");
        assert_eq!(row["rk"], json!(expected_rank[i]), "row {i}");
        assert_eq!(row["prev"], expected_lag[i], "row {i}");
    }
}

// ---------------------------------------------------------------------------
// extent signal
// ---------------------------------------------------------------------------

#[test]
fn extent_stores_signal() {
    let values = vec![json!({"v": 3}), json!({"v": 7}), json!({"v": 1})];
    let (out, signals) = run(
        values,
        vec![json!({"type": "extent", "field": "v", "signal": "vext"})],
    );

    assert_eq!(out.len(), 3, "extent should not change rows");
    // `extent` stores raw f64 values (not normalized to JSON integers).
    assert_eq!(signals.get("vext"), Some(&json!([1.0, 7.0])));
}

// ---------------------------------------------------------------------------
// bin
// ---------------------------------------------------------------------------

#[test]
fn bin_creates_bin_bounds_and_signal() {
    let values = vec![json!({"x": 1}), json!({"x": 9})];
    let (out, signals) = run(
        values,
        vec![json!({
            "type": "bin",
            "field": "x",
            "maxbins": 5,
            "as": ["b0", "b1"],
            "signal": "bins"
        })],
    );

    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0]["b0"], json!(0), "{out:?}");
    assert_eq!(out[0]["b1"], json!(2), "{out:?}");
    assert_eq!(out[1]["b0"], json!(8), "{out:?}");
    assert_eq!(out[1]["b1"], json!(10), "{out:?}");

    let bins = signals.get("bins").expect("bins signal");
    assert_eq!(bins["start"], json!(0.0));
    assert_eq!(bins["stop"], json!(10.0));
    assert_eq!(bins["step"], json!(2.0));
    assert_eq!(bins["bins"], json!([0, 2, 4, 6, 8]));
}

// ---------------------------------------------------------------------------
// timeunit
// ---------------------------------------------------------------------------

#[test]
fn timeunit_year_start_and_end() {
    // 2020-06-15T00:00:00Z
    let values = vec![json!({"d": 1_592_179_200_000_i64})];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "timeunit",
            "field": "d",
            "units": ["year"],
            "as": ["y0", "y1"]
        })],
    );

    // Local midnight boundaries (d3-time's local intervals).
    let start = vega_svg::expression::evaluate(
        "toDate('2020-01-01 00:00:00')",
        None,
        &vega_svg::expression::EmptyEnv,
    )
    .unwrap()
    .as_f64()
    .unwrap() as i64;
    let end = vega_svg::expression::evaluate(
        "toDate('2021-01-01 00:00:00')",
        None,
        &vega_svg::expression::EmptyEnv,
    )
    .unwrap()
    .as_f64()
    .unwrap() as i64;
    assert_eq!(out[0]["y0"], json!(start), "{out:?}");
    assert_eq!(out[0]["y1"], json!(end), "{out:?}");
}

#[test]
fn timeunit_month_start_and_end() {
    // 2020-06-15T00:00:00Z
    let values = vec![json!({"d": 1_592_179_200_000_i64})];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "timeunit",
            "field": "d",
            "units": ["year", "month"],
            "as": ["m0", "m1"]
        })],
    );

    let start = vega_svg::expression::evaluate(
        "toDate('2020-06-01 00:00:00')",
        None,
        &vega_svg::expression::EmptyEnv,
    )
    .unwrap()
    .as_f64()
    .unwrap() as i64;
    let end = vega_svg::expression::evaluate(
        "toDate('2020-07-01 00:00:00')",
        None,
        &vega_svg::expression::EmptyEnv,
    )
    .unwrap()
    .as_f64()
    .unwrap() as i64;
    assert_eq!(out[0]["m0"], json!(start), "{out:?}");
    assert_eq!(out[0]["m1"], json!(end), "{out:?}");
}

// ---------------------------------------------------------------------------
// impute
// ---------------------------------------------------------------------------

#[test]
fn impute_fills_missing_keys() {
    let values = vec![
        json!({"g": "x", "k": "a", "v": 1}),
        json!({"g": "x", "k": "b", "v": 2}),
        json!({"g": "y", "k": "a", "v": 3}),
    ];
    let transforms = vec![json!({
        "type": "impute",
        "field": "v",
        "key": "k",
        "groupby": ["g"],
        "method": "value",
        "value": 0
    })];
    let (out, _) = run(values, transforms);

    assert_eq!(out.len(), 4, "one missing (g=y, k=b) row: {out:?}");
    let filled = out
        .iter()
        .find(|row| row.get("g") == Some(&json!("y")) && row.get("k") == Some(&json!("b")))
        .expect("imputed row for g=y, k=b");
    assert_eq!(filled["v"], json!(0));
}

// ---------------------------------------------------------------------------
// fold
// ---------------------------------------------------------------------------

#[test]
fn fold_turns_fields_into_key_value_rows() {
    let values = vec![json!({"a": 1, "b": 2}), json!({"a": 3, "b": 4})];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "fold",
            "fields": ["a", "b"],
            "as": ["key", "value"]
        })],
    );

    assert_eq!(out.len(), 4, "{out:?}");
    assert_eq!(out[0]["key"], json!("a"));
    assert_eq!(out[0]["value"], json!(1));
    assert_eq!(out[1]["key"], json!("b"));
    assert_eq!(out[1]["value"], json!(2));
    assert_eq!(out[2]["key"], json!("a"));
    assert_eq!(out[2]["value"], json!(3));
    assert_eq!(out[3]["key"], json!("b"));
    assert_eq!(out[3]["value"], json!(4));
}

// ---------------------------------------------------------------------------
// flatten
// ---------------------------------------------------------------------------

#[test]
fn flatten_expands_arrays() {
    let values = vec![
        json!({"name": "x", "arr": [1, 2]}),
        json!({"name": "y", "arr": [3]}),
    ];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "flatten",
            "fields": ["arr"],
            "as": ["item"]
        })],
    );

    assert_eq!(out.len(), 3, "{out:?}");
    assert_eq!(out[0]["name"], json!("x"));
    assert_eq!(out[0]["item"], json!(1));
    assert_eq!(out[1]["name"], json!("x"));
    assert_eq!(out[1]["item"], json!(2));
    assert_eq!(out[2]["name"], json!("y"));
    assert_eq!(out[2]["item"], json!(3));
}

// ---------------------------------------------------------------------------
// sequence
// ---------------------------------------------------------------------------

#[test]
fn sequence_produces_values() {
    let (out, _) = run(
        Vec::new(),
        vec![json!({
            "type": "sequence",
            "start": 0,
            "stop": 5,
            "step": 1,
            "as": "x"
        })],
    );

    assert_eq!(out.len(), 5, "{out:?}");
    for (i, row) in out.iter().enumerate() {
        // Sequence values are stored as raw f64s.
        assert_eq!(row["x"], json!(i as f64), "row {i}");
    }
}

// ---------------------------------------------------------------------------
// project
// ---------------------------------------------------------------------------

#[test]
fn project_selects_and_renames_fields() {
    let values = vec![json!({"a": 1, "b": 2, "c": 3})];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "project",
            "fields": ["a", "b"],
            "as": ["x", "y"]
        })],
    );

    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0], json!({"x": 1, "y": 2}));
    assert!(out[0].get("c").is_none(), "{out:?}");
}

// ---------------------------------------------------------------------------
// pivot
// ---------------------------------------------------------------------------

#[test]
fn pivot_creates_columns_from_key_field() {
    let values = vec![
        json!({"g": "x", "k": "a", "v": 1}),
        json!({"g": "x", "k": "b", "v": 2}),
        json!({"g": "y", "k": "a", "v": 3}),
    ];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "pivot",
            "groupby": ["g"],
            "field": "k",
            "value": "v"
        })],
    );

    assert_eq!(out.len(), 2, "{out:?}");
    let x = find(&out, "g", &json!("x"));
    assert_eq!(x["a"], json!(1));
    assert_eq!(x["b"], json!(2));

    let y = find(&out, "g", &json!("y"));
    assert_eq!(y["a"], json!(3));
    assert!(y.get("b").is_none(), "no b column for group y: {out:?}");
}

// ---------------------------------------------------------------------------
// CSV via load (temporary file)
// ---------------------------------------------------------------------------

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn load_parses_csv_from_a_file() {
    let path = std::env::temp_dir().join(format!("vega_svg_transforms_{}.csv", std::process::id()));
    let _guard = TempFile(path.clone());
    std::fs::write(&path, "a,b\n1,2\n3,4\n").expect("write temp csv");

    let data = json!([{
        "name": "t",
        "url": path.to_string_lossy(),
        "format": {"type": "csv", "parse": {"b": "number"}}
    }]);
    let store = load_spec(&data, std::env::temp_dir().as_path());

    let rows = store.get("t").expect("t dataset");
    assert_eq!(rows.len(), 2, "{rows:?}");
    // Unparsed CSV fields stay strings; `format.parse` converts `b`.
    assert_eq!(rows[0]["a"], json!("1"));
    assert_eq!(rows[0]["b"], json!(2));
    assert_eq!(rows[1]["a"], json!("3"));
    assert_eq!(rows[1]["b"], json!(4));
}

// ---------------------------------------------------------------------------
// format.parse on inline JSON values
// ---------------------------------------------------------------------------

#[test]
fn format_parse_converts_date_strings_to_epoch_millis() {
    let data = json!([{
        "name": "t",
        "values": [
            {"d": "2020-01-01T00:00:00Z"},
            {"d": "2020-01-02"}
        ],
        "format": {"parse": {"d": "date"}}
    }]);
    let store = load_spec(&data, std::path::Path::new("."));

    let rows = store.get("t").expect("t dataset");
    assert_eq!(rows.len(), 2, "{rows:?}");
    // 2020-01-01T00:00:00Z and the following midnight.
    assert_eq!(rows[0]["d"], json!(1_577_836_800_000_i64));
    assert_eq!(rows[1]["d"], json!(1_577_923_200_000_i64));
}

// ---------------------------------------------------------------------------
// load wires transforms and signal outputs together
// ---------------------------------------------------------------------------

#[test]
fn load_applies_transforms_and_exposes_signals() {
    let data = json!([{
        "name": "t",
        "values": [{"v": 3}, {"v": 7}, {"v": 1}],
        "transform": [
            {"type": "extent", "field": "v", "signal": "vext"},
            {"type": "formula", "expr": "datum.v * 2", "as": "double"}
        ]
    }]);
    let store = load_spec(&data, std::path::Path::new("."));

    let rows = store.get("t").expect("t dataset");
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(rows[0]["double"], json!(6));
    assert_eq!(rows[1]["double"], json!(14));
    assert_eq!(rows[2]["double"], json!(2));
    assert_eq!(store.signals.get("vext"), Some(&json!([1.0, 7.0])));
}
