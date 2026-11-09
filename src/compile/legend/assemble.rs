//! Port of `src/compile/legend/assemble.ts`.


use serde_json::{json, Map, Value};

use crate::compile::model::{is_unit_model, Model};
use crate::compile::scale::domain::assemble_domain;
use crate::scale::has_discrete_domain;
use crate::util;

use super::component::{
    legends_from_value, LegendComponent, LEGEND_SCALE_CHANNELS,
};
use super::parse::merge_legend_component;

fn set_legend_encode(
    legend: &mut Map<String, Value>,
    part: &str,
    vg_prop: &str,
    vg_ref: Value,
) {
    let encode = legend
        .entry("encode".to_string())
        .or_insert_with(|| json!({}));
    if !encode.is_object() {
        *encode = json!({});
    }
    let encode = encode.as_object_mut().unwrap();
    let part_value = encode
        .entry(part.to_string())
        .or_insert_with(|| json!({}));
    if !part_value.is_object() {
        *part_value = json!({});
    }
    let part_value = part_value.as_object_mut().unwrap();
    let update = part_value
        .entry("update".to_string())
        .or_insert_with(|| json!({}));
    if !update.is_object() {
        *update = json!({});
    }
    update
        .as_object_mut()
        .unwrap()
        .insert(vg_prop.to_string(), vg_ref);
}

/// Determines the underlying field name for a given scale channel within a
/// model hierarchy.
fn get_field_key_for_channel(model: &Model, channel: &str) -> Option<String> {
    if is_unit_model(model) {
        if let Some(field_def) = model.field_def(channel) {
            if let Some(field) = field_def.get("field").and_then(Value::as_str) {
                return Some(field.to_string());
            }
        }
    }

    let child_fields: Vec<String> = model
        .children
        .iter()
        .filter_map(|child| get_field_key_for_channel(child, channel))
        .collect();

    if !child_fields.is_empty() {
        let values: Vec<Value> = child_fields
            .iter()
            .map(|f| Value::String(f.clone()))
            .collect();
        let unique = util::unique(&values, util::hash);
        if unique.len() == 1 {
            return unique[0].as_str().map(str::to_string);
        }
        return None;
    }

    None
}

fn legends_are_merge_compatible(model: &Model, channel_a: &str, channel_b: &str) -> bool {
    if channel_a == channel_b {
        return true;
    }
    let type_a = model.get_scale_type(channel_a);
    let type_b = model.get_scale_type(channel_b);
    let (type_a, type_b) = match (type_a, type_b) {
        (Some(a), Some(b)) => (a, b),
        _ => return false,
    };
    has_discrete_domain(type_a) == has_discrete_domain(type_b)
}

fn get_legend_group_key(field_key: Option<&str>, channel: &str) -> String {
    match field_key {
        Some(field) => format!("field:{field}"),
        None => format!("channel:{channel}"),
    }
}

fn extract_discrete_values_from_domain(domain: &Value) -> Option<Vec<Value>> {
    if let Value::Array(items) = domain {
        let primitives: Vec<Value> = items
            .iter()
            .filter(|v| util::is_primitive(v))
            .cloned()
            .collect();
        return if primitives.is_empty() {
            None
        } else {
            Some(primitives)
        };
    }

    if let Some(fields) = domain.get("fields").and_then(Value::as_array) {
        let mut values: Vec<Value> = Vec::new();
        for field in fields {
            if let Value::Array(items) = field {
                values.extend(items.iter().filter(|v| util::is_primitive(v)).cloned());
            }
        }
        if !values.is_empty() {
            return Some(util::unique(&values, util::hash));
        }
    }

    None
}

fn get_discrete_values_for_channel(model: &Model, channel: &str) -> Option<Vec<Value>> {
    let domain = assemble_domain(model, channel);
    if domain.is_null() {
        return None;
    }
    extract_discrete_values_from_domain(&domain)
}

fn union_discrete_values_for_channels(
    model: &Model,
    channel_a: &str,
    channel_b: &str,
) -> Option<Vec<Value>> {
    let values_a = get_discrete_values_for_channel(model, channel_a);
    let values_b = get_discrete_values_for_channel(model, channel_b);
    match (values_a, values_b) {
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(util::unique(&a, util::hash))
        }
        _ => None,
    }
}

fn set_implicit_legend_values(
    component: &mut LegendComponent,
    values: Option<Vec<Value>>,
    warn_message: Option<String>,
) {
    if let Some(values) = values {
        if !values.is_empty() {
            let values_prop = component.get_with_explicit("values");
            if !values_prop.explicit {
                if let Some(message) = warn_message {
                    crate::log::warn(message);
                }
                component.set("values", Value::Array(values), false);
            }
        }
    }
}

fn domains_explicit_and_equal(model: &Model, channel_a: &str, channel_b: &str) -> bool {
    let component_a = match model.get_scale_component(channel_a) {
        Some(c) => c,
        None => return false,
    };
    let component_b = match model.get_scale_component(channel_b) {
        Some(c) => c,
        None => return false,
    };

    let domains_a = component_a.get_with_explicit("domains");
    let domains_b = component_b.get_with_explicit("domains");
    if !(domains_a.explicit && domains_b.explicit) {
        return false;
    }

    let assembled_a = assemble_domain(model, channel_a);
    let assembled_b = assemble_domain(model, channel_b);
    util::hash(&assembled_a) == util::hash(&assembled_b)
}

struct LegendEntry {
    channel: String,
    component: LegendComponent,
}

/// Port of `assembleLegends`.
pub fn assemble_legends(model: &Model) -> Vec<Value> {
    let legend_component_index = legends_from_value(&model.component.legends);
    let mut legends_by_group: Vec<(String, Vec<LegendEntry>)> = Vec::new();

    for (channel, component) in &legend_component_index {
        let field_key = get_field_key_for_channel(model, channel);
        let group_key = get_legend_group_key(field_key.as_deref(), channel);

        let group = match legends_by_group
            .iter_mut()
            .find(|(key, _)| *key == group_key)
            .map(|(_, group)| group)
        {
            Some(group) => group,
            None => {
                legends_by_group.push((
                    group_key.clone(),
                    vec![LegendEntry {
                        channel: channel.clone(),
                        component: component.clone_component(),
                    }],
                ));
                continue;
            }
        };

        let mut merged_into_existing = false;
        for existing in group.iter_mut() {
            if !legends_are_merge_compatible(model, &existing.channel, channel) {
                continue;
            }
            if merge_legend_component(&mut existing.component, component) {
                let type_a = model.get_scale_type(&existing.channel);
                let type_b = model.get_scale_type(channel);
                if let (Some(type_a), Some(type_b)) = (type_a, type_b) {
                    if has_discrete_domain(type_a) && has_discrete_domain(type_b) {
                        if domains_explicit_and_equal(model, &existing.channel, channel) {
                            let values = get_discrete_values_for_channel(model, &existing.channel);
                            set_implicit_legend_values(&mut existing.component, values, None);
                        } else {
                            let values = union_discrete_values_for_channels(
                                model,
                                &existing.channel,
                                channel,
                            );
                            let message = format!(
                                "Unioning discrete legend values from {} and {}.",
                                existing.channel, channel
                            );
                            set_implicit_legend_values(
                                &mut existing.component,
                                values,
                                Some(message),
                            );
                        }
                    }
                }
                merged_into_existing = true;
                break;
            }
        }

        if !merged_into_existing {
            group.push(LegendEntry {
                channel: channel.clone(),
                component: component.clone_component(),
            });
        }
    }

    let config = &model.config;
    let mut out = Vec::new();
    for (_, group) in &legends_by_group {
        for entry in group {
            if let Some(legend) = assemble_legend(&entry.component, config) {
                out.push(legend);
            }
        }
    }
    out
}

/// Port of `assembleLegend`.
pub fn assemble_legend(legend_cmpt: &LegendComponent, config: &Value) -> Option<Value> {
    let mut combined = legend_cmpt.combine_map();
    let disable = combined
        .remove("disable")
        .map(|v| crate::channeldef_support::truthy(&v))
        .unwrap_or(false);
    let label_expr = combined.remove("labelExpr");
    let _selections = combined.remove("selections");

    if disable {
        return None;
    }

    if config.get("aria").and_then(Value::as_bool) == Some(false) && combined.get("aria").is_none() {
        combined.insert("aria".to_string(), Value::Bool(false));
    }

    let symbols_update = combined
        .get("encode")
        .and_then(|e| e.get("symbols"))
        .and_then(|s| s.get("update"))
        .and_then(Value::as_object)
        .cloned();
    if let Some(mut symbols) = symbols_update {
        let fill_is_transparent = symbols
            .get("fill")
            .and_then(|f| f.get("value"))
            .and_then(Value::as_str)
            == Some("transparent");

        if symbols.get("fill").is_some()
            && !fill_is_transparent
            && symbols.get("stroke").is_none()
            && combined.get("stroke").is_none()
        {
            symbols.insert("stroke".to_string(), json!({"value": "transparent"}));
        }

        for property in LEGEND_SCALE_CHANNELS {
            if combined.get(property).is_some() {
                symbols.remove(property);
            }
        }

        if let Some(update) = combined
            .get_mut("encode")
            .and_then(|e| e.get_mut("symbols"))
            .and_then(|s| s.get_mut("update"))
        {
            *update = Value::Object(symbols);
        }
    }

    if !combined
        .get("title")
        .map(crate::channeldef_support::truthy)
        .unwrap_or(false)
    {
        combined.remove("title");
    }

    if let Some(label_expr) = label_expr {
        let mut expr = label_expr.as_str().map(str::to_string).unwrap_or_default();
        let text_signal = combined
            .get("encode")
            .and_then(|e| e.get("labels"))
            .and_then(|l| l.get("update"))
            .and_then(|u| u.get("text"))
            .filter(|text| text.get("signal").is_some())
            .and_then(|text| text.get("signal"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(signal) = text_signal {
            expr = util::replace_all(&expr, "datum.label", &signal);
        }
        set_legend_encode(&mut combined, "labels", "text", json!({"signal": expr}));
    }

    Some(Value::Object(combined))
}
