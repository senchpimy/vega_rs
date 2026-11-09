//! Port of `src/compile/scale/range.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{
    get_offset_scale_channel, is_x_or_y, is_x_or_y_offset, ANGLE, COLOR, FILL,
    FILLOPACITY, OPACITY, RADIUS, SCALE_CHANNELS, SHAPE, STROKE, STROKEDASH,
    STROKEOPACITY, STROKEWIDTH, THETA, TIME, X, XOFFSET, YOFFSET,
};
use crate::channeldef::{is_field_def, is_field_or_datum_def};
use crate::scale::{
    channel_scale_property_incompatability, has_continuous_domain, has_discrete_domain,
    is_continuous_to_discrete, is_extended_scheme, scale_type_support_property, ScaleType,
};
use crate::type_::is_discrete;

use super::super::model::Model;
use super::super::split::{make_explicit, make_implicit, Explicit};
use super::super::support::{self, get_step_for, is_step};
use super::component::ScaleComponentIndex;

pub const RANGE_PROPERTIES: [&str; 2] = ["range", "scheme"];
pub const MAX_SIZE_RANGE_STEP_RATIO: f64 = 0.95;

/// Port of `parseUnitScaleRange`.
pub fn parse_unit_scale_range(model: &mut Model) {
    // use SCALE_CHANNELS instead of scales.keys() to ensure that x, y come first!
    for channel in SCALE_CHANNELS {
        if !model.component.scales.contains_key(channel) {
            continue;
        }
        let range = parse_range_for_channel(channel, model);
        if let Some(cmpt) = model.component.scales.get_mut(channel) {
            cmpt.set_with_explicit("range", range);
        }
    }
}

/// Port of `parseRangeForChannel`.
pub fn parse_range_for_channel(channel: &str, model: &mut Model) -> Explicit {
    let specified_scale = model
        .unit
        .specified_scales
        .get(channel)
        .cloned()
        .unwrap_or(Value::Null);
    let size = model.size.clone();
    let scale_type = model.get_scale_component(channel).and_then(|c| c.get_type());
    let scale_type_str = scale_type.map(|t| t.as_str()).unwrap_or("");

    for property in RANGE_PROPERTIES {
        let specified_value = specified_scale.get(property).cloned().unwrap_or(Value::Null);
        if specified_value.is_null() {
            continue;
        }
        let supported = scale_type
            .map(|t| scale_type_support_property(t, property))
            .unwrap_or(false);
        let incompat = channel_scale_property_incompatability(support::channel_static(channel), property);
        if !supported {
            crate::log::warn(support::scale_property_not_work_with_scale_type(
                scale_type_str,
                property,
                channel,
            ));
        } else if let Some(msg) = incompat {
            crate::log::warn(msg);
        } else {
            match property {
                "range" => {
                    if let Some(range) = specified_value.as_array() {
                        if is_x_or_y(channel) {
                            let mapped = range
                                .iter()
                                .map(|v| {
                                    if v.as_str() == Some("width") || v.as_str() == Some("height")
                                    {
                                        let size_signal = model.get_name(v.as_str().unwrap());
                                        let signal = model.get_signal_name(&size_signal);
                                        json!({"signal": signal})
                                    } else {
                                        v.clone()
                                    }
                                })
                                .collect();
                            return make_explicit(Value::Array(mapped));
                        }
                    } else if specified_value.is_object()
                        && specified_value.get("field").is_some()
                    {
                        let field = specified_value.get("field").cloned().unwrap_or(Value::Null);
                        let sort_field = model.vg_field(channel);
                        return make_explicit(json!({
                            "data": model.request_data_name("main"),
                            "field": field,
                            "sort": {"op": "min", "field": sort_field}
                        }));
                    }
                    return make_explicit(specified_value);
                }
                "scheme" => {
                    return make_explicit(parse_scheme(&specified_value));
                }
                _ => {}
            }
        }
    }

    let size_channel = if channel == X || channel == "xOffset" {
        "width"
    } else {
        "height"
    };
    let size_value = size.get(size_channel).cloned().unwrap_or(Value::Null);
    if is_step(&size_value) {
        if is_x_or_y(channel) {
            if let Some(st) = scale_type {
                if has_discrete_domain(st) {
                    let step = get_position_step(&size_value, model, channel);
                    if !step.is_null() {
                        return make_explicit(json!({"step": step}));
                    }
                } else {
                    crate::log::warn(support::step_dropped(size_channel));
                }
            }
        } else if is_x_or_y_offset(channel) {
            let position_channel = if channel == XOFFSET { "x" } else { "y" };
            let position_scale_type = model
                .get_scale_component(position_channel)
                .and_then(|c| c.get_type());
            if position_scale_type == Some(ScaleType::Band) {
                let step = get_offset_step(&size_value, scale_type);
                if !step.is_null() {
                    return make_explicit(step);
                }
            }
        }
    }

    let range_min = specified_scale.get("rangeMin").cloned().unwrap_or(Value::Null);
    let range_max = specified_scale.get("rangeMax").cloned().unwrap_or(Value::Null);
    let d = default_range(channel, model);

    if (!range_min.is_null() || !range_max.is_null())
        && scale_type
            .map(|t| scale_type_support_property(t, "rangeMin"))
            .unwrap_or(false)
        && d.as_array().map(|a| a.len() == 2).unwrap_or(false)
    {
        let arr = d.as_array().unwrap();
        return make_explicit(json!([
            if range_min.is_null() { arr[0].clone() } else { range_min },
            if range_max.is_null() { arr[1].clone() } else { range_max }
        ]));
    }

    make_implicit(d)
}

fn parse_scheme(scheme: &Value) -> Value {
    if is_extended_scheme(scheme) {
        let mut out = scheme.as_object().cloned().unwrap_or_default();
        let name = out.remove("name").unwrap_or(Value::Null);
        let mut result = Map::new();
        result.insert("scheme".to_string(), name);
        for (k, v) in out {
            result.insert(k, v);
        }
        Value::Object(result)
    } else {
        json!({"scheme": scheme})
    }
}

fn full_width_or_height_range(
    channel: &str,
    model: &Model,
    scale_type: Option<ScaleType>,
    center: bool,
) -> Value {
    let size_type = if channel == X { "width" } else { "height" };
    let size_signal = model.get_name(size_type);
    let get_signal_name = |model: &Model| model.get_signal_name(&size_signal);

    if channel == "y" && scale_type.map(has_continuous_domain).unwrap_or(false) {
        if center {
            Value::Array(vec![
                json!({"signal": format!("{}/2", get_signal_name(model))}),
                json!({"signal": format!("-{}/2", get_signal_name(model))}),
            ])
        } else {
            Value::Array(vec![
                json!({"signal": get_signal_name(model)}),
                json!(0),
            ])
        }
    } else if center {
        Value::Array(vec![
            json!({"signal": format!("-{}/2", get_signal_name(model))}),
            json!({"signal": format!("{}/2", get_signal_name(model))}),
        ])
    } else {
        Value::Array(vec![json!(0), json!({"signal": get_signal_name(model)})])
    }
}

fn default_range(channel: &str, model: &mut Model) -> Value {
    let config = &model.config;
    let null_value = Value::Null;
    let view_config = config.get("view").unwrap_or(&null_value);
    let scale_config = config.get("scale").unwrap_or(&null_value);
    let mark = model.mark_type();
    let type_ = model
        .encoding()
        .and_then(|e| crate::channeldef::get_field_or_datum_def_ref(&e[channel]))
        .and_then(|f| f.get("type").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    let scale_type = model.get_scale_component(channel).and_then(|c| c.get_type());
    let specified = model
        .unit
        .specified_scales
        .get(channel)
        .unwrap_or(&null_value);
    let domain_mid = specified.get("domainMid").cloned().unwrap_or(Value::Null);

    match channel {
        X | "y" => {
            if let Some(st) = scale_type {
                if starts_with_point_or_band(st) {
                    let position_size = get_discrete_position_size(channel, &model.size, view_config);
                    if is_step(&position_size) {
                        return json!({"step": get_position_step(&position_size, model, channel)});
                    }
                }
            }
            full_width_or_height_range(channel, model, scale_type, false)
        }
        XOFFSET | YOFFSET => get_offset_range(channel, model, scale_type),
        "size" => {
            let range_min = size_range_min(&mark, config);
            let range_max = size_range_max(&mark, &model.size, model, config);
            if let Some(st) = scale_type {
                if is_continuous_to_discrete(st) {
                    return interpolate_range(
                        &range_min,
                        &range_max,
                        default_continuous_to_discrete_count(
                            st,
                            config,
                            &specified.get("domain").cloned().unwrap_or(Value::Null),
                            channel,
                        ) as f64,
                    );
                }
            }
            json!([range_min, range_max])
        }
        THETA => json!([0, std::f64::consts::PI * 2.0]),
        ANGLE => json!([0, 360]),
        RADIUS => {
            let w = model.get_signal_name(if model.parent_type == Some(super::super::model::SpecType::Facet) {
                "child_width"
            } else {
                "width"
            });
            let h = model.get_signal_name(if model.parent_type == Some(super::super::model::SpecType::Facet) {
                "child_height"
            } else {
                "height"
            });
            json!([0, {"signal": format!("min({w},{h})/2")}])
        }
        TIME => {
            let fps = scale_config
                .get("framesPerSecond")
                .and_then(Value::as_f64)
                .unwrap_or(2.0);
            json!({"step": 1000.0 / fps})
        }
        STROKEWIDTH => json!([
            scale_config.get("minStrokeWidth").cloned().unwrap_or(json!(1)),
            scale_config.get("maxStrokeWidth").cloned().unwrap_or(json!(4))
        ]),
        STROKEDASH => json!([[1, 0], [4, 2], [2, 1], [1, 1], [1, 2, 4, 2]]),
        SHAPE => Value::String("symbol".to_string()),
        COLOR | FILL | STROKE => {
            if scale_type == Some(ScaleType::Ordinal) {
                Value::String(if type_ == "nominal" { "category" } else { "ordinal" }.to_string())
            } else if !domain_mid.is_null() {
                Value::String("diverging".to_string())
            } else if mark == "rect" || mark == "geoshape" {
                Value::String("heatmap".to_string())
            } else {
                Value::String("ramp".to_string())
            }
        }
        OPACITY | FILLOPACITY | STROKEOPACITY => json!([
            scale_config.get("minOpacity").cloned().unwrap_or(json!(0.3)),
            scale_config.get("maxOpacity").cloned().unwrap_or(json!(0.8))
        ]),
        _ => Value::Null,
    }
}

fn starts_with_point_or_band(st: ScaleType) -> bool {
    matches!(st, ScaleType::Point | ScaleType::Band)
}

fn get_position_step(step: &Value, model: &mut Model, channel: &str) -> Value {
    let null_value = Value::Null;
    let encoding = model.encoding().unwrap_or(&null_value);
    let offset_channel = get_offset_scale_channel(channel);
    let offset_def = offset_channel.map(|c| &encoding[c]).unwrap_or(&Value::Null);
    let offset_is_discrete = is_field_or_datum_def(offset_def)
        && offset_def
            .get("type")
            .and_then(Value::as_str)
            .map(|t| {
                t.parse::<crate::type_::Type>()
                    .map(is_discrete)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let step_for = get_step_for(step, offset_is_discrete);

    if step_for == "offset"
        && offset_channel
            .map(|c| crate::encoding::channel_has_field_or_datum(encoding, c))
            .unwrap_or(false)
    {
        let offset_channel = offset_channel.unwrap();
        let offset_scale_name = model.scale_name(offset_channel, false).unwrap_or_default();
        let mut step_count = format!("domain('{offset_scale_name}').length");
        if model
            .get_scale_component(offset_channel)
            .and_then(|c| c.get_type())
            == Some(ScaleType::Band)
        {
            let offset_component = model.get_scale_component(offset_channel).unwrap();
            let padding_inner = offset_component
                .get("paddingInner")
                .or_else(|| offset_component.get("padding"))
                .unwrap_or(json!(0));
            let padding_outer = offset_component
                .get("paddingOuter")
                .or_else(|| offset_component.get("padding"))
                .unwrap_or(json!(0));
            step_count = format!(
                "bandspace({step_count}, {}, {})",
                support::signal_or_string_value(&padding_inner),
                support::signal_or_string_value(&padding_outer)
            );
        }
        let merged = model.get_scale_component(channel).unwrap();
        let padding_inner = merged
            .get("paddingInner")
            .or_else(|| merged.get("padding"))
            .unwrap_or(json!(0));
        let step_value = step.get("step").cloned().unwrap_or(json!(0));
        return json!({
            "signal": format!(
                "{} * {} / (1-{})",
                step_value.as_f64().unwrap_or(0.0),
                step_count,
                support::expr_from_value(&padding_inner)
            )
        });
    }

    step.get("step").cloned().unwrap_or(Value::Null)
}

fn get_offset_step(step: &Value, offset_scale_type: Option<ScaleType>) -> Value {
    let offset_is_discrete = offset_scale_type.map(has_discrete_domain).unwrap_or(false);
    if get_step_for(step, offset_is_discrete) == "offset" {
        json!({"step": step.get("step").cloned().unwrap_or(Value::Null)})
    } else {
        Value::Null
    }
}

fn get_offset_range(channel: &str, model: &mut Model, offset_scale_type: Option<ScaleType>) -> Value {
    let position_channel = if channel == XOFFSET { "x" } else { "y" };
    let position_component = model.get_scale_component(position_channel);
    if position_component.is_none() {
        return full_width_or_height_range(position_channel, model, offset_scale_type, true);
    }
    let position_component = position_component.unwrap();
    let position_scale_type = position_component.get_type();
    let position_scale_name = model.scale_name(position_channel, false).unwrap_or_default();

    if position_scale_type == Some(ScaleType::Band) {
        let view_config = model
            .config
            .get("view")
            .cloned()
            .unwrap_or(Value::Null);
        let size = get_discrete_position_size(position_channel, &model.size, &view_config);
        if is_step(&size) {
            let step = get_offset_step(&size, offset_scale_type);
            if !step.is_null() {
                return step;
            }
        }
        if channel == YOFFSET
            && offset_scale_type.map(has_continuous_domain).unwrap_or(false)
        {
            return json!([{"signal": format!("bandwidth('{position_scale_name}')")}, 0]);
        }
        return json!([0, {"signal": format!("bandwidth('{position_scale_name}')")}]);
    }

    let position_def = model.encoding().cloned().unwrap_or(Value::Null)[position_channel].clone();
    if is_field_def(&position_def)
        && position_def.get("timeUnit").map(|t| !t.is_null()).unwrap_or(false)
    {
        let duration = crate::timeunit::duration_expr_with_wrap(
            &position_def["timeUnit"],
            |expr: &str| format!("scale('{position_scale_name}', {expr})"),
        )
        .unwrap_or_default();
        let padding = model
            .config
            .get("scale")
            .and_then(|s| s.get("bandWithNestedOffsetPaddingInner"))
            .cloned()
            .unwrap_or(Value::Null);
        if !padding.is_null() {
            let p = padding.as_f64().unwrap_or(0.0);
            let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
            let config = model.config.clone();
            let band_position_offset = crate::channeldef::get_band_position_pub(
                &position_def,
                None,
                &mark_def,
                &config,
            )
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5)
                - 0.5;
            let start_ratio = p / 2.0 + band_position_offset;
            let end_ratio = 1.0 - p / 2.0 + band_position_offset;
            return json!([
                {"signal": format!(
                    "{} * ({duration})",
                    crate::util::js_number_to_string(start_ratio)
                )},
                {"signal": format!(
                    "{} * ({duration})",
                    crate::util::js_number_to_string(end_ratio)
                )}
            ]);
        }
        return json!([0, {"signal": duration}]);
    }

    panic!("Cannot use {channel} scale if {position_channel} scale is not discrete.")
}

fn get_discrete_position_size(channel: &str, size: &Value, view_config: &Value) -> Value {
    let size_channel = if channel == X { "width" } else { "height" };
    let size_value = size.get(size_channel).cloned().unwrap_or(Value::Null);
    if !size_value.is_null() {
        return size_value;
    }
    support::get_view_config_discrete_size(view_config, size_channel)
}

/// Port of `defaultContinuousToDiscreteCount`.
pub fn default_continuous_to_discrete_count(
    scale_type: ScaleType,
    config: &Value,
    domain: &Value,
    channel: &str,
) -> i64 {
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    match scale_type {
        ScaleType::Quantile => scale_config
            .get("quantileCount")
            .and_then(Value::as_i64)
            .unwrap_or(4),
        ScaleType::Quantize => scale_config
            .get("quantizeCount")
            .and_then(Value::as_i64)
            .unwrap_or(4),
        ScaleType::Threshold => {
            if let Some(arr) = domain.as_array() {
                arr.len() as i64 + 1
            } else {
                crate::log::warn(support::domain_required_for_threshold_scale(channel));
                3
            }
        }
        _ => 4,
    }
}

/// Port of `interpolateRange`.
pub fn interpolate_range(range_min: &Value, range_max: &Value, cardinality: f64) -> Value {
    let r_max = support::signal_or_string_value(range_max);
    let r_min = support::signal_or_string_value(range_min);
    let step = format!("({r_max} - {r_min}) / ({cardinality} - 1)");
    json!({"signal": format!("sequence({r_min}, {r_max} + {step}, {step})")})
}

fn size_range_min(mark: &str, config: &Value) -> Value {
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    match mark {
        "bar" | "tick" => scale_config.get("minBandSize").cloned().unwrap_or(json!(2)),
        "line" | "trail" | "rule" => scale_config
            .get("minStrokeWidth")
            .cloned()
            .unwrap_or(json!(1)),
        "text" => scale_config.get("minFontSize").cloned().unwrap_or(json!(8)),
        "point" | "square" | "circle" => {
            scale_config.get("minSize").cloned().unwrap_or(json!(4))
        }
        _ => panic!("incompatible channel size with mark {mark}"),
    }
}

fn get_bin_step_signal(model: &Model, channel: &str) -> Option<String> {
    let field_def = model.field_def(channel)?;
    let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
    let size_type = crate::channel::get_size_channel(channel).unwrap_or("width");
    let size_signal = model.get_signal_name(&model.get_name(size_type));
    let is_binning = crate::compile::data::support::is_binning(&bin);
    if bin.is_object()
        && bin.get("binned").and_then(Value::as_bool) == Some(true)
        && bin.get("step").map(|v| !v.is_null()).unwrap_or(false)
    {
        let scale_name = model.scale_name(channel, false).unwrap_or_default();
        let step = bin.get("step").cloned().unwrap_or(Value::Null);
        let bin_count = format!(
            "(domain(\"{scale_name}\")[1] - domain(\"{scale_name}\")[0]) / {}",
            support::signal_or_string_value(&step)
        );
        return Some(format!("{size_signal} / ({bin_count})"));
    }
    if is_binning {
        let field = field_def
            .get("field")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let bin_signal = crate::compile::data::bin::get_bin_signal_name(model, field, &bin);
        let updated_name = model.get_signal_name(&bin_signal);
        let bin_count = format!(
            "({updated_name}.stop - {updated_name}.start) / {updated_name}.step"
        );
        return Some(format!("{size_signal} / ({bin_count})"));
    }
    None
}

fn size_range_max(mark: &str, size: &Value, model: &Model, config: &Value) -> Value {
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    let view_config = config.get("view").cloned().unwrap_or(Value::Null);
    match mark {
        "bar" | "tick" => {
            if let Some(v) = scale_config.get("maxBandSize") {
                if !v.is_null() {
                    return v.clone();
                }
            }
            let x_signal = get_bin_step_signal(model, "x");
            let y_signal = get_bin_step_signal(model, "y");
            let min = min_xy_step(size, x_signal.as_deref(), y_signal.as_deref(), &view_config);
            if let Some(n) = min.as_f64() {
                crate::compile::data::support::number_value(n - 1.0)
            } else {
                json!({"signal": format!("{} - 1", support::signal_or_string_value(&min))})
            }
        }
        "line" | "trail" | "rule" => scale_config
            .get("maxStrokeWidth")
            .cloned()
            .unwrap_or(json!(4)),
        "text" => scale_config.get("maxFontSize").cloned().unwrap_or(json!(40)),
        "point" | "square" | "circle" => {
            if let Some(v) = scale_config.get("maxSize") {
                if v.as_f64().map(|n| n != 0.0).unwrap_or(false) {
                    return v.clone();
                }
            }
            let x_signal = get_bin_step_signal(model, "x");
            let y_signal = get_bin_step_signal(model, "y");
            let point_step =
                min_xy_step(size, x_signal.as_deref(), y_signal.as_deref(), &view_config);
            if let Some(n) = point_step.as_f64() {
                crate::compile::data::support::number_value((MAX_SIZE_RANGE_STEP_RATIO * n).powi(2))
            } else {
                json!({"signal": format!(
                    "pow({} * {}, 2)",
                    MAX_SIZE_RANGE_STEP_RATIO,
                    support::signal_or_string_value(&point_step)
                )})
            }
        }
        _ => panic!("incompatible channel size with mark {mark}"),
    }
}

fn min_xy_step(
    size: &Value,
    x_signal: Option<&str>,
    y_signal: Option<&str>,
    view_config: &Value,
) -> Value {
    let width_step = if is_step(&size["width"]) {
        size["width"]["step"].clone()
    } else {
        support::get_view_config_discrete_step(view_config, "width")
    };
    let height_step = if is_step(&size["height"]) {
        size["height"]["step"].clone()
    } else {
        support::get_view_config_discrete_step(view_config, "height")
    };
    if x_signal.is_some() || y_signal.is_some() {
        let x = x_signal
            .map(str::to_string)
            .unwrap_or_else(|| support::signal_or_string_value(&width_step));
        let y = y_signal
            .map(str::to_string)
            .unwrap_or_else(|| support::signal_or_string_value(&height_step));
        return json!({"signal": format!("min({x}, {y})")});
    }
    let w = width_step.as_f64().unwrap_or(0.0);
    let h = height_step.as_f64().unwrap_or(0.0);
    json!(w.min(h))
}

#[allow(dead_code)]
fn _unused(_: &ScaleComponentIndex) {}
