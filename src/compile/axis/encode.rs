//! Port of `src/compile/guide.ts` (guide encode entries) and
//! `src/compile/axis/encode.ts` (axis label encoding).

use serde_json::{Map, Value};

use crate::channel::get_secondary_range_channel;
use crate::channeldef::{
    channel_def_type, get_field_or_datum_def, is_conditional_def, is_conditional_parameter,
    is_field_def, is_position_field_or_datum_def,
};
use crate::channeldef_support::truthy;
use crate::compile::model::Model;
use crate::compile_common::{is_signal_ref, signal_or_value_ref};
use crate::compile_predicate::expression;
use crate::util::{array, has_property, keys};

use super::format::{format_custom_type, is_custom_format_type};
use super::properties::default_band_position;

/// `mainRefFn(def)` used by `guideEncodeEntry`: `signalOrValueRef(def.value)`.
fn main_ref(def: &Value) -> Value {
    if let Some(value) = def.get("value") {
        signal_or_value_ref(value)
    } else if is_signal_ref(def) {
        def.clone()
    } else {
        Value::Object(Map::new())
    }
}

fn wrap_condition(def: &Value, vg_channel: &str) -> Option<(String, Value)> {
    let condition = if is_conditional_def(def) {
        def.get("condition")
    } else {
        None
    };

    let mut value_refs: Vec<Value> = Vec::new();

    if let Some(condition) = condition {
        for c in array(condition) {
            let condition_value_ref = main_ref(&c);
            let test = if is_conditional_parameter(&c) {
                // Selection predicates require the selection compiler; fall back
                // to the parameter name as an opaque test expression.
                c.get("param").map(crate::util::string_value).unwrap_or_default()
            } else {
                expression(c.get("test").unwrap_or(&Value::Null))
            };

            let mut entry = Map::new();
            entry.insert("test".to_string(), Value::String(test));
            if let Some(map) = condition_value_ref.as_object() {
                for (k, v) in map {
                    entry.insert(k.clone(), v.clone());
                }
            }
            value_refs.push(Value::Object(entry));
        }
    }

    let main_value_ref = main_ref(def);
    if main_value_ref.as_object().map(|m| !m.is_empty()).unwrap_or(false) {
        value_refs.push(main_value_ref);
    }

    if value_refs.is_empty() {
        return None;
    }

    let must_array = value_refs.len() > 1
        || (value_refs.len() == 1
            && value_refs[0]
                .get("test")
                .map(truthy)
                .unwrap_or(false));

    if must_array {
        Some((vg_channel.to_string(), Value::Array(value_refs)))
    } else {
        Some((vg_channel.to_string(), value_refs.remove(0)))
    }
}

/// `guideEncodeEntry`
pub fn guide_encode_entry(encoding: &Value, _model: &Model) -> Map<String, Value> {
    let mut encode = Map::new();
    for channel in keys(encoding) {
        let channel_def = &encoding[&channel];
        if let Some((_, value)) = wrap_condition(channel_def, &channel) {
            encode.insert(channel, value);
        }
    }
    encode
}

/// `labels(model, channel, specifiedLabelsSpec)`
pub fn labels(model: &Model, channel: &str, specified_labels_spec: &Map<String, Value>) -> Map<String, Value> {
    let text_spec = labels_text_spec(model, channel, specified_labels_spec);
    labels_position_spec(model, channel, text_spec)
}

fn labels_text_spec(
    model: &Model,
    channel: &str,
    specified_labels_spec: &Map<String, Value>,
) -> Map<String, Value> {
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let config = &model.config;

    let field_or_datum_def = get_field_or_datum_def(&encoding[channel]).or_else(|| {
        get_secondary_range_channel(channel)
            .and_then(|secondary| get_field_or_datum_def(&encoding[secondary]))
    });
    let axis = model.axis(channel).unwrap_or_else(|| Value::Object(Map::new()));
    let format = axis.get("format").cloned();
    let format_type = axis.get("formatType").cloned();

    let with_text = |text: Value| {
        let mut out = Map::new();
        out.insert("text".to_string(), text);
        for (k, v) in specified_labels_spec {
            out.insert(k.clone(), v.clone());
        }
        out
    };

    if is_custom_format_type(format_type.as_ref()) {
        return with_text(format_custom_type(
            field_or_datum_def.as_ref(),
            format.as_ref(),
            format_type.as_ref().and_then(Value::as_str).unwrap_or_default(),
            None,
            false,
            config,
            Some("datum.value"),
        ));
    } else if format.is_none()
        && format_type.is_none()
        && truthy(config.get("customFormatTypes").unwrap_or(&Value::Null))
    {
        let def = field_or_datum_def.as_ref().unwrap_or(&Value::Null);
        if channel_def_type(def).as_deref() == Some("quantitative") {
            if is_position_field_or_datum_def(def)
                && def.get("stack").and_then(Value::as_str) == Some("normalize")
                && config.get("normalizedNumberFormatType").is_some()
            {
                return with_text(format_custom_type(
                    field_or_datum_def.as_ref(),
                    config.get("normalizedNumberFormat"),
                    config
                        .get("normalizedNumberFormatType")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    None,
                    false,
                    config,
                    Some("datum.value"),
                ));
            } else if config.get("numberFormatType").is_some() {
                return with_text(format_custom_type(
                    field_or_datum_def.as_ref(),
                    config.get("numberFormat"),
                    config
                        .get("numberFormatType")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    None,
                    false,
                    config,
                    Some("datum.value"),
                ));
            }
        }
        if channel_def_type(def).as_deref() == Some("temporal")
            && config.get("timeFormatType").is_some()
            && is_field_def(def)
            && !truthy(&def["timeUnit"])
        {
            return with_text(format_custom_type(
                field_or_datum_def.as_ref(),
                config.get("timeFormat"),
                config
                    .get("timeFormatType")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                None,
                false,
                config,
                Some("datum.value"),
            ));
        }
    }

    specified_labels_spec.clone()
}

fn labels_position_spec(
    model: &Model,
    channel: &str,
    specified_labels_spec: Map<String, Value>,
) -> Map<String, Value> {
    if has_property(&Value::Object(specified_labels_spec.clone()), "x")
        || has_property(&Value::Object(specified_labels_spec.clone()), "y")
    {
        return specified_labels_spec;
    }

    let inferred_band_position = default_band_position(model, channel);

    let Some(inferred) = inferred_band_position else {
        return specified_labels_spec;
    };

    let axis = model.axis(channel).unwrap_or_else(|| Value::Object(Map::new()));
    let band_position = axis
        .get("bandPosition")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or(inferred);

    if band_position.as_f64() == Some(0.5) {
        return specified_labels_spec;
    }

    let position_ref = serde_json::json!({
        "scale": model.scale_name(channel, false),
        "signal": "datum.value",
        "band": band_position,
    });

    let mut out = Map::new();
    if channel == "x" {
        out.insert("x".to_string(), position_ref);
    } else {
        out.insert("y".to_string(), position_ref);
    }
    for (k, v) in specified_labels_spec {
        out.insert(k, v);
    }
    out
}
