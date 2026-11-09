//! Port of `test/compile/data/debug.test.ts`.
//!
//! The upstream test compares the exact Graphviz output including internal
//! `uniqueId` counters.  This port uses pointer-based node identifiers (there
//! is no global id counter on dataflow nodes), so the tests verify the
//! structure and labels instead of the generated ids.

use vega_lite::compile::data::dataflow::{children, PlaceholderDataFlowNode};
use vega_lite::compile::data::debug::{dot_string, print_debug_dataflow};
use vega_lite::compile::data::optimize::check_links;
use vega_lite::compile::data::source::SourceNode;

#[test]
fn should_draw_simple_dataflow_graph() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    PlaceholderDataFlowNode::new(Some(&root), "child");
    let dot = dot_string(&[root]);
    assert!(dot.starts_with("digraph DataFlow {"));
    assert!(dot.contains("rankdir = TB;"));
    assert!(dot.contains("node [shape=record]"));
    assert_eq!(dot.matches("label = <").count(), 2);
    assert_eq!(dot.matches(" -> ").count(), 1);
}

#[test]
fn should_print_node_debug_name_when_defined() {
    let root = PlaceholderDataFlowNode::new(None, "foo");
    PlaceholderDataFlowNode::new(Some(&root), "bar");
    let dot = dot_string(&[root]);
    assert!(dot.contains("<i>foo</i>"));
    assert!(dot.contains("<i>bar</i>"));
}

#[test]
fn should_print_node_data_url_when_defined() {
    let root = SourceNode::new(Some(&serde_json::json!({"url": "foo.bar"})));
    let dot = dot_string(&[root]);
    assert!(dot.contains("Source"));
    assert!(dot.contains("<i>foo.bar</i>"));
}

#[test]
fn check_links_should_return_false_when_given_inconsistent_data_flow() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let node = PlaceholderDataFlowNode::new(Some(&root), "node");
    node.borrow_mut().parent = None;
    assert!(!check_links(&[root]));
}

#[test]
fn check_links_should_return_false_with_more_complicated_inconsistent_data_flow() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let node = PlaceholderDataFlowNode::new(Some(&root), "node");
    let node2 = PlaceholderDataFlowNode::new(Some(&node), "node2");
    node2.borrow_mut().parent = None;
    assert!(!check_links(&[root]));
}

#[test]
fn check_links_should_return_true_for_a_consistent_data_flow() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let node = PlaceholderDataFlowNode::new(Some(&root), "node");
    PlaceholderDataFlowNode::new(Some(&node), "leaf");
    assert!(check_links(&[root]));
}

#[test]
fn should_print_simple_dataflow_graph_without_panicking() {
    let root = PlaceholderDataFlowNode::new(None, "foo");
    PlaceholderDataFlowNode::new(Some(&root), "bar");
    print_debug_dataflow(&root);
    assert_eq!(children(&root).len(), 1);
}
