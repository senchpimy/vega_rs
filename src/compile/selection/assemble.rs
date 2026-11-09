//! Port of `src/compile/selection/assemble.ts`.

use serde_json::{json, Map, Value};

use crate::compile::model::{is_unit_model, Model, SpecType};
use crate::datetime::{date_time_to_expr, date_time_to_timestamp, is_date_time};
use crate::scale::has_continuous_domain;
use crate::selection::SELECTION_ID;
use crate::util::{replace_path_in_field, string_value, stringify};

use super::component::{
    is_timer_selection, selection_compilers, unit_name, MODIFY, STORE, TUPLE, VL_SELECTION_RESOLVE,
};
use super::parse::parse_selection_extent;
use super::selector::parse_selector;

pub const CURR: &str = "_curr";

/// Port of `assembleProjection`.
pub fn assemble_projection(proj: &super::component::SelectionProjection) -> Value {
    let mut out = Map::new();
    out.insert("type".to_string(), json!(proj.type_));
    out.insert(
        "field".to_string(),
        json!(replace_path_in_field(&proj.field)),
    );
    if let Some(channel) = &proj.channel {
        out.insert("channel".to_string(), json!(channel));
    }
    if let Some(geo) = &proj.geo_channel {
        out.insert("geoChannel".to_string(), json!(geo));
    }
    Value::Object(out)
}

/// Port of `assembleInit`.
pub fn assemble_init(init: &Value, is_expr: bool) -> Value {
    assemble_init_with(init, is_expr, None)
}

/// Port of `assembleInit` with a custom `wrap` function.
pub fn assemble_init_with(
    init: &Value,
    is_expr: bool,
    wrap: Option<&dyn Fn(&str) -> String>,
) -> Value {
    if let Value::Array(items) = init {
        let assembled: Vec<Value> = items
            .iter()
            .map(|v| assemble_init_with(v, is_expr, wrap))
            .collect();
        if is_expr {
            let parts: Vec<String> = assembled
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect();
            return json!(format!("[{}]", parts.join(", ")));
        }
        return Value::Array(assembled);
    } else if is_date_time(init) {
        if is_expr {
            let expr = date_time_to_expr(init.clone());
            return match wrap {
                Some(w) => json!(w(&expr)),
                None => json!(expr),
            };
        } else {
            return json!(date_time_to_timestamp(init.clone()));
        }
    }

    if is_expr {
        let s = stringify(init);
        match wrap {
            Some(w) => json!(w(&s)),
            None => json!(s),
        }
    } else {
        init.clone()
    }
}

/// Port of `assembleUnitSelectionSignals`.
pub fn assemble_unit_selection_signals(model: &Model, mut signals: Vec<Value>) -> Vec<Value> {
    for sel_cmpt in super::component::ordered_components(model).iter() {
        let name = sel_cmpt.name.clone();
        let mut modify_expr = format!(
            "{}{}, {}",
            name,
            TUPLE,
            if sel_cmpt.resolve == "global" {
                "true".to_string()
            } else {
                format!("{{unit: {}}}", unit_name(model, true))
            }
        );

        for c in selection_compilers() {
            if !(c.defined)(sel_cmpt) {
                continue;
            }
            if let Some(f) = c.signals {
                signals = f(model, sel_cmpt, signals);
            }
            if let Some(f) = c.modify_expr {
                modify_expr = f(model, sel_cmpt, &modify_expr);
            }
        }

        let modify = format!(
            "modify({}, {})",
            string_value(&json!(format!("{}{}", sel_cmpt.name, STORE))),
            modify_expr
        );

        if is_timer_selection(sel_cmpt) {
            signals.push(json!({"name": format!("{}{}", name, MODIFY), "update": modify}));
        } else {
            signals.push(json!({
                "name": format!("{}{}", name, MODIFY),
                "on": [{"events": {"signal": format!("{}{}", sel_cmpt.name, TUPLE)}, "update": modify}],
            }));
        }
    }

    cleanup_empty_on_array(signals)
}

/// Port of `assembleFacetSignals`.
pub fn assemble_facet_signals(model: &Model, mut signals: Vec<Value>) -> Vec<Value> {
    let has_selection = model
        .component
        .selection
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false);
    if has_selection {
        let name = string_value(&json!(model.get_name("cell")));
        signals.insert(
            0,
            json!({
                "name": "facet",
                "value": {},
                "on": [{
                    "events": parse_selector("pointermove", "scope"),
                    "update": format!("isTuple(facet) ? facet : group({name}).datum"),
                }],
            }),
        );
    }
    cleanup_empty_on_array(signals)
}

/// Port of `assembleTopLevelSignals`.
pub fn assemble_top_level_signals(model: &Model, mut signals: Vec<Value>) -> Vec<Value> {
    let mut has_selections = false;
    for sel_cmpt in super::component::ordered_components(model).iter() {
        let name = sel_cmpt.name.clone();
        let store = string_value(&json!(format!("{}{}", name, STORE)));
        let has_sg = signals
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some(name.as_str()));
        if !has_sg {
            let resolve = if sel_cmpt.resolve == "global" {
                "union".to_string()
            } else {
                sel_cmpt.resolve.clone()
            };
            let is_point = if sel_cmpt.type_ == "point" {
                ", true, true)"
            } else {
                ")"
            };
            signals.push(json!({
                "name": sel_cmpt.name,
                "update": format!(
                    "{}({}, {}{}",
                    VL_SELECTION_RESOLVE,
                    store,
                    string_value(&json!(resolve)),
                    is_point
                ),
            }));
        }
        has_selections = true;

        for c in selection_compilers() {
            if (c.defined)(sel_cmpt) {
                if let Some(f) = c.top_level_signals {
                    signals = f(model, sel_cmpt, signals);
                }
            }
        }
    }

    if has_selections {
        let has_unit = signals
            .iter()
            .any(|s| s.get("name").and_then(Value::as_str) == Some("unit"));
        if !has_unit {
            signals.insert(
                0,
                json!({
                    "name": "unit",
                    "value": {},
                    "on": [{"events": "pointermove", "update": "isTuple(group()) ? group() : unit"}],
                }),
            );
        }
    }

    cleanup_empty_on_array(signals)
}

/// Port of `assembleUnitSelectionData`.
pub fn assemble_unit_selection_data(model: &Model, data: Vec<Value>) -> Vec<Value> {
    let mut data = data;
    let mut selection_data: Vec<Value> = Vec::new();
    let mut animation_data: Vec<Value> = Vec::new();
    let unit = unit_name(model, false);

    for sel_cmpt in super::component::ordered_components(model).iter() {
        let mut store = Map::new();
        store.insert(
            "name".to_string(),
            json!(format!("{}{}", sel_cmpt.name, STORE)),
        );

        if sel_cmpt.project.has_selection_id {
            store.insert(
                "transform".to_string(),
                json!([{"type": "collect", "sort": {"field": SELECTION_ID}}]),
            );
        }

        if let Some(init) = &sel_cmpt.init {
            let fields: Vec<Value> = sel_cmpt
                .project
                .items
                .iter()
                .map(assemble_projection)
                .collect();
            let values: Vec<Value> = init
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|i| {
                    if sel_cmpt.project.has_selection_id {
                        let assembled = assemble_init(i, false);
                        let first = assembled
                            .as_array()
                            .and_then(|a| a.first())
                            .cloned()
                            .unwrap_or(Value::Null);
                        {
                            let mut m = Map::new();
                            m.insert("unit".to_string(), json!(unit));
                            m.insert(SELECTION_ID.to_string(), first);
                            Value::Object(m)
                        }
                    } else {
                        json!({
                            "unit": unit,
                            "fields": fields,
                            "values": assemble_init(i, false),
                        })
                    }
                })
                .collect();
            store.insert("values".to_string(), Value::Array(values));
        }

        let store_name = format!("{}{}", sel_cmpt.name, STORE);
        let contains = selection_data
            .iter()
            .chain(data.iter())
            .any(|d| d.get("name").and_then(Value::as_str) == Some(store_name.as_str()));
        if !contains {
            selection_data.push(Value::Object(store));
        }

        if is_timer_selection(sel_cmpt) && !data.is_empty() {
            let source_name = model.lookup_data_source(&model.get_data_name("main"));
            // The dataflow/source naming pipeline is not part of this port; fall
            // back to the first dataset when the main source cannot be resolved.
            let source_data = data
                .iter()
                .find(|d| d.get("name").and_then(Value::as_str) == Some(source_name.as_str()))
                .or_else(|| {
                    if source_name.is_empty()
                        || !data.iter().any(|d| {
                            d.get("name").and_then(Value::as_str) == Some(source_name.as_str())
                        })
                    {
                        data.first()
                    } else {
                        None
                    }
                });
            if let Some(source_data) = source_data {
                let transforms = source_data
                    .get("transform")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let source_filter = transforms.iter().position(|t| {
                    t.get("type").and_then(Value::as_str) == Some("filter")
                        && t.get("expr")
                            .and_then(Value::as_str)
                            .map(|e| e.contains("vlSelectionTest"))
                            .unwrap_or(false)
                });

                if let Some(index) = source_filter {
                    let source = source_data.get("name").cloned().unwrap_or(Value::Null);
                    let source_name = source.as_str().unwrap_or_default().to_string();
                    let filter = transforms[index].clone();

                    if let Some(slot) = data.iter().position(|d| {
                        d.get("name").and_then(Value::as_str) == Some(source_name.as_str())
                    }) {
                        let mut new_transforms = transforms;
                        new_transforms.remove(index);
                        data[slot]["transform"] = Value::Array(new_transforms);
                    }

                    animation_data.push(json!({
                        "name": format!("{source_name}{CURR}"),
                        "source": source,
                        "transform": [filter],
                    }));
                }
            }
        }
    }

    let mut out = selection_data;
    out.extend(data);
    out.extend(animation_data);
    out
}

/// Port of `assembleUnitSelectionMarks`.
pub fn assemble_unit_selection_marks(model: &Model, mut marks: Vec<Value>) -> Vec<Value> {
    for sel_cmpt in super::component::ordered_components(model).iter() {
        for c in selection_compilers() {
            if (c.defined)(sel_cmpt) {
                if let Some(f) = c.marks {
                    marks = f(model, sel_cmpt, marks);
                }
            }
        }
    }
    marks
}

/// Port of `assembleLayerSelectionMarks`.
pub fn assemble_layer_selection_marks(model: &Model, mut marks: Vec<Value>) -> Vec<Value> {
    for child in &model.children {
        if is_unit_model(child) {
            marks = assemble_unit_selection_marks(child, marks);
        }
    }
    marks
}

/// Port of `assembleSelectionScaleDomain`.
pub fn assemble_selection_scale_domain(
    model: &Model,
    extent: &Value,
    scale_cmpt: &crate::compile::scale::component::ScaleComponent,
    domain: &Value,
) -> Value {
    let param = extent
        .get("param")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parsed_extent = parse_selection_extent(model, param, extent);

    let continuous = scale_cmpt
        .get_type()
        .map(has_continuous_domain)
        .unwrap_or(false);
    let reversed = domain
        .as_array()
        .map(|d| {
            let first = d.first().and_then(Value::as_f64);
            let second = d.get(1).and_then(Value::as_f64);
            match (first, second) {
                (Some(a), Some(b)) => a > b,
                _ => false,
            }
        })
        .unwrap_or(false);

    if continuous && domain.is_array() && reversed {
        json!({"signal": format!("isValid({parsed_extent}) && reverse({parsed_extent})")})
    } else {
        json!({"signal": parsed_extent})
    }
}

fn cleanup_empty_on_array(signals: Vec<Value>) -> Vec<Value> {
    signals
        .into_iter()
        .map(|mut s| {
            let empty = s
                .get("on")
                .and_then(Value::as_array)
                .map(|a| a.is_empty())
                .unwrap_or(false);
            if empty {
                if let Some(obj) = s.as_object_mut() {
                    obj.remove("on");
                }
            }
            s
        })
        .collect()
}

#[allow(dead_code)]
fn _unused(_: SpecType) {}
