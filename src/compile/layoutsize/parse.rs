//! Port of `src/compile/layoutsize/parse.ts`.

use serde_json::{json, Value};

use crate::channel::{get_position_scale_channel, get_size_channel, POSITION_SCALE_CHANNELS};
use crate::scale::has_discrete_domain;

use super::super::model::Model;
use super::super::split::{merge_values_with_explicit, Explicit};
use super::super::support::{default_scale_resolve, get_step_for, is_step};
use super::component::get_size_type_from_layout_size_type;
use super::super::scale::domain::is_vg_range_step;

/// Port of `parseLayerLayoutSize`.
pub fn parse_layer_layout_size(model: &mut Model) {
    parse_children_layout_size(model);
    parse_non_unit_layout_size_for_channel(model, "width");
    parse_non_unit_layout_size_for_channel(model, "height");
}

/// Port of `parseConcatLayoutSize`.
pub fn parse_concat_layout_size(model: &mut Model) {
    parse_children_layout_size(model);

    let width_type = if model.layout.get("columns").and_then(Value::as_i64) == Some(1) {
        "width"
    } else {
        "childWidth"
    };
    let height_type = if model.layout.get("columns").is_none() {
        "height"
    } else {
        "childHeight"
    };

    parse_non_unit_layout_size_for_channel(model, width_type);
    parse_non_unit_layout_size_for_channel(model, height_type);
}

/// Port of `parseChildrenLayoutSize`.
pub fn parse_children_layout_size(model: &mut Model) {
    for child in &mut model.children {
        child.parse_layout_size();
    }
}

fn parse_non_unit_layout_size_for_channel(model: &mut Model, layout_size_type: &str) {
    let size_type = get_size_type_from_layout_size_type(layout_size_type);
    let channel = get_position_scale_channel(size_type);
    let model_type = model.type_;
    let resolve_scale = model
        .component
        .resolve
        .get_scale(channel)
        .unwrap_or_else(|| default_scale_resolve(channel, model_type).to_string());

    let mut merged_size: Option<Explicit> = None;
    for child in &model.children {
        let child_size = child.component.layout_size.get_with_explicit(size_type);
        if resolve_scale == "independent"
            && child_size.value.as_ref().and_then(Value::as_str) == Some("step")
        {
            merged_size = None;
            break;
        }

        if let Some(existing) = &merged_size {
            if resolve_scale == "independent" && existing.value != child_size.value {
                merged_size = None;
                break;
            }
            merged_size = Some(merge_values_with_explicit(
                Some(existing.clone()),
                child_size,
                size_type,
                "",
                |v1, _v2, _p, _po| v1.clone(),
            ));
        } else {
            merged_size = Some(child_size);
        }
    }

    if let Some(merged) = merged_size {
        let new_name = model.get_name(layout_size_type);
        let old_names: Vec<String> = model
            .children
            .iter()
            .map(|child| child.get_name(size_type))
            .collect();
        for old in &old_names {
            model.rename_signal(old, &new_name);
            // Upstream wraps size references in `SignalRefWrapper`, so scale
            // ranges are re-read after layout-size merging.  This port eagerly
            // materializes the signal name while parsing ranges, so rewrite the
            // stored references to keep them in sync with the rename.
            rewrite_layout_scale_signal_refs(model, old, &new_name);
        }
        for child in &mut model.children {
            child
                .component
                .layout_size
                .set(size_type, Value::String("merged".to_string()), false);
        }
        model
            .component
            .layout_size
            .set_with_explicit(layout_size_type, merged);
    } else {
        model.component.layout_size.set_with_explicit(
            layout_size_type,
            Explicit {
                explicit: false,
                value: None,
            },
        );
    }
}

/// Rewrite eagerly-assembled layout-size signal references after a rename.
fn rewrite_layout_scale_signal_refs(model: &mut Model, old: &str, new: &str) {
    for component in model.component.scales.values_mut() {
        rewrite_scale_prop(&mut component.explicit, old, new);
        rewrite_scale_prop(&mut component.implicit, old, new);
    }
    for child in &mut model.children {
        rewrite_layout_scale_signal_refs(child, old, new);
    }
}

fn rewrite_scale_prop(props: &mut std::collections::BTreeMap<String, Value>, old: &str, new: &str) {
    if let Some(range) = props.get_mut("range") {
        rewrite_signal_values(range, old, new);
    }
}

fn rewrite_signal_values(value: &mut Value, old: &str, new: &str) {
    match value {
        Value::String(text) => {
            if text.contains(old) {
                *text = text.replace(old, new);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_signal_values(item, old, new);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                rewrite_signal_values(item, old, new);
            }
        }
        _ => {}
    }
}

/// Port of `parseUnitLayoutSize`.
pub fn parse_unit_layout_size(model: &mut Model) {
    let size = model.size.clone();
    for channel in POSITION_SCALE_CHANNELS {
        let size_type = get_size_channel(channel).unwrap_or("width");
        let specified = size.get(size_type).cloned().unwrap_or(Value::Null);
        if !specified.is_null() {
            let value = if is_step(&specified) {
                Value::String("step".to_string())
            } else {
                specified
            };
            model
                .component
                .layout_size
                .set(size_type, value, true);
        } else {
            let default_size = default_unit_size(model, size_type);
            model
                .component
                .layout_size
                .set(size_type, default_size, false);
        }
    }
}

fn default_unit_size(model: &Model, size_type: &str) -> Value {
    let channel = if size_type == "width" { "x" } else { "y" };
    let view_config = model
        .config
        .get("view")
        .cloned()
        .unwrap_or(Value::Null);
    let scale_component = model.get_scale_component(channel);

    if let Some(scale_component) = scale_component {
        let scale_type = scale_component.get_type();
        let range = scale_component.get("range").unwrap_or(Value::Null);
        if let Some(scale_type) = scale_type {
            if has_discrete_domain(scale_type) {
                let size = super::super::support::get_view_config_discrete_size(
                    &view_config,
                    size_type,
                );
                if is_vg_range_step(&range) || is_step(&size) {
                    return Value::String("step".to_string());
                }
                return size;
            } else {
                return super::super::support::get_view_config_continuous_size(
                    &view_config,
                    size_type,
                );
            }
        }
    }

    if model.has_projection() || model.mark_type() == "arc" {
        return super::super::support::get_view_config_continuous_size(&view_config, size_type);
    }

    let size = super::super::support::get_view_config_discrete_size(&view_config, size_type);
    if is_step(&size) {
        return size.get("step").cloned().unwrap_or(json!(20));
    }
    size
}

#[allow(dead_code)]
fn _unused() {
    let _ = get_step_for;
}
