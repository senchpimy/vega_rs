//! Port of `test/compile/data/window.test.ts`.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::window::WindowTransformNode;

fn transform() -> serde_json::Value {
    json!({
        "window": [{"op": "row_number", "as": "ordered_row_number"}],
        "ignorePeers": false,
        "sort": [{"field": "f", "order": "ascending"}],
        "groupby": ["f"],
        "frame": [null, 0]
    })
}

#[test]
fn should_return_a_proper_vg_transform() {
    let window = WindowTransformNode::new(None, &transform());
    assert_eq!(
        assemble(&window),
        json!({
            "type": "window",
            "ops": ["row_number"],
            "fields": [null],
            "params": [null],
            "sort": {"field": ["f"], "order": ["ascending"]},
            "ignorePeers": false,
            "as": ["ordered_row_number"],
            "frame": [null, 0],
            "groupby": ["f"]
        })
    );
}

#[test]
fn should_augment_as_with_default_as() {
    let mut t = transform();
    t["window"][0] = json!({"op": "row_number"});
    let window = WindowTransformNode::new(None, &t);
    assert_eq!(assemble(&window)["as"], json!(["row_number"]));
}

#[test]
fn should_return_a_proper_produced_fields() {
    let t = json!({
        "window": [
            {"op": "row_number", "as": "ordered_row_number"},
            {"op": "count", "as": "count_field"},
            {"op": "sum", "as": "sum_field"}
        ],
        "ignorePeers": false,
        "sort": [{"field": "f", "order": "ascending"}],
        "groupby": ["g"],
        "frame": [null, 0]
    });
    let window = WindowTransformNode::new(None, &t);
    let mut fields: Vec<String> = produced_fields(&window).unwrap().into_iter().collect();
    fields.sort();
    assert_eq!(
        fields,
        vec![
            "count_field".to_string(),
            "ordered_row_number".to_string(),
            "sum_field".to_string()
        ]
    );
}

#[test]
fn should_generate_the_correct_dependent_fields() {
    let mut t = transform();
    t["groupby"] = json!(["g"]);
    let window = WindowTransformNode::new(None, &t);
    let mut fields: Vec<String> = dependent_fields(&window).into_iter().collect();
    fields.sort();
    assert_eq!(fields, vec!["f".to_string(), "g".to_string()]);
}

#[test]
fn should_generate_the_correct_dependent_fields_when_groupby_and_sort_are_undefined() {
    let t = json!({
        "window": [{"field": "w", "op": "row_number", "as": "ordered_row_number"}],
        "ignorePeers": false,
        "frame": [null, 0]
    });
    let window = WindowTransformNode::new(None, &t);
    let fields: Vec<String> = dependent_fields(&window).into_iter().collect();
    assert_eq!(fields, vec!["w".to_string()]);
}

#[test]
fn should_clone_to_an_equivalent_version() {
    let window = WindowTransformNode::new(None, &transform());
    let clone = clone_node(&window);
    assert_eq!(assemble(&clone), assemble(&window));
    assert_eq!(hash(&clone), hash(&window));
}

#[test]
fn should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let window = WindowTransformNode::new(Some(&parent_node), &json!({"window": []}));
    assert!(parent(&clone_node(&window)).is_none());
}

#[test]
fn should_generate_the_correct_hash() {
    let window = WindowTransformNode::new(None, &transform());
    assert_eq!(
        hash(&window),
        json!("WindowTransform {\"frame\":[null,0],\"groupby\":[\"f\"],\"ignorePeers\":false,\"sort\":[{\"field\":\"f\",\"order\":\"ascending\"}],\"window\":[{\"as\":\"ordered_row_number\",\"op\":\"row_number\"}]}")
    );
}

#[test]
fn window_of_aggregate_ops_over_full_frame_becomes_joinaggregate() {
    let t = json!({
        "window": [{"op": "sum", "field": "a", "as": "sum_a"}],
        "frame": [null, null],
        "groupby": ["g"]
    });
    let window = WindowTransformNode::new(None, &t);
    assert_eq!(
        assemble(&window),
        json!({
            "type": "joinaggregate",
            "as": ["sum_a"],
            "ops": ["sum"],
            "fields": ["a"],
            "groupby": ["g"]
        })
    );
}
