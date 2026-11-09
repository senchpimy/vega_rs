//! Port of `src/compile/data/optimizers.ts`.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::aggregate::AggregateNode;
use super::bin::BinNode;
use super::dataflow::{
    add_child, children, hash, num_children, parent, produced_fields, remove, remove_child,
    set_parent, FieldSet, NodeRef, OutputNode,
};
use super::facet::FacetNode;
use super::filter::FilterNode;
use super::formatparse::ParseNode;
use super::identifier::IdentifierNode;
use super::optimizer::{
    is_data_source_node, new_modified_flag, BottomUpOptimizer, Optimizer,
    TopDownOptimizer,
};
use super::source::SourceNode;
use super::timeunit::TimeUnitNode;
use crate::compile::model::Model;

fn is_node<T: crate::compile::data::dataflow::NodeBehavior + 'static>(node: &NodeRef) -> bool {
    super::dataflow::with_node::<T, _>(node, |_| ()).is_some()
}

fn node_hash_key(node: &NodeRef) -> String {
    match hash(node) {
        Value::String(s) => s,
        other => crate::util::stringify(&other),
    }
}

/// `hasIntersection` for two `FieldSet`s.
pub fn has_intersection(a: &FieldSet, b: &FieldSet) -> bool {
    a.iter().any(|x| b.contains(x))
}

/// `fieldIntersection` for optional field sets. Returns `true` when either side
/// is unknown (`None`), matching upstream, and compares access-path prefixes.
pub fn field_intersection(a: Option<&FieldSet>, b: Option<&FieldSet>) -> bool {
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        _ => return true,
    };
    let prefix = |set: &FieldSet| -> FieldSet {
        let btree: std::collections::BTreeSet<String> = set.iter().cloned().collect();
        crate::util::prefix_generator(&btree).into_iter().collect()
    };
    has_intersection(&prefix(a), &prefix(b))
}

/// Port of `MergeIdenticalNodes.mergeNodes`.
pub fn merge_nodes(parent_node: &NodeRef, nodes: &[NodeRef]) {
    let Some(merged_node) = nodes.first().cloned() else {
        return;
    };
    for node in nodes.iter().skip(1) {
        remove_child(parent_node, node);
        set_parent(node, Some(&merged_node));
        remove(node);
    }
}

/// Merge identical nodes at forks by comparing hashes.
pub fn merge_identical_nodes() -> TopDownOptimizer<impl FnMut(&NodeRef) -> bool> {
    TopDownOptimizer::new(|node: &NodeRef| {
        let kids = children(node);
        let hashes: Vec<String> = kids.iter().map(node_hash_key).collect();

        let mut buckets: Vec<(String, Vec<NodeRef>)> = Vec::new();
        for (i, h) in hashes.iter().enumerate() {
            if let Some(bucket) = buckets.iter_mut().find(|(k, _)| k == h) {
                bucket.1.push(kids[i].clone());
            } else {
                buckets.push((h.clone(), vec![kids[i].clone()]));
            }
        }

        let mut modified_any = false;
        for (_, nodes) in buckets {
            if nodes.len() > 1 {
                modified_any = true;
                let merged_node = nodes[0].clone();
                for n in nodes.iter().skip(1) {
                    remove_child(node, n);
                    set_parent(n, Some(&merged_node));
                    remove(n);
                }
            }
        }
        modified_any
    })
}

fn requires_selection_id(model: Option<&Model>) -> bool {
    model
        .map(super::super::selection::component::requires_selection_id)
        .unwrap_or(false)
}

/// Removes identifier nodes that are not needed for selections.
pub fn remove_unnecessary_identifier_nodes(
    model: Option<&Model>,
) -> TopDownOptimizer<impl FnMut(&NodeRef) -> bool> {
    let requires = requires_selection_id(model);
    TopDownOptimizer::new(move |node: &NodeRef| {
        if is_node::<IdentifierNode>(node) {
            let parent_ok = parent(node)
                .map(|p| {
                    is_data_source_node(&p)
                        || is_node::<AggregateNode>(&p)
                        || is_node::<ParseNode>(&p)
                })
                .unwrap_or(false);
            if !(requires && parent_ok) {
                remove(node);
                return true;
            }
        }
        false
    })
}

/// Removes duplicate time unit nodes.
pub struct RemoveDuplicateTimeUnits {
    modified: Rc<std::cell::Cell<bool>>,
}

impl RemoveDuplicateTimeUnits {
    pub fn new() -> Self {
        RemoveDuplicateTimeUnits {
            modified: new_modified_flag(),
        }
    }

    fn run(&self, node: &NodeRef, time_unit_fields: &FieldSet) {
        let mut produced_fields_set = FieldSet::new();
        if is_node::<TimeUnitNode>(node) {
            produced_fields_set =
                super::dataflow::produced_fields(node).unwrap_or_default();
            if has_intersection(&produced_fields_set, time_unit_fields) {
                self.modified.set(true);
                TimeUnitNode::remove_formulas(node, time_unit_fields);
                if super::dataflow::produced_fields(node)
                    .map(|p| p.is_empty())
                    .unwrap_or(true)
                {
                    remove(node);
                }
            }
        }

        for child in children(node) {
            let mut next = time_unit_fields.clone();
            next.extend(produced_fields_set.iter().cloned());
            self.run(&child, &next);
        }
    }
}

impl Default for RemoveDuplicateTimeUnits {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructor helper matching the upstream class instantiation.
pub fn remove_duplicate_time_units() -> RemoveDuplicateTimeUnits {
    RemoveDuplicateTimeUnits::new()
}

impl Optimizer for RemoveDuplicateTimeUnits {
    fn modified_flag(&self) -> bool {
        self.modified.get()
    }

    fn set_modified(&self) {
        self.modified.set(true);
    }

    fn optimize(&mut self, root: &NodeRef) -> bool {
        self.run(root, &FieldSet::new());
        self.modified_flag()
    }
}

/// Remove output nodes that are not required.
pub fn remove_unnecessary_output_nodes() -> TopDownOptimizer<impl FnMut(&NodeRef) -> bool> {
    TopDownOptimizer::new(|node: &NodeRef| {
        if is_node::<OutputNode>(node) && !OutputNode::is_required(node) {
            remove(node);
            return true;
        }
        false
    })
}

/// Move parse nodes up to forks and merge them if possible.
pub fn move_parse_up() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        if is_data_source_node(node) {
            return false;
        }
        if num_children(node) > 1 {
            return false;
        }

        let mut modified = false;
        for child in children(node) {
            if is_node::<ParseNode>(&child) {
                if is_node::<ParseNode>(node) {
                    ParseNode::merge(node, &child);
                    modified = true;
                } else {
                    // Don't swap with nodes that produce something that the
                    // parse node depends on (e.g. lookup).
                    if field_intersection(
                        produced_fields(node).as_ref(),
                        Some(&super::dataflow::dependent_fields(&child)),
                    ) {
                        continue;
                    }
                    super::dataflow::swap_with_parent(&child);
                    modified = true;
                }
            }
        }
        modified
    })
}

/// Inserts an intermediate ParseNode containing all non-conflicting parse fields.
pub fn merge_parse() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        let original_children = children(node);
        let parse_children: Vec<NodeRef> = original_children
            .iter()
            .filter(|c| is_node::<ParseNode>(c))
            .cloned()
            .collect();

        if num_children(node) > 1 && !parse_children.is_empty() {
            let mut common_parse: Map<String, Value> = Map::new();
            let mut conflicting: Vec<String> = Vec::new();

            for parse_node in &parse_children {
                let parse = super::dataflow::with_node::<ParseNode, _>(parse_node, |n| {
                    n.parse.clone()
                })
                .unwrap_or_default();
                for (k, v) in parse {
                    if !common_parse.contains_key(&k) {
                        common_parse.insert(k, v);
                    } else if common_parse.get(&k) != Some(&v)
                        && !conflicting.contains(&k) {
                            conflicting.push(k);
                        }
                }
            }

            for field in conflicting {
                common_parse.remove(&field);
            }

            if !common_parse.is_empty() {
                let mut parsed = super::formatparse::Parse::new();
                for (k, v) in common_parse {
                    parsed.insert(k, v);
                }
                let merged_parse_node = ParseNode::new(Some(node), parsed.clone());

                for child_node in original_children {
                    if is_node::<ParseNode>(&child_node) {
                        super::dataflow::with_node_mut::<ParseNode, _>(&child_node, |n| {
                            for key in parsed.keys() {
                                n.parse.shift_remove(key);
                            }
                        });
                    }

                    remove_child(node, &child_node);
                    set_parent(&child_node, Some(&merged_parse_node));

                    if is_node::<ParseNode>(&child_node)
                        && super::dataflow::with_node::<ParseNode, _>(&child_node, |n| {
                            n.parse.is_empty()
                        })
                        .unwrap_or(false)
                    {
                        remove(&child_node);
                    }
                }
                return true;
            }
        }
        false
    })
}

/// Repeatedly remove leaf nodes that are not output or facet nodes.
pub fn remove_unused_subtrees() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        if is_node::<OutputNode>(node)
            || num_children(node) > 0
            || is_node::<FacetNode>(node)
            || is_node::<SourceNode>(node)
        {
            return false;
        }
        remove(node);
        true
    })
}

/// Merge adjacent time unit nodes.
pub fn merge_time_units() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        let mut time_unit_children: Vec<NodeRef> = children(node)
            .into_iter()
            .filter(is_node::<TimeUnitNode>)
            .collect();
        let Some(combination) = time_unit_children.pop() else {
            return false;
        };
        if time_unit_children.is_empty() {
            return false;
        }
        for time_unit in time_unit_children {
            TimeUnitNode::merge(&combination, &time_unit);
        }
        true
    })
}

/// Merge adjacent aggregate nodes with the same group by.
pub fn merge_aggregates() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        let agg_children: Vec<NodeRef> = children(node)
            .into_iter()
            .filter(is_node::<AggregateNode>)
            .collect();

        let mut groups: Vec<(String, Vec<NodeRef>)> = Vec::new();
        for agg in agg_children {
            let group_by = super::dataflow::with_node::<AggregateNode, _>(&agg, |n| {
                n.dimensions.clone()
            })
            .unwrap_or_default();
            let key = crate::compile::data::support::hash_str(&crate::util::hash(
                &serde_json::json!(group_by),
            ));
            if let Some(group) = groups.iter_mut().find(|(k, _)| *k == key) {
                group.1.push(agg);
            } else {
                groups.push((key, vec![agg]));
            }
        }

        let mut modified = false;
        for (_, mut mergeable) in groups {
            if mergeable.len() > 1 {
                let merged = mergeable.pop().unwrap();
                for agg in mergeable {
                    if AggregateNode::merge(&merged, &agg) {
                        remove_child(node, &agg);
                        set_parent(&agg, Some(&merged));
                        remove(&agg);
                        modified = true;
                    }
                }
            }
        }
        modified
    })
}

/// Merge bin nodes and move them up through forks.
pub fn merge_bins(model: Option<&Model>) -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool + '_> {
    BottomUpOptimizer::new(move |node: &NodeRef| {
        let move_bins_up = !(is_data_source_node(node)
            || is_node::<FilterNode>(node)
            || is_node::<ParseNode>(node)
            || is_node::<IdentifierNode>(node));

        let mut promotable: Vec<NodeRef> = Vec::new();
        let mut remaining: Vec<NodeRef> = Vec::new();

        for child in children(node) {
            if is_node::<BinNode>(&child) {
                if move_bins_up
                    && !field_intersection(
                        produced_fields(node).as_ref(),
                        Some(&super::dataflow::dependent_fields(&child)),
                    )
                {
                    promotable.push(child);
                } else {
                    remaining.push(child);
                }
            }
        }

        let mut modified = false;
        if !promotable.is_empty() {
            let promoted = promotable.pop().unwrap();
            let model_ref = model;
            let mut rename = move |old: &str, new: &str| {
                if let Some(m) = &model_ref {
                    m.rename_signal(old, new);
                }
            };
            for bin in promotable {
                BinNode::merge(&promoted, &bin, &mut rename);
            }
            modified = true;
            if is_node::<BinNode>(node) {
                BinNode::merge(node, &promoted, &mut rename);
            } else {
                super::dataflow::swap_with_parent(&promoted);
            }
        }
        if remaining.len() > 1 {
            let remaining_bin = remaining.pop().unwrap();
            let model_ref = model;
            let mut rename = move |old: &str, new: &str| {
                if let Some(m) = &model_ref {
                    m.rename_signal(old, new);
                }
            };
            for bin in remaining {
                BinNode::merge(&remaining_bin, &bin, &mut rename);
            }
            modified = true;
        }
        modified
    })
}

/// Moves output nodes at a fork before the fork.
pub fn merge_outputs() -> BottomUpOptimizer<impl FnMut(&NodeRef) -> bool> {
    BottomUpOptimizer::new(|node: &NodeRef| {
        let kids = children(node);
        let has_output_child = kids.iter().any(is_node::<OutputNode>);
        if !has_output_child || num_children(node) <= 1 {
            return false;
        }

        let mut other_children: Vec<NodeRef> = Vec::new();
        let mut main_output: Option<NodeRef> = None;
        let mut modified = false;

        for child in kids {
            if is_node::<OutputNode>(&child) {
                let mut last_output = child.clone();
                while num_children(&last_output) == 1 {
                    let the_child = children(&last_output)[0].clone();
                    if is_node::<OutputNode>(&the_child) {
                        last_output = the_child;
                    } else {
                        break;
                    }
                }

                other_children.extend(children(&last_output));

                if let Some(main) = &main_output {
                    remove_child(node, &child);
                    if let Some(main_parent) = parent(main) {
                        set_parent(&child, Some(&main_parent));
                    }
                    if let Some(main_parent) = parent(main) {
                        remove_child(&main_parent, main);
                    }
                    set_parent(main, Some(&last_output));
                    modified = true;
                } else {
                    main_output = Some(last_output);
                }
            } else {
                other_children.push(child);
            }
        }

        if !other_children.is_empty() {
            modified = true;
            if let Some(main_output) = &main_output {
                for child in other_children {
                    if let Some(child_parent) = parent(&child) {
                        remove_child(&child_parent, &child);
                    }
                    set_parent(&child, Some(main_output));
                }
            }
        }
        modified
    })
}

/// Kept to mirror the upstream export; useful for tests.
pub fn add_child_helper(parent_node: &NodeRef, child: &NodeRef) {
    add_child(parent_node, child);
}

/// A `RefCell`-based signal rename closure factory.
pub fn rename_signal_with(model: Rc<RefCell<Model>>) -> impl FnMut(&str, &str) {
    move |old: &str, new: &str| {
        model.borrow().rename_signal(old, new);
    }
}
