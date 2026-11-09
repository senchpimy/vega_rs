//! Port of `src/compile/mark/tick.ts`.

use serde_json::{Map, Value};

use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::get_mark_prop_or_config;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::position_point::point_position;
use super::encode::position_rect::rect_position;

pub fn encode_entry(model: &Model) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");

    let vg_size_axis_channel = if orient == "horizontal" { "x" } else { "y" };
    let vg_thickness_axis_channel = if orient == "horizontal" { "y" } else { "x" };
    let vg_thickness_channel = if orient == "horizontal" { "height" } else { "width" };

    let mut entries = vec![
        base_encode_entry(
            model,
            Ignore {
                align: "ignore",
                baseline: "ignore",
                color: "include",
                orient: "ignore",
                size: "ignore",
                theta: "ignore",
            },
        ),
        rect_position(model, vg_size_axis_channel),
        point_position(
            vg_thickness_axis_channel,
            model,
            Some("mid"),
            Some(if vg_thickness_axis_channel == "y" {
                "yc"
            } else {
                "xc"
            }),
        ),
    ];

    let thickness = get_mark_prop_or_config("thickness", mark_def, config, None, false)
        .unwrap_or(Value::Null);
    let mut thickness_entry = Map::new();
    thickness_entry.insert(
        vg_thickness_channel.to_string(),
        signal_or_value_ref(&thickness),
    );
    entries.push(Value::Object(thickness_entry));

    Value::Object(merge_entries(entries))
}
