//! Port of `src/logical.ts`.

use serde_json::Value;

use crate::util::has_property;

pub fn is_logical_or(op: &Value) -> bool {
    has_property(op, "or")
}

pub fn is_logical_and(op: &Value) -> bool {
    has_property(op, "and")
}

pub fn is_logical_not(op: &Value) -> bool {
    has_property(op, "not")
}

pub fn for_each_leaf(op: &Value, f: &mut impl FnMut(&Value)) {
    if is_logical_not(op) {
        for_each_leaf(&op["not"], f);
    } else if is_logical_and(op) {
        if let Some(arr) = op["and"].as_array() {
            for subop in arr {
                for_each_leaf(subop, f);
            }
        }
    } else if is_logical_or(op) {
        if let Some(arr) = op["or"].as_array() {
            for subop in arr {
                for_each_leaf(subop, f);
            }
        }
    } else {
        f(op);
    }
}

pub fn normalize_logical_composition(
    op: &Value,
    normalizer: &mut impl FnMut(&Value) -> Value,
) -> Value {
    use serde_json::json;
    if is_logical_not(op) {
        json!({ "not": normalize_logical_composition(&op["not"], normalizer) })
    } else if is_logical_and(op) {
        let arr: Vec<Value> = op["and"]
            .as_array()
            .map(|a| a.iter().map(|o| normalize_logical_composition(o, normalizer)).collect())
            .unwrap_or_default();
        json!({ "and": arr })
    } else if is_logical_or(op) {
        let arr: Vec<Value> = op["or"]
            .as_array()
            .map(|a| a.iter().map(|o| normalize_logical_composition(o, normalizer)).collect())
            .unwrap_or_default();
        json!({ "or": arr })
    } else {
        normalizer(op)
    }
}
