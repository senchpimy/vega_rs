//! Port of `src/compile/selection/scales.ts`.

use serde_json::{json, Value};

use crate::channel::is_scale_channel;
use crate::compile::model::{is_layer_model, Model};
use crate::log;
use crate::scale::has_continuous_domain;
use crate::util::{replace_path_in_field, string_value};

use super::component::{
    SelectionCompiler, SelectionComponent, SelectionProjection, VL_SELECTION_RESOLVE,
};
use super::message;

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: Some(parse),
        signals: Some(signals),
        top_level_signals: Some(top_level_signals),
        modify_expr: None,
        marks: None,
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "interval"
        && sel_cmpt.resolve == "global"
        && sel_cmpt.bind.as_ref().map(|b| b.as_str()) == Some(Some("scales"))
}

pub fn parse(model: &mut Model, sel_cmpt: &mut SelectionComponent, _sel_def: &mut Value) {
    let mut bound: Vec<SelectionProjection> = Vec::new();

    for proj in sel_cmpt.project.items.clone() {
        let Some(channel): Option<String> = proj.channel.clone() else {
            continue;
        };
        if !is_scale_channel(&channel) {
            continue;
        }

        let scale = model.get_scale_component(&channel);
        let scale_type = scale.as_ref().and_then(|s| s.get_type());

        if scale_type == Some(crate::scale::ScaleType::Sequential) {
            log::warn(message::SEQUENTIAL_SCALE_DEPRECATED);
        }

        let continuous = scale_type.map(has_continuous_domain).unwrap_or(false);
        if scale.is_none() || !continuous {
            log::warn(message::SCALE_BINDINGS_CONTINUOUS);
            continue;
        }

        if let Some(component) = model.component.scales.get_mut(&channel) {
            component.set(
                "selectionExtent",
                json!({"param": sel_cmpt.name, "field": proj.field}),
                true,
            );
        }
        bound.push(proj);
    }

    sel_cmpt.scales = Some(bound);
}

pub fn top_level_signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let bound: Vec<SelectionProjection> = sel_cmpt
        .scales
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|proj| {
            let data = proj
                .signals
                .as_ref()
                .and_then(|s| s.data.clone())
                .unwrap_or_default();
            !signals
                .iter()
                .any(|s| s.get("name").and_then(Value::as_str) == Some(data.as_str()))
        })
        .collect();

    if model.parent_type.is_none() || is_top_level_layer(model) || bound.is_empty() {
        return signals;
    }

    let named_idx = signals
        .iter()
        .position(|s| s.get("name").and_then(Value::as_str) == Some(sel_cmpt.name.as_str()));
    let Some(named_idx) = named_idx else {
        return signals;
    };

    let mut update = signals[named_idx]
        .get("update")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if update.contains(VL_SELECTION_RESOLVE) {
        let mappings: Vec<String> = bound
            .iter()
            .map(|proj| {
                format!(
                    "{}: {}",
                    string_value(&json!(replace_path_in_field(&proj.field))),
                    proj.signals
                        .as_ref()
                        .and_then(|s| s.data.clone())
                        .unwrap_or_default()
                )
            })
            .collect();
        update = format!("{{{}}}", mappings.join(", "));
    } else {
        for proj in &bound {
            let mapping = format!(
                "{}: {}",
                string_value(&json!(replace_path_in_field(&proj.field))),
                proj.signals
                    .as_ref()
                    .and_then(|s| s.data.clone())
                    .unwrap_or_default()
            );
            if !update.contains(&mapping) {
                let prefix = &update[..update.len().saturating_sub(1)];
                update = format!("{prefix}, {mapping}}}");
            }
        }
    }
    signals[named_idx]["update"] = json!(update);

    for proj in bound {
        signals.push(json!({
            "name": proj.signals.as_ref().and_then(|s| s.data.clone()).unwrap_or_default(),
        }));
    }

    signals
}

pub fn signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    if model.parent_type.is_some() && !is_top_level_layer(model) {
        for proj in sel_cmpt.scales.clone().unwrap_or_default() {
            let data = proj
                .signals
                .as_ref()
                .and_then(|s| s.data.clone())
                .unwrap_or_default();
            if let Some(idx) = signals
                .iter()
                .position(|s| s.get("name").and_then(Value::as_str) == Some(data.as_str()))
            {
                signals[idx]["push"] = json!("outer");
                if let Some(obj) = signals[idx].as_object_mut() {
                    obj.remove("value");
                    obj.remove("update");
                }
            }
        }
    }
    signals
}

/// Port of `domain`.
pub fn domain(model: &Model, channel: &str) -> String {
    let scale = string_value(&json!(model.scale_name(channel, false).unwrap_or_default()));
    format!("domain({scale})")
}

fn is_top_level_layer(model: &Model) -> bool {
    model.parent_type == Some(crate::compile::model::SpecType::Layer) && !is_layer_model(model)
}
