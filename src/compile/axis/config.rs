//! Port of `src/compile/axis/config.ts`.

use serde_json::{Map, Value};

use crate::compile_common::is_signal_ref;
use crate::scale::{is_quantitative, ScaleType};
use crate::util::{array, string_value, title_case};

/// Port of `AxisConfigs`.
#[derive(Debug, Clone, Default)]
pub struct AxisConfigs {
    pub vl_only_axis_config: Value,
    pub vg_axis_config: Value,
    pub axis_config_style: Value,
}

/// Port of `getAxisConfigFromConfigTypes`.
fn get_axis_config_from_config_types(
    config_types: &[String],
    config: &Value,
    channel: &str,
    orient: &Value,
) -> Value {
    let mut out = Map::new();
    for config_type in config_types {
        if config_type == "axisOrient" {
            let orient1 = if channel == "x" { "bottom" } else { "left" };
            let (config1_key, config2_key) = if channel == "x" {
                ("axisBottom", "axisTop")
            } else {
                ("axisLeft", "axisRight")
            };
            let empty = Value::Object(Map::new());
            let orient_config1 = config.get(config1_key).unwrap_or(&empty);
            let orient_config2 = config.get(config2_key).unwrap_or(&empty);

            let mut props: Vec<String> = orient_config1
                .as_object()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            for prop in orient_config2
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
            {
                if !props.contains(&prop) {
                    props.push(prop);
                }
            }

            let signal = orient
                .get("signal")
                .and_then(Value::as_str)
                .unwrap_or_default();
            for prop in props {
                let v1 = orient_config1.get(&prop).unwrap_or(&Value::Null);
                let v2 = orient_config2.get(&prop).unwrap_or(&Value::Null);
                let s1 = signal_or_string_value_string(v1);
                let s2 = signal_or_string_value_string(v2);
                out.insert(
                    prop,
                    serde_json::json!({
                        "signal": format!("{signal} === \"{orient1}\" ? {s1} : {s2}")
                    }),
                );
            }
            continue;
        }

        if let Some(value) = config.get(config_type) {
            if let Some(map) = value.as_object() {
                for (k, v) in map {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }
    Value::Object(out)
}

/// Like `signalOrStringValue` but rendered as the raw expression fragment
/// (strings are not quoted, matching `vega-util`'s `stringValue`).
fn signal_or_string_value_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::String(s) => s.clone(),
        Value::Object(_) if is_signal_ref(v) => v
            .get("signal")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        other => string_value(other),
    }
}

/// `AxisConfigs`
pub fn get_axis_configs(
    channel: &str,
    scale_type: Option<ScaleType>,
    orient: &Value,
    config: &Value,
) -> AxisConfigs {
    use ScaleType::*;

    let type_based_config_types: Vec<String> = match scale_type {
        Some(Band) => vec!["axisDiscrete".into(), "axisBand".into()],
        Some(Point) => vec!["axisDiscrete".into(), "axisPoint".into()],
        Some(t) if is_quantitative(t) => vec!["axisQuantitative".into()],
        Some(Time) | Some(Utc) => vec!["axisTemporal".into()],
        _ => vec![],
    };

    let axis_channel = if channel == "x" { "axisX" } else { "axisY" };
    let axis_orient = if is_signal_ref(orient) {
        "axisOrient".to_string()
    } else {
        format!("axis{}", title_case(orient.as_str().unwrap_or("")))
    };

    let mut vl_only_config_types: Vec<String> = type_based_config_types.clone();
    for c in &type_based_config_types {
        // c.substr(4) — skip the "axis" prefix
        vl_only_config_types.push(format!("{}{}", axis_channel, &c[4..]));
    }

    let vg_config_types = vec!["axis".to_string(), axis_orient, axis_channel.to_string()];

    let mut pill: Vec<String> = vg_config_types.clone();
    pill.extend(vl_only_config_types.clone());

    AxisConfigs {
        vl_only_axis_config: get_axis_config_from_config_types(
            &vl_only_config_types,
            config,
            channel,
            orient,
        ),
        vg_axis_config: get_axis_config_from_config_types(&vg_config_types, config, channel, orient),
        axis_config_style: get_axis_config_style(&pill, config),
    }
}

/// Port of `getAxisConfigStyle`.
pub fn get_axis_config_style(axis_config_types: &[String], config: &Value) -> Value {
    let mut merged = Map::new();
    for config_type in axis_config_types {
        let style = config.get(config_type).and_then(|c| c.get("style"));
        if let Some(style) = style {
            for s in array(style) {
                let name = match s {
                    Value::String(s) => s,
                    _ => continue,
                };
                if let Some(style_config) = config.get("style").and_then(|v| v.get(&name)) {
                    if let Some(map) = style_config.as_object() {
                        for (k, v) in map {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }
    }
    Value::Object(merged)
}

/// Result of [`get_axis_config`].
#[derive(Debug, Clone, Default)]
pub struct AxisConfigValue {
    pub config_from: Option<String>,
    pub config_value: Option<Value>,
}

/// `getStyleConfig(prop, style, styleConfigIndex)`
fn get_style_config(prop: &str, style: Option<&Value>, style_config_index: &Value) -> Option<Value> {
    let style = style?;
    let mut value = None;
    for s in array(style) {
        let name = match s {
            Value::String(s) => s,
            _ => continue,
        };
        if let Some(style_config) = style_config_index.get(&name) {
            if style_config
                .as_object()
                .map(|m| m.contains_key(prop))
                .unwrap_or(false)
            {
                value = style_config.get(prop).cloned();
            }
        }
    }
    value
}

/// Port of `getAxisConfig`.
pub fn get_axis_config(
    property: &str,
    style_config_index: &Value,
    style: Option<&Value>,
    axis_configs: Option<&AxisConfigs>,
) -> AxisConfigValue {
    if let Some(style_config) = get_style_config(property, style, style_config_index) {
        return AxisConfigValue {
            config_from: Some("style".to_string()),
            config_value: Some(style_config),
        };
    }

    if let Some(axis_configs) = axis_configs {
        for (config_from, config) in [
            ("vlOnlyAxisConfig", &axis_configs.vl_only_axis_config),
            ("vgAxisConfig", &axis_configs.vg_axis_config),
            ("axisConfigStyle", &axis_configs.axis_config_style),
        ] {
            if let Some(value) = config.get(property) {
                return AxisConfigValue {
                    config_from: Some(config_from.to_string()),
                    config_value: Some(value.clone()),
                };
            }
        }
    }

    AxisConfigValue::default()
}
