#![allow(dead_code)]

//! Shared helpers for the mark compiler test suites.

use serde_json::{json, Value};

use vega_lite::compile::buildmodel::parse_model;
use vega_lite::compile::mark::mark::parse_mark_groups;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

pub fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    preserve_invalid_null(&mut model, spec);
    model
}

fn preserve_invalid_null(model: &mut Model, spec: &Value) {
    if spec
        .get("config")
        .and_then(|c| c.get("mark"))
        .and_then(|m| m.get("invalid"))
        .map(|v| v.is_null())
        .unwrap_or(false)
    {
        model.unit.mark_def["invalid"] = Value::Null;
    }
}

pub fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let mut model = parse_unit_model(spec);
    model.parse_scale();
    model
}

pub fn parse_unit_model_with_scale_and_layout(spec: &Value) -> Model {
    let mut model = parse_unit_model_with_scale(spec);
    model.parse_layout_size();
    model
}

pub fn parse_unit_model_with_scale_and_selection(spec: &Value) -> Model {
    let mut model = parse_unit_model_with_scale(spec);
    model.parse_selections();
    model
}

pub fn parse_facet_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_facet(spec, None, "", &config)
}

pub fn parse_concat_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_concat(spec, None, "", &config)
}

pub fn parse_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = parse_model(spec, &config);
    model.parse_scale();
    model
}

pub fn parse_unit_model_infer(spec: Value) -> Model {
    parse_unit_model(&spec)
}

pub fn unit_mark_groups(model: &Model) -> Vec<Value> {
    parse_mark_groups(model)
}

/// Jest-like `toMatchObject`: every property in `expected` must be present in
/// `actual` and recursively match; array lengths must agree.
pub fn assert_match_object(expected: &Value, actual: &Value) {
    match (expected, actual) {
        (Value::Object(exp), Value::Object(act)) => {
            for (key, value) in exp {
                let actual_value = act.get(key).unwrap_or_else(|| {
                    panic!(
                        "missing key {key:?}\nexpected: {}\nactual: {}",
                        serde_json::to_string_pretty(expected).unwrap(),
                        serde_json::to_string_pretty(actual).unwrap()
                    )
                });
                assert_match_object(value, actual_value);
            }
        }
        (Value::Array(exp), Value::Array(act)) => {
            assert_eq!(
                exp.len(),
                act.len(),
                "array length mismatch\nexpected: {}\nactual: {}",
                serde_json::to_string_pretty(expected).unwrap(),
                serde_json::to_string_pretty(actual).unwrap()
            );
            for (e, a) in exp.iter().zip(act.iter()) {
                assert_match_object(e, a);
            }
        }
        _ => {
            assert_eq!(
                expected,
                actual,
                "\nexpected: {}\nactual: {}",
                serde_json::to_string_pretty(expected).unwrap(),
                serde_json::to_string_pretty(actual).unwrap()
            );
        }
    }
}

pub fn assert_eq_json(expected: Value, actual: Value) {
    assert_eq!(
        expected,
        actual,
        "\nexpected: {}\nactual: {}",
        serde_json::to_string_pretty(&expected).unwrap(),
        serde_json::to_string_pretty(&actual).unwrap()
    );
}

pub fn assert_undefined(value: Option<&Value>) {
    assert!(value.is_none(), "expected undefined, got {value:?}");
}

pub fn empty_config() -> Value {
    json!({})
}
