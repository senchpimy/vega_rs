//! Port of the guide-formatting helpers from `src/compile/format.ts` that the
//! legend compiler needs.

use serde_json::{json, Value};

use crate::channeldef::{
    channel_def_type, is_field_def, is_field_or_datum_def_for_time_format,
    is_position_field_or_datum_def,
};
use crate::timeunit::{normalize_time_unit, time_unit_specifier_expression};
use crate::util::{has_property, stringify};

/// Port of `isCustomFormatType`.
pub fn is_custom_format_type(format_type: &str) -> bool {
    !format_type.is_empty() && format_type != "number" && format_type != "time"
}

fn is_signal_ref(value: &Value) -> bool {
    has_property(value, "signal")
}

/// Port of `customFormatExpr`.
fn custom_format_expr(format_type: &str, field: &str, format: Option<&Value>) -> String {
    let suffix = match format {
        Some(format) if !format.is_null() => format!(", {}", stringify(format)),
        _ => String::new(),
    };
    format!("{format_type}({field}{suffix})")
}

/// Port of `formatCustomType` for the axis/legend `datum.value` case.
pub fn format_custom_type(
    field_or_datum_def: &Value,
    format: Option<&Value>,
    format_type: &str,
    config: &Value,
    field: Option<&str>,
) -> Value {
    let field = field.unwrap_or("datum.value");
    let _ = (field_or_datum_def, config);
    json!({"signal": custom_format_expr(format_type, field, format)})
}

/// Port of `guideFormat`.
pub fn guide_format(
    field_or_datum_def: &Value,
    type_: &str,
    format: Option<&Value>,
    format_type: Option<&Value>,
    config: &Value,
    omit_time_format_config: bool,
) -> Option<Value> {
    let format_type_string = format_type.and_then(Value::as_str);

    if let Some(ft) = format_type_string {
        if is_custom_format_type(ft) {
            return None; // handled in encode block
        }
    } else if format.is_none() && config.get("customFormatTypes").and_then(Value::as_bool) == Some(true)
        && channel_def_type(field_or_datum_def).as_deref() == Some("quantitative") {
            if config.get("normalizedNumberFormatType").is_some()
                && is_position_field_or_datum_def(field_or_datum_def)
                && field_or_datum_def.get("stack").and_then(Value::as_str) == Some("normalize")
            {
                return None;
            }
            if config.get("numberFormatType").is_some() {
                return None;
            }
        }

    if is_position_field_or_datum_def(field_or_datum_def)
        && field_or_datum_def.get("stack").and_then(Value::as_str) == Some("normalize")
        && config.get("normalizedNumberFormat").is_some()
    {
        return number_format(
            Some("quantitative"),
            format,
            config,
            true,
        );
    }

    if is_field_or_datum_def_for_time_format(field_or_datum_def) {
        let time_unit = if is_field_def(field_or_datum_def) {
            normalize_time_unit(field_or_datum_def.get("timeUnit").unwrap_or(&Value::Null))
                .and_then(|params| params.unit)
        } else {
            None
        };

        if time_unit.is_none()
            && config.get("customFormatTypes").and_then(Value::as_bool) == Some(true)
            && config.get("timeFormatType").is_some()
        {
            return None; // handled in encode block
        }

        return time_format(
            format.and_then(Value::as_str),
            time_unit.as_deref(),
            config,
            omit_time_format_config,
        );
    }

    number_format(Some(type_), format, config, false)
}

/// Port of `guideFormatType`.
pub fn guide_format_type(
    format_type: Option<&Value>,
    field_or_datum_def: &Value,
    scale_type: Option<crate::scale::ScaleType>,
) -> Option<Value> {
    use crate::scale::ScaleType;

    if let Some(format_type) = format_type {
        if is_signal_ref(format_type)
            || format_type.as_str() == Some("number")
            || format_type.as_str() == Some("time")
        {
            return Some(format_type.clone());
        }
    }

    if is_field_or_datum_def_for_time_format(field_or_datum_def)
        && scale_type != Some(ScaleType::Time)
        && scale_type != Some(ScaleType::Utc)
    {
        let utc = is_field_def(field_or_datum_def)
            && normalize_time_unit(field_or_datum_def.get("timeUnit").unwrap_or(&Value::Null))
                .and_then(|params| params.utc)
                .unwrap_or(false);
        return Some(Value::String(if utc { "utc" } else { "time" }.to_string()));
    }
    None
}

/// Port of `numberFormat`.
pub fn number_format(
    type_: Option<&str>,
    specified_format: Option<&Value>,
    config: &Value,
    normalize_stack: bool,
) -> Option<Value> {
    if let Some(Value::String(s)) = specified_format {
        return Some(Value::String(s.clone()));
    }

    if type_ == Some("quantitative") {
        let key = if normalize_stack {
            "normalizedNumberFormat"
        } else {
            "numberFormat"
        };
        return config.get(key).cloned();
    }
    None
}

/// Port of `timeFormat`.
pub fn time_format(
    specified_format: Option<&str>,
    time_unit: Option<&str>,
    config: &Value,
    omit_time_format_config: bool,
) -> Option<Value> {
    if let Some(specified_format) = specified_format {
        return Some(Value::String(specified_format.to_string()));
    }

    if let Some(time_unit) = time_unit {
        return time_unit_specifier_expression(time_unit).map(|signal| json!({"signal": signal}));
    }

    if omit_time_format_config {
        None
    } else {
        config.get("timeFormat").cloned()
    }
}
