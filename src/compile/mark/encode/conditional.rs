//! Port of `src/compile/mark/encode/conditional.ts`.

use serde_json::{json, Map, Value};

use crate::channeldef::{has_conditional_field_or_datum_def, is_conditional_def};
use crate::compile::model::Model;
use crate::compile::selection::parse::parse_selection_predicate;
use crate::compile_predicate;
use crate::util::{array, has_property};

/// Port of `wrapCondition`.
pub fn wrap_condition<F>(
    model: &Model,
    channel_def: Option<&Value>,
    vg_channel: &str,
    invalid_value_ref: Option<Value>,
    main_ref_fn: F,
) -> Value
where
    F: Fn(&Value) -> Option<Value>,
{
    let condition = channel_def
        .filter(|def| is_conditional_def(def))
        .and_then(|def| def.get("condition"))
        .cloned();

    let mut value_refs: Vec<Value> = Vec::new();

    if let Some(condition) = condition {
        for c in array(&condition) {
            let condition_value_ref = main_ref_fn(&c).unwrap_or(Value::Null);
            if condition_value_ref.is_null() {
                continue;
            }
            let test = if has_property(&c, "param") {
                let mut pred = Map::new();
                pred.insert("param".to_string(), c.get("param").cloned().unwrap_or(Value::Null));
                if let Some(empty) = c.get("empty") {
                    pred.insert("empty".to_string(), empty.clone());
                }
                parse_selection_predicate(model, &Value::Object(pred), "datum")
            } else if let Some(test) = c.get("test") {
                compile_predicate::expression_with_model(model, test)
            } else {
                String::new()
            };
            let mut value_ref = condition_value_ref;
            if let Some(map) = value_ref.as_object_mut() {
                if !test.is_empty() {
                    map.insert("test".to_string(), Value::String(test));
                }
            }
            value_refs.push(value_ref);
        }
    }

    if let Some(invalid) = invalid_value_ref {
        value_refs.push(invalid);
    }

    let main = match channel_def {
        Some(channel_def) => main_ref_fn(channel_def),
        None => main_ref_fn(&Value::Null),
    };
    match main {
        Some(main) => value_refs.push(main),
        // A conditional value definition with an explicit `value: null`
        // still needs its main ref emitted (upstream `signalOrValueRef(null)`
        // returns `{value: null}`).
        None if channel_def
            .and_then(|def| def.get("value"))
            .map(Value::is_null)
            .unwrap_or(false) =>
        {
            value_refs.push(json!({"value": Value::Null}));
        }
        None => {}
    }

    let mut result = Map::new();
    if value_refs.len() > 1
        || (value_refs.len() == 1
            && value_refs[0]
                .get("test")
                .map(|t| t.as_str().map(|s| !s.is_empty()).unwrap_or(true))
                .unwrap_or(false))
    {
        result.insert(vg_channel.to_string(), Value::Array(value_refs));
    } else if value_refs.len() == 1 {
        result.insert(vg_channel.to_string(), value_refs.remove(0));
    }
    Value::Object(result)
}

#[allow(dead_code)]
fn _conditional_guard(v: &Value) -> bool {
    has_conditional_field_or_datum_def(v)
}
