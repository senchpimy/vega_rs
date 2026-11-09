//! Port of `src/compile/mark/encode/aria.ts`.

use serde_json::{json, Map, Value};

use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::get_mark_prop_or_config;

use super::conditional::wrap_condition;
use super::text::text_ref;
use super::tooltip::{tooltip_data_tuples, TooltipTuple};

const VEGAS_MARKS: [&str; 12] = [
    "arc", "area", "group", "image", "line", "path", "rect", "rule", "shape", "symbol", "text",
    "trail",
];

pub fn aria(model: &Model) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;

    let enable_aria = get_mark_prop_or_config("aria", mark_def, config, None, false);

    if enable_aria.as_ref().and_then(Value::as_bool) == Some(false) {
        return Value::Object(Map::new());
    }

    let mut out = Map::new();
    if let Some(enable_aria) = &enable_aria {
        if enable_aria.as_bool() == Some(true) {
            out.insert("aria".to_string(), enable_aria.clone());
        }
    }
    merge(&mut out, aria_role_description(model));
    merge(&mut out, description(model).unwrap_or(Value::Null));
    Value::Object(out)
}

fn merge(dest: &mut Map<String, Value>, src: Value) {
    if let Value::Object(src) = src {
        for (k, v) in src {
            dest.insert(k, v);
        }
    }
}

fn aria_role_description(model: &Model) -> Value {
    let mark = model.mark_type();
    let mark_def = &model.unit.mark_def;
    let config = &model.config;

    if config.get("aria").and_then(Value::as_bool) == Some(false) {
        return Value::Object(Map::new());
    }

    let aria_role_desc = get_mark_prop_or_config("ariaRoleDescription", mark_def, config, None, false);
    if let Some(desc) = aria_role_desc {
        if !desc.is_null() {
            return json!({"ariaRoleDescription": {"value": desc}});
        }
    }

    if VEGAS_MARKS.contains(&mark.as_str()) {
        Value::Object(Map::new())
    } else {
        json!({"ariaRoleDescription": {"value": mark}})
    }
}

pub fn description(model: &Model) -> Option<Value> {
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let stack = super::super::mark::stack_of(model);
    let channel_def = encoding.get("description").cloned();

    if let Some(channel_def) = channel_def.filter(|v| !v.is_null()) {
        return Some(wrap_condition(
            model,
            Some(&channel_def),
            "description",
            None,
            |c_def| text_ref(c_def, &model.config, "datum"),
        ));
    }

    let description_value = get_mark_prop_or_config("description", mark_def, config, None, false);
    if let Some(description_value) = description_value.filter(|v| !v.is_null()) {
        return Some(json!({"description": signal_or_value_ref(&description_value)}));
    }

    if config.get("aria").and_then(Value::as_bool) == Some(false) {
        return Some(Value::Object(Map::new()));
    }

    let data: Vec<TooltipTuple> = tooltip_data_tuples(encoding, stack.as_ref(), config, false)
        .into_iter()
        .filter(|t| !t.key.starts_with('_'))
        .collect();

    if data.is_empty() {
        return None;
    }

    Some(json!({"description": {"signal": aria_description(&data)}}))
}

fn aria_description(data: &[TooltipTuple]) -> String {
    let entries: Vec<(&String, String, Option<&String>)> = data
        .iter()
        .map(|t| (&t.key, t.value.replace("\\n", " "), t.test.as_ref()))
        .collect();

    if entries.iter().all(|(_, _, test)| test.is_none()) {
        return entries
            .iter()
            .enumerate()
            .map(|(index, (key, value, _))| {
                format!(
                    "\"{}{key}: \" + ({value})",
                    if index > 0 { "; " } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(" + ");
    }

    let segments: Vec<String> = entries
        .iter()
        .map(|(key, value, test)| {
            let segment = format!("\"; {key}: \" + ({value})");
            match test {
                Some(test) => format!("(({test}) ? {segment} : \"\")"),
                None => segment,
            }
        })
        .collect();
    format!("slice({}, 2)", segments.join(" + "))
}
