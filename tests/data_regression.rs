//! Port of `test/compile/data/regression.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::regression::RegressionTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let regression = RegressionTransformNode::new(
        None,
        &json!({
            "regression": "y",
            "on": "x",
            "groupby": ["a", "b"],
            "method": "poly",
            "order": 3,
            "extent": [0, 10],
            "params": false,
            "as": ["u", "v"]
        }),
    );
    assert_eq!(
        assemble(&regression),
        json!({
            "type": "regression",
            "x": "x",
            "y": "y",
            "groupby": ["a", "b"],
            "method": "poly",
            "order": 3,
            "extent": [0, 10],
            "params": false,
            "as": ["u", "v"]
        })
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let regression = RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x"}));
    assert_eq!(
        assemble(&regression),
        json!({"type": "regression", "x": "x", "y": "y", "as": ["x", "y"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let regression =
        RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x", "as": ["A"]}));
    assert_eq!(
        assemble(&regression),
        json!({"type": "regression", "x": "x", "y": "y", "as": ["A", "y"]})
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let regression = RegressionTransformNode::new(
        None,
        &json!({"regression": "y", "on": "x", "groupby": ["f", "g"]}),
    );
    let deps: Vec<String> = dependent_fields(&regression).into_iter().collect();
    assert_eq!(
        deps,
        vec!["y".to_string(), "x".to_string(), "f".to_string(), "g".to_string()]
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields_without_groupby() {
    let regression = RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x"}));
    let deps: Vec<String> = dependent_fields(&regression).into_iter().collect();
    assert_eq!(deps, vec!["y".to_string(), "x".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_no_as() {
    let regression = RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x"}));
    let produced: Vec<String> = produced_fields(&regression).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["x".to_string(), "y".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_complete_as() {
    let regression =
        RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x", "as": ["A", "B"]}));
    let produced: Vec<String> = produced_fields(&regression).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let regression =
        RegressionTransformNode::new(None, &json!({"regression": "y", "on": "x", "as": ["A", "B"]}));
    assert_eq!(
        hash(&regression),
        json!("RegressionTransform {\"as\":[\"A\",\"B\"],\"on\":\"x\",\"regression\":\"y\"}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let regression =
        RegressionTransformNode::new(Some(&parent_node), &json!({"regression": "y", "on": "x"}));
    assert!(parent(&clone_node(&regression)).is_none());
}
