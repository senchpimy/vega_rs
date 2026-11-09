//! Port of `src/compile/legend/parse.ts`.

use std::cell::Cell;

use serde_json::{json, Map, Value};

use crate::channel::{COLOR, SHAPE};
use crate::channeldef::{get_field_or_datum_def, is_field_def};
use crate::compile::model::{is_unit_model, Model};
use crate::compile::split::{default_tie_breaker, merge_values_with_explicit, Explicit};
use crate::compile::support::Resolve;
use crate::timeunit::normalize_time_unit;
use crate::type_::GEOJSON;
use crate::util::{deep_equal, has_property, var_name};

use super::component::{
    legend_component_properties, legends_from_value, legends_to_value, LegendComponent,
    LegendComponentIndex, LEGEND_SCALE_CHANNELS,
};
use super::encode::{legend_encode_rule, LegendEncodeParams};
use super::properties::{
    get_direction, get_legend_type, legend_rules, LegendRuleParams,
};

/// Port of `UnitModel.legend`.
pub fn model_legend(model: &Model, channel: &str) -> Option<Value> {
    model.unit.specified_legends.get(channel).cloned()
}

/// Port of `parseGuideResolve` (from `src/compile/resolve.ts`).
pub fn parse_guide_resolve(resolve: &Resolve, channel: &str) -> String {
    let channel_scale_resolve = resolve.scale.get(channel).and_then(Value::as_str);
    let guide = if channel == "x" || channel == "y" {
        "axis"
    } else {
        "legend"
    };

    if channel_scale_resolve == Some("independent") {
        let guide_map = if guide == "axis" {
            &resolve.axis
        } else {
            &resolve.legend
        };
        if guide_map.get(channel).and_then(Value::as_str) == Some("shared") {
            crate::log::warn(format!(
                "Setting the scale to be independent for \"{channel}\" means we also have to set the guide (axis or legend) to be independent."
            ));
        }
        return "independent".to_string();
    }

    resolve
        .legend
        .get(channel)
        .and_then(Value::as_str)
        .unwrap_or("shared")
        .to_string()
}

/// Port of `parseLegend`.
pub fn parse_legend(model: &mut Model) {
    if is_unit_model(model) {
        let index = parse_unit_legend(model);
        model.component.legends = legends_to_value(&index);
    } else {
        parse_non_unit_legend(model);
    }
}

fn parse_unit_legend(model: &mut Model) -> LegendComponentIndex {
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let mut legend_component = LegendComponentIndex::new();

    let mut channels: Vec<&str> = vec![COLOR];
    channels.extend(LEGEND_SCALE_CHANNELS.iter());

    for channel in channels {
        let def = match get_field_or_datum_def(&encoding[channel]) {
            Some(def) => def,
            None => continue,
        };

        if model.get_scale_component(channel).is_none() {
            continue;
        }

        if channel == SHAPE
            && is_field_def(&def)
            && def.get("type").and_then(Value::as_str) == Some(GEOJSON.as_str())
        {
            continue;
        }

        legend_component.insert(
            channel.to_string(),
            parse_legend_for_channel(model, channel),
        );
    }

    legend_component
}

/// `getLegendDefWithScale`.
fn get_legend_def_with_scale(model: &Model, channel: &str) -> Map<String, Value> {
    let scale = model.scale_name(channel, false).unwrap_or_default();
    let mark = model.mark_type();
    let mut out = Map::new();

    if mark == "trail" {
        if channel == "color" {
            out.insert("stroke".to_string(), Value::String(scale));
            return out;
        } else if channel == "size" {
            out.insert("strokeWidth".to_string(), Value::String(scale));
            return out;
        }
    }

    if channel == "color" {
        let filled = model
            .mark_def()
            .and_then(|m| m.get("filled"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.insert(
            if filled { "fill" } else { "stroke" }.to_string(),
            Value::String(scale),
        );
    } else {
        out.insert(channel.to_string(), Value::String(scale));
    }
    out
}

/// Port of `isExplicit`.
fn is_explicit(value: &Value, property: &str, legend: &Value, field_def: &Value) -> bool {
    match property {
        "disable" => !legend.is_null(), // specified legend is present
        "values" => legend
            .get("values")
            .map(crate::channeldef_support::truthy)
            .unwrap_or(false),
        "title" if value_matches_field_title(value, field_def) => true,
        _ => {
            let legend_value = legend.get(property);
            match legend_value {
                Some(legend_value) => deep_equal(value, legend_value),
                None => false,
            }
        }
    }
}

fn value_matches_field_title(value: &Value, field_def: &Value) -> bool {
    match field_def.get("title") {
        Some(title) => deep_equal(value, title),
        None => false,
    }
}

/// Port of `parseLegendForChannel`.
pub fn parse_legend_for_channel(model: &mut Model, channel: &str) -> LegendComponent {
    let legend_spec = model_legend(model, channel);
    let legend_defined = legend_spec.is_some();
    let mut legend = legend_spec.clone().unwrap_or(Value::Null);

    let mut legend_cmpt = LegendComponent::with_implicit(get_legend_def_with_scale(model, channel));

    super::super::selection::legends::parse_interactive_legend(model, channel, &mut legend_cmpt);

    let null_value = Value::Null;
    let mark_def = model.mark_def().unwrap_or(&null_value);
    let encoding = model.encoding().unwrap_or(&null_value);
    let config = &model.config;
    let legend_config = config.get("legend").unwrap_or(&null_value);

    let disable = if legend_defined {
        !crate::channeldef_support::truthy(&legend)
    } else {
        legend_config
            .get("disable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    legend_cmpt.set("disable", Value::Bool(disable), legend_defined);
    if disable {
        return legend_cmpt;
    }

    if !legend.is_object() {
        legend = json!({});
    }

    let scale_type = model
        .get_scale_component_ref(channel)
        .and_then(|component| component.get_type());
    let field_or_datum_def =
        get_field_or_datum_def(&encoding[channel]).unwrap_or(Value::Null);
    let time_unit = if is_field_def(&field_or_datum_def) {
        normalize_time_unit(
            field_or_datum_def
                .get("timeUnit")
                .unwrap_or(&Value::Null),
        )
        .and_then(|params| params.unit)
    } else {
        None
    };

    let orient = legend
        .get("orient")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            legend_config
                .get("orient")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "right".to_string());

    let legend_type = get_legend_type(
        &legend,
        channel,
        time_unit.as_deref(),
        scale_type,
    );

    let direction = get_direction(
        legend_config,
        &legend_type,
        Some(&orient),
        &legend,
    );

    let rule_params = LegendRuleParams {
        legend: &legend,
        channel,
        model,
        mark_def,
        encoding,
        field_or_datum_def: &field_or_datum_def,
        legend_config,
        config,
        scale_type,
        orient: Some(orient.clone()),
        legend_type: &legend_type,
        direction: direction.clone(),
    };

    for property in legend_component_properties() {
        if (legend_type == "gradient" && property.starts_with("symbol"))
            || (legend_type == "symbol" && property.starts_with("gradient"))
        {
            continue;
        }

        let value = if RULE_PROPERTIES.contains(&property.as_str()) {
            legend_rules(&property, &rule_params)
        } else {
            legend.get(&property).cloned()
        };

        if let Some(value) = value {
            let explicit = is_explicit(&value, &property, &legend, &field_or_datum_def);
            let config_has_property = config
                .get("legend")
                .and_then(|l| l.get(&property))
                .is_some();
            if explicit || !config_has_property {
                legend_cmpt.set(&property, value, explicit);
            }
        }
    }

    let legend_encoding = legend
        .get("encoding")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let selections = legend_cmpt.get("selections");
    let selections_len = selections
        .as_ref()
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let mut legend_encode = Map::new();

    let encode_params = LegendEncodeParams {
        field_or_datum_def: &field_or_datum_def,
        model,
        channel,
        legend_cmpt: &legend_cmpt,
        legend_type: &legend_type,
    };

    for part in ["labels", "legend", "title", "symbols", "gradient", "entries"] {
        let part_spec = legend_encoding
            .get(part)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let legend_encoding_part = guide_encode_entry(&part_spec, model);

        let value = if ["labels", "legend", "title", "symbols", "gradient", "entries"]
            .contains(&part)
            && ["symbols", "gradient", "labels", "entries"].contains(&part)
        {
            legend_encode_rule(part, &legend_encoding_part, &encode_params)
        } else {
            Some(legend_encoding_part)
        };

        if let Some(value) = value {
            if !value.is_empty() {
                let mut entry = Map::new();
                if selections_len > 0 && is_field_def(&field_or_datum_def) {
                    let field = field_or_datum_def
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    entry.insert(
                        "name".to_string(),
                        Value::String(format!("{}_legend_{}", var_name(field), part)),
                    );
                }
                if selections_len > 0 {
                    entry.insert("interactive".to_string(), Value::Bool(true));
                }
                let update = if selections_len > 0 {
                    let mut update = value.clone();
                    update.insert(
                        "cursor".to_string(),
                        json!({"value": "pointer"}),
                    );
                    update
                } else {
                    value
                };
                entry.insert("update".to_string(), Value::Object(update));
                legend_encode.insert(part.to_string(), Value::Object(entry));
            }
        }
    }

    if !legend_encode.is_empty() {
        let has_encoding = legend
            .get("encoding")
            .map(crate::channeldef_support::truthy)
            .unwrap_or(false);
        legend_cmpt.set("encode", Value::Object(legend_encode), has_encoding);
    }

    legend_cmpt
}

const RULE_PROPERTIES: [&str; 9] = [
    "direction",
    "format",
    "formatType",
    "gradientLength",
    "labelOverlap",
    "symbolType",
    "title",
    "type",
    "values",
];

/// Simplified port of `guideEncodeEntry`.
fn guide_encode_entry(encoding: &Map<String, Value>, _model: &Model) -> Map<String, Value> {
    let mut out = Map::new();
    for (channel, channel_def) in encoding {
        if let Some(value) = channel_def.get("value") {
            out.insert(
                channel.clone(),
                crate::compile_common::signal_or_value_ref(value),
            );
        } else if has_property(channel_def, "signal") {
            out.insert(channel.clone(), channel_def.clone());
        } else if let Some(condition) = channel_def.get("condition") {
            out.insert("condition".to_string(), condition.clone());
        }
    }
    out
}

fn parse_non_unit_legend(model: &mut Model) {
    let mut legends = legends_from_value(&model.component.legends);

    for child_index in 0..model.children.len() {
        parse_legend(&mut model.children[child_index]);
        let child_legends = legends_from_value(&model.children[child_index].component.legends);

        for (channel, child_legend) in child_legends {
            let resolve_value = parse_guide_resolve(&model.component.resolve, &channel);
            model
                .component
                .resolve
                .legend
                .insert(channel.clone(), Value::String(resolve_value.clone()));

            if resolve_value == "shared" {
                match legends.get_mut(&channel) {
                    None => {
                        legends.insert(channel.clone(), child_legend);
                    }
                    Some(existing) => {
                        if !merge_legend_component(existing, &child_legend) {
                            model.component.resolve.legend.insert(
                                channel.clone(),
                                Value::String("independent".to_string()),
                            );
                            legends.remove(&channel);
                        }
                    }
                }
            }
        }
    }

    let channels: Vec<String> = legends.keys().cloned().collect();
    for channel in channels {
        if model
            .component
            .resolve
            .legend
            .get(&channel)
            .and_then(Value::as_str)
            != Some("shared")
        {
            continue;
        }
        for child in &mut model.children {
            let mut child_legends = legends_from_value(&child.component.legends);
            if child_legends.remove(&channel).is_some() {
                child.component.legends = legends_to_value(&child_legends);
            }
        }
    }

    model.component.legends = legends_to_value(&legends);
}

/// Port of `mergeLegendComponent`. Returns `false` when the components cannot
/// be merged (conflicting explicit orients).
pub fn merge_legend_component(merged: &mut LegendComponent, child: &LegendComponent) -> bool {
    let merged_orient = merged.get_with_explicit("orient");
    let child_orient = child.get_with_explicit("orient");

    if merged_orient.explicit
        && child_orient.explicit
        && merged_orient.value != child_orient.value
    {
        return false;
    }

    let type_merged = Cell::new(false);
    for prop in legend_component_properties() {
        let merged_value = merge_values_with_explicit(
            Some(merged.get_with_explicit(&prop)),
            child.get_with_explicit(&prop),
            &prop,
            "legend",
            |v1: &Explicit, v2: &Explicit, property: &str, property_of: &str| match property {
                "symbolType" => merge_symbol_type(v1, v2),
                "title" => merge_title_component(v1, v2),
                "type" => {
                    type_merged.set(true);
                    crate::compile::split::make_implicit(Value::String("symbol".to_string()))
                }
                _ => default_tie_breaker(v1, v2, property, property_of),
            },
        );
        merged.set_with_explicit(&prop, merged_value);
    }

    if type_merged.get() {
        for map in [&mut merged.implicit, &mut merged.explicit] {
            if let Some(encode) = map.get_mut("encode") {
                if let Some(encode) = encode.as_object_mut() {
                    encode.remove("gradient");
                }
            }
        }
    }

    true
}

fn merge_symbol_type(st1: &Explicit, st2: &Explicit) -> Explicit {
    if st2.value.as_ref().and_then(Value::as_str) == Some("circle") {
        st2.clone()
    } else {
        st1.clone()
    }
}

/// Port of `mergeTitleComponent` for legend titles.
fn merge_title_component(v1: &Explicit, v2: &Explicit) -> Explicit {
    let v1_val = v1.value.clone();
    let v2_val = v2.value.clone();

    if v1_val.is_none() || v2_val.as_ref().map(Value::is_null).unwrap_or(false) {
        return Explicit {
            explicit: v1.explicit,
            value: Some(Value::Null),
        };
    }

    if is_text_or_signal(v1_val.as_ref().unwrap()) && is_text_or_signal(v2_val.as_ref().unwrap()) {
        return Explicit {
            explicit: v1.explicit,
            value: Some(crate::compile_common::merge_title(
                v1_val.as_ref().unwrap(),
                v2_val.as_ref().unwrap(),
            )),
        };
    } else if is_text_or_signal(v1_val.as_ref().unwrap()) {
        return Explicit {
            explicit: v1.explicit,
            value: v1_val,
        };
    } else if is_text_or_signal(v2_val.as_ref().unwrap()) {
        return Explicit {
            explicit: v1.explicit,
            value: v2_val,
        };
    }

    Explicit {
        explicit: v1.explicit,
        value: Some(crate::compile_common::merge_title_field_defs(
            &crate::util::array(v1_val.as_ref().unwrap()),
            &crate::util::array(v2_val.as_ref().unwrap()),
        )
        .into_iter()
        .collect::<Vec<_>>()
        .into()),
    }
}

fn is_text_or_signal(value: &Value) -> bool {
    matches!(value, Value::String(_)) || has_property(value, "signal")
}
