//! Port of `src/compile/selection/legends.ts`.

use serde_json::{json, Map, Value};

use crate::compile::model::Model;
use crate::log;
use crate::selection::{is_legend_binding, is_legend_stream_binding, SELECTION_ID};
use crate::util::{array, duplicate, var_name};

use super::component::{disable_direct_manipulation, SelectionCompiler, SelectionComponent, TUPLE};
use super::message;
use super::project::TUPLE_FIELDS;
use super::selector::parse_selector;
use super::toggle::TOGGLE;

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
    let spec = sel_cmpt.resolve == "global"
        && sel_cmpt
            .bind
            .as_ref()
            .map(is_legend_binding)
            .unwrap_or(false);
    let proj_len =
        sel_cmpt.project.items.len() == 1 && sel_cmpt.project.items[0].field != SELECTION_ID;
    if spec && !proj_len {
        log::warn(message::LEGEND_BINDINGS_MUST_HAVE_PROJECTION);
    }
    spec && proj_len
}

pub fn parse(_model: &mut Model, sel_cmpt: &mut SelectionComponent, sel_def: &mut Value) {
    // Allow legend items to be toggleable by default even though direct
    // manipulation is disabled.
    let mut sel_def_ = sel_def.clone();
    let select = sel_def_.get("select").cloned().unwrap_or(Value::Null);
    let toggle = sel_cmpt.toggle.clone().unwrap_or(Value::Null);
    let new_select = if select.is_string() {
        json!({"type": select, "toggle": toggle})
    } else {
        let mut obj = select.as_object().cloned().unwrap_or_default();
        obj.insert("toggle".to_string(), toggle);
        Value::Object(obj)
    };
    sel_def_["select"] = new_select;
    disable_direct_manipulation(sel_cmpt, &sel_def_);

    let select_obj = sel_def.get("select");
    if select_def_is_object(select_obj)
        && (truthy_key(select_obj, "on") || truthy_key(select_obj, "clear"))
    {
        let legend_filter = "event.item && indexof(event.item.mark.role, \"legend\") < 0";
        if let Some(events) = &mut sel_cmpt.events {
            for evt in events {
                let mut filters: Vec<Value> = evt.get("filter").map(array).unwrap_or_default();
                let has = filters.iter().any(|f| f.as_str() == Some(legend_filter));
                if !has {
                    filters.push(json!(legend_filter));
                    evt["filter"] = Value::Array(filters);
                }
            }
        }
    }

    let bind = sel_cmpt.bind.clone().unwrap_or(Value::Null);
    let evt = if is_legend_stream_binding(&bind) {
        bind.get("legend").cloned().unwrap_or(Value::Null)
    } else {
        json!("click")
    };
    let stream: Vec<Value> = if let Some(s) = evt.as_str() {
        parse_selector(s, "view")
    } else {
        array(&evt)
    };
    sel_cmpt.bind = Some(json!({"legend": {"merge": stream}}));
}

fn select_def_is_object(v: Option<&Value>) -> bool {
    v.map(|x| x.is_object()).unwrap_or(false)
}

fn truthy_key(v: Option<&Value>, key: &str) -> bool {
    v.and_then(|x| x.get(key))
        .map(|x| match x {
            Value::Null | Value::Bool(false) => false,
            Value::String(s) => !s.is_empty(),
            _ => true,
        })
        .unwrap_or(false)
}

fn mark_name(name: &str, s: &Value) -> Value {
    let mut ds: Map<String, Value> = s.as_object().cloned().unwrap_or_default();
    ds.insert("markname".to_string(), json!(name));
    Value::Object(ds)
}

pub fn top_level_signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    let sel_name = sel_cmpt.name.clone();
    let bind = sel_cmpt.bind.clone().unwrap_or(Value::Null);
    let stream = if is_legend_stream_binding(&bind) {
        bind.get("legend")
            .and_then(|l| l.get("merge"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    for proj in &sel_cmpt.project.items {
        if !proj.has_legend {
            continue;
        }
        let prefix = format!("{}_legend", var_name(&proj.field));
        let sg_name = format!("{sel_name}_{prefix}");
        let has_signal = signals
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some(sg_name.as_str()));

        if !has_signal {
            let mut events: Vec<Value> = stream
                .iter()
                .map(|m| mark_name(&format!("{prefix}_symbols"), m))
                .collect();
            events.extend(
                stream
                    .iter()
                    .map(|m| mark_name(&format!("{prefix}_labels"), m)),
            );
            events.extend(
                stream
                    .iter()
                    .map(|m| mark_name(&format!("{prefix}_entries"), m)),
            );

            let mut signal = json!({"name": sg_name});
            if sel_cmpt.init.is_none() {
                signal["value"] = Value::Null;
            }
            signal["on"] = json!([
                {
                    "events": events,
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": stream,
                    "update": format!("!event.item || !datum ? null : {sg_name}"),
                    "force": true,
                },
            ]);
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
    let tuple_name = format!("{}{}", name, TUPLE);
    let fields = format!("{}{}", name, TUPLE_FIELDS);
    let values: Vec<String> = sel_cmpt
        .project
        .items
        .iter()
        .filter(|p| p.has_legend)
        .map(|p| var_name(&format!("{}_{}_legend", name, var_name(&p.field))))
        .collect();
    let valid: Vec<String> = values.iter().map(|v| format!("{v} !== null")).collect();
    let update = format!(
        "{} ? {{fields: {}, values: [{}]}} : null",
        valid.join(" && "),
        fields,
        values.join(", ")
    );

    let tuple_idx = signals
        .iter()
        .position(|s| s.get("name").and_then(Value::as_str) == Some(tuple_name.as_str()));

    if let Some(idx) = tuple_idx {
        if sel_cmpt.events.is_some() && !values.is_empty() {
            let events: Vec<Value> = values.iter().map(|v| json!({"signal": v})).collect();
            if let Some(on) = signals[idx].get_mut("on").and_then(Value::as_array_mut) {
                on.push(json!({"events": events, "update": update}));
            }
        } else if !values.is_empty() {
            signals[idx]["update"] = json!(update);
            if let Some(obj) = signals[idx].as_object_mut() {
                obj.remove("value");
                obj.remove("on");
            }
        }
    }

    let toggle_name = format!("{}{}", name, TOGGLE);
    let toggle_idx = signals
        .iter()
        .position(|s| s.get("name").and_then(Value::as_str) == Some(toggle_name.as_str()));
    let bind = sel_cmpt.bind.clone().unwrap_or(Value::Null);
    let events = if is_legend_stream_binding(&bind) {
        bind.get("legend").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    if let Some(idx) = toggle_idx {
        if sel_cmpt.events.is_none() {
            if let Some(first) = signals[idx]
                .get_mut("on")
                .and_then(Value::as_array_mut)
                .and_then(|a| a.first_mut())
            {
                first["events"] = events;
            }
        } else {
            let first = signals[idx]
                .get("on")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or(Value::Null);
            let mut copy = first.as_object().cloned().unwrap_or_default();
            copy.insert("events".to_string(), events);
            if let Some(on) = signals[idx].get_mut("on").and_then(Value::as_array_mut) {
                on.push(Value::Object(copy));
            }
        }
    }

    signals
}

/// Port of `parseInteractiveLegend`.
///
/// The legend component has not been ported yet; a JSON placeholder with a
/// `selections` array is used instead.
pub fn parse_interactive_legend(
    model: &mut Model,
    channel: &str,
    legend_cmpt: &mut crate::compile::legend::component::LegendComponent,
) {
    let field = model
        .field_def(channel)
        .and_then(|f| f.get("field").and_then(Value::as_str).map(str::to_string));
    let mut components = super::component::selection_components(model);
    let mut changed = false;
    for sel_cmpt in components.values_mut() {
        let proj = field
            .as_deref()
            .and_then(|f| sel_cmpt.project.field(f).cloned())
            .or_else(|| sel_cmpt.project.channel(channel).cloned());
        let Some(proj) = proj else { continue };
        if defined(sel_cmpt) && sel_cmpt.project.items.len() == 1 {
            let mut selections = legend_cmpt
                .get("selections")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            selections.push(json!(sel_cmpt.name));
            legend_cmpt.set("selections", Value::Array(selections), false);
            let field_name = proj.field.clone();
            if let Some(p) = sel_cmpt.project.field_mut(&field_name) {
                p.has_legend = true;
            }
            changed = true;
        }
    }
    if changed {
        super::component::set_selection_components(model, &components);
    }
}

#[allow(dead_code)]
fn _unused(_: &SelectionComponent) {
    let _ = duplicate(&Value::Null);
}
