//! Port of `test/compile/data/filterinvalid.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, produced_fields,
};
use vega_lite::compile::data::filterinvalid::{
    DataSourcesForHandlingInvalidValues, FilterInvalidNode,
};
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

const FILTER: DataSourcesForHandlingInvalidValues = DataSourcesForHandlingInvalidValues {
    marks: "exclude-invalid-values",
    scales: "exclude-invalid-values",
};

fn parse_unit_model_with_scale(spec: &serde_json::Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn parse(model: &Model) -> Option<vega_lite::compile::data::NodeRef> {
    FilterInvalidNode::make(None, model, FILTER)
}

fn base_spec() -> serde_json::Value {
    json!({
        "mark": "point",
        "encoding": {
            "y": {"field": "qq", "type": "quantitative"},
            "x": {"field": "tt", "type": "temporal"},
            "color": {"field": "oo", "type": "ordinal"},
            "shape": {"field": "nn", "type": "nominal"}
        }
    })
}

fn filter_keys(node: &vega_lite::compile::data::NodeRef) -> Vec<String> {
    let mut keys: Vec<String> = dependent_fields(node).into_iter().collect();
    keys.sort();
    keys
}

#[test]
fn should_add_filter_null_for_q_and_t_by_default() {
    let model = parse_unit_model_with_scale(&base_spec());
    assert_eq!(
        filter_keys(&parse(&model).unwrap()),
        vec!["qq".to_string(), "tt".to_string()]
    );
}

#[test]
fn should_add_filter_null_for_q_and_t_when_invalid_is_filter() {
    let mut spec = base_spec();
    spec["config"] = json!({"mark": {"invalid": "filter"}});
    let model = parse_unit_model_with_scale(&spec);
    assert_eq!(
        filter_keys(&parse(&model).unwrap()),
        vec!["qq".to_string(), "tt".to_string()]
    );
}

#[test]
fn should_add_no_null_filter_when_invalid_is_show() {
    let mut spec = base_spec();
    spec["config"] = json!({"mark": {"invalid": "show"}});
    let model = parse_unit_model_with_scale(&spec);
    assert!(parse(&model).is_none());
}

#[test]
fn should_add_no_null_filter_for_count_field() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"y": {"aggregate": "count", "type": "quantitative"}}
    }));
    assert!(parse(&model).is_none());
}

#[test]
fn dependent_fields_should_return_the_fields_it_filters() {
    let mut filter = indexmap::IndexMap::new();
    filter.insert(
        "foo".to_string(),
        json!({"field": "foo", "type": "quantitative"}),
    );
    let node = FilterInvalidNode::new(None, filter);
    let deps: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(deps, vec!["foo".to_string()]);
}

#[test]
fn produced_fields_should_return_empty_set() {
    let mut filter = indexmap::IndexMap::new();
    filter.insert(
        "foo".to_string(),
        json!({"field": "foo", "type": "quantitative"}),
    );
    let node = FilterInvalidNode::new(None, filter);
    assert!(produced_fields(&node).unwrap().is_empty());
}

#[test]
fn clone_should_copy_filters() {
    let mut filter = indexmap::IndexMap::new();
    filter.insert(
        "foo".to_string(),
        json!({"field": "foo", "type": "quantitative"}),
    );
    let node = FilterInvalidNode::new(None, filter.clone());
    let copy = clone_node(&node);
    assert_eq!(
        vega_lite::compile::data::dataflow::with_node::<FilterInvalidNode, _>(&copy, |n| n
            .filter
            .clone())
        .unwrap(),
        filter
    );
}

#[test]
fn assemble_should_assemble_simple_filter() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"y": {"field": "foo", "type": "quantitative"}}
    }));
    assert_eq!(
        assemble(&parse(&model).unwrap()),
        json!({
            "type": "filter",
            "expr": "isValid(datum[\"foo\"]) && isFinite(+datum[\"foo\"])"
        })
    );
}

#[test]
fn assemble_should_assemble_filter_for_nested_data() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"y": {"field": "foo.bar", "type": "quantitative"}}
    }));
    assert_eq!(
        assemble(&parse(&model).unwrap()),
        json!({
            "type": "filter",
            "expr": "isValid(datum[\"foo.bar\"]) && isFinite(+datum[\"foo.bar\"])"
        })
    );
}
