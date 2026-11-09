//! Shared helpers used by the data-transform compiler.
//!
//! These are faithful ports of the small `channeldef` / `bin` / `timeunit` /
//! `predicate` helpers that the dataflow nodes need.  Keeping them here avoids
//! depending on modules that are being ported concurrently by other agents.

use indexmap::IndexSet;
use serde_json::{json, Map, Value};

use crate::util::{
    access_with_datum_to_unescaped_path, has_property, hash, js_number_to_string, logical_expr,
    replace_path_in_field, string_value, stringify, var_name,
};

pub type FieldSet = IndexSet<String>;

pub fn set_of(values: impl IntoIterator<Item = String>) -> FieldSet {
    values.into_iter().collect()
}

pub fn set_json(set: &FieldSet) -> Value {
    // Mirrors the `Set.prototype.toJSON` monkey patch in `src/util.ts`.
    let inner = set
        .iter()
        .map(|x| stringify(&Value::String(x.clone())))
        .collect::<Vec<_>>()
        .join(",");
    Value::String(format!("Set({inner})"))
}

pub fn hash_str(value: &Value) -> String {
    match hash(value) {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        other => stringify(&other),
    }
}

/// Serialize a JavaScript number, using an integer representation when the
/// value is integral (matching `JSON.stringify(0)` rather than `0.0`).
pub fn number_value(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9e15 {
        Value::Number(serde_json::Number::from(n as i64))
    } else {
        Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)))
    }
}

// ---------------------------------------------------------------------------
// Aggregate / field-ref helpers
// ---------------------------------------------------------------------------

pub fn is_parameterized_aggregate(aggregate: &Value) -> bool {
    aggregate.is_object() && (has_property(aggregate, "exponential") || has_property(aggregate, "exponentialb"))
}

pub fn is_argmin_def(aggregate: &Value) -> bool {
    has_property(aggregate, "argmin")
}

pub fn is_argmax_def(aggregate: &Value) -> bool {
    has_property(aggregate, "argmax")
}

pub fn get_aggregate_op(aggregate: &Value) -> String {
    match aggregate {
        Value::String(s) => s.clone(),
        Value::Object(_) => {
            if has_property(aggregate, "exponential") {
                "exponential".to_string()
            } else if has_property(aggregate, "exponentialb") {
                "exponentialb".to_string()
            } else if let Some(Value::String(s)) = aggregate.get("op") {
                s.clone()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

pub fn get_aggregate_param(aggregate: &Value) -> Option<f64> {
    aggregate
        .get("exponential")
        .or_else(|| aggregate.get("exponentialb"))
        .and_then(Value::as_f64)
}

/// Port of `isAggregateOp` (`AGGREGATE_OP_INDEX`).
pub fn is_aggregate_op(op: &Value) -> bool {
    matches!(
        op.as_str(),
        Some(
            "count"
                | "valid"
                | "missing"
                | "distinct"
                | "sum"
                | "product"
                | "mean"
                | "average"
                | "variance"
                | "variancep"
                | "stdev"
                | "stdevp"
                | "stderr"
                | "median"
                | "q1"
                | "q3"
                | "ci0"
                | "ci1"
                | "min"
                | "max"
                | "argmin"
                | "argmax"
                | "exponential"
                | "exponentialb"
        )
    )
}

/// The subset of `vgField` needed by the dataflow nodes.
///
/// * `field` is the raw field name (possibly escaped).
/// * `fn_name` is the aggregate / time-unit / bin prefix.
/// * `suffix` is appended after the function prefix.
/// * `for_as` removes path accesses (dots).
pub fn vg_field(field: Option<&str>, fn_name: Option<&str>, suffix: Option<&str>, for_as: bool) -> String {
    let mut field = field.map(str::to_string);
    if let Some(f) = fn_name {
        field = Some(match field {
            Some(field) if !field.is_empty() => format!("{f}_{field}"),
            _ => f.to_string(),
        });
    }
    if let Some(suffix) = suffix {
        if !suffix.is_empty() {
            let f = field.take().unwrap_or_default();
            field = Some(format!("{f}_{suffix}"));
        }
    }
    let field = field.unwrap_or_default();
    if for_as {
        crate::util::remove_path_from_field(&field)
    } else {
        replace_path_in_field(&field)
    }
}

/// Default output name for an aggregate/window/joinaggregate field definition
/// (`vgField(def)` with no options).
pub fn default_op_field_name(def: &Value) -> String {
    let op = def
        .get("op")
        .map(get_aggregate_op)
        .or_else(|| def.get("aggregate").map(get_aggregate_op))
        .unwrap_or_default();
    match def.get("field").and_then(Value::as_str) {
        Some(field) if !field.is_empty() => format!("{op}_{field}"),
        _ => op,
    }
}

pub fn default_name(def: &Value) -> String {
    match def.get("as").and_then(Value::as_str) {
        Some(as_) => as_.to_string(),
        None => default_op_field_name(def),
    }
}

// ---------------------------------------------------------------------------
// Time units
// ---------------------------------------------------------------------------

const TIMEUNIT_PARTS: [&str; 11] = [
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

pub fn is_binned_time_unit(time_unit: &Value) -> bool {
    match time_unit {
        Value::Object(o) => o.get("binned").and_then(Value::as_bool).unwrap_or(false),
        Value::String(s) => s.starts_with("binned"),
        _ => false,
    }
}

pub fn is_utc_time_unit(t: &str) -> bool {
    t.starts_with("utc")
}

/// Port of `normalizeTimeUnit`.
pub fn normalize_time_unit(time_unit: &Value) -> Option<Value> {
    if time_unit.is_null() {
        return None;
    }

    let mut params: Map<String, Value> = match time_unit {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix("binned") {
                let mut m = Map::new();
                m.insert("unit".into(), Value::String(rest.to_string()));
                m.insert("binned".into(), Value::Bool(true));
                m
            } else {
                let mut m = Map::new();
                m.insert("unit".into(), Value::String(s.clone()));
                m
            }
        }
        Value::Object(o) => o.clone(),
        _ => return None,
    };

    if let Some(Value::String(unit)) = params.get("unit").cloned() {
        if is_utc_time_unit(&unit) {
            params.insert("utc".into(), Value::Bool(true));
            params.insert("unit".into(), Value::String(unit[3..].to_string()));
        }
    }

    Some(Value::Object(params))
}

pub fn contains_time_unit(full: &str, part: &str) -> bool {
    let Some(index) = full.find(part) else {
        return false;
    };
    let bytes = full.as_bytes();
    // exclude milliseconds
    if index > 0 && part == "seconds" && bytes.get(index - 1) == Some(&b'i') {
        return false;
    }
    // exclude dayofyear
    if full.len() > index + 3 && part == "day" && bytes.get(index + 3) == Some(&b'o') {
        return false;
    }
    // exclude financial year
    if index > 0 && part == "year" && bytes.get(index - 1) == Some(&b'f') {
        return false;
    }
    true
}

pub fn get_time_unit_parts(unit: &str) -> Vec<String> {
    TIMEUNIT_PARTS
        .iter()
        .filter(|part| contains_time_unit(unit, part))
        .map(|s| s.to_string())
        .collect()
}

pub fn get_smallest_time_unit_part(unit: &str) -> Option<String> {
    get_time_unit_parts(unit).pop()
}

pub fn get_date_time_part_and_step(time_unit: &str, step: f64) -> (&'static str, f64) {
    match time_unit {
        "year" => ("year", step),
        "month" => ("month", step),
        "date" => ("date", step),
        "hours" => ("hours", step),
        "minutes" => ("minutes", step),
        "seconds" => ("seconds", step),
        "milliseconds" => ("milliseconds", step),
        "day" | "dayofyear" => ("date", step),
        "quarter" => ("month", step * 3.0),
        "week" => ("date", step * 7.0),
        _ => ("date", step),
    }
}

/// Port of `timeUnitToString`.
pub fn time_unit_to_string(time_unit: &Value) -> String {
    let Some(normalized) = normalize_time_unit(time_unit) else {
        return String::new();
    };
    let utc = normalized.get("utc").and_then(Value::as_bool).unwrap_or(false);
    let mut parts = String::new();
    let mut has_unit = false;
    for (k, v) in normalized.as_object().unwrap() {
        if k == "utc" {
            continue;
        }
        if k == "unit" {
            has_unit = true;
            parts.push_str(&var_name(&js_value_string(v)));
        } else {
            parts.push_str(&var_name(&format!("_{}_{}", k, js_value_string(v))));
        }
    }
    if has_unit {
        format!("{}{}", if utc { "utc" } else { "" }, parts)
    } else {
        format!("{}timeunit{}", if utc { "utc" } else { "" }, parts)
    }
}

/// Vega expression for a binned time-unit offset.
pub fn offset_expr(time_unit: &Value, field: &str, reverse: bool) -> String {
    let normalized = normalize_time_unit(time_unit).unwrap_or_default();
    let unit = normalized.get("unit").and_then(Value::as_str).unwrap_or("");
    let utc = normalized.get("utc").and_then(Value::as_bool).unwrap_or(false);
    let step = normalized.get("step").and_then(Value::as_f64).unwrap_or(1.0);
    let smallest = get_smallest_time_unit_part(unit).unwrap_or_else(|| "date".to_string());
    let (part, step) = get_date_time_part_and_step(&smallest, step);
    let offset_fn = if utc { "utcOffset" } else { "timeOffset" };
    let signed_step = if reverse { -step } else { step };
    format!(
        "{offset_fn}('{part}', {}, {})",
        access_with_datum_to_unescaped_path(field),
        js_number_to_string(signed_step)
    )
}

pub fn interpolate_expr(start: &str, end: &str, fraction: f64) -> String {
    format!(
        "{} * {start} + {} * {end}",
        js_number_to_string(1.0 - fraction),
        js_number_to_string(fraction)
    )
}

pub const OFFSETTED_RECT_START_SUFFIX: &str = "offsetted_rect_start";
pub const OFFSETTED_RECT_END_SUFFIX: &str = "offsetted_rect_end";

// ---------------------------------------------------------------------------
// Binning
// ---------------------------------------------------------------------------

pub const BIN_RANGE_DELIMITER: &str = " \u{2013} ";

/// JavaScript-ish string conversion used by `binToString` (`String(value)`).
pub fn js_value_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.as_f64().map(js_number_to_string).unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr.iter().map(js_value_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub fn is_binning(bin: &Value) -> bool {
    match bin {
        Value::Bool(b) => *b,
        Value::Object(o) => !o.get("binned").and_then(Value::as_bool).unwrap_or(false),
        _ => false,
    }
}

pub fn is_parameter_extent(extent: &Value) -> bool {
    has_property(extent, "param")
}

/// Port of `normalizeBin` with `channel = undefined` (`autoMaxBins` = 10).
pub fn normalize_bin(bin: &Value) -> Map<String, Value> {
    match bin {
        Value::Bool(_) => {
            let mut m = Map::new();
            m.insert("maxbins".into(), json!(10));
            m
        }
        Value::String(s) if s == "binned" => {
            let mut m = Map::new();
            m.insert("binned".into(), json!(true));
            m
        }
        Value::Object(o) => {
            let mut m = o.clone();
            let has_maxbins = m.get("maxbins").map(|v| !v.is_null()).unwrap_or(false);
            let has_step = m.get("step").map(|v| !v.is_null()).unwrap_or(false);
            if !has_maxbins && !has_step {
                m.insert("maxbins".into(), json!(10));
            }
            m
        }
        _ => Map::new(),
    }
}

// Canonical bin-property order.  JavaScript objects preserve insertion order,
// but `serde_json::Map` (without the `preserve_order` feature) sorts keys.
// Using the order in which bin properties are documented keeps signal names
// identical to the upstream implementation for the supported properties.

/// Port of `binToString`.
pub fn bin_to_string(bin: &Map<String, Value>) -> String {
    let mut out = String::from("bin");
    for (p, v) in bin {
        if is_parameter_extent(v) {
            let mut entries = String::new();
            if let Some(o) = v.as_object() {
                let mut ordered: Vec<(&String, &Value)> = Vec::new();
                for key in ["param", "field", "encoding"] {
                    if let Some((k, val)) = o.get_key_value(key) {
                        ordered.push((k, val));
                    }
                }
                for (k, val) in o {
                    if !["param", "field", "encoding"].contains(&k.as_str()) {
                        ordered.push((k, val));
                    }
                }
                entries = ordered
                    .iter()
                    .map(|(k, val)| format!("{},{}", k, js_value_string(val)))
                    .collect::<Vec<_>>()
                    .join(",");
            }
            out.push_str(&var_name(&format!("_{p}_{entries}")));
        } else {
            out.push_str(&var_name(&format!("_{p}_{}", js_value_string(v))));
        }
    }
    out
}

pub fn bin_key(bin: &Map<String, Value>, field: &str) -> String {
    format!("{}_{}", bin_to_string(bin), field)
}

/// Minimal model interface used for signal naming / selection extents.
pub trait Model {
    fn name(&self, text: &str) -> String {
        var_name(text)
    }

    /// Port of `parseSelectionExtent`.  The default treats the parameter as a
    /// variable parameter (returns its variable name).  Selection materialisation
    /// is out of scope for this port, so tests can override this when needed.
    fn selection_extent(&self, name: &str, _extent: &Value) -> String {
        var_name(name)
    }

    fn selection_is_known(&self, _name: &str) -> bool {
        false
    }
}

/// A model with no name prefix and no selection components.
pub struct SimpleModel;

impl Model for SimpleModel {}

// ---------------------------------------------------------------------------
// DateTime / predicates
// ---------------------------------------------------------------------------

const MONTHS: [&str; 12] = [
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december",
];

fn normalize_month(month: &Value) -> Value {
    if let Some(s) = month.as_str() {
        let lower = s.to_lowercase();
        if let Some(i) = MONTHS.iter().position(|m| *m == lower || m[..3] == lower) {
            return json!(i);
        }
        if let Ok(n) = lower.parse::<f64>() {
            return json!(n - 1.0);
        }
    }
    month.clone()
}

fn normalize_day(day: &Value) -> Value {
    const DAYS: [&str; 7] = [
        "sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday",
    ];
    if let Some(s) = day.as_str() {
        let lower = s.to_lowercase();
        if let Some(i) = DAYS.iter().position(|d| *d == lower || d[..3] == lower) {
            return json!(i);
        }
        if let Ok(n) = lower.parse::<f64>() {
            return json!(n);
        }
    }
    day.clone()
}

pub fn is_date_time(value: &Value) -> bool {
    if let Some(o) = value.as_object() {
        return TIMEUNIT_PARTS.iter().any(|p| has_property(&Value::Object(o.clone()), p));
    }
    false
}

fn date_time_parts(d: &Map<String, Value>, normalize: bool) -> Vec<String> {
    let mut parts = Vec::new();

    if let Some(year) = d.get("year") {
        parts.push(js_value_string(year));
    } else {
        parts.push("2012".into());
    }

    if let Some(month) = d.get("month") {
        let normalized_month = if normalize { normalize_month(month) } else { month.clone() };
        parts.push(js_value_string(&normalized_month));
    } else if let Some(quarter) = d.get("quarter") {
        let q = if normalize { normalize_quarter(quarter) } else { quarter.clone() };
        if let Some(n) = q.as_f64() {
            parts.push(js_number_to_string(n * 3.0));
        } else {
            parts.push(format!("{}*3", js_value_string(&q)));
        }
    } else {
        parts.push("0".into());
    }

    if let Some(date) = d.get("date") {
        parts.push(js_value_string(date));
    } else if let Some(day) = d.get("day") {
        let day = if normalize { normalize_day(day) } else { day.clone() };
        if let Some(n) = day.as_f64() {
            parts.push(js_number_to_string(n + 1.0));
        } else {
            parts.push(format!("{}+1", js_value_string(&day)));
        }
    } else {
        parts.push("1".into());
    }

    for unit in ["hours", "minutes", "seconds", "milliseconds"] {
        parts.push(d.get(unit).map(js_value_string).unwrap_or_else(|| "0".into()));
    }

    parts
}

fn normalize_quarter(q: &Value) -> Value {
    let n = q
        .as_f64()
        .or_else(|| q.as_str().and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0);
    json!(n - 1.0)
}

pub fn date_time_to_expr(d: &Value) -> String {
    let map = d.as_object().cloned().unwrap_or_default();
    let parts = date_time_parts(&map, true).join(", ");
    if map.get("utc").and_then(Value::as_bool).unwrap_or(false) {
        format!("utc({parts})")
    } else {
        format!("datetime({parts})")
    }
}

/// Port of `channeldef.valueExpr` for the cases needed by predicates.
pub fn value_expr(v: &Value, time_unit: Option<&Value>, wrap_time: bool) -> String {
    let unit = time_unit
        .and_then(normalize_time_unit)
        .and_then(|n| n.get("unit").and_then(Value::as_str).map(str::to_string));
    let mut is_time = unit.is_some();

    let mut expr: Option<String> = None;
    if let Some(e) = v.get("expr").and_then(Value::as_str) {
        expr = Some(e.to_string());
    } else if let Some(s) = v.get("signal").and_then(Value::as_str) {
        expr = Some(s.to_string());
    } else if is_date_time(v) {
        is_time = true;
        expr = Some(date_time_to_expr(v));
    } else if (v.is_string() || v.is_number())
        && is_time {
            expr = Some(format!("datetime({})", stringify(v)));
            if let Some(unit) = &unit {
                let numeric_small = v.as_f64().map(|n| n < 10000.0).unwrap_or(false);
                let string_nan = v
                    .as_str()
                    .map(crate::channeldef_support::date_parse_is_nan)
                    .unwrap_or(false);
                if numeric_small || string_nan {
                    let mut m = Map::new();
                    m.insert(unit.clone(), v.clone());
                    expr = Some(date_time_to_expr(&Value::Object(m)));
                }
            }
        }

    match expr {
        Some(e) if wrap_time && is_time => format!("time({e})"),
        Some(e) => e,
        None => stringify(v),
    }
}

fn is_field_equal(p: &Value) -> bool {
    has_property(p, "field") && p.get("equal").is_some()
}
fn is_field_lt(p: &Value) -> bool {
    has_property(p, "field") && p.get("lt").is_some()
}
fn is_field_gt(p: &Value) -> bool {
    has_property(p, "field") && p.get("gt").is_some()
}
fn is_field_lte(p: &Value) -> bool {
    has_property(p, "field") && p.get("lte").is_some()
}
fn is_field_gte(p: &Value) -> bool {
    has_property(p, "field") && p.get("gte").is_some()
}
fn is_field_one_of(p: &Value) -> bool {
    has_property(p, "field") && (p.get("oneOf").is_some() || p.get("in").is_some())
}
fn is_field_range(p: &Value) -> bool {
    has_property(p, "field")
        && p.get("range")
            .map(|r| r.is_array() && r.as_array().map(|a| a.len() == 2).unwrap_or(false) || r.is_object())
            .unwrap_or(false)
}
fn is_field_valid(p: &Value) -> bool {
    has_property(p, "field") && p.get("valid").is_some()
}

pub fn field_valid_predicate(field_expr: &str, valid: bool) -> String {
    if valid {
        format!("isValid({field_expr}) && isFinite(+{field_expr})")
    } else {
        format!("!isValid({field_expr}) || !isFinite(+{field_expr})")
    }
}

/// Port of `fieldFilterExpression`.
pub fn field_filter_expression(predicate: &Value) -> String {
    let field = predicate
        .get("field")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let time_unit = predicate.get("timeUnit");
    let normalized = time_unit.and_then(normalize_time_unit);
    let unit = normalized
        .as_ref()
        .and_then(|n| n.get("unit").and_then(Value::as_str));
    let binned = normalized
        .as_ref()
        .and_then(|n| n.get("binned").and_then(Value::as_bool))
        .unwrap_or(false);

    let raw_field_expr = flat_access_with_datum(field);
    let field_expr = if let Some(unit) = unit {
        if !binned {
            format!(
                "time({})",
                time_unit_field_expr(unit, field)
            )
        } else {
            format!("time({raw_field_expr})")
        }
    } else {
        raw_field_expr
    };

    let pv = |v: &Value| value_expr(v, time_unit, true);

    if is_field_equal(predicate) {
        format!("{field_expr}==={}", pv(&predicate["equal"]))
    } else if is_field_lt(predicate) {
        format!("{field_expr}<{}", pv(&predicate["lt"]))
    } else if is_field_gt(predicate) {
        format!("{field_expr}>{}", pv(&predicate["gt"]))
    } else if is_field_lte(predicate) {
        format!("{field_expr}<={}", pv(&predicate["lte"]))
    } else if is_field_gte(predicate) {
        format!("{field_expr}>={}", pv(&predicate["gte"]))
    } else if is_field_one_of(predicate) {
        let key = if predicate.get("oneOf").is_some() { "oneOf" } else { "in" };
        let values = predicate[key]
            .as_array()
            .map(|a| a.iter().map(pv).collect::<Vec<_>>().join(","))
            .unwrap_or_default();
        format!("indexof([{values}], {field_expr}) !== -1")
    } else if is_field_valid(predicate) {
        field_valid_predicate(&field_expr, predicate["valid"].as_bool().unwrap_or(true))
    } else if is_field_range(predicate) {
        let range = &predicate["range"];
        if range.is_object() {
            return field_expr;
        }
        let arr = range.as_array().cloned().unwrap_or_default();
        let lower = arr.first().cloned().unwrap_or(Value::Null);
        let upper = arr.get(1).cloned().unwrap_or(Value::Null);
        if !lower.is_null() && !upper.is_null() {
            format!("inrange({field_expr}, [{}, {}])", pv(&lower), pv(&upper))
        } else {
            let mut exprs = Vec::new();
            if !lower.is_null() {
                exprs.push(format!("{field_expr} >= {}", pv(&lower)));
            }
            if !upper.is_null() {
                exprs.push(format!("{field_expr} <= {}", pv(&upper)));
            }
            if exprs.is_empty() {
                "true".to_string()
            } else {
                exprs.join(" && ")
            }
        }
    } else {
        format!("Invalid field predicate: {}", stringify(predicate))
    }
}

fn flat_access_with_datum(field: &str) -> String {
    format!(
        "datum[{}]",
        string_value(&Value::String(
            crate::util::split_access_path(field).join(".")
        ))
    )
}

fn time_unit_field_expr(unit: &str, field: &str) -> String {
    crate::timeunit::field_expr_with_options(unit, field, false, "datum")
}

/// Convert a predicate (or logical composition) to a Vega expression.
pub fn expression(filter: &Value) -> String {
    logical_expr(filter, &mut |predicate| {
        if predicate.is_string() {
            predicate.as_str().unwrap().to_string()
        } else {
            field_filter_expression(predicate)
        }
    })
}

// ---------------------------------------------------------------------------
// Expression dependency analysis
// ---------------------------------------------------------------------------

/// Approximate port of `getDependentFields` from `vega-expression` AST walking.
/// It finds `datum.foo`, `datum["foo"]` and `datum['foo']` accesses.
pub fn get_dependent_fields(expr: &str) -> FieldSet {
    let bytes = expr.as_bytes();
    let mut out = FieldSet::new();
    let mut i = 0;
    while i + 5 <= bytes.len() {
        if &bytes[i..i + 5] == b"datum" {
            let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b'.');
            if prev_ok {
                let mut j = i + 5;
                let mut parts: Vec<String> = Vec::new();
                loop {
                    if j < bytes.len() && bytes[j] == b'.' {
                        j += 1;
                        let start = j;
                        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                            j += 1;
                        }
                        if j > start {
                            parts.push(expr[start..j].to_string());
                        } else {
                            break;
                        }
                    } else if j < bytes.len() && bytes[j] == b'[' {
                        j += 1;
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                            let quote = bytes[j];
                            j += 1;
                            let start = j;
                            while j < bytes.len() && bytes[j] != quote {
                                j += 1;
                            }
                            let mut s = expr[start..j].to_string();
                            s = s.replace("\\'", "'").replace("\\\"", "\"");
                            parts.push(s);
                            j += 1;
                            while j < bytes.len() && bytes[j] != b']' {
                                j += 1;
                            }
                            j += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if !parts.is_empty() {
                    out.insert(parts.join("."));
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Mark / channel helpers (subset)
// ---------------------------------------------------------------------------

pub fn is_rect_based_mark(mark: &str) -> bool {
    matches!(mark, "rect" | "bar" | "image" | "arc" | "tick")
}

pub fn is_xor_y(channel: &str) -> bool {
    channel == "x" || channel == "y"
}

/// Port of `getBandPosition` for the cases needed by the dataflow nodes.
pub fn get_band_position(field_def: &Value, mark: &str) -> Option<f64> {
    if let Some(bp) = field_def.get("bandPosition").and_then(Value::as_f64) {
        return Some(bp);
    }
    if field_def.get("timeUnit").is_some() {
        // defaultBarConfig / defaultRectConfig set `timeUnitBandPosition` to 0.5
        return Some(0.5);
    }
    if is_binning(field_def.get("bin").unwrap_or(&Value::Null)) {
        return Some(0.5);
    }
    let _ = mark;
    None
}

pub fn bin_requires_range(field_def: &Value, channel: &str) -> bool {
    if !is_binning(field_def.get("bin").unwrap_or(&Value::Null)) {
        return false;
    }
    let is_scale_channel = !matches!(
        channel,
        "row" | "column" | "facet" | "shape" | "text" | "tooltip" | "href" | "url" | "description"
    );
    let ty = field_def.get("type").and_then(Value::as_str).unwrap_or("");
    is_scale_channel && (ty == "ordinal" || ty == "nominal")
}
