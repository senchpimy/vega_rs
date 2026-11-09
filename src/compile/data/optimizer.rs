//! Port of `src/compile/data/optimizer.ts`.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use super::dataflow::{children, NodeRef};
use super::graticule::GraticuleNode;
use super::sequence::SequenceNode;
use super::source::SourceNode;

/// Whether this dataflow node is the source of the dataflow that produces
/// data i.e. a source or a generator.
pub fn is_data_source_node(node: &NodeRef) -> bool {
    SourceNode::is_source(node)
        || super::dataflow::with_node::<GraticuleNode, _>(node, |_| ()).is_some()
        || super::dataflow::with_node::<SequenceNode, _>(node, |_| ()).is_some()
}

/// Port of the `Optimizer` abstract class.
pub trait Optimizer {
    fn modified_flag(&self) -> bool;
    fn set_modified(&self);
    fn optimize(&mut self, root: &NodeRef) -> bool;
}

pub fn new_modified_flag() -> Rc<Cell<bool>> {
    Rc::new(Cell::new(false))
}

fn get_node_depths(
    node: &NodeRef,
    depth: usize,
    depths: &mut HashMap<usize, usize>,
    nodes: &mut Vec<(NodeRef, usize)>,
) {
    depths.insert(Rc::as_ptr(node) as usize, depth);
    nodes.push((node.clone(), depth));
    for child in children(node) {
        get_node_depths(&child, depth + 1, depths, nodes);
    }
}

/// Port of `BottomUpOptimizer`.
pub struct BottomUpOptimizer<F: FnMut(&NodeRef) -> bool> {
    pub modified: Rc<Cell<bool>>,
    pub run: F,
}

impl<F: FnMut(&NodeRef) -> bool> BottomUpOptimizer<F> {
    pub fn new(run: F) -> Self {
        BottomUpOptimizer {
            modified: new_modified_flag(),
            run,
        }
    }
}

impl<F: FnMut(&NodeRef) -> bool> BottomUpOptimizer<F> {
    /// Run the optimizer on a single node (the upstream `run` method).
    pub fn run(&mut self, node: &NodeRef) -> bool {
        (self.run)(node)
    }
}

impl<F: FnMut(&NodeRef) -> bool> Optimizer for BottomUpOptimizer<F> {
    fn modified_flag(&self) -> bool {
        self.modified.get()
    }

    fn set_modified(&self) {
        self.modified.set(true);
    }

    fn optimize(&mut self, root: &NodeRef) -> bool {
        let mut depths = HashMap::new();
        let mut nodes = Vec::new();
        get_node_depths(root, 0, &mut depths, &mut nodes);

        // sort by depth descending (stable, like Array.prototype.sort)
        nodes.sort_by_key(|(_, depth)| std::cmp::Reverse(*depth));

        for (node, _) in nodes {
            if (self.run)(&node) {
                self.set_modified();
            }
        }

        self.modified_flag()
    }
}

/// Port of `TopDownOptimizer`.
pub struct TopDownOptimizer<F: FnMut(&NodeRef) -> bool> {
    pub modified: Rc<Cell<bool>>,
    pub run: F,
}

impl<F: FnMut(&NodeRef) -> bool> TopDownOptimizer<F> {
    pub fn new(run: F) -> Self {
        TopDownOptimizer {
            modified: new_modified_flag(),
            run,
        }
    }
}

fn optimize_top_down<F: FnMut(&NodeRef) -> bool>(
    run: &mut F,
    modified: &Rc<Cell<bool>>,
    node: &NodeRef,
) {
    if run(node) {
        modified.set(true);
    }
    for child in children(node) {
        optimize_top_down(run, modified, &child);
    }
}

impl<F: FnMut(&NodeRef) -> bool> TopDownOptimizer<F> {
    /// Run the optimizer on a single node (the upstream `run` method).
    pub fn run(&mut self, node: &NodeRef) -> bool {
        (self.run)(node)
    }

    pub fn set_modified(&self) {
        self.modified.set(true);
    }
}

impl<F: FnMut(&NodeRef) -> bool> Optimizer for TopDownOptimizer<F> {
    fn modified_flag(&self) -> bool {
        self.modified.get()
    }

    fn set_modified(&self) {
        self.modified.set(true);
    }

    fn optimize(&mut self, root: &NodeRef) -> bool {
        optimize_top_down(&mut self.run, &self.modified, root);
        self.modified_flag()
    }
}
