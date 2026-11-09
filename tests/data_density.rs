//! Port of `test/compile/data/density.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::density::DensityTransformNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let density = DensityTransformNode::new(
        None,
        &json!({
            "density": "v",
            "groupby": ["a", "b"],
            "cumulative": false,
            "counts": false,
            "bandwidth": 0.5,
            "extent": [0, 10],
            "minsteps": 25,
            "maxsteps": 200,
            "as": ["x", "y"]
        }),
    );
    assert_eq!(
        assemble(&density),
        json!({
            "type": "kde",
            "field": "v",
            "groupby": ["a", "b"],
            "cumulative": false,
            "counts": false,
            "bandwidth": 0.5,
            "extent": [0, 10],
            "minsteps": 25,
            "maxsteps": 200,
            "resolve": "shared",
            "as": ["x", "y"]
        })
    );
}

#[test]
fn assemble_should_handle_missing_as_field() {
    let density = DensityTransformNode::new(None, &json!({"density": "v"}));
    assert_eq!(
        assemble(&density),
        json!({"type": "kde", "field": "v", "resolve": "shared", "as": ["value", "density"]})
    );
}

#[test]
fn assemble_should_handle_partial_as_field() {
    let density = DensityTransformNode::new(None, &json!({"density": "v", "as": ["A"]}));
    assert_eq!(
        assemble(&density),
        json!({"type": "kde", "field": "v", "resolve": "shared", "as": ["A", "density"]})
    );
}

#[test]
fn assemble_should_add_resolve_independent_if_we_set_it_explicitly() {
    let density = DensityTransformNode::new(
        None,
        &json!({"density": "v", "groupby": ["a"], "resolve": "independent"}),
    );
    assert_eq!(
        assemble(&density),
        json!({
            "type": "kde",
            "groupby": ["a"],
            "field": "v",
            "resolve": "independent",
            "as": ["value", "density"]
        })
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let density = DensityTransformNode::new(None, &json!({"density": "v", "groupby": ["f", "g"]}));
    let deps: Vec<String> = dependent_fields(&density).into_iter().collect();
    assert_eq!(
        deps,
        vec!["v".to_string(), "f".to_string(), "g".to_string()]
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields_without_groupby() {
    let density = DensityTransformNode::new(None, &json!({"density": "v"}));
    let deps: Vec<String> = dependent_fields(&density).into_iter().collect();
    assert_eq!(deps, vec!["v".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_no_as() {
    let density = DensityTransformNode::new(None, &json!({"density": "v"}));
    let produced: Vec<String> = produced_fields(&density).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["value".to_string(), "density".to_string()]);
}

#[test]
fn produced_fields_should_return_proper_produced_fields_for_complete_as() {
    let density = DensityTransformNode::new(None, &json!({"density": "v", "as": ["A", "B"]}));
    let produced: Vec<String> = produced_fields(&density).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let density = DensityTransformNode::new(None, &json!({"density": "v", "as": ["A", "B"]}));
    assert_eq!(
        hash(&density),
        json!("DensityTransform {\"as\":[\"A\",\"B\"],\"density\":\"v\",\"resolve\":\"shared\"}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let density = DensityTransformNode::new(Some(&parent_node), &json!({"density": "v"}));
    assert!(parent(&clone_node(&density)).is_none());
}
