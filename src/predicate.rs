//! Port of `src/predicate.ts`.
//!
//! Predicates are represented as [`serde_json::Value`] because the upstream
//! TypeScript module is a union of loosely-typed interfaces.

use serde_json::{Map, Value};

use crate::channeldef::{value_expr, vg_field_with, FieldRefOption, ValueExprOpt};
use crate::channeldef_support::{is_signal_ref, js_to_string, truthy};
use crate::expr::replace_expr_ref;
use crate::timeunit::{self, TimeUnitParams};
use crate::util::{has_property, stringify};

// ---------------------------------------------------------------------------
// Type guards
// ---------------------------------------------------------------------------

fn is_array(v: Option<&Value>) -> bool {
    v.map(|v| v.is_array()).unwrap_or(false)
}

fn has_field(predicate: &Value) -> bool {
    truthy(predicate.get("field").unwrap_or(&Value::Null))
}

fn has_key(predicate: &Value, key: &str) -> bool {
    predicate
        .as_object()
        .map(|map| map.contains_key(key))
        .unwrap_or(false)
}

/// Port of `isSelectionPredicate`.
pub fn is_selection_predicate(predicate: &Value) -> bool {
    is_parameter_predicate(predicate)
}

/// Port of `isParameterPredicate` (the upstream guard is named
/// `isSelectionPredicate`).
pub fn is_parameter_predicate(predicate: &Value) -> bool {
    has_property(predicate, "param")
}

/// Port of `isFieldEqualPredicate`.
pub fn is_field_equal_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "equal")
}

/// Port of `isFieldLTPredicate`.
pub fn is_field_lt_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "lt")
}

/// Port of `isFieldLTEPredicate`.
pub fn is_field_lte_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "lte")
}

/// Port of `isFieldGTPredicate`.
pub fn is_field_gt_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "gt")
}

/// Port of `isFieldGTEPredicate`.
pub fn is_field_gte_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "gte")
}

/// Port of `isFieldRangePredicate`.
pub fn is_field_range_predicate(predicate: &Value) -> bool {
    if has_field(predicate) {
        let range = predicate.get("range");
        if is_array(range) {
            if range.and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) == 2 {
                return true;
            }
        } else if let Some(range) = range {
            if is_signal_ref(range) {
                return true;
            }
        }
    }
    false
}

/// Port of `isFieldOneOfPredicate`.
pub fn is_field_one_of_predicate(predicate: &Value) -> bool {
    has_field(predicate)
        && (is_array(predicate.get("oneOf")) || is_array(predicate.get("in")))
}

/// Port of `isFieldValidPredicate`.
pub fn is_field_valid_predicate(predicate: &Value) -> bool {
    has_field(predicate) && has_key(predicate, "valid")
}

/// Port of `isFieldPredicate`.
pub fn is_field_predicate(predicate: &Value) -> bool {
    is_field_one_of_predicate(predicate)
        || is_field_equal_predicate(predicate)
        || is_field_range_predicate(predicate)
        || is_field_lt_predicate(predicate)
        || is_field_gt_predicate(predicate)
        || is_field_lte_predicate(predicate)
        || is_field_gte_predicate(predicate)
}

// ---------------------------------------------------------------------------
// Expression generation
// ---------------------------------------------------------------------------

fn predicate_value_expr(v: &Value, time_unit: Option<&str>) -> String {
    let opt = ValueExprOpt {
        time_unit: time_unit.map(|u| Value::String(u.to_string())),
        wrap_time: true,
        ..Default::default()
    };
    value_expr(v, &opt).unwrap_or_else(|| stringify(v))
}

fn predicate_values_expr(vals: &[Value], time_unit: Option<&str>) -> Vec<String> {
    vals.iter()
        .map(|v| predicate_value_expr(v, time_unit))
        .collect()
}

/// Port of `fieldFilterExpression` with the default options
/// (`useInRange = true`, `expr = "datum"`).
pub fn field_filter_expression(predicate: &Value) -> String {
    field_filter_expression_with_options(predicate, true, "datum")
}

/// Port of `fieldFilterExpression(predicate, useInRange, expr)`.
pub fn field_filter_expression_with_options(
    predicate: &Value,
    use_in_range: bool,
    expr: &str,
) -> String {
    let field = predicate
        .get("field")
        .map(js_to_string)
        .unwrap_or_default();

    let normalized_time_unit = predicate
        .get("timeUnit")
        .filter(|v| !v.is_null())
        .and_then(timeunit::normalize_time_unit);
    let unit = normalized_time_unit
        .as_ref()
        .and_then(|p| p.unit.clone());
    let binned = normalized_time_unit
        .as_ref()
        .and_then(|p| p.binned)
        .unwrap_or(false);

    let raw_field_expr = vg_field_with(
        predicate,
        &FieldRefOption {
            expr: Some(expr.to_string()),
            ..Default::default()
        },
    );

    let field_expr = match &unit {
        Some(unit) => {
            // For timeUnit, cast into integer with time() so we can use ===,
            // inrange, indexOf to compare values directly.
            // TODO: support utc
            let inner = if !binned {
                timeunit::field_expr_with_options(unit, &field, false, expr)
            } else {
                raw_field_expr.clone()
            };
            format!("time({inner})")
        }
        None => raw_field_expr.clone(),
    };

    if is_field_equal_predicate(predicate) {
        return format!(
            "{field_expr}==={}",
            predicate_value_expr(&predicate["equal"], unit.as_deref())
        );
    } else if is_field_lt_predicate(predicate) {
        let upper = &predicate["lt"];
        return format!(
            "{field_expr}<{}",
            predicate_value_expr(upper, unit.as_deref())
        );
    } else if is_field_gt_predicate(predicate) {
        let lower = &predicate["gt"];
        return format!(
            "{field_expr}>{}",
            predicate_value_expr(lower, unit.as_deref())
        );
    } else if is_field_lte_predicate(predicate) {
        let upper = &predicate["lte"];
        return format!(
            "{field_expr}<={}",
            predicate_value_expr(upper, unit.as_deref())
        );
    } else if is_field_gte_predicate(predicate) {
        let lower = &predicate["gte"];
        return format!(
            "{field_expr}>={}",
            predicate_value_expr(lower, unit.as_deref())
        );
    } else if is_field_one_of_predicate(predicate) {
        let one_of = predicate
            .get("oneOf")
            .or_else(|| predicate.get("in"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        return format!(
            "indexof([{}], {field_expr}) !== -1",
            predicate_values_expr(&one_of, unit.as_deref()).join(",")
        );
    } else if is_field_valid_predicate(predicate) {
        return field_valid_predicate(&field_expr, truthy(&predicate["valid"]));
    } else if is_field_range_predicate(predicate) {
        let predicate_map = predicate.as_object().cloned().unwrap_or_default();
        let replaced = replace_expr_ref(&predicate_map, 0);
        let range = replaced.get("range").cloned().unwrap_or(Value::Null);

        let (lower, upper) = if is_signal_ref(&range) {
            let signal = range.get("signal").and_then(|s| s.as_str()).unwrap_or("");
            (
                Value::Object(Map::from_iter([(
                    "signal".to_string(),
                    Value::String(format!("{signal}[0]")),
                )])),
                Value::Object(Map::from_iter([(
                    "signal".to_string(),
                    Value::String(format!("{signal}[1]")),
                )])),
            )
        } else {
            let arr = range.as_array().cloned().unwrap_or_default();
            (
                arr.first().cloned().unwrap_or(Value::Null),
                arr.get(1).cloned().unwrap_or(Value::Null),
            )
        };

        if !lower.is_null() && !upper.is_null() && use_in_range {
            return format!(
                "inrange({field_expr}, [{}, {}])",
                predicate_value_expr(&lower, unit.as_deref()),
                predicate_value_expr(&upper, unit.as_deref())
            );
        }

        let mut exprs: Vec<String> = Vec::new();
        if !lower.is_null() {
            exprs.push(format!(
                "{field_expr} >= {}",
                predicate_value_expr(&lower, unit.as_deref())
            ));
        }
        if !upper.is_null() {
            exprs.push(format!(
                "{field_expr} <= {}",
                predicate_value_expr(&upper, unit.as_deref())
            ));
        }

        return if exprs.is_empty() {
            "true".to_string()
        } else {
            exprs.join(" && ")
        };
    }

    panic!("Invalid field predicate: {}", stringify(predicate));
}

/// Port of `fieldValidPredicate`.
pub fn field_valid_predicate(field_expr: &str, valid: bool) -> String {
    if valid {
        format!("isValid({field_expr}) && isFinite(+{field_expr})")
    } else {
        format!("!isValid({field_expr}) || !isFinite(+{field_expr})")
    }
}

// ---------------------------------------------------------------------------
// normalizePredicate
// ---------------------------------------------------------------------------

fn time_unit_params_to_value(params: &TimeUnitParams) -> Value {
    let mut map = Map::new();
    if let Some(unit) = &params.unit {
        map.insert("unit".to_string(), Value::String(unit.clone()));
    }
    if let Some(maxbins) = &params.maxbins {
        map.insert("maxbins".to_string(), maxbins.clone());
    }
    if let Some(step) = &params.step {
        map.insert("step".to_string(), step.clone());
    }
    if let Some(utc) = params.utc {
        map.insert("utc".to_string(), Value::Bool(utc));
    }
    if let Some(binned) = params.binned {
        map.insert("binned".to_string(), Value::Bool(binned));
    }
    Value::Object(map)
}

/// Port of `normalizePredicate`.
pub fn normalize_predicate(f: &Value) -> Value {
    let time_unit = f.get("timeUnit").cloned().unwrap_or(Value::Null);
    if is_field_predicate(f) && truthy(&time_unit) {
        if let Some(normalized) = timeunit::normalize_time_unit(&time_unit) {
            let mut result = f.clone();
            if let Some(map) = result.as_object_mut() {
                map.insert("timeUnit".to_string(), time_unit_params_to_value(&normalized));
            }
            return result;
        }
    }
    f.clone()
}
