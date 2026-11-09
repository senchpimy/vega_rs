//! Port of `src/compile/axis/parse.ts`.

use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use crate::axis::{is_axis_property, is_conditional_axis_value};
use crate::channel::POSITION_SCALE_CHANNELS;
use crate::channeldef::get_field_or_datum_def;
use crate::channeldef_support::truthy;
use crate::compile::model::{Model, SpecType};
use crate::compile::split::{default_tie_breaker, merge_values_with_explicit, Explicit};
use crate::compile_common::{is_signal_ref, merge_title_field_defs};
use crate::log;
use crate::util::deep_equal;

use super::component::{
    axis_component_properties_list, axis_index_from_value, axis_index_to_value, AxisComponent,
    AxisComponentIndex,
};
use super::config::{get_axis_config, get_axis_configs};
use super::encode::{guide_encode_entry, labels};
use super::format::{guide_format, guide_format_type};
use super::properties::{
    axis_rule, default_orient, get_field_def_title, get_label_angle, is_axis_rule, AxisRuleParams,
};

fn json_number(f: f64) -> Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9e15 {
        Value::Number(Number::from(f as i64))
    } else {
        Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null)
    }
}

/// Helper returning the raw specified axis (which may be `false`/`null`).
impl Model {
    /// `UnitModel.axis(channel)`.
    pub fn axis(&self, channel: &str) -> Option<Value> {
        if let Some(encoding) = self.encoding() {
            if let Some(channel_def) = encoding.get(channel) {
                if crate::channeldef::is_field_or_datum_def(channel_def) {
                    if let Some(axis) = channel_def.get("axis") {
                        if !truthy(axis) {
                            // `axis: null` / `axis: false` are stored verbatim.
                            return Some(axis.clone());
                        }
                    }
                }
            }
        }
        self.unit
            .specified_axes
            .as_object()
            .and_then(|axes| axes.get(channel).cloned())
    }
}

/// `parseUnitAxes`
pub fn parse_unit_axes(model: &Model) -> AxisComponentIndex {
    let mut index = AxisComponentIndex::new();
    for channel in POSITION_SCALE_CHANNELS {
        if model.component.scales.contains_key(channel) {
            index.insert(channel.to_string(), vec![parse_axis(channel, model)]);
        }
    }
    index
}

const OPPOSITE_ORIENT: [(&str, &str); 4] = [
    ("bottom", "top"),
    ("top", "bottom"),
    ("left", "right"),
    ("right", "left"),
];

fn opposite_orient(orient: &str) -> &'static str {
    OPPOSITE_ORIENT
        .iter()
        .find(|(o, _)| *o == orient)
        .map(|(_, opposite)| *opposite)
        .unwrap_or("bottom")
}

/// `parseLayerAxes`
pub fn parse_layer_axes(model: &mut Model) {
    let mut axes: AxisComponentIndex = axis_index_from_value(&model.component.axes);
    let mut axis_count: BTreeMap<String, i64> = BTreeMap::new();

    // `child.parseAxesAndHeaders()`
    for child in &mut model.children {
        parse_axes_and_headers(child);
    }

    let mut children_axes: Vec<AxisComponentIndex> = model
        .children
        .iter()
        .map(|child| axis_index_from_value(&child.component.axes))
        .collect();

    for child_index in &children_axes {
        for channel in child_index.keys() {
            let mode = parse_guide_resolve(&model.component.resolve, channel);
            model
                .component
                .resolve
                .axis
                .insert(channel.clone(), Value::String(mode.clone()));
            if mode == "shared" {
                let merged = merge_axis_components(axes.get(channel), &child_index[channel]);
                match merged {
                    Some(merged) => {
                        axes.insert(channel.clone(), merged);
                    }
                    None => {
                        model
                            .component
                            .resolve
                            .axis
                            .insert(channel.clone(), Value::String("independent".to_string()));
                        axes.remove(channel);
                    }
                }
            }
        }
    }

    for channel in POSITION_SCALE_CHANNELS {
        let mode = model
            .component
            .resolve
            .axis
            .get(channel)
            .and_then(Value::as_str)
            .unwrap_or("shared")
            .to_string();

        for child_index in &mut children_axes {
            let Some(child_axes) = child_index.remove(channel) else {
                continue;
            };

            if mode == "independent" {
                let entry = axes.entry(channel.to_string()).or_default();
                for mut axis_component in child_axes {
                    if let Some(orient_value) = axis_component.get("orient") {
                        if is_signal_ref(&orient_value) {
                            entry.push(axis_component);
                            continue;
                        }
                        let orient = orient_value.as_str().unwrap_or("").to_string();
                        let explicit = axis_component.explicit.contains_key("orient");
                        let count = axis_count.get(&orient).copied().unwrap_or(0);
                        if count > 0 && !explicit {
                            let opposite = opposite_orient(&orient);
                            if count > axis_count.get(opposite).copied().unwrap_or(0) {
                                axis_component.set("orient", Value::String(opposite.to_string()), false);
                            }
                        }
                        *axis_count.entry(orient).or_insert(0) += 1;
                    }
                    entry.push(axis_component);
                }
            }
        }

        // Show gridlines for the first axis only for dual-axis charts.
        if mode == "independent" {
            if let Some(list) = axes.get_mut(channel) {
                if list.len() > 1 {
                    for (index, axis_component) in list.iter_mut().enumerate() {
                        if index > 0
                            && axis_component.get("grid").map(|v| truthy(&v)).unwrap_or(false)
                            && !axis_component.explicit.contains_key("grid")
                        {
                            axis_component
                                .implicit
                                .insert("grid".to_string(), Value::Bool(false));
                        }
                    }
                }
            }
        }
    }

    for (i, child_index) in children_axes.iter().enumerate() {
        if let Some(child) = model.children.get_mut(i) {
            child.component.axes = axis_index_to_value(child_index);
        }
    }

    model.component.axes = axis_index_to_value(&axes);
}

fn merge_axis_components(
    merged_axis_cmpts: Option<&Vec<AxisComponent>>,
    child_axis_cmpts: &[AxisComponent],
) -> Option<Vec<AxisComponent>> {
    match merged_axis_cmpts {
        Some(merged) => {
            if merged.len() != child_axis_cmpts.len() {
                return None;
            }
            let mut out = merged.clone();
            for (merged_component, child_component) in out.iter_mut().zip(child_axis_cmpts) {
                let merged_orient = merged_component.get_with_explicit("orient");
                let child_orient = child_component.get_with_explicit("orient");

                if merged_orient.explicit
                    && child_orient.explicit
                    && merged_orient.value != child_orient.value
                {
                    return None;
                }
                *merged_component = merge_axis_component(merged_component, child_component);
            }
            Some(out)
        }
        None => Some(child_axis_cmpts.iter().map(|c| c.clone_component()).collect()),
    }
}

fn merge_title_component(v1: &Explicit, v2: &Explicit) -> Explicit {
    let v1_val = v1.value.as_ref();
    let v2_val = v2.value.as_ref();

    if v1_val.map(|v| v.is_null()).unwrap_or(true) || v2_val.map(|v| v.is_null()).unwrap_or(false) {
        return Explicit {
            explicit: v1.explicit,
            value: Some(Value::Null),
        };
    }

    let v1_text = v1_val.map(crate::title::is_text).unwrap_or(false);
    let v2_text = v2_val.map(crate::title::is_text).unwrap_or(false);
    let v1_signal = v1_val.map(is_signal_ref).unwrap_or(false);
    let v2_signal = v2_val.map(is_signal_ref).unwrap_or(false);

    if (v1_text || v1_signal) && (v2_text || v2_signal) {
        return Explicit {
            explicit: v1.explicit,
            value: Some(crate::compile_common::merge_title(
                v1_val.unwrap(),
                v2_val.unwrap(),
            )),
        };
    } else if v1_text || v1_signal {
        return Explicit {
            explicit: v1.explicit,
            value: v1_val.cloned(),
        };
    } else if v2_text || v2_signal {
        return Explicit {
            explicit: v1.explicit,
            value: v2_val.cloned(),
        };
    }

    // Both are arrays of field defs.
    let f1 = v1_val
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let f2 = v2_val
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Explicit {
        explicit: v1.explicit,
        value: Some(Value::Array(merge_title_field_defs(&f1, &f2))),
    }
}

fn merge_axis_component(merged: &AxisComponent, child: &AxisComponent) -> AxisComponent {
    let mut out = merged.clone();
    for prop in axis_component_properties_list() {
        let merged_value_with_explicit = merge_values_with_explicit(
            Some(merged.get_with_explicit(&prop)),
            child.get_with_explicit(&prop),
            &prop,
            "axis",
            |v1: &Explicit, v2: &Explicit, property: &str, property_of: &str| match property {
                "title" => merge_title_component(v1, v2),
                "gridScale" => Explicit {
                    explicit: v1.explicit,
                    value: v1.value.clone().or_else(|| v2.value.clone()),
                },
                _ => default_tie_breaker(v1, v2, property, property_of),
            },
        );
        out.set_with_explicit(&prop, merged_value_with_explicit);
    }
    out
}

fn value_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(x), Some(y)) => deep_equal(x, y),
        _ => false,
    }
}

fn is_explicit(
    value: Option<&Value>,
    property: &str,
    axis: &Value,
    model: &Model,
    channel: &str,
) -> bool {
    if property == "disable" {
        return model.axis(channel).is_some();
    }

    match property {
        "titleAngle" | "labelAngle" => {
            let comparison = match axis.get("labelAngle") {
                Some(v) if is_signal_ref(v) => Some(v.clone()),
                Some(v) => v
                    .as_f64()
                    .map(|a| json_number(((a % 360.0) + 360.0) % 360.0)),
                None => None,
            };
            if value_eq(value, comparison.as_ref()) {
                return true;
            }
        }
        "values" => {
            if axis.get("values").map(truthy).unwrap_or(false) {
                return true;
            }
        }
        "encode" => {
            if axis.get("encoding").is_some() || axis.get("labelAngle").is_some() {
                return true;
            }
        }
        "title" => {
            let field_def_title = get_field_def_title(model, channel);
            if value_eq(value, field_def_title.as_ref()) {
                return true;
            }
        }
        _ => {}
    }

    value_eq(value, axis.get(property))
}

fn config_orient(config: &Value, key: &str) -> Option<Value> {
    config
        .get(key)
        .and_then(|c| c.get("orient"))
        .filter(|v| truthy(v))
        .cloned()
}

fn parse_axis(channel: &str, model: &Model) -> AxisComponent {
    let axis_spec = model.axis(channel);
    let axis_defined = axis_spec.is_some();

    let mut axis_component = AxisComponent::default();

    let field_or_datum_def = model
        .encoding()
        .and_then(|encoding| get_field_or_datum_def(&encoding[channel]));

    let config = &model.config;
    let null_value = Value::Null;
    let mark = model.mark_type();

    let axis_orient = axis_spec
        .as_ref()
        .and_then(|axis| axis.get("orient"))
        .filter(|v| truthy(v))
        .cloned();
    let orient = axis_orient
        .or_else(|| config_orient(config, if channel == "x" { "axisX" } else { "axisY" }))
        .or_else(|| config_orient(config, "axis"))
        .unwrap_or_else(|| default_orient(channel));

    let scale_type = model.get_scale_component(channel).and_then(|c| c.get_type());

    let axis_configs = get_axis_configs(channel, scale_type, &orient, config);

    let style_config_index = config.get("style").unwrap_or(&null_value);

    let disable = match &axis_spec {
        Some(axis) => !truthy(axis),
        None => get_axis_config("disable", style_config_index, None, Some(&axis_configs))
            .config_value
            .map(|v| truthy(&v))
            .unwrap_or(false),
    };
    axis_component.set("disable", Value::Bool(disable), axis_defined);
    if disable {
        return axis_component;
    }

    let axis = axis_spec.unwrap_or_else(|| Value::Object(Map::new()));

    let label_angle = get_label_angle(
        field_or_datum_def.as_ref(),
        &axis,
        channel,
        style_config_index,
        Some(&axis_configs),
    );
    let format_type = guide_format_type(
        axis.get("formatType"),
        field_or_datum_def.as_ref(),
        scale_type,
    );
    let format = guide_format(
        field_or_datum_def.as_ref(),
        field_or_datum_def
            .as_ref()
            .and_then(|d| d.get("type"))
            .and_then(Value::as_str),
        axis.get("format"),
        axis.get("formatType"),
        config,
        true,
    );

    let rule_params = AxisRuleParams {
        field_or_datum_def: field_or_datum_def.clone(),
        axis: axis.clone(),
        channel: channel.to_string(),
        model,
        mark: mark.clone(),
        scale_type,
        orient: orient.clone(),
        label_angle: label_angle.clone(),
        format: format.clone(),
        format_type: format_type.clone(),
        config: config.clone(),
    };

    const PROPS_TO_ALWAYS_INCLUDE_CONFIG: [&str; 9] = [
        "grid",
        "translate",
        "format",
        "formatType",
        "orient",
        "labelExpr",
        "tickCount",
        "position",
        "tickMinStep",
    ];

    for property in axis_component_properties_list() {
        let value = if is_axis_rule(&property) {
            axis_rule(&property, &rule_params)
        } else if is_axis_property(&property) {
            axis.get(&property).cloned()
        } else {
            None
        };

        let has_value = value.is_some();
        let explicit = is_explicit(value.as_ref(), &property, &axis, model, channel);

        if has_value && explicit {
            axis_component.set(&property, value.unwrap(), explicit);
        } else {
            let (config_value, config_from) = if is_axis_property(&property) && property != "values" {
                let result = get_axis_config(
                    &property,
                    style_config_index,
                    axis.get("style"),
                    Some(&axis_configs),
                );
                (result.config_value, result.config_from)
            } else {
                (None, None)
            };
            let has_config_value = config_value.is_some();

            if has_value && !has_config_value {
                axis_component.set(&property, value.unwrap(), explicit);
            } else if let Some(config_value) = config_value {
                if config_from.as_deref() != Some("vgAxisConfig")
                    || (PROPS_TO_ALWAYS_INCLUDE_CONFIG.contains(&property.as_str())
                        && has_config_value)
                    || is_conditional_axis_value(&config_value)
                    || is_signal_ref(&config_value)
                {
                    axis_component.set(&property, config_value, false);
                }
            }
        }
    }

    // 2) Add guide encode definition groups.
    let axis_encoding = axis
        .get("encoding")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut axis_encode = Map::new();
    for part in crate::axis::AXIS_PARTS {
        if !axis_component.has_axis_part(part) {
            continue;
        }
        let part_encoding = axis_encoding
            .get(part)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let axis_encoding_part = guide_encode_entry(&part_encoding, model);
        let value = if part == "labels" {
            labels(model, channel, &axis_encoding_part)
        } else {
            axis_encoding_part
        };

        if !value.is_empty() {
            axis_encode.insert(part.to_string(), serde_json::json!({ "update": Value::Object(value) }));
        }
    }

    if !axis_encode.is_empty() {
        let explicit = axis.get("encoding").is_some() || axis.get("labelAngle").is_some();
        axis_component.set("encode", Value::Object(axis_encode), explicit);
    }

    axis_component
}

/// `parseGuideResolve`
fn parse_guide_resolve(resolve: &crate::compile::support::Resolve, channel: &str) -> String {
    let channel_scale_resolve = resolve.get_scale(channel);
    let guide = if crate::channel::is_x_or_y(channel) {
        "axis"
    } else {
        "legend"
    };

    if channel_scale_resolve.as_deref() == Some("independent") {
        if resolve
            .axis
            .get(channel)
            .and_then(Value::as_str)
            == Some("shared")
        {
            log::warn(format!(
                "Setting the scale to be independent for \"{channel}\" means we also have to set the guide (axis or legend) to be independent."
            ));
        }
        return "independent".to_string();
    }

    let _ = guide;
    resolve
        .axis
        .get(channel)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "shared".to_string())
}

/// `parseAxesAndHeaders` for any model type.
pub fn parse_axes_and_headers(model: &mut Model) {
    match model.type_ {
        SpecType::Unit => {
            let index = parse_unit_axes(model);
            model.component.axes = axis_index_to_value(&index);
        }
        SpecType::Layer => parse_layer_axes(model),
        SpecType::Facet => {
            for child in &mut model.children {
                parse_axes_and_headers(child);
            }
            crate::compile::header::parse::parse_facet_headers(model);
        }
        SpecType::Concat => {
            for child in &mut model.children {
                parse_axes_and_headers(child);
            }
        }
    }
}

/// Convenience used by the ported tests: assemble axes from a model whose
/// axis components have already been parsed.
pub fn assemble_model_axes(model: &Model) -> Vec<Value> {
    super::assemble::assemble_axes_from_value(&model.component.axes, &model.config)
}
