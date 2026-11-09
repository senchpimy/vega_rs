//! Port of `test/compile/data/bin.test.ts` (transform / node-level tests).
//!
//! `makeFromEncoding` cases require a parsed model with scale/extent signals
//! and are skipped.  `makeFromTransform` cases are ported with an identity
//! model (the unnamed top-level model used by `parseUnitModel`).

use serde_json::{json, Map, Value};
use vega_lite::compile::data::bin::{get_bin_signal_name, BinComponent, BinNode};
use vega_lite::compile::data::dataflow::{
    assemble, children, clone_node, debug_name, dependent_fields, hash, num_children, parent,
    produced_fields, with_node, PlaceholderDataFlowNode,
};
use vega_lite::compile::data::support::Model;

struct TestModel {
    selection_field: Option<String>,
}

impl Model for TestModel {
    fn selection_extent(&self, name: &str, extent: &Value) -> String {
        let field = extent
            .get("field")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| self.selection_field.clone())
            .unwrap_or_default();
        format!(
            "{}[{}]",
            vega_lite::util::var_name(name),
            serde_json::to_string(&vega_lite::util::replace_path_in_field(&field)).unwrap()
        )
    }

    fn selection_is_known(&self, _name: &str) -> bool {
        true
    }
}

fn model() -> TestModel {
    TestModel {
        selection_field: None,
    }
}

fn selection_model() -> TestModel {
    TestModel {
        selection_field: Some("Rotten_Tomatoes_Rating".into()),
    }
}

fn bin_map(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn assemble_from_transform_with_custom_extent() {
    let t = json!({
        "bin": {"extent": [0, 100]},
        "field": "Acceleration",
        "as": "binned_acceleration"
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    assert_eq!(
        assemble(&node)[0],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "maxbins": 10,
            "as": ["binned_acceleration", "binned_acceleration_end"],
            "extent": [0, 100],
            "signal": "bin_extent_0_100_maxbins_10_Acceleration_bins"
        })
    );
}

#[test]
fn assemble_from_transform_with_maxbins_and_custom_extent() {
    let t = json!({
        "bin": {"extent": [0, 100], "maxbins": 20},
        "field": "Acceleration",
        "as": "binned_acceleration"
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    assert_eq!(
        assemble(&node)[0],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "maxbins": 20,
            "as": ["binned_acceleration", "binned_acceleration_end"],
            "extent": [0, 100],
            "signal": "bin_extent_0_100_maxbins_20_Acceleration_bins"
        })
    );
}

#[test]
fn assemble_from_transform_with_selection_extent() {
    let t = json!({
        "bin": {"extent": {"param": "foo"}},
        "field": "Acceleration",
        "as": "binned_acceleration"
    });
    let node = BinNode::make_from_transform(None, &t, &selection_model());
    let transforms = assemble(&node);
    assert_eq!(
        transforms[0],
        json!({
            "type": "extent",
            "field": "Acceleration",
            "signal": "bin_extent_param_foo_maxbins_10_Acceleration_extent"
        })
    );
    assert_eq!(
        transforms[1],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "as": ["binned_acceleration", "binned_acceleration_end"],
            "maxbins": 10,
            "signal": "bin_extent_param_foo_maxbins_10_Acceleration_bins",
            "extent": {"signal": "bin_extent_param_foo_maxbins_10_Acceleration_extent"},
            "span": {"signal": "span(foo[\"Rotten_Tomatoes_Rating\"])"}
        })
    );
}

#[test]
fn assemble_from_transform_with_selection_and_field_extent() {
    let t = json!({
        "bin": {"extent": {"param": "foo", "field": "bar"}},
        "field": "Acceleration",
        "as": "binned_acceleration"
    });
    let node = BinNode::make_from_transform(None, &t, &selection_model());
    let transforms = assemble(&node);
    assert_eq!(
        transforms[0],
        json!({
            "type": "extent",
            "field": "Acceleration",
            "signal": "bin_extent_param_foo_field_bar_maxbins_10_Acceleration_extent"
        })
    );
    assert_eq!(
        transforms[1],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "as": ["binned_acceleration", "binned_acceleration_end"],
            "maxbins": 10,
            "signal": "bin_extent_param_foo_field_bar_maxbins_10_Acceleration_bins",
            "extent": {"signal": "bin_extent_param_foo_field_bar_maxbins_10_Acceleration_extent"},
            "span": {"signal": "span(foo[\"bar\"])"}
        })
    );
}

#[test]
fn assemble_from_transform_with_anchor_property() {
    let t = json!({
        "bin": {"extent": [0, 100], "anchor": 6},
        "field": "Acceleration",
        "as": "binned_acceleration"
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    assert_eq!(
        assemble(&node)[0],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "anchor": 6,
            "maxbins": 10,
            "as": ["binned_acceleration", "binned_acceleration_end"],
            "extent": [0, 100],
            "signal": "bin_extent_0_100_anchor_6_maxbins_10_Acceleration_bins"
        })
    );
}

#[test]
fn assemble_from_transform_with_array_as() {
    let t = json!({
        "bin": {"extent": [0, 100], "anchor": 6},
        "field": "Acceleration",
        "as": ["binned_acceleration_start", "binned_acceleration_stop"]
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    assert_eq!(
        assemble(&node)[0],
        json!({
            "type": "bin",
            "field": "Acceleration",
            "anchor": 6,
            "maxbins": 10,
            "as": ["binned_acceleration_start", "binned_acceleration_stop"],
            "extent": [0, 100],
            "signal": "bin_extent_0_100_anchor_6_maxbins_10_Acceleration_bins"
        })
    );
}

#[test]
fn should_generate_the_correct_hash() {
    let t = json!({
        "bin": {"extent": [0, 100], "anchor": 6},
        "field": "Acceleration",
        "as": ["binned_acceleration_start", "binned_acceleration_stop"]
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    assert_eq!(hash(&node), json!("Bin 1499261512"));
}

#[test]
fn should_generate_the_correct_dependent_fields() {
    let t = json!({
        "bin": {"extent": [0, 100], "anchor": 6},
        "field": "Acceleration",
        "as": ["binned_acceleration_start", "binned_acceleration_stop"]
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    let fields: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(fields, vec!["Acceleration".to_string()]);
}

#[test]
fn should_generate_the_correct_produced_fields() {
    let t = json!({
        "bin": {"extent": [0, 100], "anchor": 6},
        "field": "Acceleration",
        "as": ["binned_acceleration_start", "binned_acceleration_stop"]
    });
    let node = BinNode::make_from_transform(None, &t, &model());
    let fields: Vec<String> = produced_fields(&node).unwrap().into_iter().collect();
    assert_eq!(
        fields,
        vec![
            "binned_acceleration_start".to_string(),
            "binned_acceleration_stop".to_string()
        ]
    );
}

#[test]
fn should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let bin = BinNode::new(Some(&parent_node), vega_lite::compile::data::bin::Bins::default());
    assert!(parent(&clone_node(&bin)).is_none());
}

#[test]
fn should_preserve_as_when_merging_with_other_node() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let mut bins_a: vega_lite::compile::data::bin::Bins = Default::default();
    bins_a.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["foo".into(), "foo_end".into()]]),
    );
    let bin_a = BinNode::new(Some(&parent_node), bins_a);
    let mut bins_b: vega_lite::compile::data::bin::Bins = Default::default();
    bins_b.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["bar".into(), "bar_end".into()]]),
    );
    let bin_b = BinNode::new(Some(&parent_node), bins_b);

    BinNode::merge(&bin_a, &bin_b, &mut |_, _| {});
    let bins = with_node::<BinNode, _>(&bin_a, |n| n.bins.clone()).unwrap();
    assert_eq!(
        bins["foo"].as_,
        vec![
            ["foo".to_string(), "foo_end".to_string()],
            ["bar".to_string(), "bar_end".to_string()]
        ]
    );
}

#[test]
fn should_not_have_duplicate_members_of_as_after_merging() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let mut bins_a: vega_lite::compile::data::bin::Bins = Default::default();
    bins_a.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["foo".into(), "foo_end".into()]]),
    );
    let bin_a = BinNode::new(Some(&parent_node), bins_a);
    let mut bins_b: vega_lite::compile::data::bin::Bins = Default::default();
    bins_b.insert(
        "foo".into(),
        BinComponent::new(
            bin_map(&json!({})),
            "foo",
            vec![
                ["foo".into(), "foo_end".into()],
                ["bar".into(), "bar_end".into()],
            ],
        ),
    );
    let bin_b = BinNode::new(Some(&parent_node), bins_b);

    BinNode::merge(&bin_a, &bin_b, &mut |_, _| {});
    let bins = with_node::<BinNode, _>(&bin_a, |n| n.bins.clone()).unwrap();
    assert_eq!(
        bins["foo"].as_,
        vec![
            ["foo".to_string(), "foo_end".to_string()],
            ["bar".to_string(), "bar_end".to_string()]
        ]
    );
}

#[test]
fn should_create_formulas_for_members_of_as_when_assembled() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let mut bins: vega_lite::compile::data::bin::Bins = Default::default();
    bins.insert(
        "foo".into(),
        BinComponent::new(
            bin_map(&json!({})),
            "foo",
            vec![
                ["foo".into(), "foo_end".into()],
                ["bar".into(), "bar_end".into()],
            ],
        ),
    );
    let bin = BinNode::new(Some(&parent_node), bins);
    let transforms = assemble(&bin);
    assert_eq!(
        transforms[1],
        json!({"type": "formula", "expr": "datum[\"foo\"]", "as": "bar"})
    );
    assert_eq!(
        transforms[2],
        json!({"type": "formula", "expr": "datum[\"foo_end\"]", "as": "bar_end"})
    );
}

#[test]
fn should_reassign_children_of_bin_node_when_merging() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let mut bins_a: vega_lite::compile::data::bin::Bins = Default::default();
    bins_a.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["foo".into(), "foo_end".into()]]),
    );
    let bin_a = BinNode::new(Some(&parent_node), bins_a);
    let mut bins_b: vega_lite::compile::data::bin::Bins = Default::default();
    bins_b.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["bar".into(), "bar_end".into()]]),
    );
    let bin_b = BinNode::new(Some(&parent_node), bins_b);
    let child_a = PlaceholderDataFlowNode::new(Some(&bin_a), "child_a");
    let child_b = PlaceholderDataFlowNode::new(Some(&bin_b), "child_b");

    BinNode::merge(&bin_a, &bin_b, &mut |_, _| {});

    assert_eq!(num_children(&bin_b), 0);
    assert_eq!(num_children(&bin_a), 2);
    let kids = children(&bin_a);
    assert!(kids.iter().any(|c| debug_name(c).as_deref() == Some("child_a")));
    assert!(kids.iter().any(|c| debug_name(c).as_deref() == Some("child_b")));
    let _ = (child_a, child_b);
}

#[test]
fn should_keep_non_conflicting_bins_of_bin_nodes_when_merging() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let mut bins_a: vega_lite::compile::data::bin::Bins = Default::default();
    bins_a.insert(
        "foo".into(),
        BinComponent::new(bin_map(&json!({})), "foo", vec![["foo".into(), "foo_end".into()]]),
    );
    let bin_a = BinNode::new(Some(&parent_node), bins_a);
    let mut bins_b: vega_lite::compile::data::bin::Bins = Default::default();
    bins_b.insert(
        "bar".into(),
        BinComponent::new(bin_map(&json!({})), "bar", vec![["bar".into(), "bar_end".into()]]),
    );
    let bin_b = BinNode::new(Some(&parent_node), bins_b);

    BinNode::merge(&bin_a, &bin_b, &mut |_, _| {});
    let bins = with_node::<BinNode, _>(&bin_a, |n| n.bins.clone()).unwrap();
    assert_eq!(bins.len(), 2);
    assert!(bins.contains_key("foo"));
    assert!(bins.contains_key("bar"));
}

#[test]
fn get_bin_signal_name_should_generate_correct_names() {
    let m = model();
    assert_eq!(
        get_bin_signal_name(&m, "foo.bar", &json!(true)),
        "bin_maxbins_10_foo_bar_bins"
    );
    assert_eq!(
        get_bin_signal_name(&m, "foo", &json!(true)),
        "bin_maxbins_10_foo_bins"
    );
    assert_eq!(
        get_bin_signal_name(&m, "foo", &json!({"maxbins": 20})),
        "bin_maxbins_20_foo_bins"
    );
}
