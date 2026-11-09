//! Port of `src/compile/selection/project.ts`.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::channel::{
    get_position_channel_from_lat_long, is_geo_position_channel, is_scale_channel,
    is_single_def_unit_channel,
};
use crate::compile::model::Model;
use crate::log;
use crate::scale::has_continuous_domain;
use crate::selection::SELECTION_ID;
use crate::timeunit::is_binned_time_unit;
use crate::util::{array, keys, var_name};

use super::assemble::assemble_projection;
use super::component::{
    truthy_value, ProjectionSignals, SelectionCompiler, SelectionComponent, SelectionProjection,
    SelectionProjectionComponent,
};
use super::message;

pub const TUPLE_FIELDS: &str = "_tuple_fields";

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: Some(parse),
        signals: Some(signals),
        top_level_signals: None,
        modify_expr: None,
        marks: None,
    }
}

pub fn defined(_sel_cmpt: &SelectionComponent) -> bool {
    true
}

fn signal_name(
    name: &str,
    p: &SelectionProjection,
    range: &str,
    signals: &mut BTreeSet<String>,
) -> String {
    let suffix = if range == "visual" {
        p.channel.clone().unwrap_or_default()
    } else {
        p.field.clone()
    };
    let mut sg = var_name(&format!("{name}_{suffix}"));
    let mut counter = 1;
    while signals.contains(&sg) {
        sg = var_name(&format!("{name}_{suffix}_{counter}"));
        counter += 1;
    }
    signals.insert(sg.clone());
    sg
}

pub fn parse(model: &mut Model, sel_cmpt: &mut SelectionComponent, sel_def: &mut Value) {
    let name = sel_cmpt.name.clone();
    let parsed: &mut SelectionProjectionComponent = &mut sel_cmpt.project;
    let mut signals: BTreeSet<String> = BTreeSet::new();

    let type_ = sel_cmpt.type_.clone();
    let cfg = model
        .config
        .get("selection")
        .and_then(|s| s.get(&type_))
        .cloned()
        .unwrap_or(Value::Null);

    let init_raw: Option<Vec<Value>> = sel_def.get("value").map(array);

    let select_obj = if sel_def.get("select").map(Value::is_object).unwrap_or(false) {
        sel_def.get("select").cloned().unwrap()
    } else {
        json!({})
    };

    let mut fields: Option<Vec<String>> =
        select_obj.get("fields").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        });
    let mut encodings: Option<Vec<String>> = select_obj
        .get("encodings")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        });

    if fields.is_none() && encodings.is_none() {
        if let Some(init) = &init_raw {
            for init_val in init {
                if !init_val.is_object() {
                    continue;
                }
                for key in keys(init_val) {
                    if is_single_def_unit_channel(&key) {
                        encodings.get_or_insert_with(Vec::new).push(key);
                    } else if type_ == "interval" {
                        log::warn(message::INTERVAL_INITIALIZED_WITH_POS);
                        encodings = cfg.get("encodings").and_then(Value::as_array).map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        });
                    } else {
                        fields.get_or_insert_with(Vec::new).push(key);
                    }
                }
            }
        }
    }

    if fields.is_none() && encodings.is_none() {
        encodings = cfg.get("encodings").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        });
        if let Some(cfg_fields) = cfg.get("fields").and_then(Value::as_array) {
            if !cfg_fields.is_empty() || cfg.get("fields").is_some() {
                fields = Some(
                    cfg_fields
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect(),
                );
            }
        }
    }

    for channel in encodings.clone().unwrap_or_default() {
        let field_def = model.field_def(&channel);
        let Some(field_def) = field_def else {
            log::warn(message::cannot_project_on_channel_without_field(&channel));
            continue;
        };

        let mut field = field_def
            .get("field")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if field_def
            .get("aggregate")
            .map(truthy_value)
            .unwrap_or(false)
        {
            let aggregate = field_def
                .get("aggregate")
                .and_then(Value::as_str)
                .unwrap_or_default();
            log::warn(message::cannot_project_aggregate(&channel, aggregate));
            continue;
        } else if field.is_empty() {
            log::warn(message::cannot_project_on_channel_without_field(&channel));
            continue;
        }

        if let Some(time_unit) = field_def.get("timeUnit").filter(|v| truthy_value(v)) {
            if !is_binned_time_unit(time_unit) {
                field = model.vg_field(&channel);
                let component = json!({
                    "timeUnit": time_unit,
                    "as": field,
                    "field": field_def.get("field").cloned().unwrap_or(Value::Null),
                });
                // Use the same key shape as `makeTimeunitFromEncoding` so the
                // optimizer can merge both components into one time unit.
                let key = crate::util::stringify(&crate::util::hash(&json!({
                    "as": component.get("as").cloned().unwrap_or(Value::Null),
                    "field": component.get("field").cloned().unwrap_or(Value::Null),
                    "timeUnit": component.get("timeUnit").cloned().unwrap_or(Value::Null),
                })));
                let mut tu = match parsed.time_unit.clone() {
                    Some(Value::Object(m)) => m,
                    _ => Map::new(),
                };
                tu.insert(key, component);
                parsed.time_unit = Some(Value::Object(tu));
            }
        }

        // Prevent duplicate projections on the same field.
        if parsed.field(&field).is_none() {
            let tpl_type = if type_ == "interval"
                && is_scale_channel(&channel)
                && model
                    .get_scale_component(&channel)
                    .and_then(|c| c.get_type())
                    .map(has_continuous_domain)
                    .unwrap_or(false)
            {
                "R"
            } else if field_def.get("bin").map(truthy_value).unwrap_or(false) {
                "R-RE"
            } else {
                "E"
            };

            let index = parsed.items.len();
            let mut p = SelectionProjection {
                type_: tpl_type.to_string(),
                field: field.clone(),
                index,
                channel: Some(channel.clone()),
                geo_channel: None,
                signals: None,
                has_legend: false,
            };

            let data_sg = signal_name(&name, &p, "data", &mut signals);
            let visual_sg = signal_name(&name, &p, "visual", &mut signals);
            p.signals = Some(ProjectionSignals {
                data: Some(data_sg),
                visual: Some(visual_sg),
            });

            if is_geo_position_channel(&channel) {
                p.geo_channel = Some(channel.clone());
                p.channel = get_position_channel_from_lat_long(&channel).map(str::to_string);
                if let Some(ch) = p.channel.clone() {
                    parsed.has_channel.insert(ch, index);
                }
            } else {
                parsed.has_channel.insert(channel.clone(), index);
            }

            parsed.has_selection_id = parsed.has_selection_id || field == SELECTION_ID;
            parsed.has_field.insert(field.clone(), index);
            parsed.items.push(p);
        }
    }

    for field in fields.unwrap_or_default() {
        if parsed.field(&field).is_some() {
            continue;
        }
        let index = parsed.items.len();
        let mut p = SelectionProjection {
            type_: "E".to_string(),
            field: field.clone(),
            index,
            channel: None,
            geo_channel: None,
            signals: None,
            has_legend: false,
        };
        let data_sg = signal_name(&name, &p, "data", &mut signals);
        p.signals = Some(ProjectionSignals {
            data: Some(data_sg),
            visual: None,
        });
        parsed.has_selection_id = parsed.has_selection_id || field == SELECTION_ID;
        parsed.has_field.insert(field.clone(), index);
        parsed.items.push(p);
    }

    if let Some(init) = init_raw {
        let items: Vec<(Option<String>, String, usize)> = parsed
            .items
            .iter()
            .map(|p| {
                (
                    p.geo_channel.clone().or_else(|| p.channel.clone()),
                    p.field.clone(),
                    p.index,
                )
            })
            .collect();
        let mapped: Vec<Value> = init
            .iter()
            .map(|v| {
                let values: Vec<Value> = items
                    .iter()
                    .map(|(channel, field, _)| {
                        if v.is_object() {
                            if let Some(channel) = channel {
                                if let Some(val) = v.get(channel) {
                                    return val.clone();
                                }
                            }
                            v.get(field).cloned().unwrap_or(Value::Null)
                        } else {
                            v.clone()
                        }
                    })
                    .collect();
                Value::Array(values)
            })
            .collect();
        sel_cmpt.init = Some(Value::Array(mapped));
    }
}

pub fn signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    all_signals: Vec<Value>,
) -> Vec<Value> {
    let name = format!("{}{}", sel_cmpt.name, TUPLE_FIELDS);
    let has_signal = all_signals
        .iter()
        .any(|s| s.get("name").and_then(Value::as_str) == Some(name.as_str()));

    if has_signal || sel_cmpt.project.has_selection_id {
        all_signals
    } else {
        let value: Vec<Value> = sel_cmpt
            .project
            .items
            .iter()
            .map(assemble_projection)
            .collect();
        let mut signals = all_signals;
        signals.push(json!({"name": name, "value": value}));
        signals
    }
}
