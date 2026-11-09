//! Port of `src/compile/mark/encode/position-point.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{
    get_main_range_channel, get_secondary_range_channel, get_size_channel, get_vg_position_channel,
    is_x_or_y,
};
use crate::channeldef::{is_field_def, is_field_or_datum_def};
use crate::compile::model::Model;
use crate::mark::get_mark_prop_or_config;

use super::offset::position_offset;
use super::scaled_zero_or_min_or_max::scaled_zero_or_min_or_max;
use super::valueref::{
    interpolated_signal_ref, mid_point_ref_with_position_invalid_test, value_ref_for_field_or_datum_def,
    width_height_value_or_signal_ref, MidPointParams, ValueRefEncode,
};

pub fn point_position(
    channel: &str,
    model: &Model,
    default_pos: Option<&str>,
    vg_channel: Option<&str>,
) -> Value {
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let config = &model.config;

    let channel_def = encoding.get(channel).cloned();
    let channel2_def = get_secondary_range_channel(channel)
        .and_then(|c2| encoding.get(c2).cloned());
    let scale_name = model.scale_name(channel, false).unwrap_or_default();
    let scale = model.get_scale_component(channel);

    let offset = position_offset(
        channel,
        mark_def,
        encoding,
        Some(model),
        Some(&json!(0.5)),
    );

    let default_ref = point_position_default_ref(model, default_pos, channel, &scale_name, scale.as_ref());

    let value_ref = if channel_def.is_none()
        && is_x_or_y(channel)
        && (encoding.get("latitude").map(|v| !v.is_null()).unwrap_or(false)
            || encoding.get("longitude").map(|v| !v.is_null()).unwrap_or(false))
    {
        Some(json!({"field": model.get_name(channel)}))
    } else {
        Some(position_ref(
            channel,
            channel_def.as_ref(),
            channel2_def.as_ref(),
            mark_def,
            config,
            &scale_name,
            scale.as_ref(),
            super::super::mark::stack_of(model).as_ref(),
            offset.offset.clone(),
            default_ref,
            if offset.offset_type == Some("encoding") {
                Some(json!(0))
            } else {
                None
            },
        ))
    };

    let vg = vg_channel.unwrap_or(channel);
    match value_ref {
        Some(value) if !value.is_null() => {
            let mut map = Map::new();
            map.insert(vg.to_string(), value);
            Value::Object(map)
        }
        _ => Value::Object(Map::new()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn position_ref(
    channel: &str,
    channel_def: Option<&Value>,
    channel2_def: Option<&Value>,
    mark_def: &Value,
    config: &Value,
    scale_name: &str,
    scale: Option<&crate::compile::scale::component::ScaleComponent>,
    stack: Option<&Value>,
    offset: Option<Value>,
    default_ref: Option<Value>,
    band_position: Option<Value>,
) -> Value {
    if let Some(channel_def) = channel_def {
        if is_field_or_datum_def(channel_def)
            && stack
                .and_then(|s| s.get("fieldChannel"))
                .and_then(Value::as_str)
                == Some(channel)
        {
            if is_field_def(channel_def) {
                let mut bp = channel_def.get("bandPosition").cloned();
                let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
                if bp.is_none()
                    && mark_type == "text"
                    && (channel == "radius" || channel == "theta")
                {
                    bp = Some(json!(0.5));
                }
                if let Some(bp) = bp.filter(|v| !v.is_null()) {
                    return interpolated_signal_ref(
                        scale_name,
                        channel_def,
                        None,
                        offset.as_ref(),
                        Some("start"),
                        "end",
                        &bp,
                    );
                }
            }
            return value_ref_for_field_or_datum_def(
                channel_def,
                scale_name,
                &crate::channeldef::FieldRefOption {
                    suffix: Some("end".to_string()),
                    ..Default::default()
                },
                &ValueRefEncode {
                    offset,
                    band: None,
                },
            );
        }
    }

    mid_point_ref_with_position_invalid_test(MidPointParams {
        channel,
        channel_def,
        channel2_def,
        mark_def,
        config,
        scale_name,
        scale,
        stack,
        offset,
        default_ref,
        band_position,
    })
}

pub fn point_position_default_ref(
    model: &Model,
    default_pos: Option<&str>,
    channel: &str,
    scale_name: &str,
    scale: Option<&crate::compile::scale::component::ScaleComponent>,
) -> Option<Value> {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let main_channel = get_main_range_channel(channel);
    let vg_channel = get_vg_position_channel(channel);

    if let Some(defined_value_or_config) =
        get_mark_prop_or_config(channel, mark_def, config, Some(vg_channel), false)
    {
        return Some(width_height_value_or_signal_ref(
            channel,
            &defined_value_or_config,
        ));
    }

    match default_pos {
        Some("zeroOrMin") => zero_or_min_or_max_position(
            model,
            mark_def,
            config,
            scale_name,
            scale,
            "zeroOrMin",
            main_channel,
        ),
        Some("zeroOrMax") => zero_or_min_or_max_position(
            model,
            mark_def,
            config,
            scale_name,
            scale,
            "zeroOrMax",
            main_channel,
        ),
        Some("mid") => {
            let size_ref = if get_size_channel(channel) == Some("width") {
                model.width()
            } else {
                model.height()
            };
            let mut map = size_ref.as_object().cloned().unwrap_or_default();
            map.insert("mult".to_string(), json!(0.5));
            Some(Value::Object(map))
        }
        _ => None,
    }
}

fn zero_or_min_or_max_position(
    model: &Model,
    _mark_def: &Value,
    _config: &Value,
    scale_name: &str,
    scale: Option<&crate::compile::scale::component::ScaleComponent>,
    mode: &str,
    main_channel: &str,
) -> Option<Value> {
    if let Some(scaled) = scaled_zero_or_min_or_max(scale_name, scale, mode) {
        return Some(scaled);
    }

    let is_max = mode == "zeroOrMax";
    match main_channel {
        "radius" => {
            if !is_max {
                Some(json!({"value": 0}))
            } else {
                let width = model
                    .width()
                    .get("signal")
                    .and_then(Value::as_str)
                    .unwrap_or("width")
                    .to_string();
                let height = model
                    .height()
                    .get("signal")
                    .and_then(Value::as_str)
                    .unwrap_or("height")
                    .to_string();
                Some(json!({"signal": format!("min({width},{height})/2")}))
            }
        }
        "theta" => {
            if !is_max {
                Some(json!({"value": 0}))
            } else {
                Some(json!({"signal": "2*PI"}))
            }
        }
        "x" => {
            if !is_max {
                Some(json!({"value": 0}))
            } else {
                Some(json!({"field": {"group": "width"}}))
            }
        }
        "y" => {
            if !is_max {
                Some(json!({"field": {"group": "height"}}))
            } else {
                Some(json!({"value": 0}))
            }
        }
        _ => None,
    }
}
