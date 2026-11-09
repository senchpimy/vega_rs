//! Port of `test/compile/data/joinaggregate.test.ts`.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::joinaggregate::JoinAggregateTransformNode;

#[test]
fn should_return_a_proper_vg_transform() {
    let t = json!({"joinaggregate": [{"op": "count", "as": "join_count"}], "groupby": ["f"]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    assert_eq!(
        assemble(&joinaggregate),
        json!({
            "type": "joinaggregate",
            "ops": ["count"],
            "fields": [null],
            "as": ["join_count"],
            "groupby": ["f"]
        })
    );
}

#[test]
fn should_augment_as_with_default_as() {
    let t = json!({"joinaggregate": [{"op": "count"}], "groupby": ["f"]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    assert_eq!(assemble(&joinaggregate)["as"], json!(["count"]));
}

#[test]
fn should_return_a_proper_produced_fields() {
    let t = json!({
        "joinaggregate": [
            {"op": "count", "as": "count_field"},
            {"op": "sum", "as": "sum_field"}
        ]
    });
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    let fields: Vec<String> = produced_fields(&joinaggregate).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["count_field".to_string(), "sum_field".to_string()]);
}

#[test]
fn should_generate_the_correct_dependent_fields() {
    let t = json!({"joinaggregate": [{"field": "f", "op": "mean", "as": "mean_f"}], "groupby": ["g"]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    let fields: Vec<String> = dependent_fields(&joinaggregate).into_iter().collect();
    assert_eq!(fields, vec!["g".to_string(), "f".to_string()]);
}

#[test]
fn should_generate_the_correct_dependent_fields_when_groupby_is_undefined() {
    let t = json!({"joinaggregate": [{"field": "f", "op": "mean", "as": "mean_f"}]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    let fields: Vec<String> = dependent_fields(&joinaggregate).into_iter().collect();
    assert_eq!(fields, vec!["f".to_string()]);
}

#[test]
fn should_clone_to_an_equivalent_version() {
    let t = json!({"joinaggregate": [{"op": "count", "as": "count"}], "groupby": ["f"]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    let clone = clone_node(&joinaggregate);
    assert_eq!(assemble(&clone), assemble(&joinaggregate));
    assert_eq!(hash(&clone), hash(&joinaggregate));
}

#[test]
fn should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let joinaggregate =
        JoinAggregateTransformNode::new(Some(&parent_node), &json!({"joinaggregate": []}));
    assert!(parent(&clone_node(&joinaggregate)).is_none());
}

#[test]
fn should_generate_the_correct_hash() {
    let t = json!({"joinaggregate": [{"op": "count", "as": "count"}], "groupby": ["f"]});
    let joinaggregate = JoinAggregateTransformNode::new(None, &t);
    assert_eq!(
        hash(&joinaggregate),
        json!("JoinAggregateTransform {\"groupby\":[\"f\"],\"joinaggregate\":[{\"as\":\"count\",\"op\":\"count\"}]}")
    );
}
