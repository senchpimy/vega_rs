//! Port of `src/compile/mark/encode/defined.ts`.

use indexmap::IndexSet;

use serde_json::{json, Map, Value};

use crate::aggregate::is_counting_op_name;
use crate::channel::{is_scale_channel, SCALE_CHANNELS};
use crate::channeldef::get_field_def;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::predicate::field_valid_predicate;

use super::invalid::{get_scale_invalid_data_mode, should_break_path};

pub fn defined(model: &Model) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let encoding = &model.unit.encoding;

    let mut fields_to_break_path: IndexSet<String> = IndexSet::new();

    for channel in SCALE_CHANNELS {
        if !is_scale_channel(channel) {
            continue;
        }
        let Some(field_def) = get_field_def(&encoding[channel]) else {
            continue;
        };
        let Some(scale_type) = model.get_scale_type(channel) else {
            continue;
        };
        let is_count = field_def
            .get("aggregate")
            .and_then(Value::as_str)
            .map(is_counting_op_name)
            .unwrap_or(false);
        let invalid_mode = get_scale_invalid_data_mode(
            mark_def,
            config,
            channel,
            Some(scale_type.as_str()),
            is_count,
        );
        if should_break_path(&invalid_mode) {
            let impute = super::super::mark::stack_of(model)
                .and_then(|s| s.get("impute").map(|v| v.as_bool().unwrap_or(false)))
                .unwrap_or(false);
            let field = model.vg_field_with(
                channel,
                &crate::channeldef::FieldRefOption {
                    expr: Some("datum".to_string()),
                    bin_suffix: if impute {
                        Some("mid".to_string())
                    } else {
                        None
                    },
                    ..Default::default()
                },
            );
            if !field.is_empty() {
                fields_to_break_path.insert(field);
            }
        }
    }

    if !fields_to_break_path.is_empty() {
        let signal = fields_to_break_path
            .iter()
            .map(|field| field_valid_predicate(field, true))
            .collect::<Vec<_>>()
            .join(" && ");
        return json!({"defined": {"signal": signal}});
    }
    Value::Object(Map::new())
}

pub fn value_if_defined(prop: &str, value: Option<&Value>) -> Value {
    if let Some(value) = value {
        let mut map = Map::new();
        map.insert(prop.to_string(), signal_or_value_ref(value));
        Value::Object(map)
    } else {
        Value::Object(Map::new())
    }
}
