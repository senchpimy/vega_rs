//! Port of `src/compile/axis/assemble.ts`.

use serde_json::{json, Map, Value};

use crate::axis::{AXIS_PARTS, AXIS_PROPERTIES};
use crate::channel::POSITION_SCALE_CHANNELS;
use crate::channeldef::default_title;
use crate::compile::model::Model;
use crate::compile_common::{expr_from_value_ref_or_signal_ref, is_signal_ref};
use crate::compile_predicate::expression;
use crate::title::is_text;
use crate::channeldef_support::truthy;
use crate::util::{array, has_property, replace_all};

use super::component::{axis_index_from_value, AxisComponent, AxisComponentIndex};

/// `AXIS_PROPERTY_TYPE[prop]`
fn axis_property_type(prop: &str) -> Option<&'static str> {
    let t = match prop {
        "grid" | "gridCap" | "gridColor" | "gridDash" | "gridDashOffset" | "gridOpacity"
        | "gridScale" | "gridWidth" => "grid",
        "orient" => "main",
        "bandPosition" => "both",
        "offset" | "tickMinStep" | "tickOffset" | "tickRound" | "tickWidth" => "both",
        "encode" | "scale" | "tickBand" | "tickCount" | "tickExtra" | "translate" | "values"
        | "zindex" => "both",
        // Remaining known axis properties are main-axis only.
        p if AXIS_PROPERTIES.contains(&p) => "main",
        _ => return None,
    };
    Some(t)
}

/// `CONDITIONAL_AXIS_PROP_INDEX` — `Some(Some((part, vgProp)))` when the
/// property is encoded through a Vega encode block, `Some(None)` when the
/// condition must be converted to a signal, `None` when not conditional.
fn conditional_axis_prop_index(prop: &str) -> Option<Option<(&'static str, &'static str)>> {
    let value = match prop {
        "labelAlign" => Some(("labels", "align")),
        "labelBaseline" => Some(("labels", "baseline")),
        "labelColor" => Some(("labels", "fill")),
        "labelFont" => Some(("labels", "font")),
        "labelFontSize" => Some(("labels", "fontSize")),
        "labelFontStyle" => Some(("labels", "fontStyle")),
        "labelFontWeight" => Some(("labels", "fontWeight")),
        "labelOpacity" => Some(("labels", "opacity")),
        "labelOffset" => None,
        "labelPadding" => None,
        "gridColor" => Some(("grid", "stroke")),
        "gridDash" => Some(("grid", "strokeDash")),
        "gridDashOffset" => Some(("grid", "strokeDashOffset")),
        "gridOpacity" => Some(("grid", "opacity")),
        "gridWidth" => Some(("grid", "strokeWidth")),
        "tickColor" => Some(("ticks", "stroke")),
        "tickDash" => Some(("ticks", "strokeDash")),
        "tickDashOffset" => Some(("ticks", "strokeDashOffset")),
        "tickOpacity" => Some(("ticks", "opacity")),
        "tickSize" => None,
        "tickWidth" => Some(("ticks", "strokeWidth")),
        _ => return None,
    };
    Some(value)
}

/// `setAxisEncode`
fn set_axis_encode(
    axis: &mut Map<String, Value>,
    part: &str,
    vg_prop: &str,
    vg_ref: Value,
) {
    let encode = axis
        .entry("encode".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !encode.is_object() {
        *encode = Value::Object(Map::new());
    }
    let encode_obj = encode.as_object_mut().unwrap();
    let part_value = encode_obj
        .entry(part.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !part_value.is_object() {
        *part_value = Value::Object(Map::new());
    }
    let part_obj = part_value.as_object_mut().unwrap();
    let update = part_obj
        .entry("update".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !update.is_object() {
        *update = Value::Object(Map::new());
    }
    update
        .as_object_mut()
        .unwrap()
        .insert(vg_prop.to_string(), vg_ref);
}

fn rest_without(value: &Value, key: &str) -> Map<String, Value> {
    value
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| k.as_str() != key)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn assemble_title(title: Option<&Value>, config: &Value) -> Option<Value> {
    let title = title?;
    if !truthy(title) {
        return None;
    }
    if title.is_array() && !is_text(title) {
        let parts: Vec<String> = title
            .as_array()
            .unwrap()
            .iter()
            .map(|field_def| default_title(field_def, config))
            .collect();
        return Some(Value::String(parts.join(", ")));
    }
    Some(title.clone())
}

/// `assembleAxis`
pub fn assemble_axis(
    axis_cmpt: &AxisComponent,
    kind: &str,
    config: &Value,
    header: bool,
) -> Option<Value> {
    let combined = axis_cmpt.combine();
    let mut map: Map<String, Value> = combined.into_iter().collect();

    let disable = map.remove("disable");
    let orient = map.remove("orient");
    let scale = map.remove("scale");
    let label_expr = map.remove("labelExpr");
    let title = map.remove("title");
    let zindex = map.remove("zindex");

    if disable.map(|v| truthy(&v)).unwrap_or(false) {
        return None;
    }

    let props: Vec<String> = map.keys().cloned().collect();
    for prop in props {
        let prop_value = match map.get(&prop) {
            Some(v) => v.clone(),
            None => continue,
        };
        let prop_type = axis_property_type(&prop);
        if let Some(prop_type) = prop_type {
            if prop_type != kind && prop_type != "both" {
                map.remove(&prop);
                continue;
            }
        }

        if is_conditional_value(&prop_value) {
            let condition = prop_value.get("condition").cloned().unwrap_or(Value::Null);
            let conditions = array(&condition);
            let value_or_signal_ref = prop_value
                .as_object()
                .map(|_| Value::Object(rest_without(&prop_value, "condition")))
                .unwrap_or(Value::Null);

            match conditional_axis_prop_index(&prop) {
                Some(Some((part, vg_prop))) => {
                    let mut vg_refs: Vec<Value> = Vec::new();
                    for c in &conditions {
                        let mut entry = rest_without(c, "test");
                        entry.insert(
                            "test".to_string(),
                            Value::String(expression(c.get("test").unwrap_or(&Value::Null))),
                        );
                        vg_refs.push(Value::Object(entry));
                    }
                    vg_refs.push(value_or_signal_ref);
                    set_axis_encode(&mut map, part, vg_prop, Value::Array(vg_refs));
                    map.remove(&prop);
                }
                Some(None) => {
                    let mut signal = String::new();
                    for c in &conditions {
                        let value_or_signal_c_ref = rest_without(c, "test");
                        signal.push_str(&format!(
                            "{} ? {} : ",
                            expression(c.get("test").unwrap_or(&Value::Null)),
                            expr_from_value_ref_or_signal_ref(&Value::Object(
                                value_or_signal_c_ref
                            ))
                        ));
                    }
                    signal.push_str(&expr_from_value_ref_or_signal_ref(&value_or_signal_ref));
                    map.insert(prop.clone(), json!({ "signal": signal }));
                }
                None => {}
            }
        } else if is_signal_ref(&prop_value) {
            if let Some(Some((part, vg_prop))) = conditional_axis_prop_index(&prop) {
                set_axis_encode(&mut map, part, vg_prop, prop_value.clone());
                map.remove(&prop);
            }
        }

        // Do not pass labelAlign/Baseline = null to Vega.
        if (prop == "labelAlign" || prop == "labelBaseline")
            && map.get(&prop).map(|v| v.is_null()).unwrap_or(false)
        {
            map.remove(&prop);
        }
    }

    if kind == "grid" {
        if !map.get("grid").map(truthy).unwrap_or(false) {
            return None;
        }

        if let Some(encode) = map.get_mut("encode") {
            let grid = encode.get("grid").cloned();
            let mut new_encode = Map::new();
            if let Some(grid) = grid {
                new_encode.insert("grid".to_string(), grid);
            }
            if new_encode.is_empty() {
                map.remove("encode");
            } else {
                *encode = Value::Object(new_encode);
            }
        }

        let mut out = Map::new();
        if let Some(scale) = scale {
            out.insert("scale".to_string(), scale);
        }
        if let Some(orient) = orient {
            out.insert("orient".to_string(), orient);
        }
        for (k, v) in map {
            out.insert(k, v);
        }
        out.insert("domain".to_string(), Value::Bool(false));
        out.insert("labels".to_string(), Value::Bool(false));
        out.insert("aria".to_string(), Value::Bool(false));
        out.insert("maxExtent".to_string(), json!(0));
        out.insert("minExtent".to_string(), json!(0));
        out.insert("ticks".to_string(), Value::Bool(false));
        out.insert(
            "zindex".to_string(),
            zindex.unwrap_or_else(|| json!(0)),
        );
        return Some(Value::Object(out));
    }

    // kind === 'main'
    if !header && axis_cmpt.main_extracted {
        return None;
    }

    if let Some(label_expr) = &label_expr {
        let mut expr = label_expr.as_str().unwrap_or_default().to_string();
        let text_signal = map
            .get("encode")
            .and_then(|e| e.get("labels"))
            .and_then(|l| l.get("update"))
            .and_then(|u| u.get("text"))
            .filter(|t| is_signal_ref(t))
            .and_then(|t| t.get("signal"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(signal) = text_signal {
            expr = replace_all(&expr, "datum.label", &signal);
        }
        set_axis_encode(&mut map, "labels", "text", json!({ "signal": expr }));
    }

    if map.get("labelAlign").map(Value::is_null).unwrap_or(false) {
        map.remove("labelAlign");
    }

    let should_remove_encode = if let Some(encode) = map.get_mut("encode") {
        let to_remove: Vec<String> = AXIS_PARTS
            .iter()
            .filter(|part| !axis_cmpt.has_axis_part(part))
            .map(|part| part.to_string())
            .filter(|part| encode.get(part).is_some())
            .collect();
        if let Some(encode_obj) = encode.as_object_mut() {
            for part in to_remove {
                encode_obj.remove(&part);
            }
        }
        encode.as_object().map(|m| m.is_empty()).unwrap_or(false)
    } else {
        false
    };
    if should_remove_encode {
        map.remove("encode");
    }

    let title_string = assemble_title(title.as_ref(), config);

    let mut out = Map::new();
    if let Some(scale) = scale {
        out.insert("scale".to_string(), scale);
    }
    if let Some(orient) = orient {
        out.insert("orient".to_string(), orient);
    }
    out.insert("grid".to_string(), Value::Bool(false));
    if let Some(title) = &title_string {
        if truthy(title) {
            out.insert("title".to_string(), title.clone());
        }
    }
    for (k, v) in map {
        out.insert(k, v);
    }
    if config.get("aria").and_then(Value::as_bool) == Some(false) {
        out.insert("aria".to_string(), Value::Bool(false));
    }
    out.insert(
        "zindex".to_string(),
        zindex.unwrap_or_else(|| json!(0)),
    );

    Some(Value::Object(out))
}

fn is_conditional_value(value: &Value) -> bool {
    has_property(value, "condition")
}

/// `assembleAxisSignals`
pub fn assemble_axis_signals(model: &Model) -> Vec<Value> {
    let axes = axis_index_from_value(&model.component.axes);
    let mut signals = Vec::new();

    for channel in POSITION_SCALE_CHANNELS {
        if let Some(channel_axes) = axes.get(channel) {
            for axis in channel_axes {
                let disabled = axis.get("disable").map(|v| truthy(&v)).unwrap_or(false);
                let has_grid_scale = axis.get("gridScale").is_some();
                if !disabled && !has_grid_scale {
                    let size_type = if channel == "x" { "height" } else { "width" };
                    let size_ref = model.get_size_signal_ref(size_type);
                    let update = size_ref
                        .get("signal")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();

                    if size_type != update {
                        signals.push(json!({ "name": size_type, "update": update }));
                    }
                }
            }
        }
    }

    signals
}

/// `assembleAxes`
pub fn assemble_axes(axis_components: &AxisComponentIndex, config: &Value) -> Vec<Value> {
    let empty = Vec::new();
    let x = axis_components.get("x").unwrap_or(&empty);
    let y = axis_components.get("y").unwrap_or(&empty);

    let mut axes = Vec::new();
    for axis in x {
        if let Some(a) = assemble_axis(axis, "grid", config, false) {
            axes.push(a);
        }
    }
    for axis in y {
        if let Some(a) = assemble_axis(axis, "grid", config, false) {
            axes.push(a);
        }
    }
    for axis in x {
        if let Some(a) = assemble_axis(axis, "main", config, false) {
            axes.push(a);
        }
    }
    for axis in y {
        if let Some(a) = assemble_axis(axis, "main", config, false) {
            axes.push(a);
        }
    }
    axes
}

/// Convenience wrapper around [`assemble_axes`] for the JSON representation.
pub fn assemble_axes_from_value(value: &Value, config: &Value) -> Vec<Value> {
    let index = axis_index_from_value(value);
    assemble_axes(&index, config)
}
