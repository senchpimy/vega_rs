//! Port of the guide-related helpers in `src/compile/format.ts`.
//!
//! Only the pieces used by axis compilation (`guideFormat`,
//! `guideFormatType`, `formatCustomType`, `numberFormat`, `timeFormat`) are
//! implemented here; the mark-text helpers live in their own module.

use serde_json::Value;

use crate::channeldef::{
    channel_def_type, is_field_or_datum_def_for_time_format, is_field_def,
    is_position_field_or_datum_def, vg_field_with, FieldRefOption,
};
use crate::channeldef_support::{is_binning, truthy};
use crate::predicate::field_valid_predicate;
use crate::scale::ScaleType;
use crate::timeunit::{normalize_time_unit, time_unit_specifier_expression};
use crate::util::{stringify, string_value};

pub const BIN_RANGE_DELIMITER: &str = " \u{2013} ";

/// `isCustomFormatType`.
pub fn is_custom_format_type(format_type: Option<&Value>) -> bool {
    match format_type.and_then(Value::as_str) {
        Some(t) => t != "number" && t != "time",
        None => false,
    }
}

fn custom_format_expr(format_type: &str, field: &str, format: Option<&Value>) -> String {
    let suffix = match format {
        Some(f) if truthy(f) => format!(", {}", stringify(f)),
        _ => String::new(),
    };
    format!("{format_type}({field}{suffix})")
}

fn format_expr(field: &str, format: Option<&str>) -> String {
    format!("format({field}, \"{}\")", format.unwrap_or(""))
}

fn bin_number_format_expr(
    field: &str,
    format: Option<&Value>,
    format_type: Option<&Value>,
    config: &Value,
) -> String {
    if is_custom_format_type(format_type) {
        return custom_format_expr(
            format_type.and_then(Value::as_str).unwrap_or_default(),
            field,
            format,
        );
    }
    let specified = format.and_then(Value::as_str);
    let fallback = config.get("numberFormat").and_then(Value::as_str);
    format_expr(field, specified.or(fallback))
}

fn bin_format_expression(
    start_field: &str,
    end_field: &str,
    format: Option<&Value>,
    format_type: Option<&Value>,
    config: &Value,
) -> String {
    if format.is_none()
        && format_type.is_none()
        && truthy(config.get("customFormatTypes").unwrap_or(&Value::Null))
        && config.get("numberFormatType").is_some()
    {
        let number_format = config.get("numberFormat").cloned();
        let number_format_type = config.get("numberFormatType").cloned();
        return bin_format_expression(
            start_field,
            end_field,
            number_format.as_ref(),
            number_format_type.as_ref(),
            config,
        );
    }
    let start = bin_number_format_expr(start_field, format, format_type, config);
    let end = bin_number_format_expr(end_field, format, format_type, config);
    format!(
        "{} ? \"null\" : {} + \"{}\" + {}",
        field_valid_predicate(start_field, false),
        start,
        BIN_RANGE_DELIMITER,
        end
    )
}

fn field_to_format(
    field_or_datum_def: Option<&Value>,
    expr: Option<&str>,
    normalize_stack: bool,
) -> String {
    let Some(def) = field_or_datum_def else {
        return String::new();
    };
    if is_field_def(def) {
        let opt = |suffix: Option<&str>| FieldRefOption {
            expr: expr.map(str::to_string),
            suffix: suffix.map(str::to_string),
            ..Default::default()
        };
        if normalize_stack {
            format!(
                "{}-{}",
                vg_field_with(def, &opt(Some("end"))),
                vg_field_with(def, &opt(Some("start")))
            )
        } else {
            vg_field_with(def, &opt(None))
        }
    } else {
        // `datumDefToExpr`
        let value = def.get("datum").cloned().unwrap_or(Value::Null);
        let e = expr.unwrap_or("");
        let accessor = if e.is_empty() { "datum".to_string() } else { e.to_string() };
        match value {
            Value::String(s) => format!("{accessor}[{}]", stringify(&Value::String(s))),
            other => stringify(&other),
        }
    }
}

/// `formatCustomType`
#[allow(clippy::too_many_arguments)]
pub fn format_custom_type(
    field_or_datum_def: Option<&Value>,
    format: Option<&Value>,
    format_type: &str,
    expr: Option<&str>,
    normalize_stack: bool,
    config: &Value,
    field: Option<&str>,
) -> Value {
    let field = match field {
        Some(f) => f.to_string(),
        None => field_to_format(field_or_datum_def, expr, normalize_stack),
    };

    if let Some(def) = field_or_datum_def {
        if field != "datum.value" && is_field_def(def) && is_binning(&def["bin"]) {
            let end_field = vg_field_with(
                def,
                &FieldRefOption {
                    expr: expr.map(str::to_string),
                    bin_suffix: Some("end".into()),
                    ..Default::default()
                },
            );
            return serde_json::json!({
                "signal": bin_format_expression(&field, &end_field, format, Some(&Value::String(format_type.to_string())), config)
            });
        }
    }

    serde_json::json!({ "signal": custom_format_expr(format_type, &field, format) })
}

/// `numberFormat`
pub fn number_format(
    type_: Option<&str>,
    specified_format: Option<&Value>,
    config: &Value,
    normalize_stack: bool,
) -> Option<Value> {
    if let Some(specified_format) = specified_format {
        if specified_format.is_string() {
            return Some(specified_format.clone());
        }
    }
    if type_ == Some("quantitative") {
        if normalize_stack {
            return config.get("normalizedNumberFormat").cloned();
        }
        return config.get("numberFormat").cloned();
    }
    None
}

/// `timeFormat`
pub fn time_format(
    specified_format: Option<&Value>,
    time_unit: Option<&str>,
    config: &Value,
    omit_time_format_config: bool,
) -> Option<Value> {
    if let Some(specified) = specified_format {
        if truthy(specified) {
            return Some(specified.clone());
        }
    }
    if let Some(time_unit) = time_unit {
        return time_unit_specifier_expression(time_unit).map(|signal| serde_json::json!({ "signal": signal }));
    }
    if omit_time_format_config {
        None
    } else {
        config.get("timeFormat").cloned()
    }
}

/// `guideFormatType`
pub fn guide_format_type(
    format_type: Option<&Value>,
    field_or_datum_def: Option<&Value>,
    scale_type: Option<ScaleType>,
) -> Option<Value> {
    if let Some(format_type) = format_type {
        let is_signal = crate::compile_common::is_signal_ref(format_type);
        let is_builtin = matches!(format_type.as_str(), Some("number") | Some("time"));
        if is_signal || is_builtin {
            return Some(format_type.clone());
        }
    }

    let def = field_or_datum_def.unwrap_or(&Value::Null);
    if is_field_or_datum_def_for_time_format(def)
        && scale_type != Some(ScaleType::Time)
        && scale_type != Some(ScaleType::Utc)
    {
        let is_utc = is_field_def(def)
            && normalize_time_unit(&def["timeUnit"])
                .and_then(|t| t.utc)
                .unwrap_or(false);
        return Some(Value::String(if is_utc { "utc" } else { "time" }.to_string()));
    }

    None
}

/// `guideFormat`
pub fn guide_format(
    field_or_datum_def: Option<&Value>,
    type_: Option<&str>,
    format: Option<&Value>,
    format_type: Option<&Value>,
    config: &Value,
    omit_time_format_config: bool,
) -> Option<Value> {
    if is_custom_format_type(format_type) {
        return None; // handled in encode block
    }

    let def = field_or_datum_def.unwrap_or(&Value::Null);
    let custom_format_types = truthy(config.get("customFormatTypes").unwrap_or(&Value::Null));

    if format.is_none() && format_type.is_none() && custom_format_types
        && channel_def_type(def).as_deref() == Some("quantitative") {
            if config.get("normalizedNumberFormatType").is_some()
                && is_position_field_or_datum_def(def)
                && def.get("stack").and_then(Value::as_str) == Some("normalize")
            {
                return None;
            }
            if config.get("numberFormatType").is_some() {
                return None;
            }
        }

    if is_position_field_or_datum_def(def)
        && def.get("stack").and_then(Value::as_str) == Some("normalize")
        && config.get("normalizedNumberFormat").is_some()
    {
        return number_format(Some("quantitative"), format, config, true);
    }

    if is_field_or_datum_def_for_time_format(def) {
        let time_unit = if is_field_def(def) {
            normalize_time_unit(&def["timeUnit"]).and_then(|t| t.unit)
        } else {
            None
        };
        if time_unit.is_none() && custom_format_types && config.get("timeFormatType").is_some() {
            return None;
        }
        return time_format(format, time_unit.as_deref(), config, omit_time_format_config);
    }

    number_format(type_, format, config, false)
}

/// Utility used by tests / other modules: stringify a format exactly like the
/// custom formatter does.
pub fn format_to_string(format: &Value) -> String {
    string_value(format)
}
