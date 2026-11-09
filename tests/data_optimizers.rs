//! Port of `test/compile/data/optimizers.test.ts`.

use indexmap::IndexMap;
use serde_json::json;

use vega_lite::compile::data::dataflow::{children, num_children, parent};
use vega_lite::compile::data::filter::FilterNode;
use vega_lite::compile::data::formatparse::ParseNode;
use vega_lite::compile::data::impute::ImputeNode;
use vega_lite::compile::data::optimizer::Optimizer;
use vega_lite::compile::data::optimizers::{
    merge_identical_nodes, merge_nodes, merge_parse, merge_time_units,
};
use vega_lite::compile::data::timeunit::{TimeUnitComponent, TimeUnitNode};
use vega_lite::compile::data::PlaceholderDataFlowNode;

fn parse_of(node: &vega_lite::compile::data::NodeRef) -> serde_json::Value {
    vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(node, |n| json!(n.parse))
        .unwrap()
}

#[test]
fn merge_identical_nodes_should_merge_two_impute_nodes_with_identical_transforms() {
    let transform = json!({"impute": "y", "key": "x", "method": "value", "value": 200});
    let root = PlaceholderDataFlowNode::new(None, "root");
    let transform1 = ImputeNode::new(Some(&root), &transform);
    ImputeNode::new(Some(&root), &transform);

    let mut optimizer = merge_identical_nodes();
    optimizer.optimize(&root);

    assert_eq!(children(&root).len(), 1);
    assert!(std::rc::Rc::ptr_eq(&children(&root)[0], &transform1));
    assert!(optimizer.modified_flag());
}

#[test]
fn merge_identical_nodes_should_merge_only_the_children_that_have_the_same_transform() {
    let transform = json!({"impute": "y", "key": "x", "method": "value", "value": 200});
    let root = PlaceholderDataFlowNode::new(None, "root");
    let transform1 = ImputeNode::new(Some(&root), &transform);
    ImputeNode::new(Some(&root), &transform);
    let transform3 = FilterNode::new(Some(&root), None, &json!("datum.x > 2"));
    FilterNode::new(Some(&root), None, &json!("datum.x > 2"));

    let mut optimizer = merge_identical_nodes();
    optimizer.optimize(&root);

    let kids = children(&root);
    assert_eq!(kids.len(), 2);
    assert!(std::rc::Rc::ptr_eq(&kids[0], &transform1));
    assert!(std::rc::Rc::ptr_eq(&kids[1], &transform3));
}

#[test]
fn merge_nodes_should_merge_nodes_correctly() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let a = PlaceholderDataFlowNode::new(Some(&root), "a");
    let b = PlaceholderDataFlowNode::new(Some(&root), "b");

    let a1 = PlaceholderDataFlowNode::new(Some(&a), "a1");
    let a2 = PlaceholderDataFlowNode::new(Some(&a), "a2");
    let b1 = PlaceholderDataFlowNode::new(Some(&b), "b1");
    let b2 = PlaceholderDataFlowNode::new(Some(&b), "b2");

    assert_eq!(children(&root).len(), 2);
    assert_eq!(children(&a).len(), 2);
    assert_eq!(children(&b).len(), 2);

    merge_nodes(&root, &[a.clone(), b.clone()]);
    let optimizer = merge_identical_nodes();
    optimizer.set_modified();

    assert!(optimizer.modified_flag());
    assert_eq!(children(&root).len(), 1);
    assert_eq!(children(&a).len(), 4);

    for node in [a1, a2, b1, b2] {
        let p = parent(&node).unwrap();
        assert!(std::rc::Rc::ptr_eq(&p, &a));
    }
}

#[test]
fn merge_time_units_should_merge_adjacent_time_unit_nodes() {
    let root = PlaceholderDataFlowNode::new(None, "root");

    let c1 = TimeUnitComponent {
        as_: Some("a_yr".to_string()),
        time_unit: json!("year"),
        field: "a".to_string(),
        rect_band_position: None,
    };
    let c2 = TimeUnitComponent {
        as_: Some("b_yr".to_string()),
        time_unit: json!("year"),
        field: "b".to_string(),
        rect_band_position: None,
    };
    let c3 = TimeUnitComponent {
        as_: Some("c_yr".to_string()),
        time_unit: json!("year"),
        field: "c".to_string(),
        rect_band_position: None,
    };

    let mut units1 = IndexMap::new();
    units1.insert("c1".to_string(), c1.clone());
    units1.insert("c2".to_string(), c2.clone());
    TimeUnitNode::new(Some(&root), units1);

    let mut units2 = IndexMap::new();
    units2.insert("c1".to_string(), c1.clone());
    units2.insert("c3".to_string(), c3.clone());
    TimeUnitNode::new(Some(&root), units2);

    let mut optimizer = merge_time_units();
    optimizer.run(&root);

    assert_eq!(children(&root).len(), 1);

    let merged_node = children(&root)[0].clone();
    let mut produced: Vec<String> =
        vega_lite::compile::data::dataflow::produced_fields(&merged_node)
            .unwrap()
            .into_iter()
            .collect();
    produced.sort();
    assert_eq!(
        produced,
        vec!["a_yr".to_string(), "b_yr".to_string(), "c_yr".to_string()]
    );

    let mut deps: Vec<String> =
        vega_lite::compile::data::dataflow::dependent_fields(&merged_node)
            .into_iter()
            .collect();
    deps.sort();
    assert_eq!(deps, vec!["a".to_string(), "b".to_string(), "c".to_string()]);

    assert_eq!(
        vega_lite::compile::data::dataflow::assemble(&merged_node),
        json!([
            {"field": "a", "as": ["a_yr", "a_yr_end"], "units": ["year"], "type": "timeunit"},
            {"field": "c", "as": ["c_yr", "c_yr_end"], "units": ["year"], "type": "timeunit"},
            {"field": "b", "as": ["b_yr", "b_yr_end"], "units": ["year"], "type": "timeunit"}
        ])
    );
}

#[test]
fn merge_parse_should_merge_non_conflicting_parse_nodes() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    ParseNode::new(
        Some(&root),
        [("a".to_string(), json!("number")), ("b".to_string(), json!("string"))]
            .into_iter()
            .collect(),
    );
    ParseNode::new(
        Some(&root),
        [("b".to_string(), json!("string")), ("c".to_string(), json!("boolean"))]
            .into_iter()
            .collect(),
    );

    let mut optimizer = merge_parse();
    optimizer.run(&root);

    assert_eq!(children(&root).len(), 1);
    let merged = children(&root)[0].clone();
    assert_eq!(
        parse_of(&merged),
        json!({"a": "number", "b": "string", "c": "boolean"})
    );
}

#[test]
fn merge_parse_should_not_merge_conflicting_parse_nodes() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    ParseNode::new(
        Some(&root),
        [("a".to_string(), json!("number")), ("b".to_string(), json!("string"))]
            .into_iter()
            .collect(),
    );
    ParseNode::new(
        Some(&root),
        [("a".to_string(), json!("boolean")), ("d".to_string(), json!("date"))]
            .into_iter()
            .collect(),
    );
    ParseNode::new(
        Some(&root),
        [("a".to_string(), json!("boolean")), ("b".to_string(), json!("string"))]
            .into_iter()
            .collect(),
    );

    let mut optimizer = merge_parse();
    optimizer.run(&root);

    assert_eq!(children(&root).len(), 1);
    let merged = children(&root)[0].clone();
    assert_eq!(parse_of(&merged), json!({"b": "string", "d": "date"}));
    let merged_children = children(&merged);
    assert_eq!(parse_of(&merged_children[0]), json!({"a": "number"}));
    assert_eq!(parse_of(&merged_children[1]), json!({"a": "boolean"}));
}

#[test]
fn merge_parse_should_merge_when_there_is_no_parse_node() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let parse = ParseNode::new(
        Some(&root),
        [("a".to_string(), json!("number")), ("b".to_string(), json!("string"))]
            .into_iter()
            .collect(),
    );
    let parse_child = PlaceholderDataFlowNode::new(Some(&parse), "parse_child");
    let other_child = PlaceholderDataFlowNode::new(Some(&root), "other_child");

    let mut optimizer = merge_parse();
    optimizer.run(&root);

    assert_eq!(children(&root).len(), 1);
    let merged = children(&root)[0].clone();
    assert_eq!(
        parse_of(&merged),
        json!({"a": "number", "b": "string"})
    );
    let merged_children = children(&merged);
    assert_eq!(merged_children.len(), 2);
    assert!(merged_children
        .iter()
        .any(|c| std::rc::Rc::ptr_eq(c, &other_child)));
    assert!(merged_children
        .iter()
        .any(|c| std::rc::Rc::ptr_eq(c, &parse_child)));
    assert_eq!(num_children(&merged), 2);
}
