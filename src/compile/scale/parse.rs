//! Port of `src/compile/scale/parse.ts`.

use serde_json::Value;

use crate::channel::{SCALE_CHANNELS, SHAPE};
use crate::channeldef::get_field_or_datum_def;
use crate::encoding::channel_has_nested_offset_scale;
use crate::mark::GEOSHAPE;
use crate::scale::{
    scale_compatible, scale_type_precedence, NON_TYPE_DOMAIN_RANGE_VEGA_SCALE_PROPERTIES, ScaleType,
};
use crate::type_::GEOJSON;

use super::super::model::{is_unit_model, Model};
use super::super::split::{
    default_tie_breaker, merge_values_with_explicit, Explicit,
};
use super::super::support::default_scale_resolve;
use super::component::{ScaleComponent, ScaleComponentIndex};
use super::domain::parse_scale_domain;
use super::properties::{parse_scale_property, parse_scale_range};
use super::type_::scale_type;

/// Port of `parseScales`.
pub fn parse_scales(model: &mut Model, ignore_range: bool) {
    parse_scale_core(model);
    parse_scale_domain(model);
    for prop in NON_TYPE_DOMAIN_RANGE_VEGA_SCALE_PROPERTIES {
        parse_scale_property(model, prop);
        model.sync_merged_scales();
    }
    if !ignore_range {
        parse_scale_range(model);
        model.sync_merged_scales();
    }
}

/// Port of `parseScaleCore`.
pub fn parse_scale_core(model: &mut Model) {
    if is_unit_model(model) {
        model.component.scales = parse_unit_scale_core(model);
    } else {
        model.component.scales = parse_non_unit_scale_core(model);
    }
}

fn parse_unit_scale_core(model: &Model) -> ScaleComponentIndex {
    let null_value = Value::Null;
    let encoding = model.encoding().unwrap_or(&null_value);
    let mark = model.mark_type();
    let mark_def = model.mark_def().unwrap_or(&null_value);
    let mut scale_components = ScaleComponentIndex::new();

    for channel in SCALE_CHANNELS {
        let field_or_datum_def = match get_field_or_datum_def(&encoding[channel]) {
            Some(f) => f,
            None => continue,
        };

        // Don't generate scale for shape of geoshape.
        if mark == GEOSHAPE.as_str()
            && channel == SHAPE
            && field_or_datum_def.get("type").and_then(Value::as_str) == Some(GEOJSON.as_str())
        {
            continue;
        }

        let specified_scale_value = field_or_datum_def.get("scale").cloned();

        // Upstream: `specifiedScale !== null && specifiedScale !== false`.
        // An absent scale is `undefined` and does produce a scale component.
        if let Some(value) = &specified_scale_value {
            if value.is_null() || value.as_bool() == Some(false) {
                continue;
            }
        }

        let specified_scale = match specified_scale_value {
            Some(value) if value.is_object() => value,
            _ => Value::Object(Default::default()),
        };
        let has_nested_offset_scale = channel_has_nested_offset_scale(encoding, channel);
        let s_type = scale_type(
            &specified_scale,
            channel,
            &field_or_datum_def,
            mark_def,
            has_nested_offset_scale,
        );
        let s_type = match s_type {
            Some(t) => t,
            None => continue,
        };

        let explicit = specified_scale.get("type").and_then(Value::as_str) == Some(s_type.as_str());
        scale_components.insert(
            channel.to_string(),
            ScaleComponent::new(
                &model.scale_name(channel, true).unwrap_or_default(),
                Explicit {
                    explicit,
                    value: Some(Value::String(s_type.as_str().to_string())),
                },
            ),
        );
    }

    scale_components
}

fn parse_non_unit_scale_core(model: &mut Model) -> ScaleComponentIndex {
    let mut scale_components = ScaleComponentIndex::new();
    let mut scale_type_with_explicit: indexmap::IndexMap<String, Explicit> =
        indexmap::IndexMap::new();

    let model_type = model.type_;

    for child in &mut model.children {
        parse_scale_core(child);

        let channels: Vec<String> = child.component.scales.keys().cloned().collect();
        for channel in channels {
            if !model.component.resolve.scale.contains_key(&channel) {
                model.component.resolve.scale.insert(
                    channel.clone(),
                    Value::String(default_scale_resolve(&channel, model_type).to_string()),
                );
            }

            if model.component.resolve.get_scale(&channel).as_deref() == Some("shared") {
                let child_scale_type = child.component.scales[&channel].get_with_explicit("type");
                let explicit_type = scale_type_with_explicit.get(&channel).cloned();
                if let Some(explicit_type) = explicit_type {
                    let compatible = match (
                        explicit_type.value.as_ref().and_then(Value::as_str),
                        child_scale_type.value.as_ref().and_then(Value::as_str),
                    ) {
                        (Some(a), Some(b)) => match (a.parse::<ScaleType>(), b.parse::<ScaleType>()) {
                            (Ok(a), Ok(b)) => scale_compatible(a, b),
                            _ => false,
                        },
                        _ => false,
                    };
                    if compatible {
                        let merged = merge_values_with_explicit(
                            Some(explicit_type),
                            child_scale_type,
                            "type",
                            "scale",
                            |v1, v2, _p, _po| {
                                let p1 = v1
                                    .value
                                    .as_ref()
                                    .and_then(Value::as_str)
                                    .and_then(|s| s.parse::<ScaleType>().ok())
                                    .map(scale_type_precedence)
                                    .unwrap_or(0);
                                let p2 = v2
                                    .value
                                    .as_ref()
                                    .and_then(Value::as_str)
                                    .and_then(|s| s.parse::<ScaleType>().ok())
                                    .map(scale_type_precedence)
                                    .unwrap_or(0);
                                if p1 > p2 {
                                    v1.clone()
                                } else if p1 < p2 {
                                    v2.clone()
                                } else {
                                    default_tie_breaker(v1, v2, "type", "scale")
                                }
                            },
                        );
                        scale_type_with_explicit.insert(channel, merged);
                    } else {
                        model.component.resolve.scale.insert(
                            channel.clone(),
                            Value::String("independent".to_string()),
                        );
                        scale_type_with_explicit.shift_remove(&channel);
                    }
                } else {
                    scale_type_with_explicit.insert(channel, child_scale_type);
                }
            }
        }
    }

    for (channel, type_with_explicit) in scale_type_with_explicit {
        let name = model.scale_name(&channel, true).unwrap_or_default();
        scale_components.insert(
            channel.clone(),
            ScaleComponent::new(&name, type_with_explicit),
        );

        for child in &mut model.children {
            let old_name = child
                .component
                .scales
                .get(&channel)
                .and_then(|c| c.get("name"))
                .and_then(|v| v.as_str().map(str::to_string));
            if let Some(old_name) = old_name {
                child.rename_scale(&old_name, &name);
                if let Some(child_scale) = child.component.scales.get_mut(&channel) {
                    child_scale.merged = true;
                }
                child.merged_channels.push(channel.clone());
                child.merged_scales.insert(
                    channel.clone(),
                    std::rc::Rc::new(scale_components[&channel].clone()),
                );
            }
        }
    }

    scale_components
}
