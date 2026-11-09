//! Port of `src/compile/scale/type.ts`.

use serde_json::Value;

use crate::channel::{self, RangeType};
use crate::channeldef::{
    is_field_def, is_position_field_or_datum_def, is_unbinned_quantitative_field_or_datum_def,
};
use crate::mark::{get_mark_type, is_relative_band_size};
use crate::scale::{
    channel_support_scale_type, has_discrete_domain, is_continuous_to_discrete, is_quantitative,
    scale_type_support_data_type, ScaleType,
};
use crate::type_::Type;


use super::super::support;

/// Port of `RangeType`; re-exported from the channel module.
pub type RangeTypeAlias = RangeType;

/// Determine if there is a specified scale type and if it is appropriate,
/// or determine default type if type is unspecified or inappropriate.
pub fn scale_type(
    specified_scale: &Value,
    channel: &str,
    field_def: &Value,
    mark_def: &Value,
    has_nested_offset_scale: bool,
) -> Option<ScaleType> {
    let default_scale_type = default_type(channel, field_def, mark_def, has_nested_offset_scale);

    if !channel::is_scale_channel(channel) {
        return None;
    }

    if let Some(type_) = specified_scale.get("type").and_then(Value::as_str) {
        let type_ = match type_.parse::<ScaleType>() {
            Ok(t) => t,
            Err(_) => return default_scale_type,
        };
        if !channel_support_scale_type_local(channel, type_, has_nested_offset_scale) {
            log_warn(support::scale_type_not_work_with_channel(
                channel,
                type_.as_str(),
                default_scale_type.map(|t| t.as_str()).unwrap_or("undefined"),
            ));
            return default_scale_type;
        }

        if is_field_def(field_def) {
            let field_type = field_def
                .get("type")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<Type>().ok());
            if let Some(field_type) = field_type {
                if !scale_type_support_data_type(Some(type_), field_type) {
                    log_warn(support::scale_type_not_work_with_field_def(
                        type_.as_str(),
                        default_scale_type.map(|t| t.as_str()).unwrap_or("undefined"),
                    ));
                    return default_scale_type;
                }
            }
        }

        return Some(type_);
    }

    default_scale_type
}

/// Local wrapper around `channelSupportScaleType`.
///
/// The shared `scale` module predates `time`/`utc` support in
/// `isContinuousToContinuous`; the upstream position channels accept those
/// scale types, so handle them here.
fn channel_support_scale_type_local(
    channel: &str,
    scale_type: ScaleType,
    has_nested_offset_scale: bool,
) -> bool {
    if matches!(
        channel,
        "x" | "y" | "xOffset" | "yOffset" | "theta" | "radius"
    ) && matches!(scale_type, ScaleType::Time | ScaleType::Utc)
    {
        return true;
    }
    channel_support_scale_type(support::channel_static(channel), scale_type, has_nested_offset_scale)
}

fn log_warn(message: String) {
    crate::log::warn(message);
}

/// Determine appropriate default scale type.
fn default_type(
    channel: &str,
    field_def: &Value,
    mark_def: &Value,
    has_nested_offset_scale: bool,
) -> Option<ScaleType> {
    let field_type = field_def
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("");

    match field_type {
        "nominal" | "ordinal" => {
            if channel::is_color_channel(channel)
                || channel::range_type(channel) == Some(RangeType::Discrete)
            {
                if channel == "shape" && field_type == "ordinal" {
                    log_warn(support::discrete_channel_cannot_encode(channel, "ordinal"));
                }
                return Some(ScaleType::Ordinal);
            }

            if channel::is_time(channel) {
                return Some(ScaleType::Band);
            }

            let mark_type = get_mark_type(mark_def).unwrap_or("");
            if channel::is_x_or_y(channel) || channel::is_x_or_y_offset(channel) {
                if ["rect", "bar", "image", "rule", "tick"].contains(&mark_type) {
                    // The rect/bar/tick mark should fit into a band.
                    return Some(ScaleType::Band);
                }
                if has_nested_offset_scale {
                    return Some(ScaleType::Band);
                }
            } else if mark_type == "arc"
                && channel::POLAR_POSITION_SCALE_CHANNEL_INDEX.contains(&channel)
            {
                return Some(ScaleType::Band);
            }

            if let Some(size_channel) = channel::get_size_channel(channel) {
                if is_relative_band_size(&mark_def[size_channel]) {
                    return Some(ScaleType::Band);
                }
            }

            if is_position_field_or_datum_def(field_def)
                && field_def
                    .get("axis")
                    .map(|a| crate::util::has_property(a, "tickBand"))
                    .unwrap_or(false)
            {
                return Some(ScaleType::Band);
            }

            // Otherwise, use ordinal point scale so we can easily get center
            // positions of the marks.
            Some(ScaleType::Point)
        }

        "temporal" => {
            if channel::is_color_channel(channel) {
                Some(ScaleType::Time)
            } else if channel::range_type(channel) == Some(RangeType::Discrete) {
                log_warn(support::discrete_channel_cannot_encode(channel, "temporal"));
                Some(ScaleType::Ordinal)
            } else if is_field_def(field_def)
                && field_def.get("timeUnit").map(|t| !t.is_null()).unwrap_or(false)
                && crate::channeldef_support::normalize_time_unit(&field_def["timeUnit"])
                    .get("utc")
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                Some(ScaleType::Utc)
            } else if channel::is_time(channel) {
                // TODO(jzong): when interpolation is implemented, this should be linear.
                Some(ScaleType::Band)
            } else {
                Some(ScaleType::Time)
            }
        }

        "quantitative" => {
            if channel::is_color_channel(channel) {
                if is_field_def(field_def)
                    && crate::channeldef_support::is_binning(&field_def["bin"])
                {
                    return Some(ScaleType::BinOrdinal);
                }
                Some(ScaleType::Linear)
            } else if channel::range_type(channel) == Some(RangeType::Discrete) {
                log_warn(support::discrete_channel_cannot_encode(
                    channel,
                    "quantitative",
                ));
                Some(ScaleType::Ordinal)
            } else if channel::is_time(channel) {
                // TODO(jzong): when interpolation is implemented, this should be linear.
                Some(ScaleType::Band)
            } else {
                Some(ScaleType::Linear)
            }
        }

        "geojson" => None,

        other => {
            // Keep the upstream behaviour (throw for an invalid field type) but
            // fall back to `None` instead of panicking, so a malformed spec does
            // not abort the compiler.
            let _ = other;
            None
        }
    }
}

// Keep the imports used by helper predicates alive in the public surface.
#[allow(dead_code)]
fn _unused(field_def: &Value) -> bool {
    is_unbinned_quantitative_field_or_datum_def(field_def)
        || has_discrete_domain(ScaleType::Point)
        || is_continuous_to_discrete(ScaleType::Quantize)
        || is_quantitative(ScaleType::Linear)
}
