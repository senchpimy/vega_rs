//! Port of `test/compile/data/loess.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::loess::LoessTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let loess = LoessTransformNode::new(
        None,
        &json!({"loess": "y", "on": "x", "groupby": ["a", "b"], "bandwidth": 0.3, "as": ["u", "v"]}),
    );
    assert_eq!(
        assemble(&loess),
        json!({
            "type": "loess",
            "x": "x",
            "y": "y",
            "groupby": ["a", "b"],
            "bandwidth": 0.3,
            "as": ["u", "v"]
        })
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x"}));
    assert_eq!(
        assemble(&loess),
        json!({"type": "loess", "x": "x", "y": "y", "as": ["x", "y"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x", "as": ["A"]}));
    assert_eq!(
        assemble(&loess),
        json!({"type": "loess", "x": "x", "y": "y", "as": ["A", "y"]})
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x", "groupby": ["f", "g"]}));
    let deps: Vec<String> = dependent_fields(&loess).into_iter().collect();
    assert_eq!(
        deps,
        vec!["y".to_string(), "x".to_string(), "f".to_string(), "g".to_string()]
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields_without_groupby() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x"}));
    let deps: Vec<String> = dependent_fields(&loess).into_iter().collect();
    assert_eq!(deps, vec!["y".to_string(), "x".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_no_as() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x"}));
    let produced: Vec<String> = produced_fields(&loess).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["x".to_string(), "y".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_complete_as() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x", "as": ["A", "B"]}));
    let produced: Vec<String> = produced_fields(&loess).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let loess = LoessTransformNode::new(None, &json!({"loess": "y", "on": "x", "as": ["A", "B"]}));
    assert_eq!(
        hash(&loess),
        json!("LoessTransform {\"as\":[\"A\",\"B\"],\"loess\":\"y\",\"on\":\"x\"}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let loess = LoessTransformNode::new(Some(&parent_node), &json!({"loess": "y", "on": "x"}));
    assert!(parent(&clone_node(&loess)).is_none());
}
