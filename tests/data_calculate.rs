//! Port of `test/compile/data/calculate.test.ts` (node-level tests; the
//! `parseAllForSortIndex` cases need a parsed unit model and are skipped).

use serde_json::json;
use vega_lite::compile::data::calculate::CalculateNode;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields, PlaceholderDataFlowNode,
};

#[test]
fn dependent_fields_and_produced_fields_return_the_right_fields() {
    let node = CalculateNode::new(None, &json!({"calculate": "datum.foo + 2", "as": "bar"}));
    let deps: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(deps, vec!["foo".to_string()]);
    let produced: Vec<String> = produced_fields(&node).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["bar".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let node = CalculateNode::new(
        None,
        &json!({
            "calculate": "datum[\"a\"]===\"B\" ? 0 : datum[\"a\"]===\"A\" ? 1 : datum[\"a\"]===\"C\" ? 2 : 3",
            "as": "x_a_sort_index"
        }),
    );
    assert_eq!(
        hash(&node),
        json!("Calculate {\"as\":\"x_a_sort_index\",\"calculate\":\"datum[\\\"a\\\"]===\\\"B\\\" ? 0 : datum[\\\"a\\\"]===\\\"A\\\" ? 1 : datum[\\\"a\\\"]===\\\"C\\\" ? 2 : 3\"}")
    );
}

#[test]
fn assemble_returns_the_formula_transform() {
    let node = CalculateNode::new(None, &json!({"calculate": "datum.foo + 2", "as": "bar"}));
    assert_eq!(
        assemble(&node),
        json!({"type": "formula", "expr": "datum.foo + 2", "as": "bar"})
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let calculate = CalculateNode::new(Some(&parent_node), &json!({"calculate": "foo", "as": "bar"}));
    assert!(parent(&clone_node(&calculate)).is_none());
}
