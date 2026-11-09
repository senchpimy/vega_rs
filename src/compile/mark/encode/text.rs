//! Port of `src/compile/mark/encode/text.ts`.

use serde_json::Value;

use crate::channeldef::{get_format_mixins, is_field_or_datum_def, is_value_def};
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;

use super::conditional::wrap_condition;
use super::format::format_signal_ref;

pub fn text(model: &Model, channel: &str) -> Value {
    let channel_def = model.unit.encoding.get(channel).cloned();
    wrap_condition(
        model,
        channel_def.as_ref(),
        channel,
        None,
        |c_def| text_ref(c_def, &model.config, "datum"),
    )
}

pub fn text_ref(channel_def: &Value, config: &Value, expr: &str) -> Option<Value> {
    if channel_def.is_null() {
        return None;
    }
    if is_value_def(channel_def) {
        return Some(signal_or_value_ref(&channel_def["value"]));
    }
    if is_field_or_datum_def(channel_def) {
        let mixins = get_format_mixins(channel_def);
        return format_signal_ref(
            channel_def,
            mixins.get("format"),
            mixins.get("formatType").and_then(Value::as_str),
            Some(expr),
            false,
            config,
        );
    }
    None
}
