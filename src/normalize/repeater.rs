//! Port of `src/normalize/repeater.ts`.

use serde_json::{json, Map, Value};

use crate::normalize::base::RepeaterValue;
use crate::normalize_support::*;

fn replace_repeat_in_prop(prop: &str, o: &Value, repeater: &RepeaterValue) -> Option<Value> {
    let val = get(o, prop);
    if let Some(val) = val {
        if is_repeat_ref(val) {
            let key = get(val, "repeat")
                .and_then(|r| r.as_str())
                .unwrap_or_default();
            if let Some(replacement) = repeater.get(key) {
                let mut m = o.as_object().cloned().unwrap_or_default();
                m.insert(prop.to_string(), Value::String(replacement.clone()));
                return Some(Value::Object(m));
            } else {
                crate::log::warn(format!("Unknown repeated value \"{}\".", key));
                return None;
            }
        }
    }
    Some(o.clone())
}

fn replace_repeater_in_field_def(field_def: &Value, repeater: &RepeaterValue) -> Option<Value> {
    let mut field_def = replace_repeat_in_prop("field", field_def, repeater)?;

    if is_sortable_field_def(&field_def) {
        let sort = get(&field_def, "sort").cloned().unwrap_or(Value::Null);
        if is_sort_field(&sort) {
            let replaced = replace_repeat_in_prop("field", &sort, repeater);
            if let Some(m) = field_def.as_object_mut() {
                match replaced {
                    Some(sort) => {
                        m.insert("sort".to_string(), sort);
                    }
                    None => {
                        m.remove("sort");
                    }
                }
            }
        }
    }

    Some(field_def)
}

fn replace_repeater_in_field_or_datum_def(def: &Value, repeater: &RepeaterValue) -> Option<Value> {
    if is_field_def(def) {
        replace_repeater_in_field_def(def, repeater)
    } else {
        let datum_def = replace_repeat_in_prop("datum", def, repeater)?;
        if datum_def != *def && !has_prop(&datum_def, "type") {
            let mut m = datum_def.as_object().cloned().unwrap_or_default();
            m.insert("type".to_string(), Value::String("nominal".to_string()));
            Some(Value::Object(m))
        } else {
            Some(datum_def)
        }
    }
}

fn replace_repeater_in_channel_def(channel_def: &Value, repeater: &RepeaterValue) -> Option<Value> {
    if is_field_or_datum_def(channel_def) {
        if let Some(fd) = replace_repeater_in_field_or_datum_def(channel_def, repeater) {
            return Some(fd);
        } else if is_conditional_def(channel_def) {
            return Some(json!({"condition": get(channel_def, "condition").cloned().unwrap_or(Value::Null)}));
        }
    } else if has_conditional_field_or_datum_def(channel_def) {
        let condition = get(channel_def, "condition").unwrap();
        if let Some(fd) = replace_repeater_in_field_or_datum_def(condition, repeater) {
            let mut m = channel_def.as_object().cloned().unwrap_or_default();
            m.insert("condition".to_string(), fd);
            return Some(Value::Object(m));
        } else {
            return Some(omit(channel_def, &["condition"]));
        }
    } else {
        return Some(channel_def.clone());
    }
    None
}

fn replace_repeater_in_mapping(mapping: &Value, repeater: &RepeaterValue) -> Value {
    let mut out = Map::new();
    for (channel, channel_def) in entries(mapping) {
        if let Value::Array(arr) = &channel_def {
            let mapped: Vec<Value> = arr
                .iter()
                .filter_map(|cd| replace_repeater_in_channel_def(cd, repeater))
                .collect();
            out.insert(channel, Value::Array(mapped));
        } else if let Some(cd) = replace_repeater_in_channel_def(&channel_def, repeater) {
            out.insert(channel, cd);
        }
    }
    Value::Object(out)
}

pub fn replace_repeater_in_facet(facet: &Value, repeater: Option<&RepeaterValue>) -> Value {
    let Some(repeater) = repeater else {
        return facet.clone();
    };
    if has_prop(facet, "row") || has_prop(facet, "column") {
        replace_repeater_in_mapping(facet, repeater)
    } else {
        replace_repeater_in_field_def(facet, repeater).unwrap_or(Value::Null)
    }
}

pub fn replace_repeater_in_encoding(encoding: &Value, repeater: Option<&RepeaterValue>) -> Value {
    let Some(repeater) = repeater else {
        return encoding.clone();
    };
    replace_repeater_in_mapping(encoding, repeater)
}
