//! Port of `test/normalize/toplevelselection.test.ts`.

use serde_json::{json, Value};
use vega_lite::normalize::base::NormalizerParams;
use vega_lite::normalize::normalize;
use vega_lite::normalize::toplevelselection::map as selection_map;

fn unit() -> Value {
    json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    })
}

#[test]
fn should_push_top_level_selections_to_all_units_by_default() {
    let spec = json!({
        "params": [
            {"name": "one", "select": "point"},
            {"name": "two", "select": "point"},
            {"name": "three", "select": "interval"},
            {"name": "four", "expr": "true"},
            {"name": "five", "bind": {"input": "range"}}
        ],
        "vconcat": [unit(), unit()]
    });

    let params = NormalizerParams::with_config(Value::Null);
    let normalized = selection_map(&spec, &params);
    assert_eq!(normalized["params"].as_array().unwrap().len(), 2);
    assert_eq!(normalized["vconcat"][0]["params"].as_array().unwrap().len(), 3);
    assert_eq!(normalized["vconcat"][1]["params"].as_array().unwrap().len(), 3);
}

#[test]
fn should_push_top_level_selections_to_given_units() {
    let mut a = unit();
    a["name"] = json!("a");
    let mut b = unit();
    b["name"] = json!("b");
    let mut c = unit();
    c["name"] = json!("c");
    let spec = json!({
        "params": [
            {"name": "one", "select": "point", "views": ["a"]},
            {"name": "two", "select": "point", "views": ["b"]},
            {"name": "three", "select": "interval", "views": ["a", "b"]},
            {"name": "four", "expr": "true"},
            {"name": "five", "bind": {"input": "range"}}
        ],
        "vconcat": [a, b, c, unit()]
    });

    let params = NormalizerParams::with_config(Value::Null);
    let normalized = selection_map(&spec, &params);
    assert_eq!(normalized["params"].as_array().unwrap().len(), 2);
    assert_eq!(normalized["vconcat"][0]["params"].as_array().unwrap().len(), 2);
    assert_eq!(normalized["vconcat"][1]["params"].as_array().unwrap().len(), 2);
    assert!(normalized["vconcat"][2].get("params").is_none());
    assert!(normalized["vconcat"][3].get("params").is_none());
    assert_eq!(normalized["vconcat"][0]["params"][0]["name"], json!("one"));
    assert_eq!(normalized["vconcat"][0]["params"][1]["name"], json!("three"));
    assert_eq!(normalized["vconcat"][1]["params"][0]["name"], json!("two"));
    assert_eq!(normalized["vconcat"][1]["params"][1]["name"], json!("three"));
}

#[test]
fn should_handle_nested_paths() {
    let mut a = unit();
    a["name"] = json!("a");
    let mut b = json!({
        "name": "b",
        "layer": [
            {
                "name": "c",
                "data": {"url": "data/cars.json"},
                "mark": "point",
                "encoding": {
                    "x": {"field": "Horsepower", "type": "quantitative"},
                    "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
                }
            },
            unit()
        ]
    });
    b["layer"] = json!([
        {
            "name": "c",
            "data": {"url": "data/cars.json"},
            "mark": "point",
            "encoding": {
                "x": {"field": "Horsepower", "type": "quantitative"},
                "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
            }
        },
        unit()
    ]);

    let spec = json!({
        "params": [
            {"name": "one", "select": "point", "views": ["child__row_Acceleration"]},
            {"name": "two", "select": "point", "views": ["child__row_Horsepower_b"]},
            {
                "name": "three",
                "select": "interval",
                "views": ["child__row_Horsepower_a", "child__row_Horsepower_c", "child__row_Acceleration_a"]
            },
            {"name": "four", "expr": "true"},
            {"name": "five", "bind": {"input": "range"}}
        ],
        "repeat": {"row": ["Horsepower", "Acceleration"]},
        "spec": {
            "layer": [
                a,
                b
            ]
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert_eq!(normalized["params"].as_array().unwrap().len(), 2);

    assert_eq!(normalized["concat"][0]["name"], json!("child__row_Horsepower"));
    assert!(normalized["concat"][0].get("params").is_none());

    assert_eq!(
        normalized["concat"][0]["layer"][0]["name"],
        json!("child__row_Horsepower_a")
    );
    assert_eq!(
        normalized["concat"][0]["layer"][0]["params"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        normalized["concat"][0]["layer"][0]["params"][0]["name"],
        json!("three")
    );

    assert_eq!(
        normalized["concat"][0]["layer"][1]["name"],
        json!("child__row_Horsepower_b")
    );
    assert!(normalized["concat"][0]["layer"][1]
        .get("params")
        .is_none());
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][0]["name"],
        json!("child__row_Horsepower_c")
    );
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][0]["params"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][0]["params"][0]["name"],
        json!("two")
    );
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][0]["params"][1]["name"],
        json!("three")
    );
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][1]["params"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        normalized["concat"][0]["layer"][1]["layer"][1]["params"][0]["name"],
        json!("two")
    );

    assert_eq!(
        normalized["concat"][1]["name"],
        json!("child__row_Acceleration")
    );
    assert!(normalized["concat"][1].get("params").is_none());

    assert_eq!(
        normalized["concat"][1]["layer"][0]["name"],
        json!("child__row_Acceleration_a")
    );
    assert_eq!(
        normalized["concat"][1]["layer"][0]["params"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        normalized["concat"][1]["layer"][0]["params"][0]["name"],
        json!("one")
    );
    assert_eq!(
        normalized["concat"][1]["layer"][0]["params"][1]["name"],
        json!("three")
    );

    assert_eq!(
        normalized["concat"][1]["layer"][1]["name"],
        json!("child__row_Acceleration_b")
    );
    assert!(normalized["concat"][1]["layer"][1]
        .get("params")
        .is_none());
    assert_eq!(
        normalized["concat"][1]["layer"][1]["layer"][0]["name"],
        json!("child__row_Acceleration_c")
    );
    assert_eq!(
        normalized["concat"][1]["layer"][1]["layer"][0]["params"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        normalized["concat"][1]["layer"][1]["layer"][0]["params"][0]["name"],
        json!("one")
    );
    assert_eq!(
        normalized["concat"][1]["layer"][1]["layer"][1]["params"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        normalized["concat"][1]["layer"][1]["layer"][1]["params"][0]["name"],
        json!("one")
    );
}
