//! Port of `src/compile/data/subtree.ts`.

use super::aggregate::AggregateNode;
use super::dataflow::{
    children, clone_node, num_children, parent, set_parent, with_node, NodeRef, OutputNode,
    OutputNodeType,
};
use super::facet::FacetNode;
use super::joinaggregate::JoinAggregateTransformNode;
use super::optimize::FACET_SCALE_PREFIX;
use super::stack::StackNode;
use super::window::WindowTransformNode;

fn is_node<T: crate::compile::data::dataflow::NodeBehavior + 'static>(node: &NodeRef) -> bool {
    with_node::<T, _>(node, |_| ()).is_some()
}

/// Clones the subtree and ignores output nodes except for the leaves, which are
/// renamed.
pub fn clone_subtree(node: &NodeRef, facet: &NodeRef) -> Vec<NodeRef> {
    fn clone(node: &NodeRef, facet: &NodeRef) -> Vec<NodeRef> {
        if !is_node::<FacetNode>(node) {
            let copy = clone_node(node);

            if is_node::<OutputNode>(&copy) {
                let new_name = format!(
                    "{}{}",
                    FACET_SCALE_PREFIX,
                    OutputNode::get_source(&copy)
                );
                OutputNode::set_source(&copy, &new_name);
            } else if is_node::<AggregateNode>(&copy) {
                let fields = FacetNode::node_fields(facet);
                AggregateNode::add_dimensions(&copy, &fields);
            } else if is_node::<StackNode>(&copy) {
                let fields = FacetNode::node_fields(facet);
                StackNode::add_dimensions(&copy, &fields);
            } else if is_node::<WindowTransformNode>(&copy) {
                let fields = FacetNode::node_fields(facet);
                WindowTransformNode::add_dimensions(&copy, &fields);
            } else if is_node::<JoinAggregateTransformNode>(&copy) {
                let fields = FacetNode::node_fields(facet);
                JoinAggregateTransformNode::add_dimensions(&copy, &fields);
            }

            for child in children(node) {
                for n in clone(&child, facet) {
                    set_parent(&n, Some(&copy));
                }
            }

            return vec![copy];
        }

        let mut out = Vec::new();
        for child in children(node) {
            out.extend(clone(&child, facet));
        }
        out
    }
    clone(node, facet)
}

/// Checks whether facet node can swap with the given child.
fn can_move_facet_below(facet: &NodeRef, child: &NodeRef) -> bool {
    if is_node::<OutputNode>(child) {
        return false;
    }
    if is_node::<AggregateNode>(child) {
        return FacetNode::node_can_move_below_aggregate(facet);
    }
    true
}

/// Move facet nodes down to the next fork or output node.
pub fn move_facet_down(node: &NodeRef) {
    if is_node::<FacetNode>(node) {
        let kids = children(node);
        if num_children(node) == 1 && can_move_facet_below(node, &kids[0]) {
            let child = kids[0].clone();

            if is_node::<AggregateNode>(&child)
                || is_node::<StackNode>(&child)
                || is_node::<WindowTransformNode>(&child)
                || is_node::<JoinAggregateTransformNode>(&child)
            {
                let fields = FacetNode::node_fields(node);
                if is_node::<AggregateNode>(&child) {
                    let sort_index_fields = FacetNode::node_sort_index_fields(node);
                    AggregateNode::add_dimensions(&child, &fields);
                    AggregateNode::add_dimensions(&child, &sort_index_fields);
                } else if is_node::<StackNode>(&child) {
                    StackNode::add_dimensions(&child, &fields);
                } else if is_node::<WindowTransformNode>(&child) {
                    WindowTransformNode::add_dimensions(&child, &fields);
                } else if is_node::<JoinAggregateTransformNode>(&child) {
                    JoinAggregateTransformNode::add_dimensions(&child, &fields);
                }
            }

            super::dataflow::swap_with_parent(&child);
            move_facet_down(node);
        } else {
            // Move the main output down to the facet (best effort: the main
            // output node is not reachable from the cloned model).
            let facet_main = parent(node);
            if let Some(main) = facet_main {
                move_main_down_to_facet(&main);
            }

            // replicate the subtree and place it before the facet's main node
            if let Some(facet_main) = parent(node) {
                for child in children(node) {
                    for c in clone_subtree(&child, node) {
                        set_parent(&c, Some(&facet_main));
                    }
                }
            }
        }
    } else {
        for child in children(node) {
            move_facet_down(&child);
        }
    }
}

fn move_main_down_to_facet(node: &NodeRef) {
    if is_node::<OutputNode>(node)
        && with_node::<OutputNode, _>(node, |n| n.data_source_type == OutputNodeType::Main)
            .unwrap_or(false)
        && num_children(node) == 1 {
            let child = children(node)[0].clone();
            if !is_node::<FacetNode>(&child) {
                super::dataflow::swap_with_parent(&child);
                move_main_down_to_facet(node);
            }
        }
}
