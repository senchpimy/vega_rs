//! Port of `src/compile/mark/encode/invalid.ts` and the subset of
//! `src/compile/invalid/**` needed by mark encoding.

use serde_json::{json, Value};

use crate::aggregate::is_counting_op_name;
use crate::channeldef::{get_field_def, vg_field_with, FieldRefOption};
use crate::compile::mark::encode::scaled_zero_or_min_or_max::scaled_zero_or_min_or_max;
use crate::compile::mark::encode::valueref::is_signal_ref;
use crate::compile::scale::component::ScaleComponent;
use crate::mark::{get_mark_prop_or_config, is_path_mark};
use crate::predicate::field_valid_predicate;

fn has_continuous_domain(scale_type: &str) -> bool {
    matches!(
        scale_type,
        "linear"
            | "log"
            | "pow"
            | "sqrt"
            | "symlog"
            | "time"
            | "utc"
            | "quantile"
            | "quantize"
            | "threshold"
            | "sequential"
    )
}

/// Port of `normalizeInvalidDataMode`.
pub fn normalize_invalid_data_mode(mode: Option<&Value>, is_path: bool) -> String {
    match mode {
        None => {
            if is_path {
                "break-paths-show-domains".to_string()
            } else {
                "filter".to_string()
            }
        }
        Some(Value::Null) => "show".to_string(),
        Some(v) if v.as_str() == Some("break-paths-show-path-domains") => {
            if is_path {
                "break-paths-show-domains".to_string()
            } else {
                "filter".to_string()
            }
        }
        Some(v) => v.as_str().unwrap_or("filter").to_string(),
    }
}

fn is_count_aggregate(field_def: Option<&Value>) -> bool {
    field_def
        .and_then(|f| f.get("aggregate"))
        .and_then(Value::as_str)
        .map(is_counting_op_name)
        .unwrap_or(false)
}

/// Port of `getScaleInvalidDataMode`.
pub fn get_scale_invalid_data_mode(
    mark_def: &Value,
    config: &Value,
    scale_channel: &str,
    scale_type: Option<&str>,
    is_count_aggregate: bool,
) -> String {
    let Some(scale_type) = scale_type else {
        return "always-valid".to_string();
    };
    if !has_continuous_domain(scale_type) || is_count_aggregate {
        return "always-valid".to_string();
    }

    let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");

    // An explicit null (which the config merge drops) means "show".
    if mark_def
        .as_object()
        .and_then(|m| m.get("invalid"))
        .map(|v| v.is_null())
        .unwrap_or(false)
    {
        return "show".to_string();
    }

    let invalid_mode = normalize_invalid_data_mode(
        get_mark_prop_or_config("invalid", mark_def, config, None, false).as_ref(),
        is_path_mark(mark_type),
    );

    let scale_output_for_invalid = config
        .get("scale")
        .and_then(|s| s.get("invalid"))
        .and_then(|i| i.get(scale_channel));
    if scale_output_for_invalid.is_some() {
        return "show".to_string();
    }

    invalid_mode
}

/// Port of `shouldBreakPath`.
pub fn should_break_path(mode: &str) -> bool {
    mode == "break-paths-filter-domains" || mode == "break-paths-show-domains"
}

fn ref_for_invalid_values(include_as: &Value, scale: Option<&ScaleComponent>, scale_name: &str) -> Value {
    if include_as.is_object() && include_as.get("value").is_some() {
        let value = &include_as["value"];
        if is_signal_ref(value) {
            return json!({"signal": value["signal"]});
        }
        return json!({"value": value});
    }

    scaled_zero_or_min_or_max(scale_name, scale, "zeroOrMin").unwrap_or(Value::Null)
}

/// Port of `getConditionalValueRefForIncludingInvalidValue`.
pub fn get_conditional_value_ref_for_including_invalid_value(
    scale_channel: &str,
    channel_def: Option<&Value>,
    scale: Option<&ScaleComponent>,
    scale_name: &str,
    mark_def: &Value,
    config: &Value,
) -> Option<Value> {
    let scale_type = scale
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(str::to_string));
    let field_def = channel_def.and_then(get_field_def);
    let is_count = is_count_aggregate(field_def.as_ref());

    let invalid_mode = get_scale_invalid_data_mode(
        mark_def,
        config,
        scale_channel,
        scale_type.as_deref(),
        is_count,
    );

    if let Some(field_def) = field_def {
        if invalid_mode == "show" {
            let include_as = config
                .get("scale")
                .and_then(|s| s.get("invalid"))
                .and_then(|i| i.get(scale_channel))
                .cloned()
                .unwrap_or_else(|| json!("zero-or-min"));

            let test = field_valid_predicate(
                &vg_field_with(
                    &field_def,
                    &FieldRefOption {
                        expr: Some("datum".to_string()),
                        ..Default::default()
                    },
                ),
                false,
            );
            let mut out = ref_for_invalid_values(&include_as, scale, scale_name);
            if let Some(map) = out.as_object_mut() {
                map.insert("test".to_string(), Value::String(test));
            }
            return Some(out);
        }
    }
    None
}
