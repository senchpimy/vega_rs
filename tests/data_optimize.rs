//! Port of the model-independent parts of `test/compile/data/optimize.test.ts`.
//!
//! The third upstream test exercises `compile()` (signal renaming while merging
//! bin nodes) and is not covered here because the full compile pipeline is out
//! of scope for this port.

use indexmap::IndexMap;
use serde_json::json;

use vega_lite::compile::data::dataflow::{children, OutputNode, OutputNodeType};
use vega_lite::compile::data::formatparse::{ParseNode, Parse};
use vega_lite::compile::data::index::DataComponent;
use vega_lite::compile::data::optimize::optimize_dataflow;
use vega_lite::compile::data::source::SourceNode;

fn parse_map(entries: &[(&str, &str)]) -> Parse {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect::<IndexMap<_, _>>()
}

fn parse_of(node: &vega_lite::compile::data::NodeRef) -> serde_json::Value {
    vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(node, |n| {
        json!(n.parse)
    })
    .unwrap()
}

#[test]
fn should_move_up_common_parse() {
    let source = SourceNode::new(None);
    let parse_one = ParseNode::new(Some(&source), parse_map(&[("a", "time"), ("b", "number")]));
    let parse_two = ParseNode::new(Some(&source), parse_map(&[("a", "time"), ("b", "date")]));
    let out_one = OutputNode::new(Some(&parse_one), "foo", OutputNodeType::Main);
    OutputNode::mark_required(&out_one);
    let out_two = OutputNode::new(Some(&parse_two), "bar", OutputNodeType::Main);
    OutputNode::mark_required(&out_two);

    let mut data = DataComponent {
        sources: vec![source.clone()],
        ..Default::default()
    };
    optimize_dataflow(&mut data, None);

    assert_eq!(children(&source).len(), 1);
    let common_parse = children(&source)[0].clone();
    assert!(vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(
        &common_parse,
        |_| ()
    )
    .is_some());
    assert_eq!(parse_of(&common_parse), json!({"a": "time"}));

    let common_children = children(&common_parse);
    assert_eq!(common_children.len(), 2);

    assert!(vega_lite::compile::data::dataflow::with_node::<ParseNode, _>(
        &common_children[0],
        |_| ()
    )
    .is_some());
    assert_eq!(parse_of(&common_children[0]), json!({"b": "number"}));
    assert_eq!(parse_of(&common_children[1]), json!({"b": "date"}));
}

#[test]
fn should_push_parse_up_from_lowest_level_first_to_avoid_conflicting_common_parse() {
    let source = SourceNode::new(None);
    let parse_one = ParseNode::new(Some(&source), parse_map(&[("a", "time")]));
    let parse_two = ParseNode::new(Some(&source), parse_map(&[("b", "number")]));
    let parse_three = ParseNode::new(Some(&parse_two), parse_map(&[("a", "number")]));
    let out_one = OutputNode::new(Some(&parse_one), "foo", OutputNodeType::Main);
    OutputNode::mark_required(&out_one);
    let out_two = OutputNode::new(Some(&parse_three), "bar", OutputNodeType::Main);
    OutputNode::mark_required(&out_two);

    let mut data = DataComponent {
        sources: vec![source.clone()],
        ..Default::default()
    };
    optimize_dataflow(&mut data, None);

    assert_eq!(children(&source).len(), 1);
    let common_parse = children(&source)[0].clone();
    assert_eq!(parse_of(&common_parse), json!({"b": "number"}));

    let common_children = children(&common_parse);
    assert_eq!(common_children.len(), 2);
    assert_eq!(parse_of(&common_children[0]), json!({"a": "time"}));
    assert_eq!(parse_of(&common_children[1]), json!({"a": "number"}));
}
