//! Port of `src/timeunit.ts`.

use serde_json::{json, Number, Value};

use crate::datetime::{date_time_expr_to_expr, date_time_to_expr, DateTime};
use crate::util::{access_path_with_datum, js_number_to_string, stringify, var_name};

/// Time units that correspond to only one part of Date objects.
pub const LOCAL_SINGLE_TIMEUNIT_INDEX: [&str; 11] = [
    "year",
    "quarter",
    "month",
    "week",
    "day",
    "dayofyear",
    "date",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
];

/// `keys(LOCAL_SINGLE_TIMEUNIT_INDEX)` in the upstream source.
pub const TIMEUNIT_PARTS: [&str; 11] = LOCAL_SINGLE_TIMEUNIT_INDEX;

pub fn is_local_single_time_unit(time_unit: &str) -> bool {
    LOCAL_SINGLE_TIMEUNIT_INDEX.contains(&time_unit)
}

pub const UTC_SINGLE_TIMEUNIT_INDEX: [&str; 11] = [
    "utcyear",
    "utcquarter",
    "utcmonth",
    "utcweek",
    "utcday",
    "utcdayofyear",
    "utcdate",
    "utchours",
    "utcminutes",
    "utcseconds",
    "utcmilliseconds",
];

pub const LOCAL_MULTI_TIMEUNIT_INDEX: [&str; 29] = [
    "yearquarter",
    "yearquartermonth",
    "yearmonth",
    "yearmonthdate",
    "yearmonthdatehours",
    "yearmonthdatehoursminutes",
    "yearmonthdatehoursminutesseconds",
    "yearweek",
    "yearweekday",
    "yearweekdayhours",
    "yearweekdayhoursminutes",
    "yearweekdayhoursminutesseconds",
    "yeardayofyear",
    "quartermonth",
    "monthdate",
    "monthdatehours",
    "monthdatehoursminutes",
    "monthdatehoursminutesseconds",
    "weekday",
    "weekdayhours",
    "weekdayhoursminutes",
    "weekdayhoursminutesseconds",
    "dayhours",
    "dayhoursminutes",
    "dayhoursminutesseconds",
    "hoursminutes",
    "hoursminutesseconds",
    "minutesseconds",
    "secondsmilliseconds",
];

const BINNED_LOCAL_TIMEUNIT_INDEX: [&str; 14] = [
    "binnedyear",
    "binnedyearquarter",
    "binnedyearquartermonth",
    "binnedyearmonth",
    "binnedyearmonthdate",
    "binnedyearmonthdatehours",
    "binnedyearmonthdatehoursminutes",
    "binnedyearmonthdatehoursminutesseconds",
    "binnedyearweek",
    "binnedyearweekday",
    "binnedyearweekdayhours",
    "binnedyearweekdayhoursminutes",
    "binnedyearweekdayhoursminutesseconds",
    "binnedyeardayofyear",
];

const BINNED_UTC_TIMEUNIT_INDEX: [&str; 14] = [
    "binnedutcyear",
    "binnedutcyearquarter",
    "binnedutcyearquartermonth",
    "binnedutcyearmonth",
    "binnedutcyearmonthdate",
    "binnedutcyearmonthdatehours",
    "binnedutcyearmonthdatehoursminutes",
    "binnedutcyearmonthdatehoursminutesseconds",
    "binnedutcyearweek",
    "binnedutcyearweekday",
    "binnedutcyearweekdayhours",
    "binnedutcyearweekdayhoursminutes",
    "binnedutcyearweekdayhoursminutesseconds",
    "binnedutcyeardayofyear",
];

/// `keys(BINNED_TIMEUNIT_INDEX)` in the upstream source.
pub const BINNED_TIMEUNIT_INDEX: [&str; 28] = [
    "binnedyear",
    "binnedyearquarter",
    "binnedyearquartermonth",
    "binnedyearmonth",
    "binnedyearmonthdate",
    "binnedyearmonthdatehours",
    "binnedyearmonthdatehoursminutes",
    "binnedyearmonthdatehoursminutesseconds",
    "binnedyearweek",
    "binnedyearweekday",
    "binnedyearweekdayhours",
    "binnedyearweekdayhoursminutes",
    "binnedyearweekdayhoursminutesseconds",
    "binnedyeardayofyear",
    "binnedutcyear",
    "binnedutcyearquarter",
    "binnedutcyearquartermonth",
    "binnedutcyearmonth",
    "binnedutcyearmonthdate",
    "binnedutcyearmonthdatehours",
    "binnedutcyearmonthdatehoursminutes",
    "binnedutcyearmonthdatehoursminutesseconds",
    "binnedutcyearweek",
    "binnedutcyearweekday",
    "binnedutcyearweekdayhours",
    "binnedutcyearweekdayhoursminutes",
    "binnedutcyearweekdayhoursminutesseconds",
    "binnedutcyeardayofyear",
];

// Compile-time check that the two lists above stay in sync.
const _: () = assert!(BINNED_TIMEUNIT_INDEX.len() == BINNED_LOCAL_TIMEUNIT_INDEX.len() + BINNED_UTC_TIMEUNIT_INDEX.len());

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Returns true if the time unit is binned (string or parameter object).
pub fn is_binned_time_unit(time_unit: &Value) -> bool {
    if let Value::Object(map) = time_unit {
        return map.get("binned").map(truthy).unwrap_or(false);
    }
    is_binned_time_unit_string(time_unit.as_str())
}

pub fn is_binned_time_unit_string(time_unit: Option<&str>) -> bool {
    time_unit.map(|t| t.starts_with("binned")).unwrap_or(false)
}

pub const UTC_MULTI_TIMEUNIT_INDEX: [&str; 29] = [
    "utcyearquarter",
    "utcyearquartermonth",
    "utcyearmonth",
    "utcyearmonthdate",
    "utcyearmonthdatehours",
    "utcyearmonthdatehoursminutes",
    "utcyearmonthdatehoursminutesseconds",
    "utcyearweek",
    "utcyearweekday",
    "utcyearweekdayhours",
    "utcyearweekdayhoursminutes",
    "utcyearweekdayhoursminutesseconds",
    "utcyeardayofyear",
    "utcquartermonth",
    "utcmonthdate",
    "utcmonthdatehours",
    "utcmonthdatehoursminutes",
    "utcmonthdatehoursminutesseconds",
    "utcweekday",
    "utcweekdayhours",
    "utcweekdayhoursminutes",
    "utcweekdayhoursminutesseconds",
    "utcdayhours",
    "utcdayhoursminutes",
    "utcdayhoursminutesseconds",
    "utchoursminutes",
    "utchoursminutesseconds",
    "utcminutesseconds",
    "utcsecondsmilliseconds",
];

pub fn is_utc_time_unit(t: &str) -> bool {
    t.starts_with("utc")
}

pub fn get_local_time_unit_from_utc_time_unit(t: &str) -> String {
    t.chars().skip(3).collect()
}

/// Parameters describing a time unit (port of `TimeUnitTransformParams` /
/// `TimeUnitParams`).  Optional fields record whether they were present.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeUnitParams {
    pub unit: Option<String>,
    pub maxbins: Option<Value>,
    pub step: Option<Value>,
    pub utc: Option<bool>,
    pub binned: Option<bool>,
}

impl TimeUnitParams {
    /// Optional properties other than `utc`, in the order of the upstream
    /// interface.
    fn rest_entries(&self) -> Vec<(&'static str, Value)> {
        let mut out = Vec::new();
        if let Some(unit) = &self.unit {
            out.push(("unit", Value::String(unit.clone())));
        }
        if let Some(maxbins) = &self.maxbins {
            out.push(("maxbins", maxbins.clone()));
        }
        if let Some(step) = &self.step {
            out.push(("step", step.clone()));
        }
        if let Some(binned) = self.binned {
            out.push(("binned", Value::Bool(binned)));
        }
        out
    }
}

/// In order of increasing specificity.
pub fn vegalite_timeformat() -> Value {
    json!({
        "year-month": "%b %Y ",
        "year-month-date": "%b %d, %Y ",
    })
}

pub fn get_time_unit_parts(time_unit: &str) -> Vec<&'static str> {
    TIMEUNIT_PARTS
        .iter()
        .copied()
        .filter(|part| contains_time_unit(time_unit, part))
        .collect()
}

pub fn get_smallest_time_unit_part(time_unit: &str) -> Option<&'static str> {
    get_time_unit_parts(time_unit).last().copied()
}

/// Returns true if `full_time_unit` contains `time_unit`.
pub fn contains_time_unit(full_time_unit: &str, time_unit: &str) -> bool {
    let full: Vec<char> = full_time_unit.chars().collect();

    let index = match full_time_unit.find(time_unit) {
        Some(byte_index) => full_time_unit[..byte_index].chars().count(),
        None => return false,
    };

    // exclude milliseconds
    if index > 0 && time_unit == "seconds" && full.get(index - 1) == Some(&'i') {
        return false;
    }

    // exclude dayofyear
    if full.len() > index + 3 && time_unit == "day" && full.get(index + 3) == Some(&'o') {
        return false;
    }
    if index > 0 && time_unit == "year" && full.get(index - 1) == Some(&'f') {
        return false;
    }

    true
}

/// Returns a Vega expression for a given time unit and field reference.
pub fn field_expr(full_time_unit: &str, field: &str) -> String {
    field_expr_with_options(full_time_unit, field, false, "datum")
}

pub fn field_expr_with_options(full_time_unit: &str, field: &str, end: bool, expr: &str) -> String {
    let field_ref = access_path_with_datum(field, expr);

    let utc = if is_utc_time_unit(full_time_unit) { "utc" } else { "" };

    let func = |time_unit: &str| {
        if time_unit == "quarter" {
            // quarter starting at 0 (0,3,6,9).
            format!("({utc}quarter({field_ref})-1)")
        } else {
            format!("{utc}{time_unit}({field_ref})")
        }
    };

    let mut last_time_unit: Option<&'static str> = None;
    let mut date_expr = DateTime::default();

    for part in TIMEUNIT_PARTS {
        if contains_time_unit(full_time_unit, part) {
            let value = Value::String(func(part));
            match part {
                "year" => date_expr.year = Some(value),
                "quarter" => date_expr.quarter = Some(value),
                "month" => date_expr.month = Some(value),
                "day" => date_expr.day = Some(value),
                "date" => date_expr.date = Some(value),
                "hours" => date_expr.hours = Some(value),
                "minutes" => date_expr.minutes = Some(value),
                "seconds" => date_expr.seconds = Some(value),
                "milliseconds" => date_expr.milliseconds = Some(value),
                _ => {}
            }
            last_time_unit = Some(part);
        }
    }

    if end {
        let slot = last_time_unit.and_then(|last| match last {
            "year" => date_expr.year.as_mut(),
            "quarter" => date_expr.quarter.as_mut(),
            "month" => date_expr.month.as_mut(),
            "day" => date_expr.day.as_mut(),
            "date" => date_expr.date.as_mut(),
            "hours" => date_expr.hours.as_mut(),
            "minutes" => date_expr.minutes.as_mut(),
            "seconds" => date_expr.seconds.as_mut(),
            "milliseconds" => date_expr.milliseconds.as_mut(),
            _ => None,
        });
        if let Some(Value::String(s)) = slot {
            s.push_str("+1");
        }
    }

    date_time_expr_to_expr(date_expr)
}

pub fn time_unit_specifier_expression(time_unit: &str) -> Option<String> {
    if time_unit.is_empty() {
        return None;
    }

    let parts = get_time_unit_parts(time_unit);
    let parts_value = Value::Array(parts.iter().map(|p| Value::String((*p).to_string())).collect());
    Some(format!(
        "timeUnitSpecifier({}, {})",
        stringify(&parts_value),
        stringify(&vegalite_timeformat())
    ))
}

/// Returns the signal expression used for axis labels for a time unit.
pub fn format_expression(time_unit: Option<&str>, field: &str, is_utc_scale: bool) -> Option<String> {
    let time_unit = time_unit?;
    if time_unit.is_empty() {
        return None;
    }

    let expr = time_unit_specifier_expression(time_unit)?;

    // We only use utcFormat for utc scale.  For utc time units, the data is
    // already converted as a part of the timeUnit transform.
    let utc = is_utc_scale || is_utc_time_unit(time_unit);

    Some(format!(
        "{}Format({}, {})",
        if utc { "utc" } else { "time" },
        field,
        expr
    ))
}

pub fn normalize_time_unit(time_unit: &Value) -> Option<TimeUnitParams> {
    if time_unit.is_null() {
        return None;
    }

    let mut params = match time_unit {
        Value::String(s) => {
            if s.is_empty() {
                return None;
            }
            if is_binned_time_unit_string(Some(s)) {
                TimeUnitParams {
                    unit: Some(s.chars().skip(6).collect()),
                    binned: Some(true),
                    ..Default::default()
                }
            } else {
                TimeUnitParams {
                    unit: Some(s.clone()),
                    ..Default::default()
                }
            }
        }
        Value::Object(map) => TimeUnitParams {
            unit: map.get("unit").and_then(|u| u.as_str()).map(|u| u.to_string()),
            maxbins: map.get("maxbins").filter(|v| !v.is_null()).cloned(),
            step: map.get("step").filter(|v| !v.is_null()).cloned(),
            utc: map.get("utc").filter(|v| !v.is_null()).map(truthy),
            binned: map.get("binned").filter(|v| !v.is_null()).map(truthy),
        },
        _ => return None,
    };

    if let Some(unit) = &params.unit {
        if is_utc_time_unit(unit) {
            params.utc = Some(true);
            params.unit = Some(get_local_time_unit_from_utc_time_unit(unit));
        }
    }

    Some(params)
}

fn js_value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.as_f64().map(js_number_to_string).unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => stringify(v),
    }
}

pub fn time_unit_to_string(tu: &Value) -> Option<String> {
    let params = normalize_time_unit(tu)?;
    let utc = params.utc.unwrap_or(false);
    let rest = params.rest_entries();

    if params.unit.is_some() {
        let mut result = if utc { "utc" } else { "" }.to_string();
        for (key, value) in rest {
            let prefix = if key == "unit" { String::new() } else { format!("_{key}_") };
            let name = format!("{prefix}{}", js_value_to_string(&value));
            result.push_str(&var_name(&name));
        }
        Some(result)
    } else {
        // when maxbins is specified instead of units
        let mut result = format!("{}timeunit", if utc { "utc" } else { "" });
        for (key, value) in rest {
            let name = format!("_{key}_{}", js_value_to_string(&value));
            result.push_str(&var_name(&name));
        }
        Some(result)
    }
}

fn number_value(n: f64) -> Value {
    Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
}

pub fn duration_expr(time_unit: &Value) -> Option<String> {
    duration_expr_with_wrap(time_unit, |x| x.to_string())
}

pub fn duration_expr_with_wrap<F>(time_unit: &Value, wrap: F) -> Option<String>
where
    F: Fn(&str) -> String,
{
    let normalized = normalize_time_unit(time_unit)?;
    let smallest_unit_part = get_smallest_time_unit_part(normalized.unit.as_deref()?)?;

    if smallest_unit_part != "day" {
        let start_date = DateTime::from(&json!({
            "year": 2001, // pick a non-leap year
            "month": 1,
            "date": 1,
            "hours": 0,
            "minutes": 0,
            "seconds": 0,
            "milliseconds": 0,
        }));

        let step = normalized
            .step
            .as_ref()
            .and_then(|s| s.as_f64())
            .unwrap_or(1.0);
        let (part, step) = get_date_time_part_and_step(smallest_unit_part, Some(step))?;

        let mut end_date = start_date.clone();
        let start_value = match part {
            "year" => start_date.year.as_ref(),
            "month" => start_date.month.as_ref(),
            "date" => start_date.date.as_ref(),
            "hours" => start_date.hours.as_ref(),
            "minutes" => start_date.minutes.as_ref(),
            "seconds" => start_date.seconds.as_ref(),
            "milliseconds" => start_date.milliseconds.as_ref(),
            _ => None,
        }
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
        let new_value = number_value(start_value + step);

        match part {
            "year" => end_date.year = Some(new_value),
            "month" => end_date.month = Some(new_value),
            "date" => end_date.date = Some(new_value),
            "hours" => end_date.hours = Some(new_value),
            "minutes" => end_date.minutes = Some(new_value),
            "seconds" => end_date.seconds = Some(new_value),
            "milliseconds" => end_date.milliseconds = Some(new_value),
            _ => {}
        }

        // Calculate timestamp duration for the smallest unit listed.
        return Some(format!(
            "{} - {}",
            wrap(&date_time_to_expr(end_date)),
            wrap(&date_time_to_expr(start_date))
        ));
    }

    None
}

const DATE_PARTS: [&str; 7] = [
    "year",
    "month",
    "date",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
];

pub fn is_date_part(time_unit: &str) -> bool {
    DATE_PARTS.contains(&time_unit)
}

pub fn get_date_time_part_and_step(
    time_unit: &str,
    step: Option<f64>,
) -> Option<(&'static str, f64)> {
    let step = step.unwrap_or(1.0);

    if is_date_part(time_unit) {
        let part = match time_unit {
            "year" => "year",
            "month" => "month",
            "date" => "date",
            "hours" => "hours",
            "minutes" => "minutes",
            "seconds" => "seconds",
            "milliseconds" => "milliseconds",
            _ => unreachable!(),
        };
        return Some((part, step));
    }

    match time_unit {
        "day" | "dayofyear" => Some(("date", step)),
        "quarter" => Some(("month", step * 3.0)),
        "week" => Some(("date", step * 7.0)),
        _ => None,
    }
}
