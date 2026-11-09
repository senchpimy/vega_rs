//! Port of `src/compile/mark/text.ts`.

use serde_json::Value;

use crate::compile::model::Model;
use crate::mark::get_mark_prop_or_config;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::defined::value_if_defined;
use super::encode::nonposition::{non_position, NonPositionOpt};
use super::encode::position_point::point_position;
use super::encode::text::text;

pub fn encode_entry(model: &Model) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;

    let mut entries = vec![
        base_encode_entry(
            model,
            Ignore {
                align: "include",
                baseline: "include",
                color: "include",
                size: "ignore",
                orient: "ignore",
                theta: "include",
            },
        ),
        point_position("x", model, Some("mid"), None),
        point_position("y", model, Some("mid"), None),
        text(model, "text"),
        non_position(
            "size",
            model,
            NonPositionOpt {
                vg_channel: Some("fontSize".to_string()),
                ..Default::default()
            },
        ),
        non_position("angle", model, NonPositionOpt::default()),
    ];

    let align_value = get_mark_prop_or_config("align", mark_def, config, None, false)
        .map(|_| Value::Null)
        .unwrap_or_else(|| Value::String("center".to_string()));
    if align_value.is_string() {
        entries.push(value_if_defined("align", Some(&align_value)));
    }

    let baseline_value = get_mark_prop_or_config("baseline", mark_def, config, None, false)
        .map(|_| Value::Null)
        .unwrap_or_else(|| Value::String("middle".to_string()));
    if baseline_value.is_string() {
        entries.push(value_if_defined("baseline", Some(&baseline_value)));
    }

    entries.push(point_position("radius", model, None, None));
    entries.push(point_position("theta", model, None, None));

    let mut out = merge_entries(entries);
    // `config.text.size` maps to Vega's fontSize. The shared helper does not
    // fall back from a vgChannel to the VL property, so handle it here.
    if !out.contains_key("fontSize") {
        let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
        let style_value = config
            .get("style")
            .and_then(|style| crate::mark::get_mark_style_config("size", mark_def, style));
        let specific_value = config
            .get(mark_type)
            .and_then(|m| m.get("size"))
            .filter(|v| !v.is_null());
        let value = style_value.or_else(|| specific_value.cloned());
        if let Some(value) = value {
            out.insert(
                "fontSize".to_string(),
                crate::compile_common::signal_or_value_ref(&value),
            );
        }
    }

    Value::Object(out)
}
