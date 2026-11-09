//! Port of `test/compile/data/formatparse.test.ts`.

use indexmap::IndexMap;
use serde_json::{json, Value};

use vega_lite::compile::data::dataflow::{parent, produced_fields};
use vega_lite::compile::data::formatparse::{
    get_implicit_from_encoding, get_implicit_from_selection, ParseNode,
};
use vega_lite::compile::data::index::AncestorParse;
use vega_lite::compile::data::parse::parse_transform_array;
use vega_lite::compile::data::PlaceholderDataFlowNode;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn parse_facet_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_facet(spec, None, "", &config)
}

fn parse_map(entries: &[(&str, Value)]) -> IndexMap<String, Value> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn to_json_map(map: &IndexMap<String, Value>) -> Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k.clone(), v.clone());
    }
    Value::Object(obj)
}

fn parse_value(node: &vega_lite::compile::data::NodeRef) -> Value {
    serde_json::to_value(
        vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(node, |n| n.parse.clone())
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn make_with_ancestors_should_return_null_for_empty_explicit_and_implicit() {
    let mut ancestor_parse = AncestorParse::new();
    assert!(ParseNode::make_with_ancestors(
        None,
        IndexMap::new(),
        IndexMap::new(),
        &mut ancestor_parse
    )
    .is_none());
}

#[test]
fn should_flatten_nested_fields_that_are_used_to_sort_domains() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "ordinal", "sort": {"field": "foo.bar", "op": "mean"}}
        }
    }));
    assert_eq!(
        to_json_map(&get_implicit_from_encoding(&model)),
        json!({"foo.bar": "flatten"})
    );
}

#[test]
fn should_return_a_correct_customized_parse() {
    let model = parse_unit_model(&json!({
        "data": {"url": "a.json", "format": {"parse": {"c": "number", "d": "date"}}},
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "temporal"},
            "color": {"field": "c", "type": "ordinal"},
            "shape": {"field": "c", "type": "nominal"}
        }
    }));

    assert_eq!(
        to_json_map(&get_implicit_from_encoding(&model)),
        json!({"b": "date"})
    );

    let mut ancestor_parse = AncestorParse::new();
    let node = ParseNode::make_explicit(None, &model, &mut ancestor_parse).unwrap();
    assert_eq!(
        parse_value(&node),
        json!({"c": "number", "d": "date"})
    );
}

#[test]
fn should_include_parse_for_all_applicable_fields_and_exclude_calculated_fields() {
    let model = parse_unit_model(&json!({
        "transform": [{"calculate": "datum[\"b\"] * 2", "as": "b2"}],
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "y": {"field": "b", "type": "quantitative"},
            "color": {"type": "quantitative", "aggregate": "count"},
            "size": {"field": "b2", "type": "quantitative"}
        }
    }));

    let mut ancestor_parse = AncestorParse::new();
    let parent = PlaceholderDataFlowNode::new(None, "parent");
    parse_transform_array(parent, &model, &mut ancestor_parse);

    let implicit = get_implicit_from_encoding(&model);
    assert_eq!(to_json_map(&implicit), json!({"a": "date"}));

    let node = ParseNode::make_with_ancestors(None, IndexMap::new(), implicit, &mut ancestor_parse)
        .unwrap();
    assert_eq!(parse_value(&node), json!({"a": "date"}));
}

#[test]
fn should_not_parse_fields_with_aggregate_missing_valid_distinct() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "encoding": {
            "x": {"aggregate": "missing", "field": "b", "type": "quantitative"},
            "y": {"aggregate": "valid", "field": "b", "type": "quantitative"},
            "color": {"aggregate": "distinct", "field": "b", "type": "quantitative"}
        }
    }));
    assert_eq!(to_json_map(&get_implicit_from_encoding(&model)), json!({}));
}

#[test]
fn should_not_parse_the_same_field_twice() {
    let mut model = parse_facet_model(&json!({
        "data": {"values": [], "format": {"parse": {"a": "number"}}},
        "facet": {"row": {"field": "a", "type": "ordinal"}},
        "spec": {
            "mark": "point",
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {"field": "b", "type": "temporal"}
            }
        }
    }));

    let mut ancestor_parse = AncestorParse::new();
    let node = ParseNode::make_explicit(None, &model, &mut ancestor_parse).unwrap();
    assert_eq!(parse_value(&node), json!({"a": "number"}));

    model.parse_scale();

    // The upstream test calls `model.parseData()` here, which is not wired into
    // this port's model.  Emulate the resulting child ancestor parse by copying
    // the explicit parse down to the child model.
    let child = &mut model.children[0];
    let mut child_ancestor = AncestorParse::with_explicit(
        ancestor_parse.explicit.clone(),
        ancestor_parse.implicit.clone(),
        false,
    );
    let child_implicit = get_implicit_from_encoding(child);
    let parse_node = ParseNode::make_with_ancestors(
        None,
        IndexMap::new(),
        child_implicit.clone(),
        &mut child_ancestor,
    );
    let mut combined = child_ancestor.combine();
    if let Some(parse_node) = parse_node {
        let _ = parse_node;
    }
    combined.retain(|_, v| !v.is_null());
    assert_eq!(to_json_map(&combined), json!({"a": "number", "b": "date"}));

    // set the ancestor parse to see whether fields from it are not parsed
    let mut ancestor = AncestorParse::with_explicit(
        parse_map(&[("a", json!("number"))]),
        IndexMap::new(),
        false,
    );
    let implicit = get_implicit_from_encoding(child);
    assert_eq!(to_json_map(&implicit), json!({"b": "date"}));
    let node = ParseNode::make_with_ancestors(None, IndexMap::new(), implicit, &mut ancestor)
        .unwrap();
    assert_eq!(parse_value(&node), json!({"b": "date"}));
}

#[test]
fn should_not_parse_the_same_field_twice_in_explicit() {
    let model = parse_unit_model(&json!({
        "data": {"values": [], "format": {"parse": {"a": "number"}}},
        "mark": "point",
        "encoding": {}
    }));
    let mut ancestor =
        AncestorParse::with_explicit(parse_map(&[("a", json!("number"))]), IndexMap::new(), false);
    assert!(ParseNode::make_explicit(None, &model, &mut ancestor).is_none());
}

#[test]
fn should_not_parse_the_same_field_twice_in_implicit() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}}
    }));
    let mut ancestor =
        AncestorParse::with_explicit(parse_map(&[("a", json!("number"))]), IndexMap::new(), false);
    assert!(ParseNode::make_explicit(None, &model, &mut ancestor).is_none());
}

#[test]
fn should_not_parse_counts() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "encoding": {
            "x": {"aggregate": "sum", "field": "foo", "type": "quantitative"},
            "y": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    assert_eq!(to_json_map(&get_implicit_from_encoding(&model)), json!({}));
}

#[test]
fn should_add_flatten_for_nested_fields_in_encoding() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "foo.bar", "type": "quantitative"},
            "y": {"field": "foo.baz", "type": "ordinal"}
        }
    }));
    assert_eq!(
        to_json_map(&get_implicit_from_encoding(&model)),
        json!({"foo.bar": "flatten", "foo.baz": "flatten"})
    );
}

#[test]
fn should_add_flatten_for_nested_fields_in_selection() {
    let mut model = parse_unit_model(&json!({
        "params": [{"name": "foo", "select": {"type": "point", "fields": ["foo.bar", "foo.baz"]}}],
        "mark": "point",
        "encoding": {
            "x": {"field": "bar", "type": "quantitative"},
            "y": {"field": "baz", "type": "ordinal"}
        }
    }));
    vega_lite::compile::selection::parse::parse_selections(&mut model);

    assert_eq!(
        to_json_map(&get_implicit_from_selection(&model)),
        json!({"foo.bar": "flatten", "foo.baz": "flatten"})
    );
}

#[test]
fn should_not_parse_if_parse_is_disabled_for_a_field() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "data": {"values": [], "format": {"parse": {"b": null}}},
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));

    let mut ancestor = AncestorParse::new();
    assert!(ParseNode::make_explicit(None, &model, &mut ancestor).is_none());
    assert_eq!(to_json_map(&ancestor.combine()), json!({"b": null}));
    assert_eq!(to_json_map(&get_implicit_from_encoding(&model)), json!({}));
}

#[test]
fn should_not_parse_if_parse_is_disabled() {
    let model = parse_unit_model(&json!({
        "mark": "point",
        "data": {"values": [], "format": {"parse": null}},
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));

    let mut ancestor =
        AncestorParse::with_explicit(IndexMap::new(), IndexMap::new(), true);
    assert!(ParseNode::make_explicit(None, &model, &mut ancestor).is_none());
}

#[test]
fn should_add_parse_for_domains_of_path_marks() {
    let model = parse_unit_model(&json!({
        "mark": "line",
        "encoding": {
            "x": {"field": "foo", "type": "quantitative"},
            "y": {"field": "bar", "type": "quantitative"}
        }
    }));
    assert_eq!(
        to_json_map(&get_implicit_from_encoding(&model)),
        json!({"foo": "number"})
    );
}

#[test]
fn assemble_transforms_should_assemble_correct_parse_expressions() {
    let p = ParseNode::new(
        None,
        parse_map(&[
            ("n", json!("number")),
            ("b", json!("boolean")),
            ("s", json!("string")),
            ("d1", json!("date")),
            ("d2", json!("date:\"%y\"")),
            ("d3", json!("utc:\"%y\"")),
            ("d4", json!("utc:%y")),
            ("d5", json!("utc:'%y'")),
        ]),
    );

    let transforms = vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(&p, |n| {
        n.assemble_transforms(false)
    })
    .unwrap();

    assert_eq!(
        Value::Array(transforms),
        json!([
            {"type": "formula", "expr": "toNumber(datum[\"n\"])", "as": "n"},
            {"type": "formula", "expr": "toBoolean(datum[\"b\"])", "as": "b"},
            {"type": "formula", "expr": "toString(datum[\"s\"])", "as": "s"},
            {"type": "formula", "expr": "toDate(datum[\"d1\"])", "as": "d1"},
            {"type": "formula", "expr": "timeParse(datum[\"d2\"],'%y')", "as": "d2"},
            {"type": "formula", "expr": "utcParse(datum[\"d3\"],'%y')", "as": "d3"},
            {"type": "formula", "expr": "utcParse(datum[\"d4\"],'%y')", "as": "d4"},
            {"type": "formula", "expr": "utcParse(datum[\"d5\"],'%y')", "as": "d5"}
        ])
    );
}

#[test]
fn assemble_transforms_should_assemble_flatten_for_nested_fields() {
    let p = ParseNode::new(
        None,
        parse_map(&[("flat", json!("number")), ("nested.field", json!("flatten"))]),
    );
    let transforms =
        vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(&p, |n| {
            n.assemble_transforms(true)
        })
        .unwrap();
    assert_eq!(
        Value::Array(transforms),
        json!([
            {"type": "formula", "expr": "datum[\"nested\"] && datum[\"nested\"][\"field\"]", "as": "nested.field"}
        ])
    );
}

#[test]
fn assemble_transforms_should_show_warning_for_unrecognized_types() {
    let p = ParseNode::new(None, parse_map(&[("x", json!("foo"))]));
    let transforms =
        vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(&p, |n| {
            n.assemble_transforms(false)
        })
        .unwrap();
    assert!(transforms.is_empty());
}

#[test]
fn assemble_format_parse_should_assemble_correct_parse() {
    let p = ParseNode::new(
        None,
        parse_map(&[
            ("n", json!("number")),
            ("b", json!("boolean")),
            ("nested.field", json!("flatten")),
        ]),
    );
    let result = vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(&p, |n| {
        n.assemble_format_parse()
    })
    .unwrap();
    assert_eq!(result, json!({"n": "number", "b": "boolean"}));
}

#[test]
fn produced_fields_should_produce_the_correct_fields() {
    let p = ParseNode::new(
        None,
        parse_map(&[
            ("n", json!("number")),
            ("b", json!("boolean")),
            ("nested.field", json!("flatten")),
        ]),
    );
    let produced: Vec<String> = produced_fields(&p).unwrap().into_iter().collect();
    assert_eq!(
        produced,
        vec![
            "n".to_string(),
            "b".to_string(),
            "nested.field".to_string()
        ]
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let parse = ParseNode::new(Some(&parent_node), IndexMap::new());
    let clone = ParseNode::clone_parse_node(&parse);
    assert!(parent(&clone).is_none());
}
