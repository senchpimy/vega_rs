//! Port of `src/compile/layoutsize/assemble.ts`.

use serde_json::{json, Value};

use crate::config::get_view_config_continuous_size;
use crate::scale::has_discrete_domain;
use crate::util::get_first_defined;

use super::super::model::Model;
use super::super::scale::component::ScaleComponent;
use super::super::scale::domain::is_vg_range_step;

/// Port of `assembleLayoutSignals`.
pub fn assemble_layout_signals(model: &Model) -> Vec<Value> {
    let mut signals = Vec::new();
    for size_type in ["width", "height", "childWidth", "childHeight"] {
        signals.extend(size_signals(model, size_type));
    }
    signals
}

/// Port of `sizeSignals`.
pub fn size_signals(model: &Model, size_type: &str) -> Vec<Value> {
    let channel = if size_type == "width" || size_type == "childWidth" {
        "x"
    } else {
        "y"
    };
    let size = match model.component.layout_size.get(size_type) {
        Some(v) => v,
        None => return Vec::new(),
    };
    if size.is_null() || size.as_str() == Some("merged") {
        return Vec::new();
    }

    let name = model
        .get_size_signal_ref(size_type)
        .get("signal")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    if size.as_str() == Some("step") {
        if let Some(scale_component) = model.get_scale_component(channel) {
            let type_ = scale_component.get_type();
            let range = scale_component.get("range").unwrap_or(Value::Null);
            let discrete = type_.map(has_discrete_domain).unwrap_or(false);
            if discrete && is_vg_range_step(&range) {
                let scale_name = model.scale_name(channel, false).unwrap_or_default();
                // Facet parent with independent resolve: only emit the step signal.
                if model.parent_type == Some(super::super::model::SpecType::Facet)
                    && model
                    .facet_scale_resolve
                    .get(channel)
                    .map(|r| r == "independent")
                    .unwrap_or(false)
                {
                    return vec![step_signal(&scale_name, &range)];
                }
                return vec![
                    step_signal(&scale_name, &range),
                    json!({
                        "name": name,
                        "update": size_expr(&scale_name, &scale_component, &format!("domain('{scale_name}').length")),
                    }),
                ];
            }
        }
        return Vec::new();
    } else if size.as_str() == Some("container") {
        let is_width = size_type.ends_with("width");
        let expr = if is_width {
            "containerSize()[0]"
        } else {
            "containerSize()[1]"
        };
        let view_config = model.config.get("view").cloned().unwrap_or(Value::Null);
        let default_value =
            get_view_config_continuous_size(&view_config, if is_width { "width" } else { "height" })
                .unwrap_or_else(|| json!(300));
        let default_value = default_value
            .as_f64()
            .map(crate::util::js_number_to_string)
            .unwrap_or_else(|| default_value.to_string());
        let safe_expr = format!("isFinite({expr}) ? {expr} : {default_value}");
        return vec![json!({
            "name": name,
            "init": safe_expr,
            "on": [
                {"events": "window:resize", "update": safe_expr},
                {"events": "container:resize", "update": safe_expr},
            ],
        })];
    }

    vec![json!({"name": name, "value": size})]
}

fn step_signal(scale_name: &str, range: &Value) -> Value {
    let name = format!("{scale_name}_step");
    let step = range.get("step").cloned().unwrap_or(Value::Null);
    if crate::compile_common::is_signal_ref(&step) {
        json!({"name": name, "update": step.get("signal").cloned().unwrap_or(Value::Null)})
    } else {
        json!({"name": name, "value": step})
    }
}

/// Port of `sizeExpr`.
pub fn size_expr(scale_name: &str, scale_component: &ScaleComponent, cardinality: &str) -> String {
    let type_ = scale_component.get_type();
    let padding = scale_component.get("padding");
    let padding_outer =
        get_first_defined(&[scale_component.get("paddingOuter").as_ref(), padding.as_ref()])
            .cloned()
            .unwrap_or(Value::Null);

    let mut padding_inner = scale_component.get("paddingInner");
    padding_inner = if type_.map(|t| t == crate::scale::ScaleType::Band).unwrap_or(false) {
        padding_inner.or(padding)
    } else {
        Some(Value::from(1))
    };
    let padding_inner = padding_inner.unwrap_or(Value::Null);

    format!(
        "bandspace({cardinality}, {}, {}) * {scale_name}_step",
        signal_or_string_value(&padding_inner),
        signal_or_string_value(&padding_outer)
    )
}

fn signal_or_string_value(v: &Value) -> String {
    if let Some(signal) = v.get("signal") {
        return signal.as_str().unwrap_or_default().to_string();
    }
    crate::util::string_value(v)
}
