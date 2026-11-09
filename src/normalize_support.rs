//! Private helpers backing the `normalize` module.
//!
//! Upstream `normalize/*` leans on a large part of the crate (`channel`,
//! `channeldef`, `encoding`, `bin`, `scale`, `stack`, `compile/*`,
//! `compositemark/*`). Those modules were still stubs while this layer was
//! ported, so the faithful helpers needed here live in this single support
//! file instead.

use serde_json::{json, Map, Value};

use crate::mark;
use crate::util::{self, has_property};

// ---------------------------------------------------------------------------
// Generic object helpers (JS object semantics on serde_json::Value)
// ---------------------------------------------------------------------------

pub fn is_object(v: &Value) -> bool {
    v.is_object()
}

pub fn is_array(v: &Value) -> bool {
    v.is_array()
}

pub fn as_map(v: &Value) -> Option<&Map<String, Value>> {
    v.as_object()
}

pub fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_object().and_then(|o| o.get(key))
}

pub fn has_prop(v: &Value, key: &str) -> bool {
    has_property(v, key)
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in entries {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

pub fn empty_object() -> Value {
    Value::Object(Map::new())
}

pub fn insert(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.insert(key.to_string(), value);
}

pub fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.to_string(), v);
    }
}

/// `{...base, ...over}` where non-object operands contribute nothing.
pub fn spread(base: Option<&Value>, over: Option<&Value>) -> Value {
    let mut m = Map::new();
    if let Some(Value::Object(b)) = base {
        for (k, v) in b {
            m.insert(k.clone(), v.clone());
        }
    }
    if let Some(Value::Object(o)) = over {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    Value::Object(m)
}

/// Extend `dest` with all own properties of `src`.
pub fn assign(dest: &mut Map<String, Value>, src: &Value) {
    if let Value::Object(s) = src {
        for (k, v) in s {
            dest.insert(k.clone(), v.clone());
        }
    }
}

pub fn omit(v: &Value, props: &[&str]) -> Value {
    match v {
        Value::Object(o) => {
            let mut m = o.clone();
            for p in props {
                m.remove(*p);
            }
            Value::Object(m)
        }
        other => other.clone(),
    }
}

pub fn pick(v: &Value, props: &[&str]) -> Value {
    let mut m = Map::new();
    if let Value::Object(o) = v {
        for p in props {
            if let Some(val) = o.get(*p) {
                m.insert((*p).to_string(), val.clone());
            }
        }
    }
    Value::Object(m)
}

pub fn keys(v: &Value) -> Vec<String> {
    match v {
        Value::Object(o) => o.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

pub fn entries(v: &Value) -> Vec<(String, Value)> {
    match v {
        Value::Object(o) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    }
}

pub fn is_empty_object(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.is_empty(),
        Value::Null => true,
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

pub fn deep_equal(a: &Value, b: &Value) -> bool {
    util::deep_equal(a, b)
}

pub fn var_name(s: &str) -> String {
    util::var_name(s)
}

// ---------------------------------------------------------------------------
// Channel constants and helpers
// ---------------------------------------------------------------------------

pub const ROW: &str = "row";
pub const COLUMN: &str = "column";
pub const FACET: &str = "facet";

pub const X: &str = "x";
pub const Y: &str = "y";
pub const X2: &str = "x2";
pub const Y2: &str = "y2";
pub const XOFFSET: &str = "xOffset";
pub const YOFFSET: &str = "yOffset";
pub const LATITUDE: &str = "latitude";
pub const LONGITUDE: &str = "longitude";
pub const LATITUDE2: &str = "latitude2";
pub const LONGITUDE2: &str = "longitude2";
pub const THETA: &str = "theta";
pub const THETA2: &str = "theta2";
pub const RADIUS: &str = "radius";
pub const RADIUS2: &str = "radius2";

pub const FACET_CHANNELS: [&str; 3] = [FACET, ROW, COLUMN];

pub const SECONDARY_RANGE_CHANNELS: [&str; 6] =
    [X2, Y2, LATITUDE2, LONGITUDE2, THETA2, RADIUS2];

pub const UNIT_CHANNELS: [&str; 34] = [
    X, Y, X2, Y2, THETA, THETA2, RADIUS, RADIUS2, LONGITUDE, LONGITUDE2, LATITUDE, LATITUDE2,
    XOFFSET, YOFFSET, "color", "fill", "stroke", "time", "opacity", "fillOpacity", "strokeOpacity",
    "strokeWidth", "strokeDash", "size", "angle", "shape", "order", "text", "detail", "key",
    "tooltip", "href", "url", "description",
];

pub const NONPOSITION_CHANNELS: [&str; 20] = [
    "color", "fill", "stroke", "time", "opacity", "fillOpacity", "strokeOpacity", "strokeWidth",
    "strokeDash", "size", "angle", "shape", "order", "text", "detail", "key", "tooltip", "href",
    "url", "description",
];

pub fn is_channel(c: &str) -> bool {
    UNIT_CHANNELS.contains(&c) || FACET_CHANNELS.contains(&c)
}

pub fn is_xor_y(c: &str) -> bool {
    c == X || c == Y
}

pub fn is_secondary_range_channel(c: &str) -> bool {
    SECONDARY_RANGE_CHANNELS.contains(&c)
}

pub fn get_main_range_channel(c: &str) -> &str {
    match c {
        X2 => X,
        Y2 => Y,
        LATITUDE2 => LATITUDE,
        LONGITUDE2 => LONGITUDE,
        THETA2 => THETA,
        RADIUS2 => RADIUS,
        other => other,
    }
}

pub fn get_secondary_range_channel(c: &str) -> Option<&'static str> {
    match c {
        X => Some(X2),
        Y => Some(Y2),
        LATITUDE => Some(LATITUDE2),
        LONGITUDE => Some(LONGITUDE2),
        THETA => Some(THETA2),
        RADIUS => Some(RADIUS2),
        _ => None,
    }
}

pub fn get_offset_scale_channel(c: &str) -> Option<&'static str> {
    match c {
        X => Some(XOFFSET),
        Y => Some(YOFFSET),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Field / datum definition predicates (from `channeldef.ts`)
// ---------------------------------------------------------------------------

pub fn is_repeat_ref(v: &Value) -> bool {
    !v.is_string() && has_prop(v, "repeat")
}

pub fn is_field_def(v: &Value) -> bool {
    has_prop(v, "field")
        || matches!(get(v, "aggregate"), Some(Value::String(s)) if s == "count")
}

pub fn is_datum_def(v: &Value) -> bool {
    has_prop(v, "datum")
}

pub fn is_field_or_datum_def(v: &Value) -> bool {
    is_field_def(v) || is_datum_def(v)
}

pub fn is_value_def(v: &Value) -> bool {
    has_prop(v, "value")
}

pub fn is_conditional_def(v: &Value) -> bool {
    has_prop(v, "condition")
}

pub fn has_conditional_field_def(v: &Value) -> bool {
    match get(v, "condition") {
        Some(c) if !c.is_array() => is_field_def(c),
        _ => false,
    }
}

pub fn has_conditional_field_or_datum_def(v: &Value) -> bool {
    match get(v, "condition") {
        Some(c) if !c.is_array() => is_field_or_datum_def(c),
        _ => false,
    }
}

pub fn is_typed_field_def(v: &Value) -> bool {
    (has_prop(v, "field")
        || matches!(get(v, "aggregate"), Some(Value::String(s)) if s == "count"))
        && has_prop(v, "type")
}

pub fn is_sortable_field_def(v: &Value) -> bool {
    has_prop(v, "sort")
}

pub fn is_scale_field_def(v: &Value) -> bool {
    has_prop(v, "scale") || has_prop(v, "sort")
}

pub fn is_position_field_or_datum_def(v: &Value) -> bool {
    has_prop(v, "axis") || has_prop(v, "stack") || has_prop(v, "impute")
}

pub fn is_numeric_data_def(v: &Value) -> bool {
    is_datum_def(v) && get(v, "datum").map(|d| d.is_number()).unwrap_or(false)
}

pub fn is_discrete(v: &Value) -> bool {
    match get(v, "type").and_then(|t| t.as_str()) {
        Some("nominal") | Some("ordinal") | Some("geojson") => true,
        Some("quantitative") => is_field_def(v) && truthy(get(v, "bin").unwrap_or(&Value::Null)),
        Some("temporal") => false,
        _ => false,
    }
}

pub fn is_continuous_field_or_datum_def(v: &Value) -> bool {
    (is_typed_field_def(v) && !is_discrete(v)) || is_numeric_data_def(v)
}

pub fn is_unbinned_quantitative_field_or_datum_def(v: &Value) -> bool {
    (is_typed_field_def(v)
        && get(v, "type").and_then(|t| t.as_str()) == Some("quantitative")
        && !truthy(get(v, "bin").unwrap_or(&Value::Null)))
        || is_numeric_data_def(v)
}

pub fn is_temporal_field_def(v: &Value) -> bool {
    get(v, "type").and_then(|t| t.as_str()) == Some("temporal")
        || (is_field_def(v) && truthy(get(v, "timeUnit").unwrap_or(&Value::Null)))
}

pub fn is_field_or_datum_def_for_time_format(v: &Value) -> bool {
    let format_type = get_format_mixins(v)
        .get("formatType")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());
    format_type.as_deref() == Some("time")
        || (format_type.is_none() && is_temporal_field_def(v))
}

fn get_format_mixins(v: &Value) -> Value {
    if get(v, "format").is_some() || get(v, "formatType").is_some() {
        return json!({
            "format": get(v, "format").cloned().unwrap_or(Value::Null),
            "formatType": get(v, "formatType").cloned().unwrap_or(Value::Null),
        });
    }
    // guides
    for guide in ["axis", "legend", "header"] {
        if let Some(g) = get(v, guide) {
            if is_object(g)
                && (g.get("format").is_some() || g.get("formatType").is_some()) {
                    return json!({
                        "format": g.get("format").cloned().unwrap_or(Value::Null),
                        "formatType": g.get("formatType").cloned().unwrap_or(Value::Null),
                    });
                }
        }
    }
    empty_object()
}

pub fn is_bin_params(bin: &Value) -> bool {
    is_object(bin)
}

pub fn is_binning(bin: &Value) -> bool {
    bin == &Value::Bool(true) || (is_bin_params(bin) && !truthy(get(bin, "binned").unwrap_or(&Value::Null)))
}

pub fn is_binned(bin: &Value) -> bool {
    bin.as_str() == Some("binned")
        || (is_bin_params(bin) && get(bin, "binned") == Some(&Value::Bool(true)))
}

pub fn get_field_def(channel_def: &Value) -> Option<&Value> {
    if is_field_def(channel_def) {
        Some(channel_def)
    } else if has_conditional_field_def(channel_def) {
        get(channel_def, "condition")
    } else {
        None
    }
}

pub fn is_sort_field(sort: &Value) -> bool {
    truthy(sort)
        && (get(sort, "op").and_then(|o| o.as_str()) == Some("count") || has_prop(sort, "field"))
}

/// Simplified port of `vgField` covering the cases needed by normalization.
pub fn vg_field(field_def: &Value, for_as: bool) -> Option<String> {
    let mut field = match get(field_def, "field") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => {
            if is_count(field_def) {
                util::internal_field("count")
            } else {
                return None;
            }
        }
    };

    if is_count(field_def) {
        field = util::internal_field("count");
    } else {
        let bin = get(field_def, "bin").cloned().unwrap_or(Value::Null);
        let aggregate = get(field_def, "aggregate").cloned().unwrap_or(Value::Null);
        let time_unit = get(field_def, "timeUnit").cloned().unwrap_or(Value::Null);
        let mut suffix: Option<String> = None;
        let mut fn_name: Option<String> = None;

        if is_binning(&bin) {
            fn_name = Some(bin_to_string(&bin));
            suffix = Some(String::new());
        } else if truthy(&aggregate) {
            if let Some(argmax) = get(&aggregate, "argmax") {
                field = format!("argmax_{}", stringify_scalar(argmax));
            } else if let Some(argmin) = get(&aggregate, "argmin") {
                field = format!("argmin_{}", stringify_scalar(argmin));
            } else if let Some(op) = aggregate.as_str() {
                fn_name = Some(op.to_string());
            }
        } else if truthy(&time_unit) && !is_binned_time_unit(&time_unit) {
            if let Some(s) = crate::timeunit::time_unit_to_string(&time_unit) {
                fn_name = Some(s);
                suffix = Some(String::new());
            }
        }

        if let Some(f) = fn_name {
            field = if field.is_empty() {
                f
            } else {
                format!("{}_{}", f, field)
            };
        }
        if let Some(s) = suffix {
            if !s.is_empty() {
                field = format!("{}_{}", field, s);
            }
        }
    }

    if for_as {
        Some(util::remove_path_from_field(&field))
    } else {
        Some(util::replace_path_in_field(&field))
    }
}

fn stringify_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn is_count(field_def: &Value) -> bool {
    get(field_def, "aggregate").and_then(|a| a.as_str()) == Some("count")
        && !has_prop(field_def, "field")
}

fn is_binned_time_unit(tu: &Value) -> bool {
    if let Some(s) = tu.as_str() {
        return s == "binned";
    }
    get(tu, "binned") == Some(&Value::Bool(true))
}

fn bin_to_string(bin: &Value) -> String {
    if bin == &Value::Bool(true) {
        return "bin".to_string();
    }
    let mut parts: Vec<String> = vec!["bin".to_string()];
    for (key, prop) in [
        ("step", "step"),
        ("steps", "steps"),
        ("minstep", "minstep"),
        ("base", "base"),
        ("divide", "divide"),
        ("maxbins", "maxbins"),
    ] {
        if let Some(v) = get(bin, prop) {
            if truthy(v) {
                parts.push(format!("{}_{}", key, stringify_scalar(v)));
            }
        }
    }
    if let Some(extent) = get(bin, "extent") {
        if extent.is_array() {
            let vals: Vec<String> = extent.as_array().unwrap().iter().map(stringify_scalar).collect();
            parts.push(format!("extent_{}", vals.join("_")));
        }
    }
    parts.join("_")
}

/// Port of `normalizeBinExtent`.
pub fn normalize_bin_extent(bin: &Value) -> Value {
    if let Some(ext) = get(bin, "extent") {
        if let Some(sel) = get(ext, "selection") {
            let mut rest = ext.as_object().cloned().unwrap_or_default();
            rest.remove("selection");
            let mut m = bin.as_object().cloned().unwrap_or_default();
            let mut extent = rest;
            extent.insert("param".to_string(), sel.clone());
            m.insert("extent".to_string(), Value::Object(extent));
            return Value::Object(m);
        }
    }
    bin.clone()
}

pub fn normalize_predicate_operand(op: &Value) -> Value {
    op.clone()
}

// ---------------------------------------------------------------------------
// Encoding helpers (from `encoding.ts`)
// ---------------------------------------------------------------------------

pub fn channel_has_field(encoding: &Value, channel: &str) -> bool {
    match get(encoding, channel) {
        None | Some(Value::Null) => false,
        Some(Value::Array(arr)) => arr.iter().any(|fd| truthy(get(fd, "field").unwrap_or(&Value::Null))),
        Some(cd) => is_field_def(cd) || has_conditional_field_def(cd),
    }
}

pub fn channel_has_field_or_datum(encoding: &Value, channel: &str) -> bool {
    match get(encoding, channel) {
        None | Some(Value::Null) => false,
        Some(Value::Array(arr)) => arr.iter().any(|fd| truthy(get(fd, "field").unwrap_or(&Value::Null))),
        Some(cd) => is_field_def(cd) || is_datum_def(cd) || has_conditional_field_or_datum_def(cd),
    }
}

pub fn is_aggregate(encoding: &Value) -> bool {
    UNIT_CHANNELS.iter().any(|channel| {
        if channel_has_field(encoding, channel) {
            match get(encoding, channel) {
                Some(Value::Array(arr)) => {
                    arr.iter().any(|fd| truthy(get(fd, "aggregate").unwrap_or(&Value::Null)))
                }
                Some(cd) => get_field_def(cd)
                    .map(|fd| truthy(get(fd, "aggregate").unwrap_or(&Value::Null)))
                    .unwrap_or(false),
                None => false,
            }
        } else {
            false
        }
    })
}

pub fn channel_has_quantitative_offset(encoding: &Value, channel: &str) -> bool {
    match get_offset_scale_channel(channel) {
        Some(offset) => is_unbinned_quantitative_field_or_datum_def(
            get(encoding, offset).unwrap_or(&Value::Null),
        ),
        None => false,
    }
}

pub fn field_defs(encoding: &Value) -> Vec<Value> {
    let mut arr = Vec::new();
    let channels = keys(encoding);
    for channel in channels {
        if channel_has_field(encoding, &channel) {
            let channel_def = get(encoding, &channel).unwrap();
            let defs: Vec<&Value> = match channel_def {
                Value::Array(a) => a.iter().collect(),
                other => vec![other],
            };
            for def in defs {
                if is_field_def(def) {
                    arr.push(def.clone());
                } else if has_conditional_field_def(def) {
                    arr.push(get(def, "condition").unwrap().clone());
                }
            }
        }
    }
    arr
}

pub fn to_string_field_def(field_def: &Value) -> Value {
    omit(field_def, &["legend", "axis", "header", "scale"])
}

/// Port of `extractTransformsFromEncoding` (subset needed by composite marks).
pub fn extract_transforms_from_encoding(
    old_encoding: &Value,
    _config: &Value,
) -> ExtractedTransforms {
    let mut groupby: Vec<Value> = Vec::new();
    let mut bins: Vec<Value> = Vec::new();
    let mut time_units: Vec<Value> = Vec::new();
    let mut aggregate: Vec<Value> = Vec::new();
    let mut encoding = Map::new();

    for (channel, channel_def) in entries(old_encoding) {
        let defs: Vec<Value> = match &channel_def {
            Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        for channel_def in defs {
            if is_field_def(&channel_def) {
                let field = get(&channel_def, "field").cloned().unwrap_or(Value::Null);
                let agg = get(&channel_def, "aggregate").cloned().unwrap_or(Value::Null);
                let bin = get(&channel_def, "bin").cloned().unwrap_or(Value::Null);
                let time_unit = get(&channel_def, "timeUnit").cloned().unwrap_or(Value::Null);

                if truthy(&agg) || truthy(&time_unit) || truthy(&bin) {
                    let mut new_field_def = omit(&channel_def, &["field", "aggregate", "bin", "timeUnit"]);
                    let mut new_field = vg_field(&channel_def, true).unwrap_or_default();
                    if truthy(&agg) {
                        let mut op: Option<String> = None;
                        if let Some(argmax) = get(&agg, "argmax") {
                            op = Some("argmax".to_string());
                            new_field = vg_field(&json!({"op": "argmax", "field": argmax}), true)
                                .unwrap_or_default();
                            if let Some(m) = new_field_def.as_object_mut() {
                                m.insert(
                                    "field".to_string(),
                                    Value::String(format!("{}.{}", new_field, stringify_scalar(&field))),
                                );
                            }
                        } else if let Some(argmin) = get(&agg, "argmin") {
                            op = Some("argmin".to_string());
                            new_field = vg_field(&json!({"op": "argmin", "field": argmin}), true)
                                .unwrap_or_default();
                            if let Some(m) = new_field_def.as_object_mut() {
                                m.insert(
                                    "field".to_string(),
                                    Value::String(format!("{}.{}", new_field, stringify_scalar(&field))),
                                );
                            }
                        } else if let Some(s) = agg.as_str() {
                            if s != "boxplot" && s != "errorbar" && s != "errorband" {
                                op = Some(s.to_string());
                            }
                        }
                        if let Some(op) = op {
                            let mut entry = Map::new();
                            entry.insert("op".to_string(), Value::String(op));
                            if truthy(&field) {
                                entry.insert("field".to_string(), field.clone());
                            }
                            entry.insert("as".to_string(), Value::String(new_field.clone()));
                            aggregate.push(Value::Object(entry));
                        }
                    } else {
                        groupby.push(Value::String(new_field.clone()));
                        if truthy(&bin) && is_binning(&bin) {
                            let mut b = Map::new();
                            b.insert("bin".to_string(), bin.clone());
                            b.insert("field".to_string(), field.clone());
                            b.insert("as".to_string(), Value::String(new_field.clone()));
                            bins.push(Value::Object(b));
                            groupby.push(Value::String(format!("{}_end", new_field)));
                            if let Some(m) = new_field_def.as_object_mut() {
                                m.insert("bin".to_string(), Value::String("binned".to_string()));
                                if !is_secondary_range_channel(&channel) {
                                    m.insert("type".to_string(), Value::String("quantitative".to_string()));
                                }
                            }
                            encoding.insert(
                                format!("{}2", channel),
                                json!({"field": format!("{}_end", new_field)}),
                            );
                        } else if truthy(&time_unit) && !is_binned_time_unit(&time_unit) {
                            let mut tu = Map::new();
                            tu.insert("timeUnit".to_string(), time_unit.clone());
                            tu.insert("field".to_string(), field.clone());
                            tu.insert("as".to_string(), Value::String(new_field.clone()));
                            time_units.push(Value::Object(tu));
                        }
                    }
                    if !has_prop(&new_field_def, "field") {
                        if let Some(m) = new_field_def.as_object_mut() {
                            m.insert("field".to_string(), Value::String(new_field));
                        }
                    }
                    encoding.insert(channel.clone(), new_field_def);
                } else {
                    groupby.push(field);
                    encoding.insert(channel.clone(), channel_def);
                }
            } else {
                encoding.insert(channel.clone(), channel_def);
            }
        }
    }

    ExtractedTransforms {
        bins,
        time_units,
        aggregate,
        groupby,
        encoding: Value::Object(encoding),
    }
}

pub struct ExtractedTransforms {
    pub bins: Vec<Value>,
    pub time_units: Vec<Value>,
    pub aggregate: Vec<Value>,
    pub groupby: Vec<Value>,
    pub encoding: Value,
}

/// Port of `initChannelDef`.
pub fn init_channel_def(channel_def: &Value, channel: &str, config: &Value, composite: bool) -> Value {
    if channel_def.is_string() || channel_def.is_number() || channel_def.is_boolean() {
        let primitive_type = if channel_def.is_string() {
            "string"
        } else if channel_def.is_number() {
            "number"
        } else {
            "boolean"
        };
        crate::log::warn(format!(
            "Channel {} is a {}. Converted to {{value: {}}}.",
            channel,
            primitive_type,
            util::stringify(channel_def)
        ));
        return json!({"value": channel_def.clone()});
    }

    if is_field_or_datum_def(channel_def) {
        return init_field_or_datum_def(channel_def, channel, config, composite);
    } else if has_conditional_field_or_datum_def(channel_def) {
        let mut m = channel_def.as_object().cloned().unwrap_or_default();
        m.insert(
            "condition".to_string(),
            init_field_or_datum_def(get(channel_def, "condition").unwrap(), channel, config, composite),
        );
        return Value::Object(m);
    }
    channel_def.clone()
}

fn init_field_or_datum_def(field_def: &Value, channel: &str, _config: &Value, _composite: bool) -> Value {
    if is_field_def(field_def) {
        init_field_def(field_def, channel)
    } else {
        field_def.clone()
    }
}

fn init_field_def(field_def: &Value, channel: &str) -> Value {
    let mut m = field_def.as_object().cloned().unwrap_or_default();
    if let Some(field) = get(field_def, "field") {
        if !field.is_array() {
            if let Some(s) = field.as_str() {
                m.insert("field".to_string(), Value::String(s.to_string()));
            }
        }
    }
    let time_unit = get(field_def, "timeUnit").cloned().unwrap_or(Value::Null);
    if truthy(&time_unit) {
        m.insert(
            "timeUnit".to_string(),
            crate::channeldef_support::normalize_time_unit(&time_unit),
        );
    }
    let bin = get(field_def, "bin").cloned().unwrap_or(Value::Null);
    if is_binning(&bin) {
        m.insert(
            "bin".to_string(),
            crate::channeldef::normalize_bin(&bin, Some(channel)),
        );
    }
    if is_typed_field_def(field_def) {
        // type already present
    } else if !is_secondary_range_channel(channel) {
        let new_type = default_type(field_def, channel);
        m.insert("type".to_string(), Value::String(new_type));
    }
    Value::Object(m)
}

pub fn default_type(field_def: &Value, channel: &str) -> String {
    match channel {
        "latitude" | "longitude" => return "quantitative".to_string(),
        "row" | "column" | "facet" | "shape" | "strokeDash" => return "nominal".to_string(),
        "order" => return "ordinal".to_string(),
        _ => {}
    }
    if is_sortable_field_def(field_def) && get(field_def, "sort").map(|s| s.is_array()).unwrap_or(false) {
        return "ordinal".to_string();
    }
    if truthy(get(field_def, "timeUnit").unwrap_or(&Value::Null)) {
        return "temporal".to_string();
    }
    let aggregate = get(field_def, "aggregate").cloned().unwrap_or(Value::Null);
    if truthy(get(field_def, "bin").unwrap_or(&Value::Null))
        || (truthy(&aggregate) && !has_prop(&aggregate, "argmax") && !has_prop(&aggregate, "argmin"))
    {
        return "quantitative".to_string();
    }
    if let Some(scale_type) = get(field_def, "scale").and_then(|s| get(s, "type")).and_then(|t| t.as_str()) {
        match scale_type {
            "linear" | "log" | "pow" | "sqrt" | "symlog" | "quantile" | "quantize" | "threshold"
            | "bin-ordinal" => return "quantitative".to_string(),
            "time" | "utc" => return "temporal".to_string(),
            _ => {}
        }
    }
    "nominal".to_string()
}

pub fn normalize_encoding(encoding: &Value, config: &Value) -> Value {
    let mut normalized = Map::new();
    if let Value::Object(enc) = encoding {
        for (channel, channel_def) in enc {
            normalized.insert(
                channel.clone(),
                init_channel_def(channel_def, channel, config, true),
            );
        }
    }
    Value::Object(normalized)
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Minimal faithful subset of `initConfig`: it fills in the defaults that the
/// normalizers read (`mark`, per-mark configs, composite marks).
pub fn init_config(specified: Option<&Value>) -> Value {
    let mut config = json!({
        "mark": mark::default_mark_config(),
        "arc": {},
        "area": {},
        "bar": mark::default_bar_config(),
        "circle": {},
        "geoshape": {},
        "image": {},
        "line": {},
        "point": {},
        "rect": mark::default_rect_config(),
        "rule": {"color": "black"},
        "square": {},
        "text": {"color": "black"},
        "tick": mark::default_tick_config(),
        "trail": {},
        "boxplot": {
            "size": 14,
            "extent": 1.5,
            "box": {},
            "median": {"color": "white"},
            "outliers": {},
            "rule": {},
            "ticks": null
        },
        "errorbar": {"center": "mean", "rule": true, "ticks": false},
        "errorband": {"band": {"opacity": 0.3}, "borders": false},
        "style": {}
    });
    if let Some(spec) = specified {
        util::merge_deep(&mut config, spec);
    }
    config
}

pub fn get_mark_prop_or_config(prop: &str, mark: &Value, config: &Value) -> Option<Value> {
    mark::get_mark_prop_or_config(prop, mark, config, None, false)
}

// ---------------------------------------------------------------------------
// Mark initialization & stack (from `compile/mark/init.ts` and `stack.ts`)
// ---------------------------------------------------------------------------

pub fn init_markdef(original: &Value, encoding: &Value, config: &Value) -> Value {
    let mut mark_def = original.as_object().cloned().unwrap_or_default();
    let mark_type = mark::get_mark_type(original).unwrap_or("").to_string();
    let specified_orient = get_mark_prop_or_config("orient", original, config);
    let orient_value = orient(&mark_type, encoding, specified_orient.as_ref());
    let mut md_value = Value::Object(mark_def.clone());
    if let Some(m) = md_value.as_object_mut() {
        if let Some(o) = orient_value {
            m.insert("orient".to_string(), Value::String(o));
        } else {
            m.remove("orient");
        }
    }
    mark_def = md_value.as_object().cloned().unwrap_or_default();

    let specified_opacity = get_mark_prop_or_config("opacity", &Value::Object(mark_def.clone()), config);
    let specified_fill_opacity =
        get_mark_prop_or_config("fillOpacity", &Value::Object(mark_def.clone()), config);
    if specified_opacity.is_none() && specified_fill_opacity.is_none() {
        if let Some(o) = opacity(&mark_type, encoding) {
            mark_def.insert("opacity".to_string(), json!(o));
        }
    }
    Value::Object(mark_def)
}

fn opacity(mark_type: &str, encoding: &Value) -> Option<f64> {
    if matches!(mark_type, "point" | "tick" | "circle" | "square") && !is_aggregate(encoding) {
        Some(0.7)
    } else {
        None
    }
}

fn orient(mark_type: &str, encoding: &Value, specified: Option<&Value>) -> Option<String> {
    let specified_str = specified.and_then(|v| v.as_str()).map(|s| s.to_string());
    match mark_type {
        "point" | "circle" | "square" | "rect" | "image" => return None,
        _ => {}
    }
    let x = get(encoding, X).cloned().unwrap_or(Value::Null);
    let y = get(encoding, Y).cloned().unwrap_or(Value::Null);
    let x2 = get(encoding, X2).cloned().unwrap_or(Value::Null);
    let y2 = get(encoding, Y2).cloned().unwrap_or(Value::Null);
    let x_offset_is_measure = channel_has_quantitative_offset(encoding, X);
    let y_offset_is_measure = channel_has_quantitative_offset(encoding, Y);

    match mark_type {
        "text" | "bar" => {
            if !truthy(&y) && y_offset_is_measure {
                return Some(specified_str.unwrap_or_else(|| "vertical".to_string()));
            }
            if !truthy(&x) && x_offset_is_measure {
                return Some(specified_str.unwrap_or_else(|| "horizontal".to_string()));
            }
            if is_field_def(&x)
                && (is_binned(get(&x, "bin").unwrap_or(&Value::Null))
                    || (is_field_def(&y)
                        && truthy(get(&y, "aggregate").unwrap_or(&Value::Null))
                        && !truthy(get(&x, "aggregate").unwrap_or(&Value::Null))))
            {
                return Some("vertical".to_string());
            }
            if is_field_def(&y)
                && (is_binned(get(&y, "bin").unwrap_or(&Value::Null))
                    || (is_field_def(&x)
                        && truthy(get(&x, "aggregate").unwrap_or(&Value::Null))
                        && !truthy(get(&y, "aggregate").unwrap_or(&Value::Null))))
            {
                return Some("horizontal".to_string());
            }
            if truthy(&y2) || truthy(&x2) {
                if let Some(s) = specified_str {
                    return Some(s);
                }
                if !truthy(&x2) {
                    if ((get(&x, "type").and_then(|t| t.as_str()) == Some("quantitative")
                        && !is_binning(get(&x, "bin").unwrap_or(&Value::Null)))
                        || is_numeric_data_def(&x))
                        && is_field_def(&y) && is_binned(get(&y, "bin").unwrap_or(&Value::Null)) {
                            return Some("horizontal".to_string());
                        }
                    return Some("vertical".to_string());
                }
                if !truthy(&y2) {
                    if ((get(&y, "type").and_then(|t| t.as_str()) == Some("quantitative")
                        && !is_binning(get(&y, "bin").unwrap_or(&Value::Null)))
                        || is_numeric_data_def(&y))
                        && is_field_def(&x) && is_binned(get(&x, "bin").unwrap_or(&Value::Null)) {
                            return Some("vertical".to_string());
                        }
                    return Some("horizontal".to_string());
                }
            }
            // falls through to rule
            if truthy(&x2)
                && !(is_field_def(&x) && is_binned(get(&x, "bin").unwrap_or(&Value::Null)))
                && truthy(&y2)
                && !(is_field_def(&y) && is_binned(get(&y, "bin").unwrap_or(&Value::Null)))
            {
                return None;
            }
            // falls through to area
            orient_area(
                mark_type,
                &x,
                &y,
                &x2,
                &y2,
                x_offset_is_measure,
                y_offset_is_measure,
                specified_str,
            )
        }
        "rule" => {
            if truthy(&x2)
                && !(is_field_def(&x) && is_binned(get(&x, "bin").unwrap_or(&Value::Null)))
                && truthy(&y2)
                && !(is_field_def(&y) && is_binned(get(&y, "bin").unwrap_or(&Value::Null)))
            {
                return None;
            }
            orient_area(
                mark_type,
                &x,
                &y,
                &x2,
                &y2,
                x_offset_is_measure,
                y_offset_is_measure,
                specified_str,
            )
        }
        "area" => orient_area(
            mark_type,
            &x,
            &y,
            &x2,
            &y2,
            x_offset_is_measure,
            y_offset_is_measure,
            specified_str,
        ),
        "line" | "tick" => {
            let x_is_measure = is_unbinned_quantitative_field_or_datum_def(&x);
            let y_is_measure = is_unbinned_quantitative_field_or_datum_def(&y);
            if let Some(s) = specified_str.clone() {
                return Some(s);
            }
            if x_is_measure && !y_is_measure {
                return Some(if mark_type != "tick" { "horizontal" } else { "vertical" }.to_string());
            } else if !x_is_measure && y_is_measure {
                return Some(if mark_type != "tick" { "vertical" } else { "horizontal" }.to_string());
            } else if x_is_measure && y_is_measure {
                return Some("vertical".to_string());
            }
            let x_is_temporal =
                is_typed_field_def(&x) && get(&x, "type").and_then(|t| t.as_str()) == Some("temporal");
            let y_is_temporal =
                is_typed_field_def(&y) && get(&y, "type").and_then(|t| t.as_str()) == Some("temporal");
            if x_is_temporal && !y_is_temporal {
                return Some("vertical".to_string());
            } else if !x_is_temporal && y_is_temporal {
                return Some("horizontal".to_string());
            }
            None
        }
        _ => Some("vertical".to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn orient_area(
    mark_type: &str,
    x: &Value,
    y: &Value,
    x2: &Value,
    y2: &Value,
    x_offset_is_measure: bool,
    y_offset_is_measure: bool,
    specified: Option<String>,
) -> Option<String> {
    if !truthy(y) && y_offset_is_measure {
        return Some(specified.unwrap_or_else(|| "vertical".to_string()));
    }
    if !truthy(x) && x_offset_is_measure {
        return Some(specified.unwrap_or_else(|| "horizontal".to_string()));
    }
    if truthy(y2) {
        if is_field_def(y) && is_binned(get(y, "bin").unwrap_or(&Value::Null)) {
            return Some("horizontal".to_string());
        }
        return Some("vertical".to_string());
    } else if truthy(x2) {
        if is_field_def(x) && is_binned(get(x, "bin").unwrap_or(&Value::Null)) {
            return Some("vertical".to_string());
        }
        return Some("horizontal".to_string());
    } else if mark_type == "rule" {
        if truthy(x) && !truthy(y) {
            return Some("vertical".to_string());
        } else if truthy(y) && !truthy(x) {
            return Some("horizontal".to_string());
        }
    }

    let x_is_measure = is_unbinned_quantitative_field_or_datum_def(x);
    let y_is_measure = is_unbinned_quantitative_field_or_datum_def(y);
    let x_is_temporal =
        is_typed_field_def(x) && get(x, "type").and_then(|t| t.as_str()) == Some("temporal");
    let y_is_temporal =
        is_typed_field_def(y) && get(y, "type").and_then(|t| t.as_str()) == Some("temporal");

    if let Some(s) = specified {
        return Some(s);
    } else if x_is_measure && !y_is_measure {
        return Some(if mark_type != "tick" { "horizontal" } else { "vertical" }.to_string());
    } else if !x_is_measure && y_is_measure {
        return Some(if mark_type != "tick" { "vertical" } else { "horizontal" }.to_string());
    } else if (x_is_measure && y_is_measure) || (x_is_temporal && !y_is_temporal) {
        return Some("vertical".to_string());
    } else if !x_is_temporal && y_is_temporal {
        return Some("horizontal".to_string());
    }
    None
}

pub struct StackProperties {
    pub field_channel: String,
    pub offset: String,
}

pub fn is_stack_offset(s: &str) -> bool {
    matches!(s, "zero" | "center" | "normalize")
}

pub fn stack(mark: &Value, encoding: &Value) -> Option<StackProperties> {
    let mark_def = if mark::is_mark_def(mark) {
        mark.clone()
    } else {
        json!({"type": mark.clone()})
    };
    let mark_type = mark::get_mark_type(&mark_def).unwrap_or("").to_string();

    const STACKABLE: [&str; 10] = [
        "arc", "bar", "area", "rule", "point", "circle", "square", "line", "text", "tick",
    ];
    if !STACKABLE.contains(&mark_type.as_str()) {
        return None;
    }

    let field_channel = potential_stacked_channel(encoding, X, &mark_def)
        .or_else(|| potential_stacked_channel(encoding, THETA, &mark_def))?;

    let stacked_field_def = get(encoding, &field_channel).cloned().unwrap_or(Value::Null);
    let stacked_field = if is_field_def(&stacked_field_def) {
        vg_field(&stacked_field_def, false)
    } else {
        None
    };

    if !has_prop(&stacked_field_def, "stack")
        && ((field_channel == X && !truthy(get(encoding, Y).unwrap_or(&Value::Null))
            && channel_has_quantitative_offset(encoding, Y))
            || (field_channel == Y && !truthy(get(encoding, X).unwrap_or(&Value::Null))
                && channel_has_quantitative_offset(encoding, X)))
    {
        return None;
    }

    let dimension_channel = get_dimension_channel(&field_channel);
    let mut groupby_fields: Vec<String> = Vec::new();
    if let Some(dim) = dimension_channel {
        if let Some(dimension_def) = get(encoding, dim) {
            let dimension_field = if is_field_def(dimension_def) {
                vg_field(dimension_def, false)
            } else {
                None
            };
            if let Some(f) = dimension_field {
                if Some(&f) != stacked_field.as_ref() {
                    groupby_fields.push(f);
                }
            }
        }
        let dimension_offset_channel = if dim == X { XOFFSET } else { YOFFSET };
        if let Some(offset_def) = get(encoding, dimension_offset_channel) {
            let offset_field = if is_field_def(offset_def) {
                vg_field(offset_def, false)
            } else {
                None
            };
            if let Some(f) = offset_field {
                if Some(&f) != stacked_field.as_ref() {
                    groupby_fields.push(f);
                }
            }
        }
    }

    let mut stack_by: Vec<Value> = Vec::new();
    for channel in NONPOSITION_CHANNELS {
        if channel == "tooltip" || !channel_has_field(encoding, channel) {
            continue;
        }
        let channel_def = get(encoding, channel).unwrap();
        let defs: Vec<Value> = match channel_def {
            Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        for def in defs {
            let Some(field_def) = get_field_def(&def) else {
                continue;
            };
            if truthy(get(field_def, "aggregate").unwrap_or(&Value::Null)) {
                continue;
            }
            let f = vg_field(field_def, false);
            let include = match &f {
                None => true,
                Some(f) => !groupby_fields.iter().any(|g| g == f),
            };
            if include {
                stack_by.push(json!({"channel": channel, "fieldDef": field_def.clone()}));
            }
        }
    }

    let mut offset: Option<String> = None;
    if let Some(stack_prop) = get(&stacked_field_def, "stack") {
        if stack_prop.is_boolean() {
            if truthy(stack_prop) {
                offset = Some("zero".to_string());
            }
        } else if let Some(s) = stack_prop.as_str() {
            offset = Some(s.to_string());
        }
    } else if matches!(mark_type.as_str(), "bar" | "area" | "arc") {
        offset = Some("zero".to_string());
    }

    let offset = offset?;
    if !is_stack_offset(&offset) {
        return None;
    }

    if is_aggregate(encoding) && stack_by.is_empty() {
        return None;
    }

    if let Some(secondary) = get_secondary_range_channel(&field_channel) {
        if is_field_or_datum_def(get(encoding, secondary).unwrap_or(&Value::Null)) {
            return None;
        }
    }

    Some(StackProperties {
        field_channel,
        offset,
    })
}

fn get_dimension_channel(channel: &str) -> Option<&'static str> {
    match channel {
        X => Some(Y),
        Y => Some(X),
        THETA => Some(RADIUS),
        RADIUS => Some(THETA),
        _ => None,
    }
}

fn potential_stacked_channel(
    encoding: &Value,
    x: &str,
    mark_def: &Value,
) -> Option<String> {
    let y = if x == X { Y } else { RADIUS };
    let mark_type = mark::get_mark_type(mark_def).unwrap_or("");
    let is_cartesian_bar_or_area = x == X && matches!(mark_type, "bar" | "area");
    let orient = get(mark_def, "orient").and_then(|o| o.as_str());

    let x_def = get(encoding, x).cloned().unwrap_or(Value::Null);
    let y_def = get(encoding, y).cloned().unwrap_or(Value::Null);

    if is_field_def(&x_def) && is_field_def(&y_def) {
        if is_unbinned_quantitative_field_or_datum_def(&x_def)
            && is_unbinned_quantitative_field_or_datum_def(&y_def)
        {
            if truthy(get(&x_def, "stack").unwrap_or(&Value::Null)) {
                return Some(x.to_string());
            } else if truthy(get(&y_def, "stack").unwrap_or(&Value::Null)) {
                return Some(y.to_string());
            }
            let x_aggregate = truthy(get(&x_def, "aggregate").unwrap_or(&Value::Null));
            let y_aggregate = truthy(get(&y_def, "aggregate").unwrap_or(&Value::Null));
            if x_aggregate != y_aggregate {
                return Some(if x_aggregate { x } else { y }.to_string());
            }
            if is_cartesian_bar_or_area {
                if orient == Some("vertical") {
                    return Some(y.to_string());
                } else if orient == Some("horizontal") {
                    return Some(x.to_string());
                }
            }
        } else if is_unbinned_quantitative_field_or_datum_def(&x_def) {
            return Some(x.to_string());
        } else if is_unbinned_quantitative_field_or_datum_def(&y_def) {
            return Some(y.to_string());
        }
    } else if is_unbinned_quantitative_field_or_datum_def(&x_def) {
        if is_cartesian_bar_or_area && orient == Some("vertical") {
            return None;
        }
        return Some(x.to_string());
    } else if is_unbinned_quantitative_field_or_datum_def(&y_def) {
        if is_cartesian_bar_or_area && orient == Some("horizontal") {
            return None;
        }
        return Some(y.to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// Composite marks are implemented in `src/compositemark/`; this wrapper is
// kept so existing callers of `normalize_support` keep working.
pub fn normalize_error_bar(spec: &Value, config: &Value) -> Result<Value, String> {
    crate::compositemark::errorbar::normalize_error_bar(spec, config)
}

/// Coerce a `Text`-like value to a plain string.
pub fn title_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => util::stringify(other),
    }
}

// ---------------------------------------------------------------------------
// Spec predicates (from `spec/*.ts`)
// ---------------------------------------------------------------------------

pub fn is_unit_spec(spec: &Value) -> bool {
    has_prop(spec, "mark")
}

pub fn is_layer_spec(spec: &Value) -> bool {
    has_prop(spec, "layer")
}

pub fn is_facet_spec(spec: &Value) -> bool {
    has_prop(spec, "facet")
}

pub fn is_repeat_spec(spec: &Value) -> bool {
    has_prop(spec, "repeat")
}

pub fn is_hconcat_spec(spec: &Value) -> bool {
    has_prop(spec, "hconcat")
}

pub fn is_vconcat_spec(spec: &Value) -> bool {
    has_prop(spec, "vconcat")
}

pub fn is_concat_spec(spec: &Value) -> bool {
    has_prop(spec, "concat")
}

pub fn is_layer_repeat_spec(spec: &Value) -> bool {
    match get(spec, "repeat") {
        Some(Value::Array(_)) => false,
        Some(r) => has_prop(r, "layer"),
        None => false,
    }
}

pub fn is_facet_mapping(facet: &Value) -> bool {
    has_prop(facet, "row") || has_prop(facet, "column")
}
