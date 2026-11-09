//! Port of `test/compile/data/flatten.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{assemble, dependent_fields, hash, produced_fields};
use vega_lite::compile::data::flatten::FlattenTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let flatten = FlattenTransformNode::new(
        None,
        &json!({"flatten": ["a", "b"], "as": ["a", "b"]}),
    );
    assert_eq!(
        assemble(&flatten),
        json!({"type": "flatten", "fields": ["a", "b"], "as": ["a", "b"]})
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let flatten = FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"]}));
    assert_eq!(
        assemble(&flatten),
        json!({"type": "flatten", "fields": ["a", "b"], "as": ["a", "b"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let flatten = FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"], "as": ["A"]}));
    assert_eq!(
        assemble(&flatten),
        json!({"type": "flatten", "fields": ["a", "b"], "as": ["A", "b"]})
    );
}

#[test]
fn dependent_fields_should_return_proper_fields() {
    let flatten = FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"]}));
    let deps: Vec<String> = dependent_fields(&flatten).into_iter().collect();
    assert_eq!(deps, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_fields() {
    let flatten = FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"]}));
    let produced: Vec<String> = produced_fields(&flatten).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_fields_with_as() {
    let flatten =
        FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"], "as": ["foo", "bar"]}));
    let produced: Vec<String> = produced_fields(&flatten).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["foo".to_string(), "bar".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let flatten = FlattenTransformNode::new(None, &json!({"flatten": ["a", "b"]}));
    assert_eq!(
        hash(&flatten),
        json!("FlattenTransform {\"as\":[\"a\",\"b\"],\"flatten\":[\"a\",\"b\"]}")
    );
}
