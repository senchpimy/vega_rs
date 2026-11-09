//! Port of `src/compile/selection/inputs.ts`.

use serde_json::{json, Value};

use crate::compile::model::Model;
use crate::selection::is_legend_binding;
use crate::util::{flat_access_with_datum, var_name};

use super::assemble::assemble_init;
use super::component::{disable_direct_manipulation, SelectionCompiler, SelectionComponent, TUPLE};
use super::nearest;
use super::project::TUPLE_FIELDS;

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
    sel_cmpt.type_ == "point"
        && sel_cmpt.resolve == "global"
        && sel_cmpt.bind.is_some()
        && sel_cmpt.bind.as_ref().map(|b| b.as_str()) != Some(Some("scales"))
        && !sel_cmpt
            .bind
            .as_ref()
            .map(is_legend_binding)
            .unwrap_or(false)
}

pub fn parse(_model: &mut Model, sel_cmpt: &mut SelectionComponent, sel_def: &mut Value) {
    disable_direct_manipulation(sel_cmpt, sel_def);
}

pub fn top_level_signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let bind = sel_cmpt.bind.clone().unwrap_or(Value::Null);
    let init = sel_cmpt
        .init
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .cloned();
    let datum = if nearest::defined(sel_cmpt) {
        "(item().isVoronoi ? datum.datum : datum)"
    } else {
        "datum"
    };

    for (i, p) in sel_cmpt.project.items.iter().enumerate() {
        let sgname = var_name(&format!("{}_{}", name, p.field));
        let has_signal = signals
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some(sgname.as_str()));

        if !has_signal {
            let mut signal = json!({"name": sgname});
            if let Some(init) = &init {
                let init_value = init
                    .as_array()
                    .and_then(|a| a.get(i))
                    .cloned()
                    .unwrap_or(Value::Null);
                signal["init"] = assemble_init(&init_value, true);
            } else {
                signal["value"] = Value::Null;
            }

            if let Some(events) = &sel_cmpt.events {
                signal["on"] = json!([{
                    "events": events,
                    "update": format!(
                        "datum && item().mark.marktype !== 'group' ? {} : null",
                        flat_access_with_datum(&p.field, datum)
                    ),
                }]);
            }

            let mut bind_value = bind
                .get(&p.field)
                .cloned()
                .or_else(|| p.channel.as_ref().and_then(|c| bind.get(c)).cloned())
                .unwrap_or_else(|| bind.clone());
            if bind_value.is_null() {
                bind_value = bind.clone();
            }
            signal["bind"] = bind_value;

            signals.insert(0, signal);
        }
    }

    signals
}

pub fn signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let fields = format!("{}{}", name, TUPLE_FIELDS);
    let values: Vec<String> = sel_cmpt
        .project
        .items
        .iter()
        .map(|p| var_name(&format!("{}_{}", name, p.field)))
        .collect();
    let valid: Vec<String> = values.iter().map(|v| format!("{v} !== null")).collect();

    let tuple_name = format!("{}{}", name, TUPLE);
    let idx = signals
        .iter()
        .position(|s| s.get("name").and_then(Value::as_str) == Some(tuple_name.as_str()));

    if let Some(idx) = idx {
        if !values.is_empty() {
            signals[idx]["update"] = json!(format!(
                "{} ? {{fields: {}, values: [{}]}} : null",
                valid.join(" && "),
                fields,
                values.join(", ")
            ));
        }
        if let Some(obj) = signals[idx].as_object_mut() {
            obj.remove("value");
            obj.remove("on");
        }
    }

    signals
}
