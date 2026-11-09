//! Port of `src/compile/data/optimize.ts`.

use super::dataflow::{children, num_children, NodeRef};
use super::index::DataComponent;
use super::optimizer::Optimizer;
use super::optimizers;
use super::subtree::move_facet_down;
use crate::compile::model::Model;

pub const FACET_SCALE_PREFIX: &str = "scale_";
pub const MAX_OPTIMIZATION_RUNS: usize = 5;

/// Iterates over a dataflow graph and checks whether all links are consistent.
pub fn check_links(nodes: &[NodeRef]) -> bool {
    for node in nodes {
        for child in children(node) {
            if !super::dataflow::parent(&child)
                .map(|p| std::rc::Rc::ptr_eq(&p, node))
                .unwrap_or(false)
            {
                return false;
            }
        }
        if !check_links(&children(node)) {
            return false;
        }
    }
    true
}

fn run_optimizer(optimizer: &mut dyn Optimizer, nodes: &[NodeRef]) -> bool {
    let mut modified = false;
    for node in nodes {
        modified = optimizer.optimize(node) || modified;
    }
    modified
}

fn optimization_dataflow_helper(
    data_component: &mut DataComponent,
    model: Option<&Model>,
    first_pass: bool,
) -> bool {
    let mut roots = data_component.sources.clone();
    let mut modified = false;

    modified = run_optimizer(&mut optimizers::remove_unnecessary_output_nodes(), &roots) || modified;
    modified = run_optimizer(
        &mut optimizers::remove_unnecessary_identifier_nodes(model),
        &roots,
    ) || modified;

    // remove source nodes that don't have any children because they also don't
    // have output nodes
    roots.retain(|r| num_children(r) > 0);

    modified = run_optimizer(&mut optimizers::remove_unused_subtrees(), &roots) || modified;

    roots.retain(|r| num_children(r) > 0);

    if !first_pass {
        modified = run_optimizer(&mut optimizers::move_parse_up(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_bins(model), &roots) || modified;
        modified = run_optimizer(&mut optimizers::remove_duplicate_time_units(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_parse(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_aggregates(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_time_units(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_identical_nodes(), &roots) || modified;
        modified = run_optimizer(&mut optimizers::merge_outputs(), &roots) || modified;
    }

    data_component.sources = roots;

    modified
}

/// Optimizes the dataflow of the passed in data component.
pub fn optimize_dataflow(data: &mut DataComponent, model: Option<&Model>) {
    // check before optimizations
    check_links(&data.sources);

    let mut first_pass_counter = 0;
    let mut second_pass_counter = 0;

    for _ in 0..MAX_OPTIMIZATION_RUNS {
        if !optimization_dataflow_helper(data, model, true) {
            break;
        }
        first_pass_counter += 1;
    }

    // move facets down and make a copy of the subtree so that we can have
    // scales at the top level
    for node in data.sources.clone() {
        move_facet_down(&node);
    }

    for _ in 0..MAX_OPTIMIZATION_RUNS {
        if !optimization_dataflow_helper(data, model, false) {
            break;
        }
        second_pass_counter += 1;
    }

    // check after optimizations
    check_links(&data.sources);

    if first_pass_counter.max(second_pass_counter) == MAX_OPTIMIZATION_RUNS {
        crate::log::warn(format!(
            "Maximum optimization runs({MAX_OPTIMIZATION_RUNS}) reached."
        ));
    }
}
