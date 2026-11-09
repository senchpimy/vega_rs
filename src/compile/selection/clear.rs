//! Port of `src/compile/selection/clear.ts`.

use serde_json::{json, Value};

use crate::compile::model::Model;
use crate::util::var_name;

use super::component::{is_timer_selection, SelectionCompiler, SelectionComponent, TUPLE};
use super::inputs;
use super::selector::parse_selector;
use super::toggle::{self, TOGGLE};

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
    sel_cmpt.clear.is_some()
        && sel_cmpt.clear != Some(Value::Bool(false))
        && !is_timer_selection(sel_cmpt)
}

pub fn parse(_model: &mut Model, sel_cmpt: &mut SelectionComponent, _sel_def: &mut Value) {
    if let Some(clear) = sel_cmpt.clear.clone() {
        if let Some(s) = clear.as_str() {
            sel_cmpt.clear = Some(Value::Array(parse_selector(s, "view")));
        }
    }
}

pub fn top_level_signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    if inputs::defined(sel_cmpt) {
        for proj in &sel_cmpt.project.items {
            let name = var_name(&format!("{}_{}", sel_cmpt.name, proj.field));
            let idx = signals
                .iter()
                .position(|n| n.get("name").and_then(Value::as_str) == Some(name.as_str()));
            if let Some(idx) = idx {
                if let Some(on) = signals[idx].get_mut("on").and_then(Value::as_array_mut) {
                    on.push(json!({"events": sel_cmpt.clear, "update": "null"}));
                }
            }
        }
    }
    signals
}

pub fn signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    fn add_clear(signals: &mut [Value], idx: Option<usize>, update: Value, events: &Value) {
        if let Some(idx) = idx {
            if let Some(on) = signals[idx].get_mut("on").and_then(Value::as_array_mut) {
                on.push(json!({"events": events, "update": update}));
            }
        }
    }

    if sel_cmpt.type_ == "interval" {
        for proj in &sel_cmpt.project.items {
            let visual = proj
                .signals
                .as_ref()
                .and_then(|s| s.visual.clone())
                .unwrap_or_default();
            let v_idx = signals
                .iter()
                .position(|n| n.get("name").and_then(Value::as_str) == Some(visual.as_str()));
            add_clear(
                &mut signals,
                v_idx,
                json!("[0, 0]"),
                sel_cmpt.clear.as_ref().unwrap(),
            );

            if v_idx.is_none() {
                let data = proj
                    .signals
                    .as_ref()
                    .and_then(|s| s.data.clone())
                    .unwrap_or_default();
                let d_idx = signals
                    .iter()
                    .position(|n| n.get("name").and_then(Value::as_str) == Some(data.as_str()));
                add_clear(
                    &mut signals,
                    d_idx,
                    json!("null"),
                    sel_cmpt.clear.as_ref().unwrap(),
                );
            }
        }
    } else {
        let tuple_name = format!("{}{}", sel_cmpt.name, TUPLE);
        let t_idx = signals
            .iter()
            .position(|n| n.get("name").and_then(Value::as_str) == Some(tuple_name.as_str()));
        add_clear(
            &mut signals,
            t_idx,
            json!("null"),
            sel_cmpt.clear.as_ref().unwrap(),
        );

        if toggle::defined(sel_cmpt) {
            let toggle_name = format!("{}{}", sel_cmpt.name, TOGGLE);
            let t_idx = signals
                .iter()
                .position(|n| n.get("name").and_then(Value::as_str) == Some(toggle_name.as_str()));
            add_clear(
                &mut signals,
                t_idx,
                json!("false"),
                sel_cmpt.clear.as_ref().unwrap(),
            );
        }
    }

    signals
}

#[allow(dead_code)]
fn _unused(_: Model) {}
