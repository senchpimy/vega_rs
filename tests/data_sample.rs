//! Port of `test/compile/data/sample.test.ts`.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields, PlaceholderDataFlowNode,
};
use vega_lite::compile::data::sample::SampleTransformNode;

#[test]
fn should_return_a_proper_vg_transform() {
    let sample = SampleTransformNode::new(None, &json!({"sample": 500}));
    assert_eq!(assemble(&sample), json!({"type": "sample", "size": 500}));
}

#[test]
fn should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let sample = SampleTransformNode::new(Some(&parent_node), &json!({"sample": 500}));
    assert!(parent(&clone_node(&sample)).is_none());
}

#[test]
fn should_generate_the_correct_hash() {
    let sample = SampleTransformNode::new(None, &json!({"sample": 500}));
    assert_eq!(hash(&sample), json!("SampleTransform {\"sample\":500}"));
}

#[test]
fn should_produce_different_hashes_for_different_samples() {
    let sample1 = SampleTransformNode::new(None, &json!({"sample": 42}));
    let sample2 = SampleTransformNode::new(None, &json!({"sample": 123}));
    assert_ne!(hash(&sample1), hash(&sample2));
}

#[test]
fn produced_fields_should_return_empty_set() {
    let sample = SampleTransformNode::new(None, &json!({"sample": 500}));
    assert!(produced_fields(&sample).unwrap().is_empty());
}

#[test]
fn dependent_fields_should_return_empty_set() {
    let sample = SampleTransformNode::new(None, &json!({"sample": 500}));
    assert!(dependent_fields(&sample).is_empty());
}
