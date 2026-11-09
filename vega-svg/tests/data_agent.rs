//! Regression tests for data/expression fixes.
//!
//! Focus areas: null handling in extent/bin/aggregate, lookup joins, and
//! expression functions used by compiled Vega-Lite specs (inrange with a
//! range array, timeUnitSpecifier, toDate, timeOffset).

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

#[test]
fn extent_ignores_null_and_invalid_values() {
    let values = vec![
        json!({"v": 2}),
        json!({"v": null}),
        json!({"v": "nope"}),
        json!({"other": 9}),
        json!({"v": 8}),
    ];
    let (_, signals) = run(
        values,
        vec![json!({"type": "extent", "field": "v", "signal": "e"})],
    );
    let extent = signals["e"].as_array().expect("extent array");
    assert_eq!(extent[0].as_f64(), Some(2.0));
    assert_eq!(extent[1].as_f64(), Some(8.0));
}

#[test]
fn bin_skips_null_values_and_uses_valid_extent() {
    let values = vec![
        json!({"v": 1.6}),
        json!({"v": null}),
        json!({"v": 9.2}),
    ];
    let (out, signals) = run(
        values,
        vec![
            json!({"type": "extent", "field": "v", "signal": "e"}),
            json!({
                "type": "bin", "field": "v", "extent": {"signal": "e"},
                "maxbins": 10, "signal": "b", "as": ["b0", "b1"]
            }),
        ],
    );
    let bins = &signals["b"];
    assert_eq!(bins["start"].as_f64(), Some(1.0));
    assert_eq!(bins["stop"].as_f64(), Some(10.0));
    assert_eq!(out[0]["b0"].as_f64(), Some(1.0));
    assert!(out[1].get("b0").is_none(), "{:?}", out[1]);
    assert_eq!(out[2]["b0"].as_f64(), Some(9.0));
}

#[test]
fn aggregate_ignores_null_values() {
    let values = vec![
        json!({"g": "a", "v": 1}),
        json!({"g": "a", "v": null}),
        json!({"g": "a", "v": 3}),
    ];
    let (out, _) = run(
        values,
        vec![json!({
            "type": "aggregate",
            "groupby": ["g"],
            "ops": ["mean", "sum", "min", "max", "valid", "missing"],
            "fields": ["v", "v", "v", "v", "v", "v"],
            "as": ["mean", "sum", "min", "max", "valid", "missing"],
        })],
    );
    assert_eq!(out[0]["mean"], json!(2));
    assert_eq!(out[0]["sum"], json!(4));
    assert_eq!(out[0]["min"], json!(1));
    assert_eq!(out[0]["max"], json!(3));
    assert_eq!(out[0]["valid"], json!(2));
    assert_eq!(out[0]["missing"], json!(1));
}

#[test]
fn lookup_joins_fields_from_another_dataset() {
    let store = load(json!([
        {"name": "people", "values": [
            {"name": "Alan", "age": 25, "height": 180},
            {"name": "Fred", "age": 39, "height": 182}
        ]},
        {"name": "groups", "values": [
            {"group": 1, "person": "Alan"},
            {"group": 2, "person": "Fred"},
            {"group": 3, "person": "Nobody"}
        ], "transform": [
            {"type": "lookup", "from": "people", "key": "name",
             "fields": ["person"], "values": ["age", "height"]}
        ]}
    ]));
    let rows = store.get("groups").expect("groups");
    assert_eq!(rows[0]["age"], json!(25));
    assert_eq!(rows[0]["height"], json!(180));
    assert_eq!(rows[1]["age"], json!(39));
    assert_eq!(rows[2]["age"], Value::Null);
}

#[test]
fn lookup_supports_default_values() {
    let store = load(json!([
        {"name": "people", "values": [{"name": "Alan", "age": 25}]},
        {"name": "groups", "values": [{"person": "Nobody"}], "transform": [
            {"type": "lookup", "from": "people", "key": "name",
             "fields": ["person"], "values": ["age"], "default": 0}
        ]}
    ]));
    assert_eq!(store.get("groups").unwrap()[0]["age"], json!(0));
}

#[test]
fn inrange_accepts_range_array() {
    let env = EmptyEnv;
    assert_eq!(
        evaluate("inrange(3, [1, 5])", None, &env).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate("inrange(9, [1, 5])", None, &env).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate("inrange(3, 5, 1)", None, &env).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn time_unit_specifier_matches_vega() {
    let env = EmptyEnv;
    assert_eq!(
        evaluate("timeUnitSpecifier(['month'], {})", None, &env).unwrap(),
        Value::String("%b".into())
    );
    assert_eq!(
        evaluate("timeUnitSpecifier(['year', 'month'], {})", None, &env).unwrap(),
        Value::String("%Y-%m".into())
    );
    assert_eq!(
        evaluate("timeUnitSpecifier(['year', 'week'], {})", None, &env).unwrap(),
        Value::String("%Y W%U".into())
    );
    assert_eq!(
        evaluate(
            "timeUnitSpecifier(['year', 'month'], {'year-month': '%b %Y '})",
            None,
            &env
        )
        .unwrap(),
        Value::String("%b %Y".into())
    );
}

#[test]
fn to_date_and_time_offset() {
    let env = EmptyEnv;
    assert_eq!(
        evaluate(
            "year(toDate('2020-06-15T12:00:00')) === 2020 && month(toDate('2020-06-15T12:00:00')) === 5 && date(toDate('2020-06-15T12:00:00')) === 15",
            None,
            &env
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate(
            "timeOffset('month', datetime(2020, 0, 31), 1) === datetime(2020, 1, 29)",
            None,
            &env
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate(
            "timeOffset('date', datetime(2020, 0, 1), 2) === datetime(2020, 0, 3)",
            None,
            &env
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate(
            "timeOffset('year', datetime(2020, 1, 29), 1) === datetime(2021, 1, 28)",
            None,
            &env
        )
        .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn time_returns_millis_on_a_stable_timeline() {
    let env = EmptyEnv;
    // `time()` uses the same local-wall timeline as the local date accessors,
    // so differences remain exact day lengths.
    let value = evaluate(
        "time(datetime(1970, 0, 2)) - time(datetime(1970, 0, 1))",
        None,
        &env,
    )
    .unwrap();
    assert_eq!(value.as_f64(), Some(86_400_000.0));
}

#[test]
fn timeunit_normalizes_to_reference_year() {
    let env = EmptyEnv;
    let num = |expr: &str| evaluate(expr, None, &env).unwrap().as_f64().unwrap();
    // Local midnights (unzoned date-time strings parse in local time).
    let monday = num("toDate('2013-07-15 00:00:00')");
    let sunday = num("toDate('2013-07-21 00:00:00')");
    let store = load(json!([{
        "name": "d",
        "values": [{"t": monday}, {"t": sunday}],
        "transform": [
            {"type": "timeunit", "field": "t", "units": ["day"], "as": ["day", "day_end"]},
            {"type": "timeunit", "field": "t", "units": ["hours"], "as": ["hour", "hour_end"]}
        ]
    }]));
    let rows = store.get("d").expect("rows");
    // Day-of-week maps into 2012 (2012-01-01 was a Sunday). Timeunit output
    // is local midnight of the reference year (d3-time's local intervals).
    assert_eq!(rows[0]["day"].as_f64(), Some(num("toDate('2012-01-02 00:00:00')")));
    assert_eq!(rows[1]["day"].as_f64(), Some(num("toDate('2012-01-01 00:00:00')")));
    // Hour-only units also use the reference year rather than the input date.
    assert_eq!(rows[0]["hour"].as_f64(), Some(num("toDate('2012-01-01 00:00:00')")));
    assert_eq!(rows[1]["hour"].as_f64(), Some(num("toDate('2012-01-01 00:00:00')")));
}

#[test]
fn month_timeunit_end_is_next_month() {
    let env = EmptyEnv;
    let num = |expr: &str| evaluate(expr, None, &env).unwrap().as_f64().unwrap();
    let store = load(json!([{
        "name": "d",
        "values": [{"t": num("datetime(2015, 0, 15)")}],
        "transform": [
            {"type": "timeunit", "field": "t", "units": ["month"], "as": ["m", "m_end"]}
        ]
    }]));
    let row = &store.get("d").unwrap()[0];
    assert_eq!(row["m"].as_f64(), Some(num("toDate('2012-01-01 00:00:00')")));
    assert_eq!(row["m_end"].as_f64(), Some(num("toDate('2012-02-01 00:00:00')")));
}

#[test]
fn time_format_quarter_directive() {
    let env = EmptyEnv;
    assert_eq!(
        evaluate(
            "timeFormat(datetime(2020, 4, 15), timeUnitSpecifier(['quarter'], {}))",
            None,
            &env
        )
        .unwrap(),
        Value::String("Q2".into())
    );
}
