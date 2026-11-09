//! Port of `test/compile/data/stack.test.ts`.
//!
//! `makeFromEncoding` cases are reconstructed with an explicit
//! `StackComponent` because the encoding/stack-property parser is out of scope.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::stack::{StackComponent, StackNode};

fn component(
    dimension_field_defs: Vec<serde_json::Value>,
    stack_field: &str,
    stackby: Option<Vec<&str>>,
    sort: serde_json::Value,
    impute: Option<bool>,
    as_: [&str; 2],
) -> StackComponent {
    StackComponent {
        dimension_field_defs,
        stack_field: stack_field.to_string(),
        stackby: stackby.map(|v| v.into_iter().map(str::to_string).collect()),
        sort,
        offset: "zero".into(),
        impute,
        as_: [as_[0].to_string(), as_[1].to_string()],
        ..Default::default()
    }
}

#[test]
fn make_from_transform_should_fill_in_offset_and_sort_properly() {
    let t = json!({"stack": "people", "groupby": ["age"], "as": ["v1", "v2"]});
    let stack = StackNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&stack),
        json!([{
            "type": "stack",
            "groupby": ["age"],
            "field": "people",
            "offset": "zero",
            "sort": {"field": [], "order": []},
            "as": ["v1", "v2"]
        }])
    );
}

#[test]
fn make_from_transform_should_fill_in_partial_as_field_properly() {
    let t = json!({"stack": "people", "groupby": ["age", "gender"], "offset": "normalize", "as": "val"});
    let stack = StackNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&stack),
        json!([{
            "type": "stack",
            "groupby": ["age", "gender"],
            "field": "people",
            "offset": "normalize",
            "sort": {"field": [], "order": []},
            "as": ["val", "val_end"]
        }])
    );
}

#[test]
fn make_from_transform_should_handle_complete_sort() {
    let t = json!({
        "stack": "people",
        "groupby": ["age", "gender"],
        "offset": "normalize",
        "sort": [
            {"field": "height", "order": "ascending"},
            {"field": "weight", "order": "descending"}
        ],
        "as": "val"
    });
    let stack = StackNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&stack),
        json!([{
            "type": "stack",
            "groupby": ["age", "gender"],
            "field": "people",
            "offset": "normalize",
            "sort": {"field": ["height", "weight"], "order": ["ascending", "descending"]},
            "as": ["val", "val_end"]
        }])
    );
}

#[test]
fn make_from_transform_should_handle_incomplete_sort_field() {
    let t = json!({
        "stack": "people",
        "groupby": ["age", "gender"],
        "offset": "normalize",
        "sort": [{"field": "height"}],
        "as": "val"
    });
    let stack = StackNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&stack),
        json!([{
            "type": "stack",
            "groupby": ["age", "gender"],
            "field": "people",
            "offset": "normalize",
            "sort": {"field": ["height"], "order": ["ascending"]},
            "as": ["val", "val_end"]
        }])
    );
}

#[test]
fn produced_fields_should_give_produced_fields_correctly() {
    let t = json!({"stack": "people", "groupby": ["age"], "as": "people"});
    let stack = StackNode::make_from_transform(None, &t);
    let fields: Vec<String> = produced_fields(&stack).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["people".to_string(), "people_end".to_string()]);
}

#[test]
fn dependent_fields_should_give_dependent_fields_correctly() {
    let t = json!({"stack": "foo", "groupby": ["bar", "baz"], "as": "qux"});
    let stack = StackNode::make_from_transform(None, &t);
    let fields: Vec<String> = dependent_fields(&stack).into_iter().collect();
    assert_eq!(
        fields,
        vec!["foo".to_string(), "bar".to_string(), "baz".to_string()]
    );
}

#[test]
fn encoding_1d_bar_with_color() {
    let stack = StackNode::new(
        None,
        component(
            vec![],
            "sum_a",
            Some(vec!["c"]),
            json!({"field": ["c"], "order": ["ascending"]}),
            Some(false),
            ["sum_a_start", "sum_a_end"],
        ),
    );
    assert_eq!(
        assemble(&stack),
        json!([{
            "type": "stack",
            "groupby": [],
            "field": "sum_a",
            "sort": {"field": ["c"], "order": ["ascending"]},
            "as": ["sum_a_start", "sum_a_end"],
            "offset": "zero"
        }])
    );
}

#[test]
fn encoding_area_with_color_and_order() {
    let stack = StackNode::new(
        None,
        component(
            vec![json!({"field": "b", "type": "nominal"})],
            "sum_a",
            Some(vec!["c"]),
            json!({"field": ["mean_d"], "order": ["ascending"]}),
            Some(true),
            ["sum_a_start", "sum_a_end"],
        ),
    );
    assert_eq!(
        assemble(&stack),
        json!([
            {
                "type": "impute",
                "field": "sum_a",
                "groupby": ["c"],
                "key": "b",
                "method": "value",
                "value": 0
            },
            {
                "type": "stack",
                "groupby": ["b"],
                "field": "sum_a",
                "sort": {"field": ["mean_d"], "order": ["ascending"]},
                "as": ["sum_a_start", "sum_a_end"],
                "offset": "zero"
            }
        ])
    );
}

#[test]
fn encoding_area_with_color_and_binned_dimension() {
    let stack = StackNode::new(
        None,
        component(
            vec![json!({"bin": {"maxbins": 10}, "field": "b", "type": "quantitative"})],
            "sum_a",
            Some(vec!["c"]),
            json!({"field": ["c"], "order": ["ascending"]}),
            Some(true),
            ["sum_a_start", "sum_a_end"],
        ),
    );
    assert_eq!(
        assemble(&stack),
        json!([
            {
                "type": "formula",
                "expr": "isValid(datum[\"bin_maxbins_10_b\"]) && isFinite(+datum[\"bin_maxbins_10_b\"]) ? 0.5*datum[\"bin_maxbins_10_b\"]+0.5*datum[\"bin_maxbins_10_b_end\"] : datum[\"bin_maxbins_10_b\"]",
                "as": "bin_maxbins_10_b_mid"
            },
            {
                "type": "impute",
                "field": "sum_a",
                "groupby": ["c"],
                "key": "bin_maxbins_10_b_mid",
                "method": "value",
                "value": 0
            },
            {
                "type": "stack",
                "groupby": ["bin_maxbins_10_b_mid"],
                "field": "sum_a",
                "sort": {"field": ["c"], "order": ["ascending"]},
                "as": ["sum_a_start", "sum_a_end"],
                "offset": "zero"
            }
        ])
    );
}

#[test]
fn encoding_hash_and_dependent_fields() {
    let stack = StackNode::new(
        None,
        component(
            vec![json!({"field": "b", "type": "nominal"})],
            "sum_a",
            Some(vec!["c"]),
            json!({"field": ["c"], "order": ["ascending"]}),
            Some(false),
            ["sum_a_start", "sum_a_end"],
        ),
    );
    assert_eq!(
        hash(&stack),
        json!("Stack {\"as\":[\"sum_a_start\",\"sum_a_end\"],\"dimensionFieldDefs\":[{\"field\":\"b\",\"type\":\"nominal\"}],\"facetby\":[],\"impute\":false,\"offset\":\"zero\",\"sort\":{\"field\":[\"c\"],\"order\":[\"ascending\"]},\"stackField\":\"sum_a\",\"stackby\":[\"c\"]}")
    );
    let fields: Vec<String> = dependent_fields(&stack).into_iter().collect();
    assert_eq!(
        fields,
        vec!["sum_a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let stack = StackNode::new(Some(&parent_node), StackComponent::default());
    assert!(parent(&clone_node(&stack)).is_none());
}
