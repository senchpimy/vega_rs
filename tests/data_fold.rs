//! Port of `test/compile/data/fold.test.ts`.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::fold::FoldTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let transform = json!({"fold": ["a", "b"], "as": ["a", "b"]});
    let fold = FoldTransformNode::new(None, &transform);
    assert_eq!(
        assemble(&fold),
        json!({"type": "fold", "fields": ["a", "b"], "as": ["a", "b"]})
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let transform = json!({"fold": ["a", "b"]});
    let fold = FoldTransformNode::new(None, &transform);
    assert_eq!(
        assemble(&fold),
        json!({"type": "fold", "fields": ["a", "b"], "as": ["key", "value"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let transform = json!({"fold": ["a", "b"], "as": ["A"]});
    let fold = FoldTransformNode::new(None, &transform);
    assert_eq!(
        assemble(&fold),
        json!({"type": "fold", "fields": ["a", "b"], "as": ["A", "value"]})
    );
}

#[test]
fn dependent_fields_should_return_proper_produced_fields_for_no_as() {
    let fold = FoldTransformNode::new(None, &json!({"fold": ["a", "b"]}));
    let fields: Vec<String> = dependent_fields(&fold).into_iter().collect();
    assert_eq!(fields, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_no_as() {
    let fold = FoldTransformNode::new(None, &json!({"fold": ["a", "b"]}));
    let fields: Vec<String> = produced_fields(&fold).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["key".to_string(), "value".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_complete_as() {
    let fold = FoldTransformNode::new(None, &json!({"fold": ["a", "b"], "as": ["A", "B"]}));
    let fields: Vec<String> = produced_fields(&fold).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let fold = FoldTransformNode::new(None, &json!({"fold": ["a", "b"], "as": ["A", "B"]}));
    assert_eq!(
        hash(&fold),
        json!("FoldTransform {\"as\":[\"A\",\"B\"],\"fold\":[\"a\",\"b\"]}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let fold = FoldTransformNode::new(Some(&parent_node), &json!({"fold": ["a"]}));
    let clone = clone_node(&fold);
    assert!(parent(&clone).is_none());
}
