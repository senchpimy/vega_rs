//! Port of `test/compile/data/parse.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::data::aggregate::AggregateNode;
use vega_lite::compile::data::bin::BinNode;
use vega_lite::compile::data::calculate::CalculateNode;
use vega_lite::compile::data::dataflow::NodeRef;
use vega_lite::compile::data::extent::ExtentTransformNode;
use vega_lite::compile::data::filter::FilterNode;
use vega_lite::compile::data::flatten::FlattenTransformNode;
use vega_lite::compile::data::fold::FoldTransformNode;
use vega_lite::compile::data::formatparse::ParseNode;
use vega_lite::compile::data::impute::ImputeNode;
use vega_lite::compile::data::index::AncestorParse;
use vega_lite::compile::data::parse::{find_source, parse_transform_array};
use vega_lite::compile::data::pivot::PivotTransformNode;
use vega_lite::compile::data::sample::SampleTransformNode;
use vega_lite::compile::data::source::SourceNode;
use vega_lite::compile::data::timeunit::TimeUnitNode;
use vega_lite::compile::data::window::WindowTransformNode;
use vega_lite::compile::data::PlaceholderDataFlowNode;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn is_node<T: vega_lite::compile::data::dataflow::NodeBehavior + 'static>(node: &NodeRef) -> bool {
    vega_lite::compile::data::dataflow::with_node::<T, _>(node, |_| ()).is_some()
}

fn parse_to_value(node: &NodeRef) -> Value {
    vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(node, |n| json!(n.parse))
        .unwrap()
}

fn ancestor_to_value(ancestor: &AncestorParse) -> Value {
    json!(ancestor.combine())
}

#[test]
fn should_return_a_calculate_node_and_a_filter_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"calculate": "calculate", "as": "as"}, {"filter": "filter"}],
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "month"}}
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    let mut ancestor = AncestorParse::new();
    let result = parse_transform_array(root.clone(), &model, &mut ancestor);
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<CalculateNode>(&first));
    assert!(is_node::<FilterNode>(&result));
}

#[test]
fn should_add_a_parse_node_for_filter_transforms_with_time_unit() {
    let model = parse_unit_model(&json!({
        "data": {"url": "a.json"},
        "transform": [{
            "filter": {
                "not": {
                    "and": [{
                        "or": [
                            {"timeUnit": "year", "field": "date", "equal": 2005},
                            "datum.a > 5"
                        ]
                    }]
                }
            }
        }],
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "temporal"},
            "color": {"field": "c", "type": "ordinal"},
            "shape": {"field": "d", "type": "nominal"}
        }
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    let mut ancestor = AncestorParse::new();
    let result = parse_transform_array(root.clone(), &model, &mut ancestor);

    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<ParseNode>(&first));
    assert!(is_node::<FilterNode>(&result));
    assert_eq!(parse_to_value(&first), json!({"date": "date"}));
    assert_eq!(ancestor_to_value(&ancestor), json!({"date": "date"}));
}

#[test]
fn should_return_a_bin_node_and_a_time_unit_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [
            {"bin": true, "field": "field", "as": "a"},
            {"timeUnit": "month", "field": "field", "as": "b"}
        ],
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "month"}}
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    let mut ancestor = AncestorParse::new();
    let result = parse_transform_array(root.clone(), &model, &mut ancestor);
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<BinNode>(&first));
    assert!(is_node::<TimeUnitNode>(&result));
    assert_eq!(
        ancestor_to_value(&ancestor),
        json!({"a": "number", "a_end": "number", "b": "date", "field": "date"})
    );
}

#[test]
fn should_return_a_bin_node_and_an_aggregate_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [
            {"bin": true, "field": "field", "as": "a"},
            {
                "aggregate": [
                    {"op": "count", "field": "f", "as": "b"},
                    {"op": "sum", "field": "f", "as": "c"}
                ],
                "groupby": ["field"]
            }
        ],
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "month"}}
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<BinNode>(&first));
    assert!(is_node::<AggregateNode>(&result));
}

#[test]
fn should_return_an_impute_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"impute": "x", "key": "y", "method": "mean"}],
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<ImputeNode>(&first));
    assert!(is_node::<ImputeNode>(&result));
}

#[test]
fn should_return_a_window_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"window": [{"op": "count", "field": "f", "as": "b"}]}],
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "month"}}
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<WindowTransformNode>(&first));
}

#[test]
fn should_return_a_window_transform_node_with_optional_properties() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{
            "window": [{"op": "row_number", "as": "ordered_row_number"}],
            "ignorePeers": false,
            "sort": [{"field": "f", "order": "ascending"}]
        }],
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "month"}}
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<WindowTransformNode>(&first));
}

#[test]
fn should_return_a_fold_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"fold": ["a", "b"], "as": ["A", "B"]}],
        "encoding": {
            "x": {"field": "A", "type": "temporal"},
            "y": {"field": "B", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<FoldTransformNode>(&first));
    assert!(is_node::<FoldTransformNode>(&result));
}

#[test]
fn should_return_an_extent_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"extent": "a", "param": "A"}],
        "encoding": {
            "x": {"field": "A", "type": "temporal"},
            "y": {"field": "B", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<ExtentTransformNode>(&first));
    assert!(is_node::<ExtentTransformNode>(&result));
}

#[test]
fn should_return_a_pivot_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"pivot": "a", "value": "b"}],
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<PivotTransformNode>(&first));
    assert!(is_node::<PivotTransformNode>(&result));
}

#[test]
fn should_return_a_flatten_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"flatten": ["a", "b"]}],
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<FlattenTransformNode>(&first));
    assert!(is_node::<FlattenTransformNode>(&result));
}

#[test]
fn should_return_a_sample_transform_node() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{"sample": 1000}],
        "encoding": {
            "x": {"field": "A", "type": "temporal"},
            "y": {"field": "B", "type": "quantitative"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<SampleTransformNode>(&first));
    assert!(is_node::<SampleTransformNode>(&result));
}

#[test]
fn should_return_three_transforms_from_an_impute() {
    let model = parse_unit_model(&json!({
        "data": {"values": []},
        "mark": "point",
        "transform": [{
            "impute": "y",
            "key": "x",
            "method": "max",
            "groupby": ["a", "b"],
            "frame": [-2, 2]
        }],
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"},
            "color": {"field": "c", "type": "nominal"}
        }
    }));
    let root = PlaceholderDataFlowNode::new(None, "root");
    let result = parse_transform_array(root.clone(), &model, &mut AncestorParse::new());
    let first = vega_lite::compile::data::children(&root)[0].clone();
    assert!(is_node::<ImputeNode>(&first));
    assert!(is_node::<ImputeNode>(&result));
}

// ---------------------------------------------------------------------------
// findSource
// ---------------------------------------------------------------------------

fn same_node(a: &NodeRef, b: &NodeRef) -> bool {
    std::rc::Rc::ptr_eq(a, b)
}

#[test]
fn find_source_should_find_named_source() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let actual = find_source(
        &json!({"name": "foo"}),
        &[values, url, named.clone()],
    )
    .unwrap();
    assert!(same_node(&actual, &named));
}

#[test]
fn find_source_should_not_find_source_with_different_name() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    assert!(find_source(&json!({"name": "bar"}), &[values, named, url]).is_none());
}

#[test]
fn find_source_should_find_value_source() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let actual = find_source(
        &json!({"values": [1, 2, 3]}),
        &[named, url, values.clone()],
    )
    .unwrap();
    assert!(same_node(&actual, &values));
}

#[test]
fn find_source_should_find_url_source() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let actual = find_source(
        &json!({"url": "foo.csv"}),
        &[values, named, url.clone()],
    )
    .unwrap();
    assert!(same_node(&actual, &url));
}

#[test]
fn find_source_should_find_url_source_without_a_name() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let actual = find_source(
        &json!({"url": "foo.csv", "name": "fo_csv"}),
        &[values, named, url.clone()],
    )
    .unwrap();
    assert!(same_node(&actual, &url));
}

#[test]
fn find_source_should_not_find_source_with_different_url() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let url = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    assert!(find_source(&json!({"url": "bar.csv"}), &[values, named, url]).is_none());
}

#[test]
fn find_source_should_not_find_source_with_same_url_but_different_name() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let named_url = SourceNode::new(Some(&json!({"url": "foo.csv", "name": "foo_csv"})));
    assert!(find_source(
        &json!({"url": "foo.csv", "name": "incompatible_name"}),
        &[values, named, named_url]
    )
    .is_none());
}

#[test]
fn find_source_should_find_source_with_same_url_that_has_a_name() {
    let values = SourceNode::new(Some(&json!({"values": [1, 2, 3]})));
    let named = SourceNode::new(Some(&json!({"name": "foo"})));
    let named_url = SourceNode::new(Some(&json!({"url": "foo.csv", "name": "foo_csv"})));
    let actual = find_source(
        &json!({"url": "foo.csv"}),
        &[values, named, named_url.clone()],
    )
    .unwrap();
    assert!(same_node(&actual, &named_url));
}

#[test]
fn find_source_should_not_find_a_source_with_mesh_and_feature() {
    let other = SourceNode::new(Some(&json!({
        "url": "foo.csv",
        "format": {"type": "topojson", "feature": "counties"}
    })));
    assert!(find_source(
        &json!({"url": "foo.csv", "format": {"type": "topojson", "mesh": "states"}}),
        &[other]
    )
    .is_none());
}

#[test]
fn find_source_should_not_find_a_source_with_conflicting_features() {
    let other = SourceNode::new(Some(&json!({
        "url": "foo.csv",
        "format": {"type": "topojson", "feature": "counties"}
    })));
    assert!(find_source(
        &json!({"url": "foo.csv", "format": {"type": "topojson", "feature": "states"}}),
        &[other]
    )
    .is_none());
}

#[test]
fn find_source_should_not_find_a_source_with_conflicting_meshes() {
    let other = SourceNode::new(Some(&json!({
        "url": "foo.csv",
        "format": {"type": "topojson", "mesh": "counties"}
    })));
    assert!(find_source(
        &json!({"url": "foo.csv", "format": {"type": "topojson", "mesh": "states"}}),
        &[other]
    )
    .is_none());
}
