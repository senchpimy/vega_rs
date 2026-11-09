//! Port of `src/compile/selection/interval.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{LATITUDE, LONGITUDE};
use crate::compile::model::{is_unit_model, Model};
use crate::log;
use crate::scale::has_continuous_domain;
use crate::selection::SELECTION_ID;
use crate::util::{array, keys, string_value};

use super::assemble::{assemble_init, assemble_init_with};
use super::component::{
    unit_name, SelectionCompiler, SelectionComponent, SelectionProjection, TUPLE,
};
use super::project::TUPLE_FIELDS;
use super::scales;

pub const BRUSH: &str = "_brush";
pub const SCALE_TRIGGER: &str = "_scale_trigger";
pub const GEO_INIT_TICK: &str = "geo_interval_init_tick";
const INIT: &str = "_init";
const CENTER: &str = "_center";

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: Some(parse),
        signals: Some(signals),
        top_level_signals: Some(top_level_signals),
        modify_expr: None,
        marks: Some(marks),
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "interval"
}

fn projection_name(model: &Model) -> String {
    model
        .projection_name(false)
        .unwrap_or_else(|| model.get_name("projection"))
}

pub fn parse(model: &mut Model, sel_cmpt: &mut SelectionComponent, sel_def: &mut Value) {
    if model.has_projection() {
        let mut def: Map<String, Value> =
            if sel_def.get("select").map(Value::is_object).unwrap_or(false) {
                sel_def
                    .get("select")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Map::new()
            };
        def.insert("fields".to_string(), json!([SELECTION_ID]));
        if def.get("encodings").is_none() {
            let encodings = match sel_def.get("value") {
                Some(value) if value.is_object() => keys(value),
                _ => vec![LONGITUDE.to_string(), LATITUDE.to_string()],
            };
            def.insert("encodings".to_string(), json!(encodings));
        }
        let mut new_def = Map::new();
        new_def.insert("type".to_string(), json!("interval"));
        for (k, v) in def {
            new_def.insert(k, v);
        }
        sel_def["select"] = Value::Object(new_def);
    }

    if sel_cmpt.truthy_translate() && !scales::defined(sel_cmpt) {
        let filter_expr = format!(
            "!event.item || event.item.mark.name !== {}",
            string_value(&json!(format!("{}{}", sel_cmpt.name, BRUSH)))
        );
        if let Some(events) = &mut sel_cmpt.events {
            for evt in events.iter_mut() {
                let Some(between) = evt.get_mut("between").and_then(Value::as_array_mut) else {
                    log::warn(format!(
                        "{} is not an ordered event stream for interval selections.",
                        crate::util::stringify(evt)
                    ));
                    continue;
                };
                let Some(first) = between.first_mut() else {
                    continue;
                };
                let mut filters: Vec<Value> = first.get("filter").map(array).unwrap_or_default();
                if !filters
                    .iter()
                    .any(|f| f.as_str() == Some(filter_expr.as_str()))
                {
                    filters.push(json!(filter_expr));
                    first["filter"] = Value::Array(filters);
                }
            }
        }
    }
}

pub fn signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let tuple_sg = format!("{name}{TUPLE}");
    let channels: Vec<SelectionProjection> = sel_cmpt
        .project
        .items
        .iter()
        .filter(|p| matches!(p.channel.as_deref(), Some("x") | Some("y")))
        .cloned()
        .collect();
    let init: Option<Value> = sel_cmpt
        .init
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .cloned();

    for proj in &channels {
        let init_value = init
            .as_ref()
            .and_then(|i| i.as_array())
            .and_then(|a| a.get(proj.index))
            .cloned()
            .unwrap_or(Value::Null);
        signals.extend(channel_signals(model, sel_cmpt, proj, &init_value));
    }

    if !model.has_projection() {
        // Proxy scale reactions to ensure that an infinite loop doesn't occur.
        if !scales::defined(sel_cmpt) {
            let trigger_sg = format!("{name}{SCALE_TRIGGER}");
            let mut scale_triggers: Vec<String> = Vec::new();
            for proj in &channels {
                let channel = proj.channel.clone().unwrap_or_default();
                let signals_part = proj.signals.clone().unwrap_or_default();
                let dname = signals_part.data.clone().unwrap_or_default();
                let vname = signals_part.visual.clone().unwrap_or_default();
                let scale_name = string_value(&json!(model
                    .scale_name(&channel, false)
                    .unwrap_or_default()));
                let scale_type = model
                    .get_scale_component(&channel)
                    .and_then(|c| c.get_type());
                let to_num = if scale_type.map(has_continuous_domain).unwrap_or(false) {
                    "+"
                } else {
                    ""
                };
                scale_triggers.push(format!(
                    "(!isArray({dname}) || ({to_num}invert({scale_name}, {vname})[0] === {to_num}{dname}[0] && {to_num}invert({scale_name}, {vname})[1] === {to_num}{dname}[1]))"
                ));
            }

            if !scale_triggers.is_empty() {
                let events: Vec<Value> = channels
                    .iter()
                    .map(|proj| {
                        json!({"scale": model.scale_name(&proj.channel.clone().unwrap_or_default(), false).unwrap_or_default()})
                    })
                    .collect();
                signals.push(json!({
                    "name": trigger_sg,
                    "value": {},
                    "on": [{
                        "events": events,
                        "update": format!("{} ? {trigger_sg} : {{}}", scale_triggers.join(" && ")),
                    }],
                }));
            }
        }

        let data_signals: Vec<String> = channels
            .iter()
            .map(|p| {
                p.signals
                    .as_ref()
                    .and_then(|s| s.data.clone())
                    .unwrap_or_default()
            })
            .collect();
        let update = format!(
            "unit: {}, fields: {}{}, values",
            unit_name(model, true),
            name,
            TUPLE_FIELDS
        );
        let mut tuple = Map::new();
        tuple.insert("name".to_string(), json!(tuple_sg));
        if let Some(init) = &init {
            tuple.insert(
                "init".to_string(),
                json!(format!(
                    "{{{update}: {}}}",
                    assemble_init(init, true).as_str().unwrap_or_default()
                )),
            );
        }
        if !data_signals.is_empty() {
            tuple.insert(
                "on".to_string(),
                json!([{
                    "events": [{"signal": data_signals.join(" || ")}],
                    "update": format!(
                        "{} ? {{{update}: [{}]}} : null",
                        data_signals.join(" && "),
                        data_signals.join(",")
                    ),
                }]),
            );
        }
        signals.push(Value::Object(tuple));
        signals
    } else {
        let projection = string_value(&json!(projection_name(model)));
        let center_sg = format!("{}{}", projection_name(model), CENTER);
        let x = sel_cmpt.project.channel("x").cloned();
        let y = sel_cmpt.project.channel("y").cloned();
        let xvname = x
            .as_ref()
            .and_then(|p| p.signals.as_ref().and_then(|s| s.visual.clone()));
        let yvname = y
            .as_ref()
            .and_then(|p| p.signals.as_ref().and_then(|s| s.visual.clone()));
        let xinit = match &x {
            Some(p) => init
                .as_ref()
                .and_then(|i| i.as_array())
                .and_then(|a| a.get(p.index))
                .cloned()
                .unwrap_or(Value::Null),
            None => json!(format!("{center_sg}[0]")),
        };
        let yinit = match &y {
            Some(p) => init
                .as_ref()
                .and_then(|i| i.as_array())
                .and_then(|a| a.get(p.index))
                .cloned()
                .unwrap_or(Value::Null),
            None => json!(format!("{center_sg}[1]")),
        };
        let size_sg = |layout: &str| {
            model
                .get_size_signal_ref(layout)
                .get("signal")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let value_0 = |v: &Value| -> String {
            v.as_array()
                .and_then(|a| a.first())
                .map(crate::util::stringify)
                .unwrap_or_else(|| v.as_str().unwrap_or_default().to_string())
        };
        let value_1 = |v: &Value| -> String {
            v.as_array()
                .and_then(|a| a.get(1))
                .map(crate::util::stringify)
                .unwrap_or_else(|| v.as_str().unwrap_or_default().to_string())
        };
        let x0 = value_0(&xinit);
        let x1 = value_1(&xinit);
        let y0 = value_0(&yinit);
        let y1 = value_1(&yinit);
        let bbox = format!(
            "[[{}, {}],[{}, {}]]",
            xvname
                .as_ref()
                .map(|v| format!("{v}[0]"))
                .unwrap_or_else(|| "0".to_string()),
            yvname
                .as_ref()
                .map(|v| format!("{v}[0]"))
                .unwrap_or_else(|| "0".to_string()),
            xvname
                .as_ref()
                .map(|v| format!("{v}[1]"))
                .unwrap_or_else(|| size_sg("width")),
            yvname
                .as_ref()
                .map(|v| format!("{v}[1]"))
                .unwrap_or_else(|| size_sg("height")),
        );

        if init.is_some() {
            signals.insert(
                0,
                json!({
                    "name": format!("{name}{INIT}"),
                    "init": format!("[scale({projection}, [{x0}, {y0}]), scale({projection}, [{x1}, {y1}])]"),
                }),
            );

            if x.is_none() || y.is_none() {
                let has_center = signals
                    .iter()
                    .any(|s| s.get("name").and_then(Value::as_str) == Some(center_sg.as_str()));
                if !has_center {
                    signals.insert(
                        0,
                        json!({
                            "name": center_sg,
                            "update": format!("invert({projection}, [{}/2, {}/2])", size_sg("width"), size_sg("height")),
                        }),
                    );
                }
            }
        }

        let intersect = format!(
            "intersect({bbox}, {{markname: {}}}, unit.mark)",
            string_value(&json!(model.get_name("marks")))
        );
        let base = format!("{{unit: {}}}", unit_name(model, true));
        let update = format!("vlSelectionTuples({intersect}, {base})");
        let visual_signals: Vec<String> = channels
            .iter()
            .map(|p| {
                p.signals
                    .as_ref()
                    .and_then(|s| s.visual.clone())
                    .unwrap_or_default()
            })
            .collect();

        let mut events: Vec<Value> = Vec::new();
        if !visual_signals.is_empty() {
            events.push(json!({"signal": visual_signals.join(" || ")}));
        }
        if init.is_some() {
            events.push(json!({"signal": GEO_INIT_TICK}));
        }

        signals.push(json!({
            "name": tuple_sg,
            "on": [{"events": events, "update": update}],
        }));
        signals
    }
}

pub fn top_level_signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    if is_unit_model(model) && model.has_projection() && sel_cmpt.init.is_some() {
        let has_tick = signals
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some(GEO_INIT_TICK));
        if !has_tick {
            signals.insert(
                0,
                json!({
                    "name": GEO_INIT_TICK,
                    "value": null,
                    "on": [{
                        "events": "timer{1}",
                        "update": format!("{GEO_INIT_TICK} === null ? {{}} : {GEO_INIT_TICK}"),
                    }],
                }),
            );
        }
    }
    signals
}

pub fn marks(model: &Model, sel_cmpt: &SelectionComponent, marks: Vec<Value>) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let x = sel_cmpt.project.channel("x");
    let y = sel_cmpt.project.channel("y");
    let xvname = x
        .as_ref()
        .and_then(|p| p.signals.as_ref().and_then(|s| s.visual.clone()))
        .unwrap_or_default();
    let yvname = y
        .as_ref()
        .and_then(|p| p.signals.as_ref().and_then(|s| s.visual.clone()))
        .unwrap_or_default();
    let store = format!(
        "data({})",
        string_value(&json!(format!(
            "{}{}",
            sel_cmpt.name,
            super::component::STORE
        )))
    );

    if scales::defined(sel_cmpt) || (x.is_none() && y.is_none()) {
        return marks;
    }

    let mut update = Map::new();
    update.insert(
        "x".to_string(),
        if x.is_some() {
            json!({"signal": format!("{xvname}[0]")})
        } else {
            json!({"value": 0})
        },
    );
    update.insert(
        "y".to_string(),
        if y.is_some() {
            json!({"signal": format!("{yvname}[0]")})
        } else {
            json!({"value": 0})
        },
    );
    update.insert(
        "x2".to_string(),
        if x.is_some() {
            json!({"signal": format!("{xvname}[1]")})
        } else {
            json!({"field": {"group": "width"}})
        },
    );
    update.insert(
        "y2".to_string(),
        if y.is_some() {
            json!({"signal": format!("{yvname}[1]")})
        } else {
            json!({"field": {"group": "height"}})
        },
    );

    if sel_cmpt.resolve == "global" {
        let unit = unit_name(model, true);
        for key in keys(&Value::Object(update.clone())) {
            let existing = update.get(&key).cloned().unwrap_or(Value::Null);
            let mut rule = existing.as_object().cloned().unwrap_or_default();
            rule.insert(
                "test".to_string(),
                json!(format!("{store}.length && {store}[0].unit === {unit}")),
            );
            update.insert(key, json!([Value::Object(rule), {"value": 0}]));
        }
    }

    let mark = sel_cmpt.mark.clone().unwrap_or_else(|| json!({}));
    let mut mark_obj = mark.as_object().cloned().unwrap_or_default();
    let fill = mark_obj.remove("fill").unwrap_or(Value::Null);
    let fill_opacity = mark_obj.remove("fillOpacity").unwrap_or(Value::Null);
    let cursor = mark_obj.remove("cursor").unwrap_or(Value::Null);
    let stroke = mark_obj;

    let test = [
        x.is_some().then(|| format!("{xvname}[0] !== {xvname}[1]")),
        y.is_some().then(|| format!("{yvname}[0] !== {yvname}[1]")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" && ");

    let mut vg_stroke = Map::new();
    for (k, v) in &stroke {
        vg_stroke.insert(
            k.clone(),
            json!([{"test": test, "value": v}, {"value": null}]),
        );
    }

    let vg_cursor = if cursor.is_null() {
        if sel_cmpt.truthy_translate() {
            json!("move")
        } else {
            Value::Null
        }
    } else {
        cursor
    };

    let mut bg_enter = Map::new();
    bg_enter.insert("fill".to_string(), json!({"value": fill}));
    bg_enter.insert("fillOpacity".to_string(), json!({"value": fill_opacity}));

    let mut brush_update = update.clone();
    for (k, v) in vg_stroke {
        brush_update.insert(k, v);
    }

    let mut brush_enter = Map::new();
    if !vg_cursor.is_null() {
        brush_enter.insert("cursor".to_string(), json!({"value": vg_cursor}));
    }
    brush_enter.insert("fill".to_string(), json!({"value": "transparent"}));

    let mut out = vec![json!({
        "name": format!("{name}{BRUSH}_bg"),
        "type": "rect",
        "clip": true,
        "encode": {
            "enter": Value::Object(bg_enter),
            "update": Value::Object(update),
        },
    })];
    out.extend(marks);
    out.push(json!({
        "name": format!("{name}{BRUSH}"),
        "type": "rect",
        "clip": true,
        "encode": {
            "enter": Value::Object(brush_enter),
            "update": Value::Object(brush_update),
        },
    }));
    out
}

/// Returns the visual and data signals for an interval selection.
fn channel_signals(
    model: &Model,
    sel_cmpt: &SelectionComponent,
    proj: &SelectionProjection,
    init: &Value,
) -> Vec<Value> {
    let scaled_interval = !model.has_projection();
    let channel = proj.channel.clone().unwrap_or_default();
    let vname = proj
        .signals
        .as_ref()
        .and_then(|s| s.visual.clone())
        .unwrap_or_default();

    let scale_name = string_value(&json!(if scaled_interval {
        model.scale_name(&channel, false).unwrap_or_default()
    } else {
        projection_name(model)
    }));
    let scaled = |s: &str| format!("scale({scale_name}, {s})");

    let size = model
        .get_size_signal_ref(if channel == "x" { "width" } else { "height" })
        .get("signal")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let coord = format!("{channel}(unit)");
    let mut von: Vec<Value> = Vec::new();
    for evt in sel_cmpt.events.clone().unwrap_or_default() {
        let between = evt.get("between").cloned().unwrap_or(Value::Null);
        let first = between
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        von.push(json!({"events": first, "update": format!("[{coord}, {coord}]")}));
        von.push(json!({
            "events": evt,
            "update": format!("[{vname}[0], clamp({coord}, 0, {size})]"),
        }));
    }

    if scaled_interval {
        let dname = proj
            .signals
            .as_ref()
            .and_then(|s| s.data.clone())
            .unwrap_or_default();
        let has_scales = scales::defined(sel_cmpt);
        let scale_type = model
            .get_scale_component(&channel)
            .and_then(|c| c.get_type());
        let continuous = scale_type.map(has_continuous_domain).unwrap_or(false);

        von.push(json!({
            "events": {"signal": format!("{}{}", sel_cmpt.name, SCALE_TRIGGER)},
            "update": if continuous {
                format!("[{}, {}]", scaled(&format!("{dname}[0]")), scaled(&format!("{dname}[1]")))
            } else {
                "[0, 0]".to_string()
            },
        }));

        if has_scales {
            vec![json!({"name": dname, "on": []})]
        } else {
            let mut visual = Map::new();
            visual.insert("name".to_string(), json!(vname));
            if init.is_array() {
                let wrapped = assemble_init_with(
                    init,
                    true,
                    Some(&|s: &str| format!("scale({scale_name}, {s})")),
                );
                visual.insert("init".to_string(), wrapped);
            } else {
                visual.insert("value".to_string(), json!([]));
            }
            visual.insert("on".to_string(), Value::Array(von));

            let mut data = Map::new();
            data.insert("name".to_string(), json!(dname));
            if !init.is_null() {
                data.insert("init".to_string(), assemble_init(init, true));
            }
            data.insert(
                "on".to_string(),
                json!([{
                    "events": {"signal": vname},
                    "update": format!("{vname}[0] === {vname}[1] ? null : invert({scale_name}, {vname})"),
                }]),
            );

            vec![Value::Object(visual), Value::Object(data)]
        }
    } else {
        let init_idx = if channel == "x" { 0 } else { 1 };
        let init_sg = format!("{}{}", sel_cmpt.name, INIT);
        let mut visual = Map::new();
        visual.insert("name".to_string(), json!(vname));
        if !init.is_null() {
            visual.insert(
                "init".to_string(),
                json!(format!(
                    "[{init_sg}[0][{init_idx}], {init_sg}[1][{init_idx}]]"
                )),
            );
        } else {
            visual.insert("value".to_string(), json!([]));
        }
        visual.insert("on".to_string(), Value::Array(von));
        vec![Value::Object(visual)]
    }
}

#[allow(dead_code)]
fn _unused() {}
