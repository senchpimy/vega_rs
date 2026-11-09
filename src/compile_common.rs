//! Port of the model-independent helpers in `src/compile/common.ts`.

use serde_json::{Map, Value};

use crate::expr::is_expr_ref;
use crate::util::{array, deep_equal, string_value};

pub const BIN_RANGE_DELIMITER: &str = " \u{2013} ";

pub fn is_signal_ref(v: &Value) -> bool {
    crate::util::has_property(v, "signal")
}

/// `signalRefOrValue`: `{expr: e, ...rest}` -> `{signal: e, ...rest}`.
pub fn signal_ref_or_value(value: &Value) -> Value {
    if is_expr_ref(value) {
        let mut out = value.as_object().cloned().unwrap_or_default();
        let expr = out.remove("expr").unwrap_or(Value::Null);
        out.insert("signal".to_string(), expr);
        Value::Object(out)
    } else {
        value.clone()
    }
}

/// `conditionalSignalRefOrValue`
pub fn conditional_signal_ref_or_value(value: &Value) -> Value {
    signal_ref_or_value(value)
}

/// `signalOrValueRef`: returns `{value: v}`, a signal ref, or the input if it is
/// already a signal ref.
pub fn signal_or_value_ref(value: &Value) -> Value {
    if is_expr_ref(value) {
        let mut out = value.as_object().cloned().unwrap_or_default();
        let expr = out.remove("expr").unwrap_or(Value::Null);
        out.insert("signal".to_string(), expr);
        return Value::Object(out);
    }
    if is_signal_ref(value) {
        return value.clone();
    }
    serde_json::json!({ "value": value })
}

/// `signalOrValueRefWithCondition`
pub fn signal_or_value_ref_with_condition(value: &Value) -> Value {
    let mut out = value.as_object().cloned().unwrap_or_default();
    if let Some(condition) = out.get("condition").cloned() {
        let new_condition = match &condition {
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(conditional_signal_ref_or_value)
                    .collect::<Vec<_>>(),
            ),
            other => conditional_signal_ref_or_value(other),
        };
        out.insert("condition".to_string(), new_condition);
    }
    Value::Object(out)
}

pub fn expr_from_signal_ref_or_value(value: &Value) -> String {
    if let Some(signal) = value.get("signal") {
        return signal.as_str().unwrap_or_default().to_string();
    }
    string_value(value)
}

pub fn expr_from_value_ref_or_signal_ref(value: &Value) -> String {
    if let Some(signal) = value.get("signal") {
        return signal.as_str().unwrap_or_default().to_string();
    }
    string_value(value.get("value").unwrap_or(&Value::Null))
}

pub fn signal_or_string_value(v: &Value) -> Value {
    if let Some(signal) = v.get("signal") {
        return signal.clone();
    }
    if v.is_null() {
        Value::Null
    } else {
        Value::String(string_value(v))
    }
}

pub fn merge_title_field_defs(f1: &[Value], f2: &[Value]) -> Vec<Value> {
    let mut merged: Vec<Value> = f1.to_vec();
    for fd_to_merge in f2 {
        if merged.iter().any(|fd| deep_equal(fd, fd_to_merge)) {
            continue;
        }
        merged.push(fd_to_merge.clone());
    }
    merged
}

pub fn merge_title(title1: &Value, title2: &Value) -> Value {
    if deep_equal(title1, title2) || is_falsy(title2) {
        title1.clone()
    } else if is_falsy(title1) {
        title2.clone()
    } else {
        let mut items = array(title1);
        items.extend(array(title2));
        let parts: Vec<String> = items
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => string_value(other),
            })
            .collect();
        Value::String(parts.join(", "))
    }
}

fn is_falsy(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::String(s) => s.is_empty(),
        Value::Number(n) => n.as_f64() == Some(0.0),
        _ => false,
    }
}

#[allow(dead_code)]
fn _map_empty() -> Map<String, Value> {
    Map::new()
}
