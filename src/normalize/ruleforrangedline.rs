//! Port of `src/normalize/ruleforrangedline.ts`.

use serde_json::Value;

use crate::normalize::base::NormalizerParams;
use crate::normalize::core;
use crate::normalize_support::*;

pub fn has_matching_type(spec: &Value) -> bool {
    if is_unit_spec(spec) {
        let encoding = get(spec, "encoding").cloned().unwrap_or(empty_object());
        let mark = get(spec, "mark").cloned().unwrap_or(Value::Null);
        if mark.as_str() == Some("line")
            || (crate::mark::is_mark_def(&mark)
                && get(&mark, "type").and_then(|t| t.as_str()) == Some("line"))
        {
            for channel in SECONDARY_RANGE_CHANNELS {
                let main_channel = get_main_range_channel(channel);
                let main_channel_def = get(&encoding, main_channel).cloned().unwrap_or(Value::Null);
                if truthy(get(&encoding, channel).unwrap_or(&Value::Null))
                    && ((is_field_def(&main_channel_def)
                        && !is_binned(get(&main_channel_def, "bin").unwrap_or(&Value::Null)))
                        || is_datum_def(&main_channel_def))
                    {
                        return true;
                    }
            }
        }
    }
    false
}

pub fn run(
    spec: &Value,
    params: &NormalizerParams,
) -> Result<Value, String> {
    let encoding = get(spec, "encoding").cloned().unwrap_or(empty_object());
    let mark = get(spec, "mark").cloned().unwrap_or(Value::Null);
    crate::log::warn(format!(
        "Line mark is for continuous lines and thus cannot be used with {}. We will use the rule mark (line segments) instead.",
        if truthy(get(&encoding, "x2").unwrap_or(&Value::Null))
            && truthy(get(&encoding, "y2").unwrap_or(&Value::Null))
        {
            "x2 and y2"
        } else if truthy(get(&encoding, "x2").unwrap_or(&Value::Null)) {
            "x2"
        } else {
            "y2"
        }
    ));

    let mut new_spec = spec.clone();
    let new_mark = if is_object(&mark) {
        let mut m = mark.as_object().cloned().unwrap_or_default();
        m.insert("type".to_string(), Value::String("rule".to_string()));
        Value::Object(m)
    } else {
        Value::String("rule".to_string())
    };
    if let Some(m) = new_spec.as_object_mut() {
        m.insert("mark".to_string(), new_mark);
    }
    core::map_layer_or_unit(&new_spec, params)
}
