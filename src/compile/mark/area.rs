//! Port of `src/compile/mark/area.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::defined::defined;
use super::encode::position_range::point_or_range_position;

pub fn encode_entry(model: &Model) -> Value {
    let x_range_from_offset = super::mark::is_ranged_offset(model, "x");
    let y_range_from_offset = super::mark::is_ranged_offset(model, "y");
    let has_offset_driven_range = x_range_from_offset || y_range_from_offset;
    let orient = model
        .unit
        .mark_def
        .get("orient")
        .and_then(Value::as_str)
        .unwrap_or("");

    let x_is_range = x_range_from_offset || (!has_offset_driven_range && orient == "horizontal");
    let y_is_range = y_range_from_offset || (!has_offset_driven_range && orient == "vertical");
    let y_default_pos = if y_range_from_offset
        && model
            .unit
            .encoding
            .get("y")
            .map(|v| v.is_null())
            .unwrap_or(true)
    {
        "zeroOrMax"
    } else {
        "zeroOrMin"
    };

    let out = merge_entries(vec![
        base_encode_entry(
            model,
            Ignore {
                align: "ignore",
                baseline: "ignore",
                color: "include",
                orient: "include",
                size: "ignore",
                theta: "ignore",
            },
        ),
        point_or_range_position("x", model, Some("zeroOrMin"), "zeroOrMin", x_is_range),
        point_or_range_position("y", model, Some(y_default_pos), "zeroOrMin", y_is_range),
        defined(model),
    ]);
    Value::Object(out)
}
