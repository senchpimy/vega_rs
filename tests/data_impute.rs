//! Port of `test/compile/data/impute.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, PlaceholderDataFlowNode,
};
use vega_lite::compile::data::impute::ImputeNode;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_unit_model_with_scale(spec: &serde_json::Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

#[test]
fn should_return_a_proper_vg_transform() {
    let impute = ImputeNode::new(
        None,
        &json!({"impute": "y", "key": "x", "method": "value", "value": 200}),
    );
    assert_eq!(
        assemble(&impute),
        json!([{"type": "impute", "field": "y", "key": "x", "method": "value", "value": 200}])
    );
}

#[test]
fn should_use_keyvals_and_mean_correctly() {
    let impute = ImputeNode::new(
        None,
        &json!({"impute": "y", "key": "x", "keyvals": [2, 3], "method": "mean"}),
    );
    assert_eq!(
        assemble(&impute),
        json!([
            {"type": "impute", "field": "y", "key": "x", "keyvals": [2, 3], "method": "value", "value": null},
            {
                "type": "window",
                "as": ["imputed_y_value"],
                "ops": ["mean"],
                "fields": ["y"],
                "frame": [null, null],
                "ignorePeers": false
            },
            {"type": "formula", "expr": "datum.y === null ? datum.imputed_y_value : datum.y", "as": "y"}
        ])
    );
}

#[test]
fn should_handle_every_property_correctly() {
    let impute = ImputeNode::new(
        None,
        &json!({
            "impute": "y",
            "key": "x",
            "keyvals": [3, 5],
            "method": "max",
            "groupby": ["a", "b"]
        }),
    );
    assert_eq!(
        assemble(&impute),
        json!([
            {"type": "impute", "field": "y", "key": "x", "keyvals": [3, 5], "method": "value", "groupby": ["a", "b"], "value": null},
            {
                "type": "window",
                "as": ["imputed_y_value"],
                "ops": ["max"],
                "fields": ["y"],
                "frame": [null, null],
                "ignorePeers": false,
                "groupby": ["a", "b"]
            },
            {"type": "formula", "expr": "datum.y === null ? datum.imputed_y_value : datum.y", "as": "y"}
        ])
    );
}

#[test]
fn should_handle_sequence_keyvals() {
    let impute = ImputeNode::new(
        None,
        &json!({
            "impute": "y",
            "key": "x",
            "keyvals": {"start": 3, "stop": 5},
            "method": "max",
            "groupby": ["a", "b"]
        }),
    );
    assert_eq!(
        assemble(&impute),
        json!([
            {"type": "impute", "field": "y", "key": "x", "keyvals": {"signal": "sequence(3,5)"}, "method": "value", "groupby": ["a", "b"], "value": null},
            {
                "type": "window",
                "as": ["imputed_y_value"],
                "ops": ["max"],
                "fields": ["y"],
                "frame": [null, null],
                "ignorePeers": false,
                "groupby": ["a", "b"]
            },
            {"type": "formula", "expr": "datum.y === null ? datum.imputed_y_value : datum.y", "as": "y"}
        ])
    );
}

#[test]
fn should_handle_sequence_keyvals_without_start() {
    let impute = ImputeNode::new(
        None,
        &json!({
            "impute": "y",
            "key": "x",
            "keyvals": {"stop": 5},
            "method": "max",
            "groupby": ["a", "b"]
        }),
    );
    assert_eq!(
        assemble(&impute),
        json!([
            {"type": "impute", "field": "y", "key": "x", "keyvals": {"signal": "sequence(0,5)"}, "method": "value", "groupby": ["a", "b"], "value": null},
            {
                "type": "window",
                "as": ["imputed_y_value"],
                "ops": ["max"],
                "fields": ["y"],
                "frame": [null, null],
                "ignorePeers": false,
                "groupby": ["a", "b"]
            },
            {"type": "formula", "expr": "datum.y === null ? datum.imputed_y_value : datum.y", "as": "y"}
        ])
    );
}

#[test]
fn should_handle_window_correctly() {
    let impute = ImputeNode::new(
        None,
        &json!({
            "impute": "y",
            "key": "x",
            "method": "max",
            "groupby": ["a", "b"],
            "frame": [-2, 2]
        }),
    );
    assert_eq!(
        assemble(&impute),
        json!([
            {"type": "impute", "field": "y", "key": "x", "method": "value", "groupby": ["a", "b"], "value": null},
            {
                "type": "window",
                "as": ["imputed_y_value"],
                "ops": ["max"],
                "fields": ["y"],
                "frame": [-2, 2],
                "ignorePeers": false,
                "groupby": ["a", "b"]
            },
            {"type": "formula", "expr": "datum.y === null ? datum.imputed_y_value : datum.y", "as": "y"}
        ])
    );
}

#[test]
fn should_generate_the_correct_hash() {
    let impute = ImputeNode::new(
        None,
        &json!({"impute": "y", "key": "x", "method": "value", "value": 200}),
    );
    assert_eq!(
        hash(&impute),
        json!("Impute {\"impute\":\"y\",\"key\":\"x\",\"method\":\"value\",\"value\":200}")
    );
}

#[test]
fn encoding_should_work_for_value_impute() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"value": 500}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([{"type": "impute", "field": "variety", "key": "yield", "method": "value", "groupby": ["site"], "value": 500}])
    );
}

#[test]
fn encoding_should_work_for_falsy_value_impute() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"value": 0}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([{"type": "impute", "field": "variety", "key": "yield", "method": "value", "groupby": ["site"], "value": 0}])
    );
}

#[test]
fn encoding_should_work_for_method_impute() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"method": "max"}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([
            {"type": "impute", "field": "variety", "key": "yield", "method": "value", "groupby": ["site"], "value": null},
            {
                "type": "window",
                "as": ["imputed_variety_value"],
                "ops": ["max"],
                "fields": ["variety"],
                "frame": [null, null],
                "ignorePeers": false,
                "groupby": ["site"]
            },
            {"type": "formula", "expr": "datum.variety === null ? datum.imputed_variety_value : datum.variety", "as": "variety"}
        ])
    );
}

#[test]
fn encoding_should_handle_sequence_keyvals() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"method": "max", "keyvals": {"start": 3, "stop": 5}}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([
            {"type": "impute", "field": "variety", "key": "yield", "keyvals": {"signal": "sequence(3,5)"}, "method": "value", "groupby": ["site"], "value": null},
            {
                "type": "window",
                "as": ["imputed_variety_value"],
                "ops": ["max"],
                "fields": ["variety"],
                "frame": [null, null],
                "ignorePeers": false,
                "groupby": ["site"]
            },
            {"type": "formula", "expr": "datum.variety === null ? datum.imputed_variety_value : datum.variety", "as": "variety"}
        ])
    );
}

#[test]
fn encoding_should_work_when_method_and_frame_are_specified() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"method": "mean", "frame": [-2, 2]}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([
            {"type": "impute", "field": "variety", "key": "yield", "method": "value", "groupby": ["site"], "value": null},
            {
                "type": "window",
                "as": ["imputed_variety_value"],
                "ops": ["mean"],
                "fields": ["variety"],
                "frame": [-2, 2],
                "ignorePeers": false,
                "groupby": ["site"]
            },
            {"type": "formula", "expr": "datum.variety === null ? datum.imputed_variety_value : datum.variety", "as": "variety"}
        ])
    );
}

#[test]
fn encoding_should_work_when_value_and_frame_are_specified() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "quantitative", "impute": {"method": "value", "value": 20, "frame": [-2, 2]}},
            "color": {"field": "site", "type": "nominal"}
        }
    }));
    let result = ImputeNode::make_from_encoding(None, &model).unwrap();
    assert_eq!(
        assemble(&result),
        json!([{"type": "impute", "field": "variety", "key": "yield", "method": "value", "groupby": ["site"], "value": 20}])
    );
}

#[test]
fn dependent_fields_should_return_proper_fields_with_groupby() {
    let impute = ImputeNode::new(None, &json!({"impute": "y", "key": "x", "groupby": ["f", "g"]}));
    let deps: Vec<String> = dependent_fields(&impute).into_iter().collect();
    assert_eq!(
        deps,
        vec!["y".to_string(), "x".to_string(), "f".to_string(), "g".to_string()]
    );
}

#[test]
fn dependent_fields_should_return_proper_fields_without_groupby() {
    let impute = ImputeNode::new(None, &json!({"impute": "y", "key": "x"}));
    let deps: Vec<String> = dependent_fields(&impute).into_iter().collect();
    assert_eq!(deps, vec!["y".to_string(), "x".to_string()]);
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let impute = ImputeNode::new(
        Some(&parent_node),
        &json!({
            "impute": "y",
            "key": "x",
            "method": "max",
            "groupby": ["a", "b"],
            "frame": [-2, 2]
        }),
    );
    assert!(parent(&clone_node(&impute)).is_none());
}
