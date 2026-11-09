//! Port of `test/compile/data/pivot.test.ts`.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::pivot::PivotTransformNode;

#[test]
fn should_return_a_proper_vg_transform() {
    let transform = json!({"pivot": "a", "value": "b", "limit": 1, "op": "count", "groupby": ["c"]});
    let pivot = PivotTransformNode::new(None, &transform);
    assert_eq!(
        assemble(&pivot),
        json!({"type": "pivot", "field": "a", "value": "b", "limit": 1, "op": "count", "groupby": ["c"]})
    );
}

#[test]
fn should_handle_missing_fields() {
    let pivot = PivotTransformNode::new(None, &json!({"pivot": "a", "value": "b"}));
    assert_eq!(
        assemble(&pivot),
        json!({"type": "pivot", "field": "a", "value": "b"})
    );
}

#[test]
fn should_add_dimensions_to_the_groupby_attribute() {
    let pivot = PivotTransformNode::new(None, &json!({"pivot": "a", "value": "b"}));
    PivotTransformNode::add_dimensions(&pivot, &["c".into(), "d".into()]);
    assert_eq!(assemble(&pivot)["groupby"], json!(["c", "d"]));
}

#[test]
fn should_return_undefined_for_produced_fields() {
    let pivot = PivotTransformNode::new(None, &json!({"pivot": "a", "value": "b"}));
    assert!(produced_fields(&pivot).is_none());
}

#[test]
fn should_return_relevant_dependent_fields() {
    let pivot = PivotTransformNode::new(None, &json!({"pivot": "a", "value": "b"}));
    let fields: Vec<String> = dependent_fields(&pivot).into_iter().collect();
    assert_eq!(fields, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn should_generate_the_correct_hash() {
    let pivot = PivotTransformNode::new(None, &json!({"pivot": "a", "value": "b"}));
    assert_eq!(hash(&pivot), json!("PivotTransform {\"pivot\":\"a\",\"value\":\"b\"}"));
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let pivot = PivotTransformNode::new(Some(&parent_node), &json!({"pivot": "a", "value": "b"}));
    assert!(parent(&clone_node(&pivot)).is_none());
}
