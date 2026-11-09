//! Port of `src/compile/mark/bar.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::corner_radius::corner_radius_end;
use super::encode::position_rect::rect_position;

pub fn encode_entry(model: &Model) -> Value {
    let encode_entry = Value::Object(merge_entries(vec![
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
        rect_position(model, "x"),
        rect_position(model, "y"),
    ]));

    let out = merge_entries(vec![encode_entry.clone(), corner_radius_end(model, &encode_entry)]);
    Value::Object(out)
}
