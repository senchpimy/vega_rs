//! Helper implementations used by [`crate::channeldef`].
//!
//! These mirror the parts of `channel.ts`, `bin.ts`, `scale.ts`, `mark.ts`,
//! `aggregate.ts`, `timeunit.ts`, `datetime.ts`, `sort.ts` and
//! `compile/common.ts` that `channeldef.ts` depends on.  They live in a single
//! support module so that `channeldef` compiles independently of the other
//! (separately ported) modules.

use serde_json::{json, Map, Value};

use crate::util::{has_property, internal_field, js_number_to_string, stringify, var_name};

// ---------------------------------------------------------------------------
// Generic helpers
// ---------------------------------------------------------------------------

/// JavaScript truthiness for a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub fn is_expr_ref(v: &Value) -> bool {
    has_property(v, "expr")
}

pub fn is_signal_ref(v: &Value) -> bool {
    has_property(v, "signal")
}

pub fn is_object(v: &Value) -> bool {
    v.is_object()
}

/// `JS` string conversion for field names / values.
pub fn js_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Number(n) => js_number_to_string(n.as_f64().unwrap_or(0.0)),
        _ => stringify(v),
    }
}

// ---------------------------------------------------------------------------
// config.ts
// ---------------------------------------------------------------------------

/// Subset of `defaultConfig` relevant to `channeldef`.
pub fn default_config() -> Value {
    json!({
        "background": "white",
        "padding": 5,
        "timeFormat": "%b %d, %Y",
        "countTitle": "Count of Records",
        "view": {"continuousWidth": 300, "continuousHeight": 300},
        "mark": {},
        "arc": {},
        "area": {},
        "bar": {},
        "circle": {},
        "geoshape": {},
        "image": {},
        "line": {},
        "point": {},
        "rect": {},
        "rule": {"color": "black"},
        "square": {},
        "text": {"color": "black"},
        "tick": {},
        "trail": {}
    })
}

// ---------------------------------------------------------------------------
// channel.ts
// ---------------------------------------------------------------------------

pub const CHANNELS: &[&str] = &[
    "x",
    "y",
    "x2",
    "y2",
    "xOffset",
    "yOffset",
    "theta",
    "theta2",
    "radius",
    "radius2",
    "latitude",
    "longitude",
    "latitude2",
    "longitude2",
    "color",
    "fill",
    "stroke",
    "time",
    "opacity",
    "fillOpacity",
    "strokeOpacity",
    "strokeWidth",
    "strokeDash",
    "size",
    "angle",
    "shape",
    "order",
    "text",
    "detail",
    "key",
    "tooltip",
    "href",
    "url",
    "description",
    "row",
    "column",
    "facet",
];

pub fn is_channel(channel: &str) -> bool {
    CHANNELS.contains(&channel)
}

const SECONDARY_RANGE_CHANNELS: &[&str] = &["x2", "y2", "latitude2", "longitude2", "theta2", "radius2"];

pub fn is_secondary_range_channel(channel: &str) -> bool {
    SECONDARY_RANGE_CHANNELS.contains(&channel)
}

pub fn is_xor_y(channel: &str) -> bool {
    channel == "x" || channel == "y"
}

const SCALE_CHANNELS: &[&str] = &[
    "x",
    "y",
    "theta",
    "radius",
    "xOffset",
    "yOffset",
    "color",
    "fill",
    "stroke",
    "opacity",
    "fillOpacity",
    "strokeOpacity",
    "strokeWidth",
    "strokeDash",
    "size",
    "angle",
    "shape",
];

pub fn is_scale_channel(channel: &str) -> bool {
    SCALE_CHANNELS.contains(&channel)
}

/// Port of `getSizeChannel`.
pub fn get_size_channel(channel: &str) -> Option<&'static str> {
    match channel {
        "x" | "x2" => Some("width"),
        "y" | "y2" => Some("height"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// bin.ts
// ---------------------------------------------------------------------------

/// Port of `autoMaxBins`.
pub fn auto_max_bins(channel: Option<&str>) -> i64 {
    match channel {
        Some("row")
        | Some("column")
        | Some("size")
        | Some("color")
        | Some("fill")
        | Some("stroke")
        | Some("strokeWidth")
        | Some("opacity")
        | Some("fillOpacity")
        | Some("strokeOpacity")
        | Some("shape") => 6,
        Some("strokeDash") => 4,
        _ => 10,
    }
}

pub fn is_bin_params(bin: &Value) -> bool {
    bin.is_object()
}

/// Port of `isBinning`.
pub fn is_binning(bin: &Value) -> bool {
    (*bin == Value::Bool(true)) || (is_bin_params(bin) && !truthy(&bin["binned"]))
}

/// Port of `isBinned`.
pub fn is_binned(bin: &Value) -> bool {
    bin.as_str() == Some("binned") || (is_bin_params(bin) && bin.get("binned") == Some(&Value::Bool(true)))
}

/// Port of `binToString`.
pub fn bin_to_string(bin: &Value) -> String {
    let bin = if bin.is_boolean() {
        crate::channeldef::normalize_bin(bin, None)
    } else {
        bin.clone()
    };
    let map = bin.as_object();
    let mut out = String::from("bin");
    if let Some(map) = map {
        for (p, v) in map {
            if v.is_object() && v.get("param").is_some() {
                // Port of `varName(`_${p}_${entries(v)}`)`: flatten the
                // parameter extent instead of stringifying it as JSON.
                let mut entries = String::new();
                if let Some(obj) = v.as_object() {
                    let mut ordered: Vec<(&String, &Value)> = Vec::new();
                    for key in ["param", "field", "encoding"] {
                        if let Some((k, val)) = obj.get_key_value(key) {
                            ordered.push((k, val));
                        }
                    }
                    for (k, val) in obj {
                        if !["param", "field", "encoding"].contains(&k.as_str()) {
                            ordered.push((k, val));
                        }
                    }
                    entries = ordered
                        .iter()
                        .map(|(k, val)| format!("{},{}", k, js_to_string_no_quotes(val)))
                        .collect::<Vec<_>>()
                        .join(",");
                }
                out.push_str(&var_name(&format!("_{p}_{entries}")));
            } else {
                out.push_str(&var_name(&format!("_{p}_{}", js_to_string(v))));
            }
        }
    }
    out
}

/// Stringify a value the way `Array.prototype.join` / vega-util `entries`
/// does: strings are emitted verbatim, without JSON quotes.
fn js_to_string_no_quotes(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => js_to_string(other),
    }
}

// ---------------------------------------------------------------------------
// scale.ts
// ---------------------------------------------------------------------------

/// Port of `SCALE_CATEGORY_INDEX` lookup.
pub fn scale_category(scale_type: &str) -> Option<&'static str> {
    match scale_type {
        "linear" | "log" | "pow" | "sqrt" | "symlog" | "identity" | "sequential" => Some("numeric"),
        "time" | "utc" => Some("time"),
        "ordinal" => Some("ordinal"),
        "bin-ordinal" => Some("bin-ordinal"),
        "point" | "band" => Some("ordinal-position"),
        "quantile" | "quantize" | "threshold" => Some("discretizing"),
        _ => None,
    }
}

/// Port of `hasDiscreteDomain`.
pub fn has_discrete_domain(scale_type: &str) -> bool {
    matches!(scale_type, "ordinal" | "bin-ordinal" | "point" | "band")
}

/// Port of `isContinuousToDiscrete`.
pub fn is_continuous_to_discrete(scale_type: &str) -> bool {
    matches!(scale_type, "quantile" | "quantize" | "threshold")
}

// ---------------------------------------------------------------------------
// mark.ts
// ---------------------------------------------------------------------------

const RECT_BASED_MARKS: &[&str] = &["rect", "bar", "image", "arc", "tick"];

pub fn is_rect_based_mark(mark: &str) -> bool {
    RECT_BASED_MARKS.contains(&mark)
}

// ---------------------------------------------------------------------------
// aggregate.ts
// ---------------------------------------------------------------------------

const AGGREGATE_OPS: &[&str] = &[
    "argmax",
    "argmin",
    "average",
    "count",
    "distinct",
    "exponential",
    "exponentialb",
    "product",
    "max",
    "mean",
    "median",
    "min",
    "missing",
    "q1",
    "q3",
    "ci0",
    "ci1",
    "stderr",
    "stdev",
    "stdevp",
    "sum",
    "valid",
    "values",
    "variance",
    "variancep",
];

pub const COUNTING_OPS: &[&str] = &["count", "valid", "missing", "distinct"];

pub fn is_argmin_def(a: &Value) -> bool {
    has_property(a, "argmin")
}

pub fn is_argmax_def(a: &Value) -> bool {
    has_property(a, "argmax")
}

pub fn is_exponential_def(a: &Value) -> bool {
    has_property(a, "exponential")
}

pub fn is_exponentialb_def(a: &Value) -> bool {
    has_property(a, "exponentialb")
}

pub fn is_parameterized_aggregate_def(a: &Value) -> bool {
    is_exponential_def(a) || is_exponentialb_def(a)
}

/// Port of `getAggregateOp`. Returns `None` where TS returns `undefined`.
pub fn get_aggregate_op(a: &Value) -> Option<String> {
    if is_exponential_def(a) {
        Some("exponential".to_string())
    } else if is_exponentialb_def(a) {
        Some("exponentialb".to_string())
    } else { a.as_str().map(|s| s.to_string()) }
}

pub fn is_aggregate_op(a: &Value) -> bool {
    a.as_str().map(|s| AGGREGATE_OPS.contains(&s)).unwrap_or(false)
}

pub fn is_counting_aggregate_op(a: &Value) -> bool {
    a.as_str().map(|s| COUNTING_OPS.contains(&s)).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// timeunit.ts
// ---------------------------------------------------------------------------

pub const TIMEUNIT_PARTS: &[&str] = &[
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

pub fn is_local_single_time_unit(unit: &str) -> bool {
    TIMEUNIT_PARTS.contains(&unit)
}

pub fn is_utc_time_unit(t: &str) -> bool {
    t.starts_with("utc")
}

fn is_binned_time_unit_string(t: Option<&str>) -> bool {
    t.map(|s| s.starts_with("binned")).unwrap_or(false)
}

/// Port of `isBinnedTimeUnit`.
pub fn is_binned_time_unit(time_unit: &Value) -> bool {
    if time_unit.is_object() {
        truthy(&time_unit["binned"])
    } else {
        is_binned_time_unit_string(time_unit.as_str())
    }
}

fn contains_time_unit(full: &str, unit: &str) -> bool {
    let index = match full.find(unit) {
        Some(i) => i,
        None => return false,
    };
    let bytes = full.as_bytes();
    if index > 0 && unit == "seconds" && bytes.get(index - 1) == Some(&b'i') {
        return false;
    }
    if full.len() > index + 3 && unit == "day" && bytes.get(index + 3) == Some(&b'o') {
        return false;
    }
    if index > 0 && unit == "year" && bytes.get(index - 1) == Some(&b'f') {
        return false;
    }
    true
}

/// Port of `getTimeUnitParts`.
pub fn get_time_unit_parts(time_unit: &str) -> Vec<String> {
    TIMEUNIT_PARTS
        .iter()
        .filter(|part| contains_time_unit(time_unit, part))
        .map(|s| s.to_string())
        .collect()
}

/// Port of `normalizeTimeUnit`. Returns `Value::Null` where TS returns `undefined`.
pub fn normalize_time_unit(time_unit: &Value) -> Value {
    if !truthy(time_unit) {
        return Value::Null;
    }
    let mut params = if let Some(s) = time_unit.as_str() {
        if is_binned_time_unit_string(Some(s)) {
            json!({"unit": &s[6..], "binned": true})
        } else {
            json!({"unit": s})
        }
    } else if let Some(map) = time_unit.as_object() {
        let mut m = map.clone();
        if let Some(unit) = map.get("unit") {
            if truthy(unit) {
                m.insert("unit".to_string(), unit.clone());
            }
        }
        Value::Object(m)
    } else {
        return Value::Null;
    };

    let unit = params.get("unit").and_then(|u| u.as_str()).map(str::to_string);
    if let Some(unit) = unit {
        if is_utc_time_unit(&unit) {
            params["utc"] = Value::Bool(true);
            params["unit"] = Value::String(unit[3..].to_string());
        }
    }
    params
}

/// Port of `timeUnitToString`.
pub fn time_unit_to_string(tu: &Value) -> String {
    let normalized = normalize_time_unit(tu);
    let mut rest: Map<String, Value> = normalized.as_object().cloned().unwrap_or_default();
    let utc = rest.remove("utc").map(|v| truthy(&v)).unwrap_or(false);
    let prefix = if utc { "utc" } else { "" };

    if truthy(rest.get("unit").unwrap_or(&Value::Null)) {
        let mut out = String::from(prefix);
        for (p, v) in &rest {
            let s = if p == "unit" {
                js_to_string(v).to_string()
            } else {
                format!("_{}_{}", p, js_to_string(v))
            };
            out.push_str(&var_name(&s));
        }
        out
    } else {
        let mut out = format!("{prefix}timeunit");
        for (p, v) in &rest {
            out.push_str(&var_name(&format!("_{}_{}", p, js_to_string(v))));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// datetime.ts
// ---------------------------------------------------------------------------

pub const MONTHS: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

pub const DAYS: &[&str] = &[
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// Port of `isDateTime`.
pub fn is_date_time(o: &Value) -> bool {
    if o.is_object() {
        for part in TIMEUNIT_PARTS {
            if has_property(o, part) {
                return true;
            }
        }
    }
    false
}

fn normalize_month(m: &Value) -> f64 {
    if let Some(n) = as_numeric(m) {
        return n - 1.0;
    }
    let s = js_to_string(m).to_lowercase();
    if let Some(i) = MONTHS.iter().position(|x| *x == s) {
        return i as f64;
    }
    let short = &s[..s.len().min(3)];
    if let Some(i) = MONTHS.iter().position(|x| &x[..x.len().min(3)] == short) {
        return i as f64;
    }
    panic!("{}", crate::log::message::invalid_time_unit("month", m));
}

fn normalize_day(d: &Value) -> f64 {
    if let Some(n) = as_numeric(d) {
        return n % 7.0;
    }
    let s = js_to_string(d).to_lowercase();
    if let Some(i) = DAYS.iter().position(|x| *x == s) {
        return i as f64;
    }
    let short = &s[..s.len().min(3)];
    if let Some(i) = DAYS.iter().position(|x| &x[..x.len().min(3)] == short) {
        return i as f64;
    }
    panic!("{}", crate::log::message::invalid_time_unit("day", d));
}

fn normalize_quarter(q: &Value) -> f64 {
    if let Some(n) = as_numeric(q) {
        if n > 4.0 {
            crate::log::warn(crate::log::message::invalid_time_unit("quarter", &json!(n)));
        }
        return n - 1.0;
    }
    panic!("{}", crate::log::message::invalid_time_unit("quarter", q));
}

fn as_numeric(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    v.as_str().and_then(|s| s.trim().parse::<f64>().ok())
}

fn push_number_or_expr(parts: &mut Vec<String>, value: &Value, expr_suffix: &str) {
    if let Some(n) = as_numeric(value) {
        parts.push(js_number_to_string(n));
    } else {
        parts.push(format!("{}{}", js_to_string(value), expr_suffix));
    }
}

fn date_time_parts(d: &Value, normalize: bool) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut d = d.clone();

    if normalize && d.get("day").map(|v| !v.is_null()).unwrap_or(false)
        && d.as_object().map(|m| m.len()).unwrap_or(0) > 1 {
            crate::log::warn(crate::log::message::dropped_day(&d));
            if let Some(map) = d.as_object_mut() {
                map.remove("day");
            }
        }

    match d.get("year").filter(|v| !v.is_null()) {
        Some(y) => parts.push(js_to_string(y)),
        None => parts.push("2012".to_string()),
    }

    if let Some(month) = d.get("month").filter(|v| !v.is_null()) {
        let month = if normalize {
            Value::from(normalize_month(month))
        } else {
            month.clone()
        };
        parts.push(js_to_string(&month));
    } else if let Some(quarter) = d.get("quarter").filter(|v| !v.is_null()) {
        let quarter = if normalize {
            Value::from(normalize_quarter(quarter))
        } else {
            quarter.clone()
        };
        if let Some(n) = as_numeric(&quarter) {
            parts.push(js_number_to_string(n * 3.0));
        } else {
            parts.push(format!("{}*3", js_to_string(&quarter)));
        }
    } else {
        parts.push("0".to_string());
    }

    if let Some(date) = d.get("date").filter(|v| !v.is_null()) {
        parts.push(js_to_string(date));
    } else if let Some(day) = d.get("day").filter(|v| !v.is_null()) {
        let day = if normalize {
            Value::from(normalize_day(day))
        } else {
            day.clone()
        };
        push_number_or_expr(&mut parts, &day, "+1");
    } else {
        parts.push("1".to_string());
    }

    for unit in ["hours", "minutes", "seconds", "milliseconds"] {
        match d.get(unit) {
            Some(v) if !v.is_null() => push_number_or_expr(&mut parts, v, ""),
            _ => parts.push("0".to_string()),
        }
    }

    parts
}

/// Port of `dateTimeToExpr`.
pub fn date_time_to_expr(d: &Value) -> String {
    let parts = date_time_parts(d, true);
    let body = parts.join(", ");
    if truthy(d.get("utc").unwrap_or(&Value::Null)) {
        format!("utc({body})")
    } else {
        format!("datetime({body})")
    }
}

/// Approximate `isNaN(Date.parse(v))` for strings.
pub fn date_parse_is_nan(s: &str) -> bool {
    // Accept the common ISO-ish formats used by Vega-Lite specs.
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return true;
    }
    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    if !has_digit {
        return true;
    }
    // Very permissive: YYYY[-/]MM[-/]DD, HH:MM, or fully qualified timestamps.
    trimmed.chars().all(|c| {
        c.is_ascii_digit()
            || matches!(c, '-' | '/' | ':' | '.' | 'T' | 'Z' | '+' | ' ' | ',')
    })
}

// ---------------------------------------------------------------------------
// sort.ts
// ---------------------------------------------------------------------------

const SORT_BY_CHANNELS: &[&str] = &[
    "x",
    "y",
    "color",
    "fill",
    "stroke",
    "strokeWidth",
    "size",
    "shape",
    "fillOpacity",
    "strokeOpacity",
    "opacity",
    "text",
];

pub fn is_sort_by_channel(c: &str) -> bool {
    SORT_BY_CHANNELS.contains(&c)
}

// ---------------------------------------------------------------------------
// compile/format.ts
// ---------------------------------------------------------------------------

pub fn is_custom_format_type(format_type: Option<&str>) -> bool {
    match format_type {
        Some(t) => !t.is_empty() && t != "number" && t != "time",
        None => false,
    }
}

// ---------------------------------------------------------------------------
// log messages not (yet) present in `log::message`
// ---------------------------------------------------------------------------

pub fn primitive_channel_def(channel: &str, t: &str, value: &Value) -> String {
    format!(
        "Channel {channel} is a {t}. Converted to {{value: {}}}.",
        stringify(value)
    )
}

pub fn custom_format_type_not_allowed(channel: &str) -> String {
    format!(
        "Config.customFormatTypes is not true, thus custom format type and format for channel {channel} are dropped."
    )
}

pub fn channel_should_not_be_used_for_binned(channel: &str) -> String {
    format!("Channel {channel} should not be used with \"binned\" bin.")
}

pub fn channel_should_be_discrete_or_discretizing(channel: &str) -> String {
    format!(
        "{channel} encoding should be discrete (ordinal / nominal / binned) or use a discretizing scale (e.g. threshold)."
    )
}

// ---------------------------------------------------------------------------
// compile/common.ts
// ---------------------------------------------------------------------------

fn get_style_config(prop: &str, styles: &[String], style_config_index: Option<&Value>) -> Option<Value> {
    let index = style_config_index?;
    let mut value: Option<Value> = None;
    for style in styles {
        if let Some(style_config) = index.get(style) {
            if let Some(v) = style_config.get(prop) {
                value = Some(v.clone());
            }
        }
    }
    value
}

fn get_styles(mark: &Value) -> Vec<String> {
    match mark.get("style") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

/// Port of `getMarkConfig`.
pub fn get_mark_config(prop: &str, mark: &Value, config: &Value) -> Option<Value> {
    get_mark_config_with(prop, mark, config, None)
}

/// Port of `getMarkConfig` with the optional `vgChannel` argument.
pub fn get_mark_config_with(
    prop: &str,
    mark: &Value,
    config: &Value,
    vg_channel: Option<&str>,
) -> Option<Value> {
    let styles = get_styles(mark);
    let cfg = get_style_config(prop, &styles, config.get("style"));
    let mark_type = mark.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mark_config = config.get(mark_type);
    let general_mark = config.get("mark");

    let candidates: [Option<Value>; 5] = [
        if vg_channel.is_some() { cfg.clone() } else { None },
        cfg,
        vg_channel.and_then(|vg| mark_config.and_then(|m| m.get(vg)).cloned()),
        mark_config.and_then(|m| m.get(prop)).cloned(),
        if let Some(vg) = vg_channel {
            general_mark.and_then(|m| m.get(vg)).cloned()
        } else {
            general_mark.and_then(|m| m.get(prop)).cloned()
        },
    ];
    candidates.into_iter().flatten().next()
}

/// Port of `getMarkPropOrConfig`.
pub fn get_mark_prop_or_config(
    channel: &str,
    mark: &Value,
    config: &Value,
    vg_channel: Option<&str>,
    ignore_vg_config: bool,
) -> Option<Value> {
    if let Some(vg) = vg_channel {
        if has_property(mark, vg) {
            return mark.get(vg).cloned();
        }
    }
    if let Some(v) = mark.get(channel) {
        if !v.is_null() {
            return Some(v.clone());
        }
    }
    if ignore_vg_config && (vg_channel.is_none() || vg_channel == Some(channel)) {
        return None;
    }
    get_mark_config_with(channel, mark, config, vg_channel)
}

/// Convenience re-export used by `channeldef::is_count`.
pub fn internal_count_field() -> String {
    internal_field("count")
}
