//! Port of `src/compile/mark/encode/position-rect.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{
    get_offset_channel, get_offset_scale_channel, get_secondary_range_channel, get_size_channel,
    get_vg_position_channel, is_polar_position_channel, is_x_or_y,
};
use crate::channeldef::{
    get_band_position_pub, get_band_size, is_field_def, is_field_or_datum_def, vg_field_with,
    FieldRefOption,
};
use crate::channeldef_support::{has_discrete_domain, is_binned, is_binning, is_bin_params, truthy};
use crate::compile::support::{get_view_config_discrete_step, signal_or_string_value};
use crate::compile::model::Model;
use crate::mark::{get_mark_config, get_mark_prop_or_config, is_relative_band_size};
use crate::util::get_first_defined;

use super::nonposition::{non_position, NonPositionOpt};
use super::offset::position_offset;
use super::position_align::vg_aligned_position_channel;
use super::position_point::{point_position, point_position_default_ref};
use super::position_range::range_position;
use super::valueref::{
    bin_size_expr, interpolated_signal_ref, mid_point_ref_with_position_invalid_test,
    value_ref_for_field_or_datum_def, ValueRefEncode,
};

const OFFSETTED_RECT_START_SUFFIX: &str = "offsetted_rect_start";
const OFFSETTED_RECT_END_SUFFIX: &str = "offsetted_rect_end";

pub fn rect_position(model: &Model, channel: &str) -> Value {
    let config = &model.config;
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let mark = mark_def.get("type").and_then(Value::as_str).unwrap_or("");

    let channel2 = get_secondary_range_channel(channel).unwrap_or("");
    let size_channel = get_size_channel(channel).unwrap_or("");
    let channel_def = encoding.get(channel).cloned();
    let channel_def2 = encoding.get(channel2).cloned();

    let scale = model.get_scale_component(channel);
    let scale_type = scale
        .as_ref()
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(str::to_string));

    let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");

    let has_size_def = encoding
        .get(size_channel)
        .filter(|v| !v.is_null())
        .cloned()
        .or_else(|| encoding.get("size").filter(|v| !v.is_null()).cloned())
        .or_else(|| get_mark_prop_or_config("size", mark_def, config, Some(size_channel), false));

    let offset_scale_channel = get_offset_channel(channel).unwrap_or("");

    let is_bar_or_tick_band = (mark == "bar"
        && ((channel == "x" && orient == "vertical") || (channel == "y" && orient == "horizontal")))
        || (mark == "tick"
            && ((channel == "y" && orient == "vertical") || (channel == "x" && orient == "horizontal")));

    let is_image = mark == "image";

    let channel_def_is_field = channel_def
        .as_ref()
        .map(is_field_def)
        .unwrap_or(false);

    if channel_def_is_field
        && (is_binning(&channel_def.as_ref().unwrap()["bin"])
            || is_binned(&channel_def.as_ref().unwrap()["bin"])
            || (truthy(&channel_def.as_ref().unwrap()["timeUnit"])
                && channel_def2.as_ref().map(|v| v.is_null()).unwrap_or(true)))
        && !(has_size_def
            .as_ref()
            .map(|s| !is_relative_band_size(s))
            .unwrap_or(false))
        && !encoding.get(offset_scale_channel).map(truthy).unwrap_or(false)
        && !scale_type
            .as_deref()
            .map(has_discrete_domain)
            .unwrap_or(false)
    {
        rect_bin_position(model, channel, &channel_def.unwrap(), channel_def2.as_ref())
    } else if is_field_or_datum_def(channel_def.as_ref().unwrap_or(&Value::Null))
        && super::super::mark::is_ranged_offset(model, channel)
    {
        range_position(channel, model, Some("zeroOrMax"), "zeroOrMin")
    } else if is_image
        && channel_def.as_ref().map(|v| !v.is_null()).unwrap_or(false)
        && has_size_def.is_none()
        && channel_def2.as_ref().map(|v| v.is_null()).unwrap_or(true)
    {
        let align_channel = if channel == "x" { "align" } else { "baseline" };
        let align = get_mark_prop_or_config(align_channel, mark_def, config, None, false)
            .unwrap_or_else(|| {
                Value::String(if channel == "x" { "center" } else { "middle" }.to_string())
            });
        let mut out = point_position(channel, model, Some("mid"), None)
            .as_object()
            .cloned()
            .unwrap_or_default();
        out.insert(
            align_channel.to_string(),
            crate::compile_common::signal_or_value_ref(&align),
        );
        Value::Object(out)
    } else if ((is_field_or_datum_def(channel_def.as_ref().unwrap_or(&Value::Null))
        && scale_type
            .as_deref()
            .map(has_discrete_domain)
            .unwrap_or(false))
        || is_bar_or_tick_band)
        && channel_def2.as_ref().map(|v| v.is_null()).unwrap_or(true)
    {
        position_and_size(model, channel, channel_def.as_ref())
    } else {
        range_position(channel, model, Some("zeroOrMax"), "zeroOrMin")
    }
}

fn default_size_ref(
    size_channel: &str,
    scale_name: &str,
    scale: Option<&crate::compile::scale::component::ScaleComponent>,
    config: &Value,
    band_size: &Value,
    has_field_def: bool,
    mark: &str,
) -> Value {
    if is_relative_band_size(band_size) {
        let band = band_size.get("band").and_then(Value::as_f64).unwrap_or(1.0);
        if let Some(scale) = scale {
            let scale_type = scale.get("type").and_then(|v| v.as_str().map(str::to_string));
            if scale_type.as_deref() == Some("band") {
                let mut band_width = format!("bandwidth('{scale_name}')");
                if band != 1.0 {
                    band_width = format!("{band} * {band_width}");
                }
                let min_band_size = get_mark_config("minBandSize", &json!({"type": mark}), config, None);
                let signal = match min_band_size {
                    Some(min) if truthy(&min) => {
                        format!("max({}, {band_width})", signal_or_string_value(&min))
                    }
                    _ => band_width,
                };
                return json!({"signal": signal});
            } else if band != 1.0 {
                crate::log::warn(format!(
                    "Cannot use relative band size with {scale_type:?} scale."
                ));
            }
        } else {
            return json!({"mult": band, "field": {"group": size_channel}});
        }
    } else if band_size.is_object() && band_size.get("signal").is_some() {
        return band_size.clone();
    } else if crate::expr::is_expr_ref(band_size) {
        return crate::compile_common::signal_or_value_ref(band_size);
    } else if truthy(band_size) {
        return json!({"value": band_size});
    }

    if let Some(scale) = scale {
        if let Some(range) = scale.get("range") {
            if range.is_object() {
                if let Some(step) = range.get("step").and_then(Value::as_f64) {
                    return json!({"value": step - 2.0});
                }
            }
        }
    }

    if !has_field_def {
        let scale_config = config.get("scale").unwrap_or(&Value::Null);
        let band_padding_inner = scale_config.get("bandPaddingInner");
        let bar_padding = scale_config.get("barBandPaddingInner");
        let rect_padding = scale_config.get("rectBandPaddingInner");
        let tick_padding = scale_config.get("tickBandPaddingInner");
        let mark_specific = if mark == "tick" {
            tick_padding
        } else if mark == "bar" {
            bar_padding
        } else {
            rect_padding
        };
        let padding = get_first_defined(&[
            band_padding_inner,
            mark_specific,
        ])
        .cloned();
        if let Some(padding) = padding {
            if padding.get("signal").is_some() {
                return json!({"signal": format!(
                    "(1 - ({})) * {}",
                    padding["signal"].as_str().unwrap_or(""),
                    size_channel
                )});
            } else if let Some(p) = padding.as_f64() {
                return json!({"signal": format!("{} * {}", 1.0 - p, size_channel)});
            }
        }
    }

    let default_step = get_view_config_discrete_step(config.get("view").unwrap_or(&Value::Null), size_channel);
    let step = default_step.as_f64().unwrap_or(20.0);
    json!({"value": step - 2.0})
}

fn position_and_size(model: &Model, channel: &str, field_def: Option<&Value>) -> Value {
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;
    let config = &model.config;

    let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");
    let scale_name = model.scale_name(channel, false).unwrap_or_default();
    let scale = model.get_scale_component(channel);
    let vg_size_channel = get_size_channel(channel).unwrap_or("");
    let channel2 = get_secondary_range_channel(channel).unwrap_or("");

    let offset_scale_channel = get_offset_channel(channel);
    let offset_scale_name = offset_scale_channel
        .and_then(|c| model.scale_name(c, false))
        .unwrap_or_default();
    let offset_scale = get_offset_scale_channel(channel).and_then(|c| model.get_scale_component(c));

    let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");

    let use_vl_size_channel = mark_type == "tick"
        || (orient == "horizontal" && channel == "y")
        || (orient == "vertical" && channel == "x");

    let style_size = model
        .config
        .get("style")
        .and_then(|style| crate::mark::get_mark_style_config("size", mark_def, style));

    let mut size_mixins: Option<Value> = None;
    if encoding.get("size").map(truthy).unwrap_or(false)
        || mark_def.get("size").map(truthy).unwrap_or(false)
        || style_size.is_some()
    {
        if use_vl_size_channel {
            size_mixins = Some(non_position(
                "size",
                model,
                NonPositionOpt {
                    vg_channel: Some(vg_size_channel.to_string()),
                    default_ref: mark_def
                        .get("size")
                        .map(crate::compile_common::signal_or_value_ref),
                    ..Default::default()
                },
            ));
        } else {
            crate::log::warn(format!("Cannot apply size to non-oriented mark {mark_type}."));
        }
    }
    let has_size_from_mark_or_encoding = size_mixins
        .as_ref()
        .map(|v| v.as_object().map(|m| !m.is_empty()).unwrap_or(false))
        .unwrap_or(false);

    let scale_type_for_size = scale
        .as_ref()
        .or(offset_scale.as_ref())
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(str::to_string));

    let band_size = get_band_size(
        channel,
        field_def.unwrap_or(&Value::Null),
        None,
        mark_def,
        config,
        scale_type_for_size.as_deref(),
        use_vl_size_channel,
    );

    if size_mixins.is_none() {
        let mut map = Map::new();
        map.insert(
            vg_size_channel.to_string(),
            default_size_ref(
                vg_size_channel,
                if offset_scale_name.is_empty() {
                    &scale_name
                } else {
                    &offset_scale_name
                },
                offset_scale.as_ref().or(scale.as_ref()),
                config,
                &band_size,
                field_def
                    .map(|f| is_field_or_datum_def(f) || is_field_def(f))
                    .unwrap_or(false),
                mark_type,
            ),
        );
        size_mixins = Some(Value::Object(map));
    }

    let default_band_align = if scale
        .as_ref()
        .or(offset_scale.as_ref())
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(|s| s == "band"))
        .unwrap_or(false)
        && is_relative_band_size(&band_size)
        && !has_size_from_mark_or_encoding
    {
        "top"
    } else {
        "middle"
    };

    let vg_channel = vg_aligned_position_channel(channel, mark_def, config, default_band_align);
    let center = vg_channel == "xc" || vg_channel == "yc";
    let offset_info = position_offset(
        channel,
        mark_def,
        encoding,
        Some(model),
        Some(&json!(if center { 0.5 } else { 0.0 })),
    );

    let time_unit_band_position = if center
        && offset_info.offset_type != Some("encoding")
        && field_def.map(is_field_def).unwrap_or(false)
        && truthy(&field_def.unwrap()["timeUnit"])
        && encoding.get(channel2).map(|v| v.is_null()).unwrap_or(true)
    {
        get_band_position_pub(
            field_def.unwrap(),
            None,
            mark_def,
            config,
        )
    } else {
        None
    };

    let band_position = time_unit_band_position.unwrap_or_else(|| {
        if center {
            if offset_info.offset_type == Some("encoding") {
                json!(0)
            } else {
                json!(0.5)
            }
        } else if band_size.get("signal").is_some() {
            json!({"signal": format!(
                "(1-{})/2",
                band_size["signal"].as_str().unwrap_or("")
            )})
        } else if is_relative_band_size(&band_size) {
            let band = band_size.get("band").and_then(Value::as_f64).unwrap_or(1.0);
            json!((1.0 - band) / 2.0)
        } else {
            json!(0)
        }
    });

    let pos_ref = mid_point_ref_with_position_invalid_test(
        super::valueref::MidPointParams {
            channel,
            channel_def: field_def,
            channel2_def: None,
            mark_def,
            config,
            scale_name: &scale_name,
            scale: scale.as_ref(),
            stack: super::super::mark::stack_of(model).as_ref(),
            offset: offset_info.offset.clone(),
            default_ref: point_position_default_ref(
                model,
                Some("mid"),
                channel,
                &scale_name,
                scale.as_ref(),
            ),
            band_position: Some(band_position),
        },
    );

    if !vg_size_channel.is_empty() {
        let mut map = Map::new();
        map.insert(vg_channel, pos_ref);
        if let Some(mixins) = size_mixins.and_then(|v| v.as_object().cloned()) {
            for (k, v) in mixins {
                map.insert(k, v);
            }
        }
        return Value::Object(map);
    }

    let vg_channel2 = get_vg_position_channel(channel2).to_string();
    let size_ref = size_mixins
        .as_ref()
        .and_then(|v| v.get(vg_size_channel))
        .cloned()
        .unwrap_or(Value::Null);
    let size_offset = match &offset_info.offset {
        Some(offset) => {
            let mut m = size_ref.as_object().cloned().unwrap_or_default();
            m.insert("offset".to_string(), offset.clone());
            Value::Object(m)
        }
        None => size_ref,
    };

    let mut map = Map::new();
    let second_ref = if let Value::Array(arr) = &pos_ref {
        if arr.len() == 2 {
            let mut second = arr[1].as_object().cloned().unwrap_or_default();
            second.insert("offset".to_string(), size_offset);
            Value::Array(vec![arr[0].clone(), Value::Object(second)])
        } else {
            pos_ref.clone()
        }
    } else {
        let mut second = pos_ref.as_object().cloned().unwrap_or_default();
        second.insert("offset".to_string(), size_offset);
        Value::Object(second)
    };
    map.insert(vg_channel, pos_ref);
    map.insert(vg_channel2, second_ref);
    Value::Object(map)
}

fn get_bin_spacing(
    channel: &str,
    spacing: f64,
    reverse: Option<&Value>,
    axis_translate: Option<&Value>,
    offset: Option<&Value>,
    min_band_size: Option<&Value>,
    band_size_expr: &str,
) -> Value {
    if is_polar_position_channel(channel) {
        return json!(0);
    }

    let is_end = channel == "x" || channel == "y2";
    let spacing_offset = if is_end { -spacing / 2.0 } else { spacing / 2.0 };

    let reverse_is_signal = reverse.map(|v| v.get("signal").is_some()).unwrap_or(false);
    let offset_is_signal = offset.map(|v| v.get("signal").is_some()).unwrap_or(false);
    let axis_is_signal = axis_translate.map(|v| v.get("signal").is_some()).unwrap_or(false);
    let min_band_size_truthy = min_band_size.map(truthy).unwrap_or(false);

    if reverse_is_signal || offset_is_signal || axis_is_signal || min_band_size_truthy {
        let reverse_expr = reverse.map(signal_or_string_value).unwrap_or_default();
        let offset_expr = offset.map(signal_or_string_value).unwrap_or_default();
        let axis_translate_expr = axis_translate.map(signal_or_string_value).unwrap_or_default();
        let min_band_size_expr = min_band_size.map(signal_or_string_value).unwrap_or_default();

        let sign = if is_end { "" } else { "-" };

        let spacing_and_size_offset = if min_band_size_truthy {
            format!(
                "({band_size_expr} < {min_band_size_expr} ? {sign}0.5 * ({min_band_size_expr} - ({band_size_expr})) : {})",
                crate::util::js_number_to_string(spacing_offset)
            )
        } else {
            crate::util::js_number_to_string(spacing_offset)
        };

        let t = if axis_translate_expr.is_empty() {
            String::new()
        } else {
            format!("{axis_translate_expr} + ")
        };
        let r = if reverse_expr.is_empty() {
            String::new()
        } else {
            format!("({reverse_expr} ? -1 : 1) * ")
        };
        let o = if offset_expr.is_empty() {
            spacing_and_size_offset
        } else {
            format!("({offset_expr} + {spacing_and_size_offset})")
        };

        return json!({"signal": format!("{t}{r}{o}")});
    }

    let offset = offset.and_then(Value::as_f64).unwrap_or(0.0);
    let translate = axis_translate.and_then(Value::as_f64).unwrap_or(0.0);
    let reverse_bool = reverse.and_then(Value::as_bool).unwrap_or(false);

    let value = if reverse_bool {
        translate + (-offset - spacing_offset)
    } else {
        translate + (offset + spacing_offset)
    };
    if value == value.trunc() {
        json!(value as i64)
    } else {
        json!(value)
    }
}

fn rect_bin_position(
    model: &Model,
    channel: &str,
    field_def: &Value,
    field_def2: Option<&Value>,
) -> Value {
    let config = &model.config;
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;

    let scale = model.get_scale_component(channel);
    let scale_name = model.scale_name(channel, false).unwrap_or_default();
    let scale_type = scale
        .as_ref()
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(str::to_string));
    let reverse = scale.as_ref().and_then(|s| s.get("reverse"));

    let band_size = get_band_size(
        channel,
        field_def,
        field_def2,
        mark_def,
        config,
        scale_type.as_deref(),
        false,
    );

    let axis = model
        .component
        .axes
        .get(channel)
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|a| a.get("translate"))
        .cloned();
    let axis_translate = axis.unwrap_or(json!(0.5));

    let spacing = if is_x_or_y(channel) {
        get_mark_prop_or_config("binSpacing", mark_def, config, None, false)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let channel2 = get_secondary_range_channel(channel).unwrap_or("");
    let vg_channel = get_vg_position_channel(channel).to_string();
    let vg_channel2 = get_vg_position_channel(channel2).to_string();
    let min_band_size = get_mark_config("minBandSize", mark_def, config, None);

    let offset = position_offset(channel, mark_def, encoding, Some(model), Some(&json!(0))).offset;
    let offset2 = position_offset(channel2, mark_def, encoding, Some(model), Some(&json!(0))).offset;

    let band_size_expr = bin_size_expr(&scale_name, field_def);
    let bin_spacing_offset = get_bin_spacing(
        channel,
        spacing,
        reverse.as_ref(),
        Some(&axis_translate),
        offset.as_ref(),
        min_band_size.as_ref(),
        &band_size_expr,
    );
    let bin_spacing_offset2 = get_bin_spacing(
        channel2,
        spacing,
        reverse.as_ref(),
        Some(&axis_translate),
        offset2.as_ref().or(offset.as_ref()),
        min_band_size.as_ref(),
        &band_size_expr,
    );

    let band_position_for_band_size = if band_size.get("signal").is_some() {
        json!({"signal": format!("(1-{})/2", band_size["signal"].as_str().unwrap_or(""))})
    } else if is_relative_band_size(&band_size) {
        let band = band_size.get("band").and_then(Value::as_f64).unwrap_or(1.0);
        json!((1.0 - band) / 2.0)
    } else {
        json!(0.5)
    };

    let band_position = get_band_position_pub(field_def, field_def2, mark_def, config);

    if is_binning(&field_def["bin"]) || truthy(&field_def["timeUnit"]) {
        let use_rect_offset_field = truthy(&field_def["timeUnit"])
            && band_position.as_ref().and_then(Value::as_f64) != Some(0.5);

        let mut out = Map::new();
        out.insert(
            vg_channel2,
            rect_bin_ref(
                field_def,
                &scale_name,
                &band_position_for_band_size,
                Some(&bin_spacing_offset2),
                use_rect_offset_field,
            ),
        );
        let bp = if band_position_for_band_size.get("signal").is_some() {
            json!({"signal": format!(
                "1-{}",
                band_position_for_band_size["signal"].as_str().unwrap_or("")
            )})
        } else {
            let b = band_position_for_band_size.as_f64().unwrap_or(0.5);
            json!(1.0 - b)
        };
        out.insert(
            vg_channel,
            rect_bin_ref(
                field_def,
                &scale_name,
                &bp,
                Some(&bin_spacing_offset),
                use_rect_offset_field,
            ),
        );
        return Value::Object(out);
    } else if is_binned(&field_def["bin"]) {
        let start_ref = value_ref_for_field_or_datum_def(
            field_def,
            &scale_name,
            &FieldRefOption::default(),
            &ValueRefEncode {
                offset: Some(bin_spacing_offset2.clone()),
                band: None,
            },
        );

        if let Some(field_def2) = field_def2.filter(|f| is_field_def(f)) {
            let mut out = Map::new();
            out.insert(vg_channel2, start_ref);
            out.insert(
                vg_channel,
                value_ref_for_field_or_datum_def(
                    field_def2,
                    &scale_name,
                    &FieldRefOption::default(),
                    &ValueRefEncode {
                        offset: Some(bin_spacing_offset.clone()),
                        band: None,
                    },
                ),
            );
            return Value::Object(out);
        } else if is_bin_params(&field_def["bin"]) {
            if let Some(step) = field_def["bin"].get("step") {
                let mut out = Map::new();
                out.insert(vg_channel2, start_ref);
                out.insert(
                    vg_channel,
                    json!({
                        "signal": format!(
                            "scale(\"{scale_name}\", {} + {})",
                            vg_field_with(field_def, &FieldRefOption { expr: Some("datum".to_string()), ..Default::default() }),
                            step
                        ),
                        "offset": bin_spacing_offset,
                    }),
                );
                return Value::Object(out);
            }
        }
    }

    crate::log::warn(format!("Channel {channel2} should be used with bin."));
    Value::Object(Map::new())
}

fn rect_bin_ref(
    field_def: &Value,
    scale_name: &str,
    band_position: &Value,
    offset: Option<&Value>,
    use_rect_offset_field: bool,
) -> Value {
    interpolated_signal_ref(
        scale_name,
        field_def,
        None,
        offset,
        if use_rect_offset_field {
            Some(OFFSETTED_RECT_START_SUFFIX)
        } else {
            None
        },
        if use_rect_offset_field {
            OFFSETTED_RECT_END_SUFFIX
        } else {
            "end"
        },
        band_position,
    )
}
