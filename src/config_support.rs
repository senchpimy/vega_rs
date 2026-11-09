//! Support helpers for `src/config.rs`.
//!
//! Upstream `config.ts` depends on a number of modules that have not been ported
//! yet (`legend`, `selection`, `header`, `compositemark`) plus on the
//! `mergeConfig` / `writeConfig` helpers from `vega-util`.  The faithful copies
//! of the small runtime pieces needed by the config module live here.

use serde_json::{Map, Value};

use crate::util::{array, is_empty, keys};

/// `AXIS_CONFIGS` from `src/axis.ts`, preserving upstream insertion order.
pub const AXIS_CONFIGS: [&str; 22] = [
    "axis",
    "axisBand",
    "axisBottom",
    "axisDiscrete",
    "axisLeft",
    "axisPoint",
    "axisQuantitative",
    "axisRight",
    "axisTemporal",
    "axisTop",
    "axisX",
    "axisXBand",
    "axisXDiscrete",
    "axisXPoint",
    "axisXQuantitative",
    "axisXTemporal",
    "axisY",
    "axisYBand",
    "axisYDiscrete",
    "axisYPoint",
    "axisYQuantitative",
    "axisYTemporal",
];

/// `HEADER_CONFIGS` from `src/header.ts`, preserving upstream insertion order.
pub const HEADER_CONFIGS: [&str; 4] = ["header", "headerRow", "headerColumn", "headerFacet"];

/// `SELECTION_ID` from `src/selection.ts`.
pub const SELECTION_ID: &str = "_vgsid_";

/// `defaultLegendConfig` from `src/legend.ts`.
pub fn default_legend_config() -> Value {
    serde_json::json!({
        "gradientHorizontalMaxLength": 200,
        "gradientHorizontalMinLength": 100,
        "gradientVerticalMaxLength": 200,
        "gradientVerticalMinLength": 64,
        "unselectedOpacity": 0.35,
    })
}

/// `defaultScaleConfig` from `src/scale.ts`.
pub fn default_scale_config() -> Value {
    serde_json::json!({
        "pointPadding": 0.5,
        "barBandPaddingInner": 0.1,
        "rectBandPaddingInner": 0,
        "tickBandPaddingInner": 0.25,
        "bandWithNestedOffsetPaddingInner": 0.2,
        "bandWithNestedOffsetPaddingOuter": 0.2,
        "minBandSize": 2,
        "minFontSize": 8,
        "maxFontSize": 40,
        "minOpacity": 0.3,
        "maxOpacity": 0.8,
        "minSize": 4,
        "minStrokeWidth": 1,
        "maxStrokeWidth": 4,
        "quantileCount": 4,
        "quantizeCount": 4,
        "zero": true,
        "framesPerSecond": 2,
        "animationDuration": 5,
    })
}

/// `defaultConfig` from `src/selection.ts`.
pub fn default_selection_config() -> Value {
    serde_json::json!({
        "point": {
            "on": "click",
            "fields": [SELECTION_ID],
            "toggle": "event.shiftKey",
            "resolve": "global",
            "clear": "dblclick",
        },
        "interval": {
            "on": "[pointerdown, window:pointerup] > window:pointermove!",
            "encodings": ["x", "y"],
            "translate": "[pointerdown, window:pointerup] > window:pointermove!",
            "zoom": "wheel!",
            "mark": {"fill": "#333", "fillOpacity": 0.125, "stroke": "white"},
            "resolve": "global",
            "clear": "dblclick",
        },
    })
}

/// `isConditionalAxisValue` from `src/axis.ts` (`return v?.condition`).
///
/// Note: this is intentionally *not* `crate::axis::is_conditional_axis_value`,
/// whose port also treats `{expr: ...}` and arrays as conditional.  Upstream
/// only checks for a truthy `condition` property.
pub fn is_conditional_axis_value(v: &Value) -> bool {
    match v {
        Value::Object(map) => map.get("condition").map(truthy).unwrap_or(false),
        _ => false,
    }
}

/// JavaScript truthiness for JSON values.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `vega-util`'s `isObject` (`obj === Object(obj)`): arrays count as objects.
pub fn is_object(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Array(_))
}

/// `vega-util`'s `isEmpty`: no own enumerable keys.
pub fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Object(map) => is_empty(map),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

fn is_legal_key(key: &str) -> bool {
    key != "__proto__" && key != "constructor" && key != "prototype"
}

/// Recursion strategy of `vega-util`'s `mergeConfig`.
#[derive(Clone, Copy)]
pub enum RecurseStrategy<'a> {
    None,
    /// `true`: recurse into every property of the value.
    All,
    /// `{key: 1}`: recurse into the listed properties only.
    Keys(&'a [&'a str]),
}

/// `vega-util`'s `writeConfig`.
pub fn write_config(
    output: &mut Map<String, Value>,
    key: &str,
    value: &Value,
    recurse: RecurseStrategy<'_>,
) {
    if !is_legal_key(key) {
        return;
    }

    if let Value::Object(value_obj) = value {
        if !output.get(key).map(Value::is_object).unwrap_or(false) {
            output.insert(key.to_string(), Value::Object(Map::new()));
        }
        let out = output.get_mut(key).unwrap().as_object_mut().unwrap();
        for (k, v) in value_obj {
            if !is_legal_key(k) {
                continue;
            }
            let do_recurse = match recurse {
                RecurseStrategy::None => false,
                RecurseStrategy::All => true,
                RecurseStrategy::Keys(keys) => keys.contains(&k.as_str()),
            };
            if do_recurse {
                write_config(out, k, v, RecurseStrategy::None);
            } else {
                out.insert(k.clone(), v.clone());
            }
        }
    } else {
        output.insert(key.to_string(), value.clone());
    }
}

/// `mergeNamed` from `vega-util`'s `mergeConfig`: `b` (the source) comes first
/// and takes precedence over `a` for signals with the same name.
fn merge_named(a: Option<&Value>, b: Option<&Value>) -> Value {
    let a_items = a.map(array).unwrap_or_default();
    let b_items = b.map(array).unwrap_or_default();

    if a.is_none() {
        return Value::Array(b_items);
    }
    if b.is_none() {
        return Value::Array(a_items);
    }

    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let add = |item: &Value, out: &mut Vec<Value>, seen: &mut Vec<String>| {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !seen.iter().any(|n| n == &name) {
            seen.push(name);
            out.push(item.clone());
        }
    };

    for item in &b_items {
        add(item, &mut out, &mut seen);
    }
    for item in &a_items {
        add(item, &mut out, &mut seen);
    }

    Value::Array(out)
}

/// `vega-util`'s `mergeConfig`.
pub fn merge_config(configs: &[Value]) -> Value {
    let mut out = Map::new();

    for source in configs {
        let source_obj = match source {
            Value::Object(map) => map,
            _ => continue,
        };

        for (key, value) in source_obj {
            if key == "signals" {
                let merged = merge_named(out.get("signals"), Some(value));
                out.insert("signals".to_string(), merged);
            } else {
                let strategy = match key.as_str() {
                    "legend" => RecurseStrategy::Keys(&["layout"]),
                    "style" => RecurseStrategy::All,
                    _ => RecurseStrategy::None,
                };
                write_config(&mut out, key, value, strategy);
            }
        }
    }

    Value::Object(out)
}

/// `assembleParameterSignals` from `src/parameter.ts`.
pub fn assemble_parameter_signals(params: &[Value]) -> Vec<Value> {
    let mut signals = Vec::new();

    for param in params {
        // Selection parameters are handled separately elsewhere.
        if crate::util::has_property(param, "select") {
            continue;
        }

        let mut rest = param.as_object().cloned().unwrap_or_default();
        let expr = rest.remove("expr");
        let bind = rest.remove("bind");

        match (bind, expr) {
            (Some(bind), Some(expr)) => {
                let mut signal = rest;
                signal.insert("bind".to_string(), bind);
                signal.insert("init".to_string(), expr);
                signals.push(Value::Object(signal));
            }
            (bind, expr) => {
                let mut signal = rest;
                if let Some(expr) = expr {
                    signal.insert("update".to_string(), expr);
                }
                if let Some(bind) = bind {
                    signal.insert("bind".to_string(), bind);
                }
                signals.push(Value::Object(signal));
            }
        }
    }

    signals
}

/// Convenience used by the config module: the keys of an object value.
pub fn object_keys(v: &Value) -> Vec<String> {
    keys(v)
}
