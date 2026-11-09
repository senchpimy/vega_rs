//! Port of `src/compile/scale/properties.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::channel::{
    get_main_channel_from_offset_channel, get_secondary_range_channel, is_x_or_y,
    is_x_or_y_offset, POLAR_POSITION_SCALE_CHANNELS, POSITION_SCALE_CHANNELS,
};
use crate::channeldef::{
    get_field_def, get_field_or_datum_def, is_field_def, is_field_or_datum_def, value_expr,
    ValueExprOpt,
};
use crate::encoding::channel_has_nested_offset_scale;
use crate::mark::is_bar_or_area;
use crate::scale::{
    channel_scale_property_incompatability, has_continuous_domain, is_continuous_to_continuous,
    is_continuous_to_discrete, scale_type_support_property, ScaleType,
};
use crate::util::{contains, get_first_defined};

use super::super::model::{is_unit_model, Model};
use super::super::split::{merge_values_with_explicit, tie_break_by_comparing, Explicit};
use super::super::support;
use super::domain::normalize_bin_for_channel;
use super::range::parse_unit_scale_range;

/// Port of `parseScaleProperty`.
pub fn parse_scale_property(model: &mut Model, property: &str) {
    if is_unit_model(model) {
        parse_unit_scale_property(model, property);
    } else {
        parse_non_unit_scale_property(model, property);
    }
}

fn parse_unit_scale_property(model: &mut Model, property: &str) {
    let channels: Vec<String> = model.component.scales.keys().cloned().collect();

    // Resolve every channel against an immutable view of the model first, then
    // apply the resulting writes. This avoids deep-cloning `config`, `encoding`
    // and `markDef` once per channel/property (upstream is a cheap reference).
    enum Action {
        Set { value: Value, explicit: bool },
        CopyKey,
        None,
    }

    let null_value = Value::Null;
    let updates: Vec<(String, Action)> = {
        let config = &model.config;
        let encoding = model.encoding().unwrap_or(&null_value);
        let mark_def = model.mark_def().unwrap_or(&null_value);

        channels
            .iter()
            .map(|channel| {
                let specified_scale = model
                    .unit
                    .specified_scales
                    .get(channel)
                    .unwrap_or(&null_value);
                let merged_scale = model.get_scale_component_ref(channel);
                let scale_type = merged_scale.and_then(|c| c.get_type());
                let scale_type_str = scale_type.map(|t| t.as_str()).unwrap_or("");
                let scale_padding = merged_scale
                    .and_then(|c| c.get_ref("padding"))
                    .unwrap_or(&null_value);
                let scale_padding_inner = merged_scale
                    .and_then(|c| c.get_ref("paddingInner"))
                    .unwrap_or(&null_value);
                let field_or_datum_def =
                    crate::channeldef::get_field_or_datum_def_ref(&encoding[channel])
                        .unwrap_or(&null_value);

                let specified_value = specified_scale.get(property).cloned().unwrap_or(Value::Null);
                let supported = scale_type
                    .map(|t| scale_type_support_property(t, property))
                    .unwrap_or(false);
                let incompat = channel_scale_property_incompatability(
                    support::channel_static(channel),
                    property,
                );

                if !specified_value.is_null() {
                    if !supported {
                        crate::log::warn(support::scale_property_not_work_with_scale_type(
                            scale_type_str,
                            property,
                            channel,
                        ));
                    } else if let Some(msg) = incompat.clone() {
                        crate::log::warn(msg);
                    }
                }

                if !supported || incompat.is_some() {
                    return (channel.clone(), Action::None);
                }

                let has_nested_offset_scale = channel_has_nested_offset_scale(encoding, channel);
                let has_secondary_range_channel = get_secondary_range_channel(channel)
                    .map(|c| get_field_or_datum_def(&encoding[c]).is_some())
                    .unwrap_or(false);

                if !specified_value.is_null() {
                    let time_unit = field_or_datum_def
                        .get("timeUnit")
                        .cloned()
                        .unwrap_or(Value::Null);
                    let type_ = field_or_datum_def
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("");

                    if property == "domainMax" || property == "domainMin" {
                        if crate::channeldef_support::is_date_time(&specified_value)
                            || type_ == "temporal"
                            || !time_unit.is_null()
                        {
                            let expr = value_expr(
                                &specified_value,
                                &ValueExprOpt {
                                    time_unit: if time_unit.is_null() {
                                        None
                                    } else {
                                        Some(time_unit.clone())
                                    },
                                    type_: Some(type_.to_string()),
                                    ..Default::default()
                                },
                            )
                            .unwrap_or_default();
                            return (
                                channel.clone(),
                                Action::Set {
                                    value: json!({"signal": expr}),
                                    explicit: true,
                                },
                            );
                        }
                        return (
                            channel.clone(),
                            Action::Set {
                                value: specified_value,
                                explicit: true,
                            },
                        );
                    }
                    return (channel.clone(), Action::CopyKey);
                }

                let value = match property {
                    "bins" => bins_for(channel, field_or_datum_def, model),
                    "interpolate" => interpolate(
                        channel,
                        field_or_datum_def.get("type").and_then(Value::as_str).unwrap_or(""),
                    ),
                    "nice" => nice_rule(scale_type, channel, specified_scale, field_or_datum_def),
                    "padding" => padding(channel, scale_type, config, field_or_datum_def, mark_def),
                    "paddingInner" => padding_inner(
                        scale_padding,
                        channel,
                        mark_def,
                        scale_type,
                        config,
                        has_nested_offset_scale,
                    ),
                    "paddingOuter" => padding_outer(
                        scale_padding,
                        channel,
                        scale_type,
                        scale_padding_inner,
                        config,
                        has_nested_offset_scale,
                    ),
                    "reverse" => {
                        let sort = if is_field_def(field_or_datum_def) {
                            field_or_datum_def.get("sort").cloned().unwrap_or(Value::Null)
                        } else {
                            Value::Null
                        };
                        reverse(scale_type, &sort, channel, config)
                    }
                    "zero" => {
                        let has_ranged_offset = is_x_or_y_offset(channel)
                            && get_main_channel_from_offset_channel(channel)
                                .map(|c| is_ranged_offset(model, c))
                                .unwrap_or(false);
                        zero(
                            channel,
                            field_or_datum_def,
                            &specified_scale.get("domain").cloned().unwrap_or(Value::Null),
                            mark_def,
                            scale_type,
                            config,
                            has_secondary_range_channel,
                            has_ranged_offset,
                        )
                    }
                    _ => config
                        .get("scale")
                        .and_then(|s| s.get(property))
                        .cloned()
                        .unwrap_or(Value::Null),
                };

                if !value.is_null() {
                    (
                        channel.clone(),
                        Action::Set {
                            value,
                            explicit: false,
                        },
                    )
                } else {
                    (channel.clone(), Action::None)
                }
            })
            .collect()
    };

    for (channel, action) in updates {
        match action {
            Action::Set { value, explicit } => {
                if let Some(cmpt) = model.component.scales.get_mut(&channel) {
                    cmpt.set(property, value, explicit);
                }
            }
            Action::CopyKey => {
                if let Some(cmpt) = model.component.scales.get_mut(&channel) {
                    cmpt.copy_key_from_object(property, &model.unit.specified_scales[&channel]);
                }
            }
            Action::None => {}
        }
    }
}

/// Port of `parseNonUnitScaleProperty`.
pub fn parse_non_unit_scale_property(model: &mut Model, property: &str) {
    for child in &mut model.children {
        if property == "range" {
            super::properties::parse_scale_range(child);
        } else {
            parse_scale_property(child, property);
        }
    }

    let channels: Vec<String> = model.component.scales.keys().cloned().collect();
    for channel in channels {
        let mut value_with_explicit: Option<Explicit> = None;
        for child in &model.children {
            if let Some(child_component) = child.component.scales.get(&channel) {
                let child_value = child_component.get_with_explicit(property);
                value_with_explicit = Some(merge_values_with_explicit(
                    value_with_explicit,
                    child_value,
                    property,
                    "scale",
                    tie_break_by_comparing(|v1, v2| {
                        if property == "range" {
                            let step1 = v1.get("step").and_then(Value::as_f64);
                            let step2 = v2.get("step").and_then(Value::as_f64);
                            if let (Some(a), Some(b)) = (step1, step2) {
                                return (a - b) as i64;
                            }
                        }
                        0
                    }),
                ));
            }
        }
        if let Some(value) = value_with_explicit {
            if let Some(cmpt) = model.component.scales.get_mut(&channel) {
                cmpt.set_with_explicit(property, value);
            }
        }
    }
}

/// Port of `parseScaleRange`.
pub fn parse_scale_range(model: &mut Model) {
    if is_unit_model(model) {
        parse_unit_scale_range(model);
    } else {
        parse_non_unit_scale_property(model, "range");
    }
}

pub fn bins_for(channel: &str, field_def: &Value, model: &Model) -> Value {
    let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
    if crate::channeldef_support::is_binning(&bin) {
        let field = field_def.get("field").and_then(Value::as_str).unwrap_or("");
        let normalized = normalize_bin_for_channel(&bin, channel);
        let key = bin_key(&normalized, field);
        return json!({"signal": model.get_name(&format!("{key}_bins"))});
    } else if crate::channeldef_support::is_binned(&bin)
        && crate::channeldef_support::is_bin_params(&bin)
    {
        if let Some(step) = bin.get("step") {
            if !step.is_null() {
                return json!({"step": step});
            }
        }
    }
    Value::Null
}

fn bin_key(bin: &Map<String, Value>, field: &str) -> String {
    super::super::data::support::bin_key(bin, field)
}

pub fn interpolate(channel: &str, type_: &str) -> Value {
    if ["color", "fill", "stroke"].contains(&channel) && type_ != "nominal" {
        return Value::String("hcl".to_string());
    }
    Value::Null
}

pub fn nice_rule(
    scale_type: Option<ScaleType>,
    channel: &str,
    specified_scale: &Value,
    field_or_datum_def: &Value,
) -> Value {
    let bin = get_field_def(field_or_datum_def)
        .and_then(|f| f.get("bin").cloned())
        .unwrap_or(Value::Null);
    let domain_max = specified_scale.get("domainMax").cloned().unwrap_or(Value::Null);
    let domain_min = specified_scale.get("domainMin").cloned().unwrap_or(Value::Null);
    if crate::channeldef_support::truthy(&bin)
        || specified_scale.get("domain").map(Value::is_array).unwrap_or(false)
        || !domain_max.is_null()
        || !domain_min.is_null()
        || scale_type
            .map(|t| contains(&[ScaleType::Time, ScaleType::Utc], &t))
            .unwrap_or(false)
    {
        return Value::Null;
    }
    if is_x_or_y(channel) {
        Value::Bool(true)
    } else {
        Value::Null
    }
}

pub fn padding(
    channel: &str,
    scale_type: Option<ScaleType>,
    config: &Value,
    field_or_datum_def: &Value,
    mark_def: &Value,
) -> Value {
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    let bar_config = config.get("bar").cloned().unwrap_or(Value::Null);
    if is_x_or_y(channel) {
        let st = match scale_type {
            Some(t) => t,
            None => return Value::Null,
        };
        if is_continuous_to_continuous(st) {
            if let Some(p) = scale_config.get("continuousPadding") {
                if !p.is_null() {
                    return p.clone();
                }
            }
            let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
            let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");
            if mark_type == "bar"
                && !(is_field_def(field_or_datum_def)
                    && (crate::channeldef_support::truthy(&field_or_datum_def["bin"])
                        || crate::channeldef_support::truthy(&field_or_datum_def["timeUnit"])))
                && ((orient == "vertical" && channel == "x")
                    || (orient == "horizontal" && channel == "y"))
                {
                    return bar_config
                        .get("continuousBandSize")
                        .cloned()
                        .unwrap_or(Value::Null);
                }
        }
        if st == ScaleType::Point {
            return scale_config.get("pointPadding").cloned().unwrap_or(Value::Null);
        }
    }
    Value::Null
}

pub fn padding_inner(
    padding_value: &Value,
    channel: &str,
    mark_def: &Value,
    scale_type: Option<ScaleType>,
    config: &Value,
    has_nested_offset_scale: bool,
) -> Value {
    if !padding_value.is_null() {
        return Value::Null;
    }
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    if is_x_or_y(channel) {
        if has_nested_offset_scale {
            return scale_config
                .get("bandWithNestedOffsetPaddingInner")
                .cloned()
                .unwrap_or(Value::Null);
        }
        let mark = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
        let band = scale_config.get("bandPaddingInner").cloned();
        let mark_specific = if mark == "bar" {
            scale_config.get("barBandPaddingInner").cloned()
        } else if mark == "tick" {
            scale_config.get("tickBandPaddingInner").cloned()
        } else {
            scale_config.get("rectBandPaddingInner").cloned()
        };
        return get_first_defined(&[band.as_ref(), mark_specific.as_ref()])
            .cloned()
            .unwrap_or(Value::Null);
    } else if is_x_or_y_offset(channel) && scale_type == Some(ScaleType::Band) {
        return scale_config
            .get("offsetBandPaddingInner")
            .cloned()
            .unwrap_or(Value::Null);
    }
    Value::Null
}

pub fn padding_outer(
    padding_value: &Value,
    channel: &str,
    scale_type: Option<ScaleType>,
    padding_inner_value: &Value,
    config: &Value,
    has_nested_offset_scale: bool,
) -> Value {
    if !padding_value.is_null() {
        return Value::Null;
    }
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    if is_x_or_y(channel) {
        if has_nested_offset_scale {
            return scale_config
                .get("bandWithNestedOffsetPaddingOuter")
                .cloned()
                .unwrap_or(Value::Null);
        }
        if scale_type == Some(ScaleType::Band) {
            if let Some(outer) = scale_config.get("bandPaddingOuter") {
                if !outer.is_null() {
                    return outer.clone();
                }
            }
            if crate::channeldef_support::is_signal_ref(padding_inner_value) {
                let signal = padding_inner_value
                    .get("signal")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                return json!({"signal": format!("{signal}/2")});
            }
            if let Some(n) = padding_inner_value.as_f64() {
                return crate::compile::data::support::number_value(n / 2.0);
            }
            return Value::Null;
        }
    } else if is_x_or_y_offset(channel) {
        if scale_type == Some(ScaleType::Point) {
            return json!(0.5);
        } else if scale_type == Some(ScaleType::Band) {
            return scale_config
                .get("offsetBandPaddingOuter")
                .cloned()
                .unwrap_or(Value::Null);
        }
    }
    Value::Null
}

pub fn reverse(
    scale_type: Option<ScaleType>,
    sort: &Value,
    channel: &str,
    config: &Value,
) -> Value {
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    let st = match scale_type {
        Some(t) => t,
        None => return Value::Null,
    };
    if channel == "x" && !scale_config.get("xReverse").unwrap_or(&Value::Null).is_null() {
        let x_reverse = scale_config.get("xReverse").cloned().unwrap_or(Value::Null);
        if has_continuous_domain(st) && sort.as_str() == Some("descending") {
            if crate::channeldef_support::is_signal_ref(&x_reverse) {
                let signal = x_reverse.get("signal").and_then(Value::as_str).unwrap_or("");
                return json!({"signal": format!("!{signal}")});
            }
            return Value::Bool(!x_reverse.as_bool().unwrap_or(false));
        }
        return x_reverse;
    }
    if has_continuous_domain(st) && sort.as_str() == Some("descending") {
        return Value::Bool(true);
    }
    Value::Null
}

/// Port of `zero` from `src/compile/scale/properties.ts`.
// Mirrors the upstream signature, which takes the whole scale context.
#[allow(clippy::too_many_arguments)]
pub fn zero(
    channel: &str,
    field_def: &Value,
    specified_domain: &Value,
    mark_def: &Value,
    scale_type: Option<ScaleType>,
    config: &Value,
    has_secondary_range_channel: bool,
    has_ranged_offset: bool,
) -> Value {
    let st = match scale_type {
        Some(t) => t,
        None => return Value::Null,
    };
    let scale_config = config.get("scale").cloned().unwrap_or(Value::Null);
    let has_custom_domain = !specified_domain.is_null()
        && specified_domain.as_str() != Some("unaggregated");
    if has_custom_domain && has_continuous_domain(st) {
        if let Some(arr) = specified_domain.as_array() {
            let first = arr.first().and_then(Value::as_f64);
            let last = arr.last().and_then(Value::as_f64);
            if let (Some(first), Some(last)) = (first, last) {
                if first <= 0.0 && last >= 0.0 {
                    return Value::Bool(true);
                }
            }
        }
        return Value::Bool(false);
    }

    if channel == "size"
        && field_def.get("type").and_then(Value::as_str) == Some("quantitative")
        && !is_continuous_to_discrete(st)
    {
        return Value::Bool(true);
    }

    if is_x_or_y_offset(channel) {
        return Value::Bool(has_ranged_offset);
    }

    if !(is_field_def(field_def) && crate::channeldef_support::truthy(&field_def["bin"]))
        && (contains(&POSITION_SCALE_CHANNELS, &channel)
            || contains(&POLAR_POSITION_SCALE_CHANNELS, &channel))
    {
        let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");
        let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
        if ["bar", "area", "line", "trail"].contains(&mark_type)
            && ((orient == "horizontal" && channel == "y")
                || (orient == "vertical" && channel == "x"))
            {
                return Value::Bool(false);
            }
        if is_bar_or_area(mark_type) && !has_secondary_range_channel {
            return Value::Bool(true);
        }
        return scale_config.get("zero").cloned().unwrap_or(Value::Null);
    }

    Value::Bool(false)
}

/// Minimal port of `UnitModel.isRangedOffset`.
fn is_ranged_offset(model: &Model, channel: &str) -> bool {
    let encoding = match model.encoding() {
        Some(e) => e.clone(),
        None => return false,
    };
    let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
    let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
    if !is_bar_or_area(mark_type) {
        return false;
    }
    let secondary = get_secondary_range_channel(channel);
    if secondary
        .map(|c| get_field_or_datum_def(&encoding[c]).is_some())
        .unwrap_or(false)
        || !crate::encoding::channel_has_quantitative_offset(&encoding, channel)
    {
        return false;
    }
    let channel_def = &encoding[channel];
    if channel_def.is_null() {
        return true;
    }
    is_field_or_datum_def(channel_def)
        && model
            .get_scale_component(channel)
            .and_then(|c| c.get_type())
            .map(has_continuous_domain)
            .map(|continuous| !continuous)
            .unwrap_or(false)
}

#[allow(dead_code)]
fn _unused(_: BTreeMap<String, Value>) {}
