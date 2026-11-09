//! Port of `src/compile/selection/toggle.ts`.

use serde_json::{json, Value};

use crate::compile::model::Model;

use super::component::{is_timer_selection, unit_name, SelectionCompiler, SelectionComponent};

pub const TOGGLE: &str = "_toggle";

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: None,
        signals: Some(signals),
        top_level_signals: None,
        modify_expr: Some(modify_expr),
        marks: None,
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "point" && !is_timer_selection(sel_cmpt) && sel_cmpt.truthy_toggle()
}

pub fn signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    signals.push(json!({
        "name": format!("{}{}", sel_cmpt.name, TOGGLE),
        "value": false,
        "on": [{"events": sel_cmpt.events, "update": sel_cmpt.toggle}],
    }));
    signals
}

pub fn modify_expr(model: &Model, sel_cmpt: &SelectionComponent, _expr: &str) -> String {
    let tpl = format!("{}{}", sel_cmpt.name, super::component::TUPLE);
    let signal = format!("{}{}", sel_cmpt.name, TOGGLE);

    let middle = if sel_cmpt.resolve == "global" {
        format!("{signal} ? null : true, ")
    } else {
        format!("{signal} ? null : {{unit: {}}}, ", unit_name(model, true))
    };

    format!("{signal} ? null : {tpl}, {middle}{signal} ? {tpl} : null")
}
