//! Port of `test/compile/data/extent.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, hash, parent, produced_fields, PlaceholderDataFlowNode,
};
use vega_lite::compile::data::extent::{is_extent, ExtentTransformNode};

#[test]
fn is_extent_should_return_true_for_a_regular_extent_transform() {
    assert!(is_extent(&json!({"extent": "a", "param": "A"})));
}

#[test]
fn is_extent_should_return_false_for_a_regression_transform() {
    assert!(!is_extent(
        &json!({"regression": "a", "on": "b", "extent": [0, 10]})
    ));
}

#[test]
fn is_extent_should_return_false_for_a_density_transform() {
    assert!(!is_extent(&json!({"density": "a", "extent": [0, 10]})));
}

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let extent = ExtentTransformNode::new(None, &json!({"extent": "a", "param": "a_extent"}));
    assert_eq!(
        assemble(&extent),
        json!({"type": "extent", "field": "a", "signal": "a_extent"})
    );
}

#[test]
fn produced_fields_should_return_empty_set() {
    let extent = ExtentTransformNode::new(None, &json!({"extent": "a", "param": "A"}));
    assert!(produced_fields(&extent).unwrap().is_empty());
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let extent = ExtentTransformNode::new(None, &json!({"extent": "a", "param": "A"}));
    assert_eq!(
        hash(&extent),
        json!("ExtentTransform {\"extent\":\"a\",\"param\":\"A\"}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let extent = ExtentTransformNode::new(Some(&parent_node), &json!({"extent": "a", "param": "A"}));
    assert!(parent(&clone_node(&extent)).is_none());
}
