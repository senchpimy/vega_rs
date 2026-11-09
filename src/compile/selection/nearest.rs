//! Port of `src/compile/selection/nearest.ts`.

use serde_json::{json, Value};

use crate::compile::model::Model;
use crate::log;
use crate::mark::is_path_mark;

use super::component::{SelectionCompiler, SelectionComponent};
use super::message;

pub const VORONOI: &str = "voronoi";

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: Some(parse),
        signals: None,
        top_level_signals: None,
        modify_expr: None,
        marks: Some(marks),
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "point" && sel_cmpt.truthy_nearest()
}

pub fn parse(model: &mut Model, sel_cmpt: &mut SelectionComponent, _sel_def: &mut Value) {
    if let Some(events) = &mut sel_cmpt.events {
        let name = model.get_name(VORONOI);
        for s in events {
            if let Some(obj) = s.as_object_mut() {
                obj.insert("markname".to_string(), json!(name));
            }
        }
    }
}

pub fn marks(model: &Model, sel_cmpt: &SelectionComponent, mut marks: Vec<Value>) -> Vec<Value> {
    let x = sel_cmpt.project.channel("x");
    let y = sel_cmpt.project.channel("y");
    let mark_type = model.mark_type();
    if is_path_mark(mark_type.as_str()) {
        log::warn(message::nearest_not_support_for_continuous(&mark_type));
        return marks;
    }

    let mut cell_def = json!({
        "name": model.get_name(VORONOI),
        "type": "path",
        "interactive": true,
        "aria": false,
        "from": {"data": model.get_name("marks")},
        "encode": {
            "update": {
                "fill": {"value": "transparent"},
                "strokeWidth": {"value": 0.35},
                "stroke": {"value": "transparent"},
                "isVoronoi": {"value": true},
            },
        },
        "transform": [{
            "type": "voronoi",
            "x": {"expr": if x.is_some() || y.is_none() { "datum.datum.x || 0" } else { "0" }},
            "y": {"expr": if y.is_some() || x.is_none() { "datum.datum.y || 0" } else { "0" }},
            "size": [model.get_size_signal_ref("width"), model.get_size_signal_ref("height")],
        }],
    });

    let tooltip = crate::compile::mark::encode::tooltip::tooltip_with_options(model, true);
    if let Value::Object(entries) = tooltip {
        if let Some(update) = cell_def["encode"]["update"].as_object_mut() {
            for (k, v) in entries {
                update.insert(k, v);
            }
        }
    }

    let mut index = 0usize;
    let mut exists = false;
    let mark_name = model
        .component
        .mark
        .first()
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for (i, mark) in marks.iter().enumerate() {
        let name = mark.get("name").and_then(Value::as_str).unwrap_or("");
        if name == mark_name {
            index = i;
        } else if name.contains(VORONOI) {
            exists = true;
        }
    }

    if !exists {
        marks.insert(index + 1, cell_def);
    }

    marks
}
