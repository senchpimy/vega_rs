//! Port of the subset of `src/compile/format.ts` used by mark encoding.

use serde_json::{json, Value};

use crate::channeldef::{
    channel_def_type, is_field_def, is_field_or_datum_def_for_time_format, is_scale_field_def,
    vg_field_with, FieldRefOption,
};
use crate::channeldef_support::{is_binning, truthy};
use crate::compile_common::BIN_RANGE_DELIMITER;
use crate::predicate::field_valid_predicate;
use crate::timeunit::{format_expression, normalize_time_unit, time_unit_specifier_expression};
use crate::util::stringify;

pub fn is_custom_format_type(format_type: Option<&str>) -> bool {
    matches!(format_type, Some(t) if t != "number" && t != "time" && !t.is_empty())
}

fn custom_format_expr(format_type: &str, field: &str, format: Option<&Value>) -> String {
    match format.filter(|f| !f.is_null()) {
        Some(format) => format!("{format_type}({field}, {})", stringify(format)),
        None => format!("{format_type}({field})"),
    }
}

fn field_to_format(
    field_or_datum_def: &Value,
    expr: Option<&str>,
    normalize_stack: bool,
) -> String {
    if is_field_def(field_or_datum_def) {
        if normalize_stack {
            let end = vg_field_with(
                field_or_datum_def,
                &FieldRefOption {
                    expr: expr.map(str::to_string),
                    suffix: Some("end".to_string()),
                    ..Default::default()
                },
            );
            let start = vg_field_with(
                field_or_datum_def,
                &FieldRefOption {
                    expr: expr.map(str::to_string),
                    suffix: Some("start".to_string()),
                    ..Default::default()
                },
            );
            format!("{end}-{start}")
        } else {
            vg_field_with(
                field_or_datum_def,
                &FieldRefOption {
                    expr: expr.map(str::to_string),
                    ..Default::default()
                },
            )
        }
    } else {
        super::valueref::datum_def_to_expr(field_or_datum_def)
    }
}

fn number_format(
    type_: Option<&str>,
    specified_format: Option<&Value>,
    config: &Value,
    normalize_stack: bool,
) -> Option<Value> {
    if let Some(Value::String(s)) = specified_format {
        return Some(Value::String(s.clone()));
    }
    if type_ == Some("quantitative") {
        return config
            .get(if normalize_stack {
                "normalizedNumberFormat"
            } else {
                "numberFormat"
            })
            .cloned();
    }
    None
}

fn format_expr(field: &str, format: Option<&Value>) -> String {
    let format = format
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("format({field}, \"{format}\")")
}

fn bin_number_format_expr(
    field: &str,
    format: Option<&Value>,
    format_type: Option<&str>,
    config: &Value,
) -> String {
    if is_custom_format_type(format_type) {
        return custom_format_expr(format_type.unwrap(), field, format);
    }
    format_expr(
        field,
        format
            .filter(|f| f.is_string())
            .or_else(|| config.get("numberFormat"))
            .filter(|f| !f.is_null()),
    )
}

pub fn bin_format_expression(
    start_field: &str,
    end_field: &str,
    format: Option<&Value>,
    format_type: Option<&str>,
    config: &Value,
) -> String {
    if format.is_none()
        && format_type.is_none()
        && config.get("customFormatTypes").map(truthy).unwrap_or(false)
        && config.get("numberFormatType").is_some()
    {
        return bin_format_expression(
            start_field,
            end_field,
            config.get("numberFormat"),
            config.get("numberFormatType").and_then(Value::as_str),
            config,
        );
    }
    let start = bin_number_format_expr(start_field, format, format_type, config);
    let end = bin_number_format_expr(end_field, format, format_type, config);
    format!(
        "{} ? \"null\" : {start} + \"{BIN_RANGE_DELIMITER}\" + {end}",
        field_valid_predicate(start_field, false)
    )
}

pub fn time_format_expression(
    field: &str,
    time_unit: Option<&Value>,
    format: Option<&Value>,
    format_type: Option<&str>,
    raw_time_format: Option<&str>,
    is_utc_scale: bool,
) -> String {
    let normalized_unit = time_unit.and_then(|tu| {
        normalize_time_unit(tu).and_then(|params| params.unit)
    });

    if normalized_unit.is_none() || format.is_some() {
        if normalized_unit.is_none() {
            if let Some(format_type) = format_type {
                return format!(
                    "{}({}, {})",
                    format_type,
                    field,
                    stringify(format.unwrap_or(&Value::Null))
                );
            }
        }
        let format = if format.map(|f| f.is_string()).unwrap_or(false) {
            format.cloned()
        } else {
            raw_time_format.map(|s| Value::String(s.to_string()))
        };
        return format!(
            "{}Format({}, {})",
            if is_utc_scale { "utc" } else { "time" },
            field,
            stringify(format.as_ref().unwrap_or(&Value::Null))
        );
    }

    format_expression(normalized_unit.as_deref(), field, is_utc_scale).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
/// Port of `formatCustomType` from `src/compile/format.ts`.
fn custom_format_signal(
    field_or_datum_def: &Value,
    format: Option<&Value>,
    format_type: &str,
    expr: Option<&str>,
    field: &str,
    config: &Value,
) -> Value {
    if is_field_def(field_or_datum_def) && is_binning(&field_or_datum_def["bin"]) {
        let end_field = vg_field_with(
            field_or_datum_def,
            &FieldRefOption {
                expr: expr.map(str::to_string),
                bin_suffix: Some("end".to_string()),
                ..Default::default()
            },
        );
        return json!({"signal": bin_format_expression(
            field,
            &end_field,
            format,
            Some(format_type),
            config
        )});
    }
    json!({"signal": custom_format_expr(format_type, field, format)})
}

pub fn format_signal_ref(
    field_or_datum_def: &Value,
    format: Option<&Value>,
    format_type: Option<&str>,
    expr: Option<&str>,
    normalize_stack: bool,
    config: &Value,
) -> Option<Value> {
    if is_custom_format_type(format_type) {
        let field = field_to_format(field_or_datum_def, expr, normalize_stack);
        return Some(custom_format_signal(
            field_or_datum_def,
            format,
            format_type.unwrap(),
            expr,
            &field,
            config,
        ));
    }

    let field = field_to_format(field_or_datum_def, expr, normalize_stack);
    let type_ = channel_def_type(field_or_datum_def);

    if format.is_none()
        && format_type.is_none()
        && config.get("customFormatTypes").map(truthy).unwrap_or(false)
    {
        if type_.as_deref() == Some("quantitative") {
            if normalize_stack {
                if let Some(fmt_type) = config
                    .get("normalizedNumberFormatType")
                    .and_then(Value::as_str)
                {
                    let field2 = field_to_format(field_or_datum_def, expr, normalize_stack);
                    return Some(custom_format_signal(
                        field_or_datum_def,
                        config.get("normalizedNumberFormat"),
                        fmt_type,
                        expr,
                        &field2,
                        config,
                    ));
                }
            }
            if let Some(fmt_type) = config.get("numberFormatType").and_then(Value::as_str) {
                let field2 = field_to_format(field_or_datum_def, expr, normalize_stack);
                return Some(custom_format_signal(
                    field_or_datum_def,
                    config.get("numberFormat"),
                    fmt_type,
                    expr,
                    &field2,
                    config,
                ));
            }
        }
        if type_.as_deref() == Some("temporal")
            && config.get("timeFormatType").is_some()
            && is_field_def(field_or_datum_def)
            && field_or_datum_def.get("timeUnit").map(|v| v.is_null()).unwrap_or(true)
        {
            let field2 = field_to_format(field_or_datum_def, expr, normalize_stack);
            return Some(custom_format_signal(
                field_or_datum_def,
                config.get("timeFormat"),
                config.get("timeFormatType").and_then(Value::as_str).unwrap_or(""),
                expr,
                &field2,
                config,
            ));
        }
    }

    if is_field_or_datum_def_for_time_format(field_or_datum_def) {
        let (time_unit, is_utc_unit) = if is_field_def(field_or_datum_def) {
            let params = normalize_time_unit(&field_or_datum_def["timeUnit"]);
            (
                params.as_ref().and_then(|p| p.unit.clone()),
                params.as_ref().and_then(|p| p.utc).unwrap_or(false),
            )
        } else {
            (None, false)
        };
        let is_utc_scale = is_utc_unit
            || (is_scale_field_def(field_or_datum_def)
                && field_or_datum_def
                    .get("scale")
                    .and_then(|s| s.get("type"))
                    .and_then(Value::as_str)
                    == Some("utc"));
        let signal = time_format_expression(
            &field,
            time_unit.as_ref().map(|u| Value::String(u.clone())).as_ref(),
            format,
            config.get("timeFormatType").and_then(Value::as_str),
            config.get("timeFormat").and_then(Value::as_str),
            is_utc_scale,
        );
        return if signal.is_empty() {
            None
        } else {
            Some(json!({"signal": signal}))
        };
    }

    let number_format = number_format(
        type_.as_deref(),
        format,
        config,
        normalize_stack,
    );

    if is_field_def(field_or_datum_def) && is_binning(&field_or_datum_def["bin"]) {
        let end_field = vg_field_with(
            field_or_datum_def,
            &FieldRefOption {
                expr: expr.map(str::to_string),
                bin_suffix: Some("end".to_string()),
                ..Default::default()
            },
        );
        return Some(json!({"signal": bin_format_expression(
            &field,
            &end_field,
            number_format.as_ref(),
            format_type,
            config
        )}));
    } else if number_format.as_ref().map(truthy).unwrap_or(false)
        || type_.as_deref() == Some("quantitative")
    {
        return Some(json!({"signal": format_expr(&field, number_format.as_ref())}));
    }

    Some(json!({"signal": format!("isValid({field}) ? {field} : \"\"+{field}")}))
}

#[allow(dead_code)]
fn _time_specifier(unit: &str) -> Option<String> {
    time_unit_specifier_expression(unit)
}
