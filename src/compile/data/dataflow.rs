//! Port of `src/compile/data/dataflow.ts`.
//!
//! The TypeScript implementation uses a class hierarchy.  Rust has no
//! inheritance, so each node owns a `Box<dyn NodeBehavior>` and the links
//! between nodes are stored on the shared [`DataFlowNode`] wrapper (which is
//! handed around as [`NodeRef`]).

use std::any::Any;
use std::cell::{RefCell};
use std::rc::{Rc, Weak};

use indexmap::IndexSet;
use serde_json::{json, Value};

use crate::log;
use crate::util::unique_id;

/// A set of field names.  `IndexSet` keeps insertion order like JavaScript's
/// `Set`, which matters for e.g. the aggregate `as` array.
pub type FieldSet = IndexSet<String>;

pub trait NodeBehavior: Any {
    /// Deep copy of the node payload, without parent/child links.
    fn clone_box(&self) -> Box<dyn NodeBehavior>;

    /// Return a hash of the node (`string | number` upstream).
    fn hash(&self) -> Value;

    /// Fields this node depends on.
    fn dependent_fields(&self) -> FieldSet;

    /// Fields this node produces.  `None` is the Rust equivalent of the
    /// upstream `undefined` (used by `PivotTransformNode`).
    fn produced_fields(&self) -> Option<FieldSet>;

    /// Assemble the Vega transform(s) for this node.
    fn assemble(&self) -> Value {
        Value::Null
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct DataFlowNode {
    pub kind: Box<dyn NodeBehavior>,
    pub parent: Option<Weak<RefCell<DataFlowNode>>>,
    pub children: Vec<NodeRef>,
    pub debug_name: Option<String>,
}

pub type NodeRef = Rc<RefCell<DataFlowNode>>;

impl DataFlowNode {
    pub fn new(kind: Box<dyn NodeBehavior>, parent: Option<&NodeRef>, debug_name: Option<String>) -> NodeRef {
        let node = Rc::new(RefCell::new(DataFlowNode {
            kind,
            parent: None,
            children: Vec::new(),
            debug_name,
        }));
        if let Some(parent) = parent {
            set_parent(&node, Some(parent));
        }
        node
    }
    pub fn parent(&self) -> Option<NodeRef> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    pub fn num_children(&self) -> usize {
        self.children.len()
    }

    pub fn add_child(&mut self, child: &NodeRef) {
        if self.children.iter().any(|c| Rc::ptr_eq(c, child)) {
            log::warn("Attempt to add the same child twice.");
            return;
        }
        self.children.push(child.clone());
    }

    pub fn add_child_at(&mut self, child: &NodeRef, loc: usize) {
        if self.children.iter().any(|c| Rc::ptr_eq(c, child)) {
            log::warn("Attempt to add the same child twice.");
            return;
        }
        let loc = loc.min(self.children.len());
        self.children.insert(loc, child.clone());
    }

    pub fn remove_child(&mut self, old_child: &NodeRef) -> usize {
        // JS `splice(indexOf(child), 1)` removes the last element when the
        // child is not found (index -1); mirror that behaviour.
        if let Some(loc) = self.children.iter().position(|c| Rc::ptr_eq(c, old_child)) {
            self.children.remove(loc);
            loc
        } else if !self.children.is_empty() {
            let loc = self.children.len() - 1;
            self.children.pop();
            loc
        } else {
            0
        }
    }

}

/// Set the parent of `node` and add `node` to the parent's children.
pub fn set_parent(node: &NodeRef, parent: Option<&NodeRef>) {
    node.borrow_mut().parent = parent.map(Rc::downgrade);
    if let Some(parent) = parent {
        parent.borrow_mut().add_child(node);
    }
}


pub fn num_children(node: &NodeRef) -> usize {
    node.borrow().num_children()
}

pub fn children(node: &NodeRef) -> Vec<NodeRef> {
    node.borrow().children.clone()
}

pub fn parent(node: &NodeRef) -> Option<NodeRef> {
    node.borrow().parent()
}

pub fn add_child(parent: &NodeRef, child: &NodeRef) {
    parent.borrow_mut().add_child(child);
}

pub fn remove_child(parent: &NodeRef, child: &NodeRef) -> usize {
    parent.borrow_mut().remove_child(child)
}

/// `DataFlowNode.remove()`.
pub fn remove(node: &NodeRef) {
    let Some(parent) = parent(node) else {
        return;
    };
    let loc = remove_child(&parent, node);

    let kids = children(node);
    for (i, child) in kids.into_iter().enumerate() {
        child.borrow_mut().parent = Some(Rc::downgrade(&parent));
        parent.borrow_mut().add_child_at(&child, loc + i);
    }
}

/// `DataFlowNode.insertAsParentOf(other)`.
pub fn insert_as_parent_of(this: &NodeRef, other: &NodeRef) {
    if let Some(other_parent) = parent(other) {
        // Only detach nodes that are actually children: `remove_child` mimics
        // JavaScript's `splice(-1, 1)` fallback for missing children.
        if children(&other_parent)
            .iter()
            .any(|c| std::rc::Rc::ptr_eq(c, this))
        {
            remove_child(&other_parent, this);
        }
        if children(&other_parent)
            .iter()
            .any(|c| std::rc::Rc::ptr_eq(c, other))
        {
            remove_child(&other_parent, other);
        }
        set_parent(this, Some(&other_parent));
    }
    set_parent(other, Some(this));
}

/// `DataFlowNode.swapWithParent()`.
pub fn swap_with_parent(this: &NodeRef) {
    let Some(parent_node) = parent(this) else {
        return;
    };
    let Some(new_parent) = parent(&parent_node) else {
        return;
    };

    // Reconnect the children.
    for child in children(this) {
        set_parent(&child, Some(&parent_node));
    }

    // Remove old links.
    this.borrow_mut().children.clear();
    remove_child(&parent_node, this);
    let loc = remove_child(&new_parent, &parent_node);

    // Swap the two nodes while maintaining order.
    this.borrow_mut().parent = Some(Rc::downgrade(&new_parent));
    new_parent.borrow_mut().add_child_at(this, loc);

    set_parent(&parent_node, Some(this));
}

/// Deep clone the node payload, without cloning parent or children links.
pub fn clone_node(node: &NodeRef) -> NodeRef {
    let kind = node.borrow().kind.clone_box();
    let debug_name = node
        .borrow()
        .debug_name
        .as_ref()
        .map(|n| format!("clone_{n}"));
    DataFlowNode::new(kind, None, debug_name)
}

pub fn hash(node: &NodeRef) -> Value {
    node.borrow().kind.hash()
}

pub fn dependent_fields(node: &NodeRef) -> FieldSet {
    node.borrow().kind.dependent_fields()
}

pub fn produced_fields(node: &NodeRef) -> Option<FieldSet> {
    node.borrow().kind.produced_fields()
}

pub fn assemble(node: &NodeRef) -> Value {
    node.borrow().kind.assemble()
}

pub fn debug_name(node: &NodeRef) -> Option<String> {
    node.borrow().debug_name.clone()
}

/// Downcast helper.
pub fn with_node<T: NodeBehavior + 'static, R>(node: &NodeRef, f: impl FnOnce(&T) -> R) -> Option<R> {
    let n = node.borrow();
    n.kind.as_any().downcast_ref::<T>().map(f)
}

/// Mutable downcast helper.
pub fn with_node_mut<T: NodeBehavior + 'static, R>(node: &NodeRef, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    let mut n = node.borrow_mut();
    n.kind.as_any_mut().downcast_mut::<T>().map(f)
}

// ---------------------------------------------------------------------------
// OutputNode
// ---------------------------------------------------------------------------

/// Simplified port of `OutputNode`.  The reference counting map is kept local
/// to this node; the data-parsing layer is out of scope for this port.
pub struct OutputNode {
    pub source: String,
    pub name: String,
    pub data_source_type: OutputNodeType,
    pub ref_count: u64,
    pub cached_hash: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputNodeType {
    Main,
    Raw,
    Lookup,
    Filter,
    Facet,
    Data,
    PreFilterInvalid,
    PostFilterInvalid,
}

impl NodeBehavior for OutputNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(OutputNode {
            source: self.source.clone(),
            name: format!("clone_{}", self.name),
            data_source_type: self.data_source_type,
            ref_count: 0,
            cached_hash: None,
        })
    }

    fn hash(&self) -> Value {
        // `hash()` caches a unique id upstream.  We cannot mutate through
        // `&self`, so derive a stable value from the source name instead.
        if let Some(h) = &self.cached_hash {
            h.clone()
        } else {
            Value::String(format!("Output {}", self.name))
        }
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl OutputNode {
    pub fn new(
        parent: Option<&NodeRef>,
        source: &str,
        data_source_type: OutputNodeType,
    ) -> NodeRef {
        DataFlowNode::new(
            Box::new(OutputNode {
                source: source.to_string(),
                name: source.to_string(),
                data_source_type,
                ref_count: 0,
                cached_hash: None,
            }),
            parent,
            Some(source.to_string()),
        )
    }

    pub fn get_source(node: &NodeRef) -> String {
        with_node_mut::<OutputNode, _>(node, |n| {
            n.ref_count += 1;
            n.source.clone()
        })
        .unwrap_or_default()
    }

    pub fn is_required(node: &NodeRef) -> bool {
        with_node::<OutputNode, _>(node, |n| n.ref_count > 0).unwrap_or(false)
    }

    /// Mark the node as required (equivalent to incrementing the reference
    /// counter through `getSource`).
    pub fn set_ref_count(node: &NodeRef, count: u64) {
        with_node_mut::<OutputNode, _>(node, |n| n.ref_count = count);
    }

    pub fn mark_required(node: &NodeRef) {
        OutputNode::set_ref_count(node, 1);
    }

    pub fn set_source(node: &NodeRef, source: &str) {
        with_node_mut::<OutputNode, _>(node, |n| n.source = source.to_string());
    }

    pub fn assemble_output(node: &NodeRef) -> Value {
        with_node::<OutputNode, _>(node, |n| {
            json!({
                "name": n.name,
                "source": n.source,
            })
        })
        .unwrap_or(Value::Null)
    }
}

/// Convenience re-export used by `OutputNode`.
pub fn output_unique_id() -> Value {
    Value::String(unique_id(None))
}

// ---------------------------------------------------------------------------
// PlaceholderDataFlowNode (used by upstream dataflow tests)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PlaceholderDataFlowNode;

impl PlaceholderDataFlowNode {
    pub fn new(parent: Option<&NodeRef>, debug_name: impl Into<String>) -> NodeRef {
        DataFlowNode::new(Box::new(PlaceholderDataFlowNode), parent, Some(debug_name.into()))
    }
}

impl NodeBehavior for PlaceholderDataFlowNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(unique_id(None))
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
