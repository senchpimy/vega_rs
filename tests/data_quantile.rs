//! Port of `test/compile/data/quantile.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::quantile::QuantileTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let quantile = QuantileTransformNode::new(
        None,
        &json!({"quantile": "x", "groupby": ["a", "b"], "step": 0.05, "as": ["p", "v"]}),
    );
    assert_eq!(
        assemble(&quantile),
        json!({
            "type": "quantile",
            "field": "x",
            "groupby": ["a", "b"],
            "step": 0.05,
            "as": ["p", "v"]
        })
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x"}));
    assert_eq!(
        assemble(&quantile),
        json!({"type": "quantile", "field": "x", "as": ["prob", "value"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x", "as": ["A"]}));
    assert_eq!(
        assemble(&quantile),
        json!({"type": "quantile", "field": "x", "as": ["A", "value"]})
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let quantile =
        QuantileTransformNode::new(None, &json!({"quantile": "x", "groupby": ["f", "g"]}));
    let deps: Vec<String> = dependent_fields(&quantile).into_iter().collect();
    assert_eq!(deps, vec!["x".to_string(), "f".to_string(), "g".to_string()]);
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields_without_groupby() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x"}));
    let deps: Vec<String> = dependent_fields(&quantile).into_iter().collect();
    assert_eq!(deps, vec!["x".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_no_as() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x"}));
    let produced: Vec<String> = produced_fields(&quantile).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["prob".to_string(), "value".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_complete_as() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x", "as": ["A", "B"]}));
    let produced: Vec<String> = produced_fields(&quantile).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let quantile = QuantileTransformNode::new(None, &json!({"quantile": "x", "as": ["A", "B"]}));
    assert_eq!(
        hash(&quantile),
        json!("QuantileTransform {\"as\":[\"A\",\"B\"],\"quantile\":\"x\"}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let quantile = QuantileTransformNode::new(Some(&parent_node), &json!({"quantile": "x"}));
    assert!(parent(&clone_node(&quantile)).is_none());
}
