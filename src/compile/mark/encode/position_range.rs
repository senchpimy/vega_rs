//! Port of `src/compile/mark/encode/position-range.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{
    get_main_range_channel, get_offset_scale_channel, get_secondary_range_channel, get_size_channel,
    get_vg_position_channel,
};
use crate::channeldef::{get_field_or_datum_def, is_field_or_datum_def};
use crate::compile::model::Model;
use crate::mark::{get_mark_style_config, is_relative_band_size};

use super::offset::position_offset;
use super::position_align::vg_aligned_position_channel;
use super::position_point::{point_position, point_position_default_ref};
use super::valueref::{
    mid_point_ref_with_position_invalid_test, value_ref_for_field_or_datum_def, width_height_value_or_signal_ref,
    MidPointParams, ValueRefEncode,
};

pub fn point_or_range_position(
    channel: &str,
    model: &Model,
    default_pos: Option<&str>,
    default_pos2: &str,
    range: bool,
) -> Value {
    if range {
        range_position(channel, model, default_pos, default_pos2)
    } else {
        point_position(channel, model, default_pos, None)
    }
}

pub fn range_position(
    channel: &str,
    model: &Model,
    default_pos: Option<&str>,
    default_pos2: &str,
) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let channel2 = get_secondary_range_channel(channel).unwrap_or("");
    let size_channel = get_size_channel(channel);

    let pos2_mixins = point_position2_or_size(model, default_pos2, channel2);

    let vg_channel = if size_channel
        .map(|s| pos2_mixins.get(s).is_some())
        .unwrap_or(false)
    {
        vg_aligned_position_channel(channel, mark_def, config, "middle")
    } else {
        get_vg_position_channel(channel).to_string()
    };

    let mut out = point_position(channel, model, default_pos, Some(&vg_channel))
        .as_object()
        .cloned()
        .unwrap_or_default();

    if let Some(map) = pos2_mixins.as_object() {
        for (k, v) in map {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out)
}

fn has_key(value: &Value, key: &str) -> bool {
    value
        .as_object()
        .map(|m| m.get(key).map(|v| !v.is_null()).unwrap_or(false))
        .unwrap_or(false)
}

fn point_position2_or_size(
    model: &Model,
    default_pos: &str,
    channel: &str,
) -> Value {
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let config = &model.config;

    let base_channel = get_main_range_channel(channel);
    let size_channel = get_size_channel(channel);
    let vg_channel = get_vg_position_channel(channel);

    let channel_def = encoding.get(base_channel).cloned();
    let scale_name = model.scale_name(base_channel, false).unwrap_or_default();
    let scale = model.get_scale_component(base_channel);

    let offset = if has_key(encoding, channel) || has_key(mark_def, channel) {
        position_offset(channel, mark_def, encoding, Some(model), None).offset
    } else {
        position_offset(base_channel, mark_def, encoding, Some(model), None).offset
    };

    if channel_def.is_none()
        && matches!(channel, "x2" | "y2")
        && (has_key(encoding, "latitude") || has_key(encoding, "longitude"))
    {
        let vg_size_channel = get_size_channel(channel).unwrap_or("");
        if let Some(size) = mark_def.get(vg_size_channel).filter(|v| !v.is_null()) {
            return json!({vg_size_channel: {"value": size}});
        } else {
            return json!({vg_channel: {"field": model.get_name(channel)}});
        }
    }

    let value_ref = position2_ref(
        channel,
        channel_def.as_ref(),
        encoding.get(channel).cloned().as_ref(),
        mark_def,
        config,
        &scale_name,
        scale.as_ref(),
        super::super::mark::stack_of(model).as_ref(),
        offset.clone(),
    );

    if let Some(value_ref) = value_ref.filter(|v| !v.is_null()) {
        let mut map = Map::new();
        map.insert(vg_channel.to_string(), value_ref);
        return Value::Object(map);
    }

    let mark = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
    let style = config.get("style").cloned().unwrap_or(Value::Null);

    let mut style_mark = Map::new();
    if let Some(v) = get_mark_style_config(channel, mark_def, &style) {
        style_mark.insert(channel.to_string(), v);
    }
    if let Some(size_channel) = size_channel {
        if let Some(v) = get_mark_style_config(size_channel, mark_def, &style) {
            style_mark.insert(size_channel.to_string(), v);
        }
    }

    let candidates = [
        position2or_size(channel, mark_def),
        position2or_size(channel, &Value::Object(style_mark)),
        position2or_size(channel, config.get(mark).unwrap_or(&Value::Null)),
        position2or_size(channel, config.get("mark").unwrap_or(&Value::Null)),
    ];
    if let Some(candidate) = candidates.into_iter().flatten().next() {
        return candidate;
    }

    if is_field_or_datum_def(channel_def.as_ref().unwrap_or(&Value::Null))
        && super::super::mark::is_ranged_offset(model, base_channel)
    {
        let offset_ref = value_ref_for_field_or_datum_def(
            &json!({"datum": 0}),
            &model
                .scale_name(get_offset_scale_channel(base_channel).unwrap_or(""), false)
                .unwrap_or_default(),
            &crate::channeldef::FieldRefOption::default(),
            &ValueRefEncode::default(),
        );
        let reference = value_ref_for_field_or_datum_def(
            channel_def.as_ref().unwrap(),
            &scale_name,
            &crate::channeldef::FieldRefOption::default(),
            &ValueRefEncode {
                offset: Some(offset_ref),
                band: None,
            },
        );
        return json!({vg_channel: reference});
    }

    let default_ref = point_position_default_ref(model, Some(default_pos), channel, &scale_name, scale.as_ref());
    let mut map = Map::new();
    map.insert(
        vg_channel.to_string(),
        default_ref.unwrap_or(Value::Null),
    );
    Value::Object(map)
}

/// Port of `position2Ref` from `src/compile/mark/encode/position-range.ts`.
///
/// Mirrors the upstream signature, which also takes many related inputs.
#[allow(clippy::too_many_arguments)]
pub fn position2_ref(
    channel: &str,
    channel_def: Option<&Value>,
    channel2_def: Option<&Value>,
    mark_def: &Value,
    config: &Value,
    scale_name: &str,
    scale: Option<&crate::compile::scale::component::ScaleComponent>,
    stack: Option<&Value>,
    offset: Option<Value>,
) -> Option<Value> {
    if is_field_or_datum_def(channel_def.unwrap_or(&Value::Null)) {
        if let Some(stack) = stack {
            let field_channel = stack.get("fieldChannel").and_then(Value::as_str).unwrap_or("");
            if channel.chars().next() == field_channel.chars().next() {
                return Some(value_ref_for_field_or_datum_def(
                    channel_def.unwrap(),
                    scale_name,
                    &crate::channeldef::FieldRefOption {
                        suffix: Some("start".to_string()),
                        ..Default::default()
                    },
                    &ValueRefEncode {
                        offset,
                        band: None,
                    },
                ));
            }
        }
    }
    let reference = mid_point_ref_with_position_invalid_test(MidPointParams {
        channel,
        channel_def: channel2_def,
        channel2_def: None,
        mark_def,
        config,
        scale_name,
        scale,
        stack,
        offset,
        default_ref: None,
        band_position: None,
    });
    Some(reference)
}

fn position2or_size(channel: &str, mark_def: &Value) -> Option<Value> {
    let size_channel = get_size_channel(channel);
    let vg_channel = get_vg_position_channel(channel);
    if let Some(v) = mark_def.get(vg_channel).filter(|v| !v.is_null()) {
        return Some(json!({vg_channel: width_height_value_or_signal_ref(channel, v)}));
    } else if let Some(v) = mark_def.get(channel).filter(|v| !v.is_null()) {
        return Some(json!({vg_channel: width_height_value_or_signal_ref(channel, v)}));
    } else if let Some(size_channel) = size_channel {
        if let Some(dimension_size) = mark_def.get(size_channel).filter(|v| !v.is_null()) {
            if is_relative_band_size(dimension_size) {
                crate::log::warn(format!(
                    "Relative band size is not supported for the {size_channel} channel."
                ));
            } else {
                return Some(json!({size_channel: width_height_value_or_signal_ref(channel, dimension_size)}));
            }
        }
    }
    None
}

#[allow(dead_code)]
fn _unused(v: Option<&Value>) -> Option<Value> {
    get_field_or_datum_def(v?)
}
