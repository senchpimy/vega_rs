//! Regression tests for statically evaluated selection state.
//!
//! Vega-Lite specifications can ship a selection store that already contains
//! initial tuples (fixtures generated from a view with an active selection).
//! These tests cover the `vlSelection*` expression functions and the ability
//! of data transforms to query other datasets through `data(...)`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};
use vega_svg::data::{apply_transforms, load, DataOptions};
use vega_svg::expression::{evaluate, Environment, EmptyEnv};

struct StoreEnv {
    datasets: HashMap<String, Vec<Value>>,
}

impl StoreEnv {
    fn new(entries: &[(&str, Value)]) -> Self {
        let datasets = entries
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    value.as_array().cloned().unwrap_or_default(),
                )
            })
            .collect();
        Self { datasets }
    }
}

impl Environment for StoreEnv {
    fn signal(&self, _name: &str) -> Option<Value> {
        None
    }
    fn dataset(&self, name: &str) -> Option<Vec<Value>> {
        self.datasets.get(name).cloned()
    }
}

fn store(values: Value) -> StoreEnv {
    StoreEnv::new(&[("store", values)])
}

fn bool_of(expr: &str, datum: Value, env: &dyn Environment) -> bool {
    evaluate(expr, Some(&datum), env)
        .expect("evaluate")
        .as_bool()
        .expect("boolean result")
}

#[test]
fn selection_test_matches_enumerated_values() {
    let env = store(json!([{
        "unit": "layer_1",
        "fields": [{"field": "year", "type": "E"}],
        "values": [1955]
    }]));
    assert!(bool_of(
        "vlSelectionTest('store', datum)",
        json!({"year": 1955}),
        &env
    ));
    assert!(!bool_of(
        "vlSelectionTest('store', datum)",
        json!({"year": 1956}),
        &env
    ));
}

#[test]
fn selection_test_accepts_value_arrays() {
    let env = store(json!([{
        "unit": "u",
        "fields": [{"field": "origin", "type": "E"}],
        "values": [["USA", "Japan"]]
    }]));
    assert!(bool_of(
        "vlSelectionTest('store', datum)",
        json!({"origin": "Japan"}),
        &env
    ));
    assert!(!bool_of(
        "vlSelectionTest('store', datum)",
        json!({"origin": "Europe"}),
        &env
    ));
}

#[test]
fn selection_test_ranges_and_intersection() {
    let env = store(json!([
        {
            "unit": "u1",
            "fields": [{"field": "v", "type": "R"}],
            "values": [[2, 5]]
        },
        {
            "unit": "u1",
            "fields": [{"field": "v", "type": "R"}],
            "values": [[4, 8]]
        }
    ]));
    assert!(bool_of(
        "vlSelectionTest('store', datum, 'union')",
        json!({"v": 3}),
        &env
    ));
    assert!(bool_of(
        "vlSelectionTest('store', datum, 'union')",
        json!({"v": 7}),
        &env
    ));
    assert!(!bool_of(
        "vlSelectionTest('store', datum, 'union')",
        json!({"v": 9}),
        &env
    ));
    // Intersection requires the datum to fall inside every range.
    assert!(bool_of(
        "vlSelectionTest('store', datum, 'intersect')",
        json!({"v": 4}),
        &env
    ));
    assert!(!bool_of(
        "vlSelectionTest('store', datum, 'intersect')",
        json!({"v": 6}),
        &env
    ));
}

#[test]
fn selection_test_empty_store_is_false() {
    let env = store(json!([]));
    assert!(!bool_of(
        "vlSelectionTest('store', datum)",
        json!({"year": 1955}),
        &env
    ));
    // Unselected datasets are treated as all-data by the `!length(...)` guard.
    assert!(bool_of(
        "!length(data('store')) || vlSelectionTest('store', datum)",
        json!({"year": 1955}),
        &env
    ));
}

#[test]
fn selection_resolve_union_and_intersect() {
    let values = json!([
        {
            "unit": "u1",
            "fields": [{"field": "year", "type": "E"}],
            "values": [1955]
        },
        {
            "unit": "u2",
            "fields": [{"field": "year", "type": "E"}],
            "values": [1956]
        }
    ]);
    let env = store(values.clone());
    let union = evaluate("vlSelectionResolve('store', 'union')", None, &env).unwrap();
    assert_eq!(union["year"], json!([1955, 1956]));

    let env = store(values);
    let intersect = evaluate("vlSelectionResolve('store', 'intersect')", None, &env).unwrap();
    assert_eq!(intersect["year"], json!([]));
}

#[test]
fn selection_resolve_range_and_multi() {
    let env = store(json!([{
        "unit": "u1",
        "fields": [{"field": "v", "type": "R"}],
        "values": [[2, 5]]
    }]));
    let resolved =
        evaluate("vlSelectionResolve('store', 'union', true, true)", None, &env).unwrap();
    assert_eq!(resolved["v"], json!([2, 5]));
    assert_eq!(
        resolved["vlPoint"]["or"],
        json!([{"v": [2, 5]}])
    );

    let resolved =
        evaluate("vlSelectionResolve('store', 'intersect', true, false)", None, &env).unwrap();
    assert_eq!(
        resolved["vlMulti"]["and"],
        json!([{"or": [{"v": [2, 5]}]}])
    );
}

#[test]
fn selection_id_test_and_resolve() {
    let env = store(json!([
        {"unit": "u", "_vgsid_": 7},
        {"unit": "u", "_vgsid_": 8}
    ]));
    assert!(bool_of(
        "vlSelectionIdTest('store', datum)",
        json!({"_vgsid_": 8}),
        &env
    ));
    assert!(!bool_of(
        "vlSelectionIdTest('store', datum)",
        json!({"_vgsid_": 9}),
        &env
    ));
    let resolved = evaluate("vlSelectionResolve('store', 'union')", None, &env).unwrap();
    assert_eq!(resolved["_vgsid_"], json!([7, 8]));
}

#[test]
fn selection_tuples_expand_mark_items() {
    let env = EmptyEnv;
    let base = json!({
        "unit": "layer_1",
        "fields": [{"field": "y", "type": "E"}]
    });
    let items = json!([{"datum": {"y": 3}}, {"datum": {"y": 4}}]);
    let expr = format!(
        "vlSelectionTuples({}, {})",
        serde_json::to_string(&items).unwrap(),
        serde_json::to_string(&base).unwrap()
    );
    let value = evaluate(&expr, None, &env).unwrap();
    assert_eq!(
        value,
        json!([
            {"unit": "layer_1", "fields": [{"field": "y", "type": "E"}], "values": [3]},
            {"unit": "layer_1", "fields": [{"field": "y", "type": "E"}], "values": [4]}
        ])
    );
}

fn load_entries(entries: Value) -> vega_svg::data::DataStore {
    let entries = entries.as_array().expect("data array").clone();
    let opts = DataOptions {
        base: Path::new("."),
        extra_dirs: vec![],
    };
    load(Some(&entries), &opts, &HashMap::new()).expect("load")
}

#[test]
fn filters_can_query_other_datasets() {
    let store = load_entries(json!([
        {"name": "a", "values": [{"x": 1}, {"x": 2}]},
        {
            "name": "b",
            "source": "a",
            "transform": [{"type": "filter", "expr": "length(data('a')) === 2 && datum.x === 2"}]
        }
    ]));
    let rows = store.get("b").expect("b");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["x"], json!(2));
}

#[test]
fn initial_selection_store_filters_following_datasets() {
    let store = load_entries(json!([
        {
            "name": "year_store",
            "values": [{
                "unit": "layer_1",
                "fields": [{"type": "E", "field": "year"}],
                "values": [1955]
            }]
        },
        {
            "name": "rows",
            "values": [
                {"country": "A", "year": 1955},
                {"country": "A", "year": 1990},
                {"country": "B", "year": 1955}
            ],
            "transform": [
                {"type": "filter", "expr": "!length(data('year_store')) || vlSelectionTest('year_store', datum)"}
            ]
        }
    ]));
    let rows = store.get("rows").expect("rows");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r["year"] == json!(1955)));
}

#[test]
fn selection_lookup_uses_selected_rows() {
    let store = load_entries(json!([
        {
            "name": "index_store",
            "values": [{
                "unit": "layer_0",
                "fields": [{"field": "date", "type": "E"}],
                "values": [100]
            }]
        },
        {
            "name": "source",
            "values": [
                {"symbol": "A", "date": 100, "price": 10},
                {"symbol": "A", "date": 200, "price": 20}
            ]
        },
        {
            "name": "selected",
            "source": "source",
            "transform": [{
                "type": "filter",
                "expr": "!length(data('index_store')) || vlSelectionTest('index_store', datum)"
            }]
        },
        {
            "name": "joined",
            "source": "source",
            "transform": [
                {"type": "lookup", "from": "selected", "key": "symbol",
                 "fields": ["symbol"], "as": ["index"]},
                {"type": "formula", "expr": "datum.index ? datum.index.price : 0", "as": "base"}
            ]
        }
    ]));
    let rows = store.get("joined").expect("joined");
    assert_eq!(rows.len(), 2);
    // Both rows join against the selected date, not the first row overall.
    assert_eq!(rows[0]["base"], json!(10));
    assert_eq!(rows[1]["base"], json!(10));
}

#[test]
fn transforms_without_datasets_still_work() {
    let mut signals = HashMap::new();
    let out = apply_transforms(
        vec![json!({"v": 1}), json!({"v": 2})],
        &[json!({"type": "filter", "expr": "datum.v > 1"})],
        &mut signals,
    )
    .expect("apply");
    assert_eq!(out.len(), 1);
    let env = EmptyEnv;
    assert_eq!(
        evaluate("vlSelectionTest('missing', datum)", Some(&json!({"v": 1})), &env).unwrap(),
        Value::Bool(false)
    );
}
