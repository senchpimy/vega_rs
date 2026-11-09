//! Port of `src/expr.ts`.

use serde_json::{Map, Value};

use crate::util::{keys, stringify};

pub fn is_expr_ref(o: &Value) -> bool {
    crate::util::has_property(o, "expr")
}

fn signal_ref_or_value(v: &Value) -> Value {
    if is_expr_ref(v) {
        serde_json::json!({ "signal": v["expr"].clone() })
    } else {
        v.clone()
    }
}

/// Recursively replaces `{expr: ...}` with `{signal: ...}` down to `level`.
pub fn replace_expr_ref(index: &Map<String, Value>, level: i64) -> Map<String, Value> {
    let mut new_index = Map::new();
    for prop in keys(&Value::Object(index.clone())) {
        let value = &index[&prop];
        new_index.insert(
            prop,
            if level <= 0 {
                signal_ref_or_value(value)
            } else if let Value::Object(inner) = value {
                Value::Object(replace_expr_ref(inner, level - 1))
            } else {
                value.clone()
            },
        );
    }
    new_index
}

#[allow(dead_code)]
fn _debug(v: &Value) -> String {
    stringify(v)
}
