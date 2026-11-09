//! Port of `src/compile/predicate.ts`.
//!
//! The upstream function receives a `Model` (used only for selection filters).
//! Selection predicates are resolved through the selection compiler; the
//! model-less [`expression`] entry point reports unknown parameters as
//! `!!name`, matching upstream's fallback for non-selection parameters.

use serde_json::Value;

use crate::compile::model::Model;
use crate::compile::selection::parse::parse_selection_predicate;
use crate::predicate::{field_filter_expression, is_selection_predicate};
use crate::util::logical_expr;

/// Port of `expression(model, filterOp, node)` for a known model.
pub fn expression_with_model(model: &Model, filter_op: &Value) -> String {
    expression_with_selection(filter_op, &mut |predicate: &Value| {
        parse_selection_predicate(model, predicate, "datum")
    })
}

/// Port of `expression(model, filterOp, node)` without a model.
///
/// Upstream only passes `null` here from guides, which cannot contain
/// selection predicates. If one is encountered we fall back to treating it as
/// a variable parameter (`!!name`), which is also what `parseSelectionPredicate`
/// does when a selection cannot be found.
pub fn expression(filter_op: &Value) -> String {
    expression_with_selection(filter_op, &mut |predicate: &Value| {
        let name = predicate
            .get("param")
            .and_then(Value::as_str)
            .unwrap_or_default();
        format!("!!{}", crate::util::var_name(name))
    })
}

/// Port of `expression` with an explicit handler for selection predicates
/// (upstream: `parseSelectionPredicate(model, predicate, node)`).
pub fn expression_with_selection(
    filter_op: &Value,
    selection: &mut impl FnMut(&Value) -> String,
) -> String {
    logical_expr(filter_op, &mut |predicate: &Value| {
        if let Some(s) = predicate.as_str() {
            s.to_string()
        } else if is_selection_predicate(predicate) {
            selection(predicate)
        } else {
            // Filter Object
            field_filter_expression(predicate)
        }
    })
}
