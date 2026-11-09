//! Port of `src/compile/selection/translate.ts`.

use serde_json::{json, Value};

use crate::channel::X;
use crate::compile::model::Model;

use super::component::{SelectionCompiler, SelectionComponent, SelectionProjection};
use super::interval::BRUSH as INTERVAL_BRUSH;
use super::scales::{self, domain};
use super::selector::parse_selector;

const ANCHOR: &str = "_translate_anchor";
const DELTA: &str = "_translate_delta";

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
    sel_cmpt.type_ == "interval" && sel_cmpt.truthy_translate()
}

pub fn signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let bound_scales = scales::defined(sel_cmpt);
    let anchor = format!("{name}{ANCHOR}");
    let x = sel_cmpt.project.channel("x");
    let y = sel_cmpt.project.channel("y");
    let translate = sel_cmpt
        .translate
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut events = parse_selector(translate, "scope");

    if !bound_scales {
        for e in &mut events {
            if let Some(between) = e.get_mut("between").and_then(Value::as_array_mut) {
                if let Some(first) = between.first_mut() {
                    first["markname"] = json!(format!("{name}{INTERVAL_BRUSH}"));
                }
            }
        }
    }

    let start_events: Vec<Value> = events
        .iter()
        .filter_map(|e| e.get("between").and_then(|b| b.get(0)).cloned())
        .collect();

    let mut update = "{x: x(unit), y: y(unit)".to_string();
    if let Some(x) = x {
        update.push_str(&format!(
            ", extent_x: {}",
            if bound_scales {
                domain(model, "x")
            } else {
                format!(
                    "slice({})",
                    x.signals
                        .as_ref()
                        .and_then(|s| s.visual.clone())
                        .unwrap_or_default()
                )
            }
        ));
    }
    if let Some(y) = y {
        update.push_str(&format!(
            ", extent_y: {}",
            if bound_scales {
                domain(model, "y")
            } else {
                format!(
                    "slice({})",
                    y.signals
                        .as_ref()
                        .and_then(|s| s.visual.clone())
                        .unwrap_or_default()
                )
            }
        ));
    }
    update.push('}');

    signals.push(json!({
        "name": anchor,
        "value": {},
        "on": [{"events": start_events, "update": update}],
    }));
    signals.push(json!({
        "name": format!("{name}{DELTA}"),
        "value": {},
        "on": [{
            "events": events,
            "update": format!("{{x: {anchor}.x - x(unit), y: {anchor}.y - y(unit)}}"),
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
    let anchor = format!("{name}{ANCHOR}");
    let delta = format!("{name}{DELTA}");
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
    let reversed = scale_cmpt
        .as_ref()
        .and_then(|c| c.get("reverse"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let sign = if !bound_scales {
        ""
    } else if channel == X {
        if reversed {
            ""
        } else {
            "-"
        }
    } else if reversed {
        "-"
    } else {
        ""
    };
    let extent = format!("{anchor}.extent_{channel}");
    let offset = format!(
        "{sign}{delta}.{channel} / {}",
        if bound_scales {
            size_sg.clone()
        } else {
            format!("span({extent})")
        }
    );
    let pan_fn = if !bound_scales || scale_cmpt.is_none() {
        "panLinear"
    } else {
        match scale_type.as_deref() {
            Some("log") => "panLog",
            Some("symlog") => "panSymlog",
            Some("pow") => "panPow",
            _ => "panLinear",
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
    let update = format!("{pan_fn}({extent}, {offset}{arg})");

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
