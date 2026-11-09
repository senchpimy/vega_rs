//! Port of `src/compile/selection/zoom.ts`.

use serde_json::{json, Value};

use crate::channel::X;
use crate::compile::model::Model;
use crate::util::string_value;

use super::component::{SelectionCompiler, SelectionComponent, SelectionProjection};
use super::interval::BRUSH as INTERVAL_BRUSH;
use super::scales::{self, domain};
use super::selector::parse_selector;

const ANCHOR: &str = "_zoom_anchor";
const DELTA: &str = "_zoom_delta";

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: None,
        signals: Some(signals),
        top_level_signals: None,
        modify_expr: None,
        marks: None,
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "interval" && sel_cmpt.truthy_zoom()
}

pub fn signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let bound_scales = scales::defined(sel_cmpt);
    let delta = format!("{name}{DELTA}");
    let _x = sel_cmpt.project.channel("x");
    let _y = sel_cmpt.project.channel("y");
    let sx = string_value(&json!(model.scale_name("x", false).unwrap_or_default()));
    let sy = string_value(&json!(model.scale_name("y", false).unwrap_or_default()));
    let zoom = sel_cmpt
        .zoom
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut events = parse_selector(zoom, "scope");

    if !bound_scales {
        for e in &mut events {
            if let Some(obj) = e.as_object_mut() {
                obj.insert(
                    "markname".to_string(),
                    json!(format!("{name}{INTERVAL_BRUSH}")),
                );
            }
        }
    }

    let anchor_update = if !bound_scales {
        "{x: x(unit), y: y(unit)}".to_string()
    } else {
        let mut parts: Vec<String> = Vec::new();
        if !sx.is_empty() && sx != "\"\"" {
            parts.push(format!("x: invert({sx}, x(unit))"));
        }
        if !sy.is_empty() && sy != "\"\"" {
            parts.push(format!("y: invert({sy}, y(unit))"));
        }
        format!("{{{}}}", parts.join(", "))
    };

    signals.push(json!({
        "name": format!("{name}{ANCHOR}"),
        "on": [{"events": events, "update": anchor_update}],
    }));
    signals.push(json!({
        "name": delta,
        "on": [{
            "events": events,
            "force": true,
            "update": "pow(1.001, event.deltaY * pow(16, event.deltaMode))",
        }],
    }));

    if let Some(x) = sel_cmpt.project.channel("x") {
        on_delta(model, sel_cmpt, x, "width", &mut signals);
    }
    if let Some(y) = sel_cmpt.project.channel("y") {
        on_delta(model, sel_cmpt, y, "height", &mut signals);
    }

    signals
}

fn on_delta(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    proj: &SelectionProjection,
    size: &str,
    signals: &mut [Value],
) {
    let name = sel_cmpt.name.clone();
    let channel = proj.channel.clone().unwrap_or_default();
    let bound_scales = scales::defined(sel_cmpt);
    let signal_name = proj
        .signals
        .as_ref()
        .and_then(|s| s.get(if bound_scales { "data" } else { "visual" }))
        .cloned()
        .unwrap_or_default();
    let size_sg = model
        .get_size_signal_ref(size)
        .get("signal")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let scale_cmpt = model.get_scale_component(&channel);
    let scale_type = scale_cmpt
        .as_ref()
        .and_then(|c| c.get_type())
        .map(|t| format!("{t}"));
    let base = if bound_scales {
        domain(model, &channel)
    } else {
        signal_name.clone()
    };
    let delta = format!("{name}{DELTA}");
    let anchor = format!("{name}{ANCHOR}.{channel}");
    let zoom_fn = if !bound_scales || scale_cmpt.is_none() {
        "zoomLinear"
    } else {
        match scale_type.as_deref() {
            Some("log") => "zoomLog",
            Some("symlog") => "zoomSymlog",
            Some("pow") => "zoomPow",
            _ => "zoomLinear",
        }
    };
    let arg = if !bound_scales {
        String::new()
    } else {
        match scale_type.as_deref() {
            Some("pow") => format!(
                ", {}",
                scale_cmpt
                    .as_ref()
                    .and_then(|c| c.get("exponent"))
                    .unwrap_or(json!(1))
            ),
            Some("symlog") => format!(
                ", {}",
                scale_cmpt
                    .as_ref()
                    .and_then(|c| c.get("constant"))
                    .unwrap_or(json!(1))
            ),
            _ => String::new(),
        }
    };
    let update = format!("{zoom_fn}({base}, {anchor}, {delta}{arg})");

    if let Some(idx) = signals
        .iter()
        .position(|s| s.get("name").and_then(Value::as_str) == Some(signal_name.as_str()))
    {
        if let Some(on) = signals[idx].get_mut("on").and_then(Value::as_array_mut) {
            on.push(json!({
                "events": {"signal": delta},
                "update": if bound_scales {
                    update
                } else {
                    format!("clampRange({update}, 0, {size_sg})")
                },
            }));
        }
    }
}

#[allow(dead_code)]
fn _unused(_: &str) -> &str {
    X
}
