//! Port of `test/compile/data/dataflow.test.ts` (node-level portions).

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    add_child, children, clone_node, debug_name, dependent_fields, num_children, parent,
    produced_fields, remove, insert_as_parent_of, swap_with_parent, OutputNode,
    OutputNodeType, PlaceholderDataFlowNode,
};

#[test]
fn swap_should_correctly_swap_two_nodes_in_a_simple_chain() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    let b = PlaceholderDataFlowNode::new(Some(&a), "b");
    let c = PlaceholderDataFlowNode::new(Some(&b), "c");
    let d = PlaceholderDataFlowNode::new(Some(&c), "d");

    swap_with_parent(&c);

    assert_eq!(num_children(&a), 1);
    assert_eq!(debug_name(&children(&a)[0]).as_deref(), Some("c"));
    assert_eq!(num_children(&b), 1);
    assert_eq!(debug_name(&children(&b)[0]).as_deref(), Some("d"));
    assert_eq!(num_children(&c), 1);
    assert_eq!(debug_name(&children(&c)[0]).as_deref(), Some("b"));
    assert_eq!(num_children(&d), 0);
}

#[test]
fn swap_should_correctly_swap_two_nodes() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let parent_node = PlaceholderDataFlowNode::new(Some(&root), "parent");
    let node = PlaceholderDataFlowNode::new(Some(&parent_node), "node");
    let child1 = PlaceholderDataFlowNode::new(Some(&node), "child1");
    let child2 = PlaceholderDataFlowNode::new(Some(&node), "child2");
    let parent_child1 = PlaceholderDataFlowNode::new(Some(&parent_node), "parentChild1");
    let parent_child2 = PlaceholderDataFlowNode::new(Some(&parent_node), "parentChild2");

    swap_with_parent(&node);

    assert_eq!(num_children(&root), 1);
    assert_eq!(debug_name(&children(&root)[0]).as_deref(), Some("node"));
    assert_eq!(debug_name(&parent(&node).unwrap()).as_deref(), Some("root"));

    assert_eq!(num_children(&node), 1);
    assert_eq!(debug_name(&children(&node)[0]).as_deref(), Some("parent"));
    assert_eq!(debug_name(&parent(&parent_node).unwrap()).as_deref(), Some("node"));

    assert_eq!(num_children(&parent_node), 4);
    for c in children(&parent_node) {
        assert_eq!(num_children(&c), 0);
        assert_eq!(debug_name(&parent(&c).unwrap()).as_deref(), Some("parent"));
    }
    assert_eq!(debug_name(&child1).as_deref(), Some("child1"));
    assert_eq!(debug_name(&child2).as_deref(), Some("child2"));
    assert_eq!(debug_name(&parent_child1).as_deref(), Some("parentChild1"));
    assert_eq!(debug_name(&parent_child2).as_deref(), Some("parentChild2"));
}

#[test]
fn remove_should_remove_node_from_dataflow() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    let b = PlaceholderDataFlowNode::new(Some(&a), "b");
    let c = PlaceholderDataFlowNode::new(Some(&b), "c");

    assert_eq!(children(&a).len(), 1);
    assert!(parent(&b).is_some());
    assert!(parent(&c).is_some());

    remove(&b);

    assert_eq!(children(&a).len(), 1);
    assert_eq!(debug_name(&children(&a)[0]).as_deref(), Some("c"));
    assert_eq!(debug_name(&parent(&c).unwrap()).as_deref(), Some("a"));
}

#[test]
fn remove_should_maintain_order() {
    let root = PlaceholderDataFlowNode::new(None, "root");
    let root_child1 = PlaceholderDataFlowNode::new(Some(&root), "rootChild1");
    let node = PlaceholderDataFlowNode::new(Some(&root), "node");
    let root_child2 = PlaceholderDataFlowNode::new(Some(&root), "rootChild2");
    let child1 = PlaceholderDataFlowNode::new(Some(&node), "child1");
    let child2 = PlaceholderDataFlowNode::new(Some(&node), "child2");

    assert_eq!(
        children(&root)
            .iter()
            .map(|c| debug_name(c).unwrap())
            .collect::<Vec<_>>(),
        vec!["rootChild1", "node", "rootChild2"]
    );

    remove(&node);

    assert_eq!(
        children(&root)
            .iter()
            .map(|c| debug_name(c).unwrap())
            .collect::<Vec<_>>(),
        vec!["rootChild1", "child1", "child2", "rootChild2"]
    );
    for c in [&root_child1, &root_child2, &child1, &child2] {
        assert_eq!(debug_name(&parent(c).unwrap()).as_deref(), Some("root"));
    }
}

#[test]
fn insert_as_parent_of_should_insert_node_into_dataflow() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    let another_child = PlaceholderDataFlowNode::new(Some(&a), "anotherChild");
    let b = PlaceholderDataFlowNode::new(None, "b");
    let c = PlaceholderDataFlowNode::new(Some(&a), "c");

    insert_as_parent_of(&b, &c);

    let a_children = children(&a);
    assert!(a_children.iter().any(|x| debug_name(x).as_deref() == Some("anotherChild")));
    assert!(a_children.iter().any(|x| debug_name(x).as_deref() == Some("b")));
    assert_eq!(debug_name(&parent(&b).unwrap()).as_deref(), Some("a"));
    assert_eq!(debug_name(&parent(&c).unwrap()).as_deref(), Some("b"));
    assert_eq!(
        debug_name(&parent(&another_child).unwrap()).as_deref(),
        Some("a")
    );
}

#[test]
fn add_child_should_add_child_to_node() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    let b = PlaceholderDataFlowNode::new(None, "b");
    add_child(&a, &b);
    assert!(parent(&b).is_none());
    assert_eq!(children(&a).len(), 1);
}

#[test]
fn add_child_should_not_add_the_same_child_twice() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    let b = PlaceholderDataFlowNode::new(None, "b");
    let (_, logger) = vega_lite::log::capture(|| {
        add_child(&a, &b);
        add_child(&a, &b);
    });
    assert!(parent(&b).is_none());
    assert_eq!(children(&a).len(), 1);
    assert_eq!(logger.warns[0], json!("Attempt to add the same child twice."));
}

#[test]
fn clone_should_not_copy_parent() {
    let a = PlaceholderDataFlowNode::new(None, "a");
    assert!(parent(&clone_node(&a)).is_none());
}

#[test]
fn output_node_dependent_and_produced_fields_are_empty() {
    let output = OutputNode::new(None, "src", OutputNodeType::Main);
    assert!(dependent_fields(&output).is_empty());
    assert!(produced_fields(&output).unwrap().is_empty());
}
