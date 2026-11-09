//! Port of `src/compile/scale/assemble.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{is_x_or_y, ScaleChannel};

use super::super::model::{is_concat_model, is_layer_model, Model};
use super::domain::assemble_domain;

/// Port of `assembleScales`.
pub fn assemble_scales(model: &Model) -> Vec<Value> {
    if is_layer_model(model) || is_concat_model(model) {
        let mut scales = assemble_scales_for_model(model);
        for child in &model.children {
            scales.extend(assemble_scales(child));
        }
        scales
    } else {
        assemble_scales_for_model(model)
    }
}

/// Port of `assembleScalesForModel`.
pub fn assemble_scales_for_model(model: &Model) -> Vec<Value> {
    let mut scales = Vec::new();
    // Upstream iterates `keys(model.component.scales)`: unit models insert in
    // `SCALE_CHANNELS` order, while composite parents keep the order in which
    // children contributed channels.
    let channels: Vec<String> = model.component.scales.keys().cloned().collect();
    for channel in &channels {
        let channel = channel.as_str();
        let scale_component = match model.component.scales.get(channel) {
            Some(c) => c,
            None => continue,
        };
        if scale_component.merged {
            continue;
        }
        let scale = scale_component.combine();
        let name = scale
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let type_ = scale.get("type").cloned().unwrap_or(Value::Null);
        let selection_extent = scale.get("selectionExtent").cloned();
        let mut raw_range = scale.get("range").cloned().unwrap_or(Value::Null);
        // Signal references in scale ranges are created eagerly during parse,
        // before parent models may rename layout-size signals. Resolve the
        // rename map at assembly time (upstream wraps these in lazy
        // `SignalRefWrapper`s instead).
        model.resolve_signal_names_in(&mut raw_range);
        let range = assemble_scale_range(raw_range, &name, channel, Some(model));
        let domain = assemble_domain(model, channel);

        let mut out = Map::new();
        out.insert("name".to_string(), Value::String(name));
        out.insert("type".to_string(), type_);
        if !domain.is_null() {
            out.insert("domain".to_string(), domain.clone());
        }
        if let Some(selection_extent) = selection_extent {
            let assembled = assemble_selection_scale_domain(
                model,
                &selection_extent,
                scale_component,
                &domain,
            );
            if !assembled.is_null() {
                out.insert("domainRaw".to_string(), assembled);
            }
        }
        out.insert("range".to_string(), range);
        if let Some(reverse) = scale.get("reverse") {
            out.insert("reverse".to_string(), reverse.clone());
        }
        for (k, v) in scale {
            if matches!(
                k.as_str(),
                "name" | "type" | "selectionExtent" | "domains" | "range" | "reverse"
            ) {
                continue;
            }
            let mut v = v.clone();
            model.resolve_signal_names_in(&mut v);
            out.insert(k, v);
        }
        scales.push(Value::Object(out));
    }
    scales
}

/// Port of `assembleScaleRange`.
pub fn assemble_scale_range(
    scale_range: Value,
    scale_name: &str,
    channel: &str,
    model: Option<&Model>,
) -> Value {
    if is_x_or_y(channel) {
        if scale_range.is_object() && scale_range.get("step").is_some() {
            return json!({"step": {"signal": format!("{scale_name}_step")}});
        }
    } else if let Some(model) = model {
        if scale_range.is_object()
            && scale_range.get("data").is_some()
            && scale_range.get("field").is_some()
        {
            let mut range = scale_range;
            if let Some(data) = range.get("data").and_then(Value::as_str) {
                let data = model.lookup_data_source(data);
                range["data"] = Value::String(data);
            }
            return range;
        }
    }
    scale_range
}

/// Port of `assembleSelectionScaleDomain`.
pub fn assemble_selection_scale_domain(
    model: &Model,
    selection_extent: &Value,
    scale_component: &super::component::ScaleComponent,
    domain: &Value,
) -> Value {
    super::super::selection::assemble::assemble_selection_scale_domain(
        model,
        selection_extent,
        scale_component,
        domain,
    )
}

#[allow(dead_code)]
fn _unused(_: ScaleChannel) {}
